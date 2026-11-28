//! The crate's sole shell FFI: moving one item to the Windows Recycle Bin through the shell's
//! `IFileOperation`, on the calling thread and without any window. A deletion the shell announces
//! as permanent is refused before it happens, so this adapter never deletes anything.

use std::{
    marker::PhantomData,
    os::windows::ffi::OsStrExt,
    path::{Component, Path, Prefix},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::Win32::{
    Foundation::E_ABORT,
    System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
        CoInitializeEx, CoUninitialize,
    },
    UI::Shell::{
        FILEOPERATION_FLAGS, FOF_NO_CONNECTED_ELEMENTS, FOF_NO_UI, FOFX_NOCOPYHOOKS,
        FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IFileOperationProgressSink,
        IFileOperationProgressSink_Impl, IShellItem, SHCreateItemFromParsingName,
        TSF_DELETE_RECYCLE_IF_POSSIBLE,
    },
};
use windows_core::{ComObject, HRESULT, PCWSTR, Ref, implement};

/// Why the Recycle Bin did not take an item. Either way the item was not deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NotRecycled {
    /// The shell announced that it would delete the item permanently, as it does on a volume
    /// without a Recycle Bin, and the deletion was refused before it began.
    Unavailable,
    /// The shell could not move the item, or the path never reached it.
    Failed,
}

/// The flags of every move: no progress, confirmation, error or elevation window (`FOF_NO_UI`
/// without `FOFX_SHOWELEVATIONPROMPT`); recycling (`FOFX_RECYCLEONDELETE`, Windows 8 and later)
/// without an undo record in Explorer; the named item alone, never the files Windows connects to
/// it, such as `page_files` beside `page.htm` (`FOF_NO_CONNECTED_ELEMENTS`); and no shell extension
/// copy hooks (`FOFX_NOCOPYHOOKS`). `FOF_WANTNUKEWARNING` stays off: even under `FOF_NO_UI` it asks
/// whether to delete permanently an item that cannot be recycled.
const RECYCLE: FILEOPERATION_FLAGS = FILEOPERATION_FLAGS(
    FOF_NO_UI.0 | FOFX_RECYCLEONDELETE.0 | FOF_NO_CONNECTED_ELEMENTS.0 | FOFX_NOCOPYHOOKS.0,
);

/// Moves the item at `path`, an absolute drive path, to its volume's Recycle Bin: the entry
/// itself, so a link or junction moves without what it names.
pub(super) fn recycle(path: &Path) -> Result<(), NotRecycled> {
    delete(path, RECYCLE)
}

/// Deletes the item at `path` with `flags`, refusing whatever the shell would not recycle.
fn delete(path: &Path, flags: FILEOPERATION_FLAGS) -> Result<(), NotRecycled> {
    let name = shell_name(path).ok_or(NotRecycled::Failed)?;
    let _apartment = Apartment::enter().ok_or(NotRecycled::Failed)?;
    // Every COM object of the move is released when this returns, before the apartment ends.
    perform(&name, flags)
}

/// `path` as the NUL-terminated UTF-16 the shell parses: only an absolute drive path below the
/// root, without a `..` segment or a NUL. The shell would parse a relative name against the
/// Desktop, and a NUL would end the path at one of its folders.
fn shell_name(path: &Path) -> Option<Vec<u16>> {
    let mut components = path.components();
    let drive = matches!(
        components.next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
    );
    let rooted = matches!(components.next(), Some(Component::RootDir));
    let mut below = components.peekable();
    let named =
        below.peek().is_some() && below.all(|component| matches!(component, Component::Normal(_)));
    let mut name = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if !(drive && rooted && named) || name.contains(&0) {
        return None;
    }
    name.push(0);
    Some(name)
}

/// COM initialized as a single-threaded apartment on this thread, as `IFileOperation` requires,
/// for one move. Not `Send`, so it ends on the thread it began on.
struct Apartment(PhantomData<*const ()>);

impl Apartment {
    /// `None` when this thread is already in the multithreaded apartment, which is left as it was.
    fn enter() -> Option<Self> {
        // SAFETY: The reserved pointer is null, as documented. `S_OK`, and `S_FALSE` for a thread
        // already in a single-threaded apartment, each count one initialization that `Drop`
        // balances; a failure, such as `RPC_E_CHANGED_MODE`, counts none and returns no guard.
        let entered =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        entered.is_ok().then_some(Self(PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: This guard exists only after one successful initialization on this thread, and
        // it cannot leave the thread. `delete` releases every COM object of the move before the
        // guard drops.
        unsafe { CoUninitialize() };
    }
}

/// Queues the deletion of the item `name` names, runs it on this thread and reads its outcome.
fn perform(name: &[u16], flags: FILEOPERATION_FLAGS) -> Result<(), NotRecycled> {
    let failed = |_| NotRecycled::Failed;
    // SAFETY: `name` is NUL-terminated without an interior NUL and outlives the call, which
    // copies what it parses and retains no pointer. No bind context is passed. The apartment of
    // this thread is initialized, and the returned item is owned here and released on drop.
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(PCWSTR(name.as_ptr()), None) }.map_err(failed)?;
    // SAFETY: `FileOperation` is the class's identifier, outliving the call; there is no outer
    // object; the in-process server joins this thread's initialized apartment. The returned
    // interface is owned here and released on drop.
    let operation: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER) }.map_err(failed)?;
    let reports = ComObject::new(Reports::default());
    let sink: IFileOperationProgressSink = reports.to_interface();
    // SAFETY: `operation` is a live interface owned here; the flags are a plain value.
    unsafe { operation.SetOperationFlags(flags) }.map_err(failed)?;
    // SAFETY: Each interface is live and owned here. The operation takes its own references to
    // the item and the sink and releases them when it is released.
    unsafe { operation.DeleteItem(&item, &sink) }.map_err(failed)?;
    // SAFETY: `operation` is a live interface owned here. The call runs the queued deletion on
    // this thread and returns when it ends; the sink may hear from other threads, so its reports
    // are atomic.
    let performed = unsafe { operation.PerformOperations() }.is_ok();
    // SAFETY: `operation` is a live interface owned here.
    let aborted =
        unsafe { operation.GetAnyOperationsAborted() }.map_or(true, |aborted| aborted.as_bool());
    outcome(&reports, performed, aborted)
}

/// Whether the shell, announcing a deletion with `flags` in `PreDeleteItem`, would recycle the
/// item rather than delete it permanently.
const fn recycles(flags: u32) -> bool {
    flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0.cast_unsigned() != 0
}

/// A move's outcome: a refused permanent deletion is `Unavailable`; only a recycled item, with the
/// operation run and nothing aborted or failed, is success; anything else is `Failed`.
fn outcome(reports: &Reports, performed: bool, aborted: bool) -> Result<(), NotRecycled> {
    if reports.refused.load(Ordering::Acquire) {
        Err(NotRecycled::Unavailable)
    } else if performed
        && !aborted
        && !reports.failed.load(Ordering::Acquire)
        && reports.recycled.load(Ordering::Acquire)
    {
        Ok(())
    } else {
        Err(NotRecycled::Failed)
    }
}

/// What the shell reported of one move. `#[implement]` makes the sink agile, so the shell may call
/// it from any thread and each report is atomic.
#[implement(IFileOperationProgressSink)]
#[derive(Default)]
struct Reports {
    /// The shell announced a permanent deletion, which was refused.
    refused: AtomicBool,
    /// The shell deleted the item and named the Recycle Bin's new item for it.
    recycled: AtomicBool,
    /// The shell reported a failure for the item or a part of it.
    failed: AtomicBool,
}

impl IFileOperationProgressSink_Impl for Reports_Impl {
    fn PreDeleteItem(&self, flags: u32, _: Ref<IShellItem>) -> windows_core::Result<()> {
        if recycles(flags) {
            Ok(())
        } else {
            self.refused.store(true, Ordering::Release);
            Err(E_ABORT.into())
        }
    }

    fn PostDeleteItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        deleted: HRESULT,
        created: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        if deleted.is_err() {
            self.failed.store(true, Ordering::Release);
        } else if created.as_ref().is_some() {
            self.recycled.store(true, Ordering::Release);
        } else {
            // Deleted without a Recycle Bin item: not the recycling this adapter asked for.
            self.failed.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn StartOperations(&self) -> windows_core::Result<()> {
        Ok(())
    }

    fn FinishOperations(&self, _: HRESULT) -> windows_core::Result<()> {
        Ok(())
    }

    fn PreRenameItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows_core::Result<()> {
        Ok(())
    }

    fn PostRenameItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn PreMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn PostMoveItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn PreCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn PostCopyItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn PreNewItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows_core::Result<()> {
        Ok(())
    }

    fn PostNewItem(
        &self,
        _: u32,
        _: Ref<IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Ref<IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }

    fn UpdateProgress(&self, _: u32, _: u32) -> windows_core::Result<()> {
        Ok(())
    }

    fn ResetTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }

    fn PauseTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }

    fn ResumeTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
