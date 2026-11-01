//! The crate's sole Win32 FFI: one exact namespace lookup, pipe counters, one zero-byte read
//! watch per connection, and one wait over the resulting events.

use std::{
    cell::{RefCell, UnsafeCell},
    ffi::c_void,
    fmt, io,
    marker::PhantomPinned,
    os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    pin::Pin,
    ptr,
    sync::Arc,
    time::Duration,
};
use widestring::U16CString;
use winsafe::{HFINDFILE, WIN32_FIND_DATA, co, guard::FindCloseGuard, prelude::Handle};

const ERROR_IO_PENDING: i32 = 997;
const ERROR_IO_INCOMPLETE: i32 = 996;
const INFINITE: u32 = u32::MAX;
const WAIT_FAILED: u32 = u32::MAX;
const MAXIMUM_WAIT_OBJECTS: usize = 64;
const FILE_PIPE_LOCAL_INFORMATION_CLASS: u32 = 24;
const FILE_PIPE_CONNECTED_STATE: u32 = 3;
const STATUS_PIPE_DISCONNECTED: u32 = 0xC000_00B0;
const STATUS_PIPE_CLOSING: u32 = 0xC000_00B1;
const STATUS_PIPE_BROKEN: u32 = 0xC000_014B;
const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION: u32 = 0x2;
const TIMER_ALL_ACCESS: u32 = 0x001F_0003;

/// `hEvent` is pointer-sized in the ABI and stays an integer here, so this value is `Send`.
#[repr(C)]
#[derive(Default)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: usize,
}

#[repr(C)]
#[derive(Default)]
struct IoStatusBlock {
    status: usize,
    information: usize,
}

#[repr(C)]
#[derive(Default)]
struct FilePipeLocalInformation {
    named_pipe_type: u32,
    named_pipe_configuration: u32,
    maximum_instances: u32,
    current_instances: u32,
    inbound_quota: u32,
    read_data_available: u32,
    outbound_quota: u32,
    write_quota_available: u32,
    named_pipe_state: u32,
    named_pipe_end: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "FindFirstFileW"]
    fn find_first_file(name: *const u16, data: *mut WIN32_FIND_DATA) -> *mut c_void;
    #[link_name = "CreateEventW"]
    fn create_event(
        attributes: *const c_void,
        manual_reset: i32,
        initial_state: i32,
        name: *const u16,
    ) -> *mut c_void;
    #[link_name = "SetEvent"]
    fn set_event(event: *mut c_void) -> i32;
    #[link_name = "ResetEvent"]
    fn reset_event(event: *mut c_void) -> i32;
    #[link_name = "ReadFile"]
    fn read_file(
        file: *mut c_void,
        buffer: *mut u8,
        length: u32,
        read: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    #[link_name = "GetOverlappedResult"]
    fn get_overlapped_result(
        file: *mut c_void,
        overlapped: *mut Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    #[link_name = "CancelIoEx"]
    fn cancel_io_ex(file: *mut c_void, overlapped: *mut Overlapped) -> i32;
    #[link_name = "WaitForMultipleObjects"]
    fn wait_for_multiple_objects(
        count: u32,
        handles: *const *mut c_void,
        wait_all: i32,
        milliseconds: u32,
    ) -> u32;
    #[link_name = "CreateWaitableTimerExW"]
    fn create_waitable_timer_ex(
        attributes: *const c_void,
        name: *const u16,
        flags: u32,
        access: u32,
    ) -> *mut c_void;
    #[link_name = "SetWaitableTimer"]
    fn set_waitable_timer(
        timer: *mut c_void,
        due: *const i64,
        period: i32,
        routine: *const c_void,
        argument: *const c_void,
        resume: i32,
    ) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    #[link_name = "NtQueryInformationFile"]
    fn nt_query_information_file(
        file: *mut c_void,
        status: *mut IoStatusBlock,
        information: *mut c_void,
        length: u32,
        class: u32,
    ) -> i32;
}

pub(super) fn exact_name_exists(query: &str) -> io::Result<bool> {
    let query = U16CString::from_str(query).map_err(|_| io::ErrorKind::InvalidInput)?;
    let mut data = WIN32_FIND_DATA::default();
    let invalid = HFINDFILE::INVALID.ptr();
    // SAFETY: `query` owns a terminated UTF-16 buffer without interior NULs; `data`
    // is a writable, correctly aligned WIN32_FIND_DATAW ABI value. Both outlive the
    // call and error capture. FindFirstFileW retains neither pointer and opens no peer.
    let handle = unsafe { find_first_file(query.as_ptr(), &mut data) };
    if handle == invalid {
        // Capture before either buffer is dropped or any cleanup can change last-error.
        // winsafe 0.0.29's FindFirstFile wrapper loses errors for INVALID_HANDLE_VALUE.
        let error = winsafe::GetLastError();
        return match error {
            co::ERROR::FILE_NOT_FOUND | co::ERROR::NO_MORE_FILES => Ok(false),
            error => Err(io::Error::from_raw_os_error(error.raw() as i32)),
        };
    }
    // SAFETY: A successful FindFirstFileW transfers one live search handle here.
    // This guard is its only owner and closes it with FindClose; failed handles
    // never reach this branch. The handle is never copied or used after the guard.
    let _search = unsafe { FindCloseGuard::new(HFINDFILE::from_ptr(handle)) };
    Ok(true)
}

/// One pipe end's kernel counters, sampled without consuming a byte or blocking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PipeCounters {
    pub(super) read_available: u32,
    pub(super) write_quota: u32,
    pub(super) connected: bool,
}

pub(super) fn pipe_counters(pipe: BorrowedHandle<'_>) -> io::Result<PipeCounters> {
    let mut status = IoStatusBlock::default();
    let mut information = FilePipeLocalInformation::default();
    // SAFETY: `pipe` is live for the call. Both out values are writable, correctly aligned
    // ABI values on this stack frame, the length is exact, and the query retains no pointer.
    let result = unsafe {
        nt_query_information_file(
            pipe.as_raw_handle(),
            &mut status,
            ptr::from_mut(&mut information).cast(),
            size_of::<FilePipeLocalInformation>() as u32,
            FILE_PIPE_LOCAL_INFORMATION_CLASS,
        )
    };
    if result >= 0 {
        return Ok(PipeCounters {
            read_available: information.read_data_available,
            write_quota: information.write_quota_available,
            connected: information.named_pipe_state == FILE_PIPE_CONNECTED_STATE,
        });
    }
    match result as u32 {
        STATUS_PIPE_DISCONNECTED | STATUS_PIPE_CLOSING | STATUS_PIPE_BROKEN => Ok(PipeCounters {
            read_available: 0,
            write_quota: 0,
            connected: false,
        }),
        status => Err(io::Error::other(format!(
            "pipe counters unavailable: NTSTATUS {status:#010x}"
        ))),
    }
}

fn new_event() -> io::Result<OwnedHandle> {
    // SAFETY: Null attributes and name are documented inputs; the call retains no pointer.
    let event = unsafe { create_event(ptr::null(), 1, 0, ptr::null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: A successful CreateEventW transfers one live handle to this sole owner.
    Ok(unsafe { OwnedHandle::from_raw_handle(event) })
}

/// Kernel-visible storage for one pending operation. It is pinned for a stable address, and
/// `ReadWatch` never drops it while `pending` is set.
struct WatchStorage {
    overlapped: UnsafeCell<Overlapped>,
    byte: UnsafeCell<u8>,
    _kernel_writes: PhantomPinned,
}

/// Whether one more bytes-or-closure notification is outstanding on the event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WatchArm {
    Pending,
    Completed,
}

/// One pending zero-byte read. It completes when the peer's bytes arrive or the pipe closes,
/// consumes nothing, and signals an event any number of pollers can wait on.
pub(super) struct ReadWatch {
    // Owned duplicate: the operation's handle outlives every other owner's early close.
    pipe: OwnedHandle,
    event: Arc<OwnedHandle>,
    storage: Pin<Box<WatchStorage>>,
    pending: bool,
}

impl fmt::Debug for ReadWatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadWatch")
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}

impl ReadWatch {
    pub(super) fn new(pipe: BorrowedHandle<'_>) -> io::Result<Self> {
        let event = Arc::new(new_event()?);
        Ok(Self {
            pipe: pipe.try_clone_to_owned()?,
            storage: Box::pin(WatchStorage {
                overlapped: UnsafeCell::new(Overlapped {
                    event: event.as_raw_handle() as usize,
                    ..Overlapped::default()
                }),
                byte: UnsafeCell::new(0),
                _kernel_writes: PhantomPinned,
            }),
            event,
            pending: false,
        })
    }

    pub(super) fn event(&self) -> Arc<OwnedHandle> {
        Arc::clone(&self.event)
    }

    /// Starts one operation unless one is already outstanding. A finished operation says
    /// nothing about the bytes present now, so the caller always probes after `Completed`.
    pub(super) fn arm(&mut self) -> io::Result<WatchArm> {
        let pipe = self.pipe.as_raw_handle();
        let overlapped = self.storage.overlapped.get();
        if self.pending {
            let mut transferred = 0;
            // SAFETY: `pipe` is live, `overlapped` is the exact storage of the outstanding
            // operation, and a non-waiting query retains neither pointer.
            let finished =
                unsafe { get_overlapped_result(pipe, overlapped, &mut transferred, 0) } != 0;
            if !finished && io::Error::last_os_error().raw_os_error() == Some(ERROR_IO_INCOMPLETE) {
                return Ok(WatchArm::Pending);
            }
            self.pending = false;
        }
        // SAFETY: The event is live. No operation is outstanding here, so resetting cannot
        // lose a completion, and a concurrent waiter observes the next operation's signal.
        if unsafe { reset_event(self.event.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: No operation is outstanding, so this exclusive borrow reinitializes unshared
        // storage; the event stays owned by `self` for the storage's whole lifetime.
        unsafe {
            *overlapped = Overlapped {
                event: self.event.as_raw_handle() as usize,
                ..Overlapped::default()
            };
        }
        // SAFETY: `pipe` is an overlapped handle, the buffer and OVERLAPPED have stable boxed
        // addresses, and `Drop` waits for any pending operation before releasing them. A zero
        // length never writes through the buffer.
        let started = unsafe {
            read_file(
                pipe,
                self.storage.byte.get(),
                0,
                ptr::null_mut(),
                overlapped,
            )
        };
        if started == 0 && io::Error::last_os_error().raw_os_error() == Some(ERROR_IO_PENDING) {
            self.pending = true;
            return Ok(WatchArm::Pending);
        }
        Ok(WatchArm::Completed)
    }
}

impl Drop for ReadWatch {
    fn drop(&mut self) {
        let pipe = self.pipe.as_raw_handle();
        let overlapped = self.storage.overlapped.get();
        if self.pending {
            let mut transferred = 0;
            // SAFETY: The handle and storage are live. Cancellation failure means the operation
            // already finished. The waiting query returns only after the kernel released the
            // storage, which is the condition for freeing it below.
            unsafe {
                cancel_io_ex(pipe, overlapped);
                get_overlapped_result(pipe, overlapped, &mut transferred, 1);
            }
        }
        // SAFETY: The event is live; a waiter on another clone must observe this closure.
        unsafe {
            set_event(self.event.as_raw_handle());
        }
    }
}

/// One thread's precise deadline source; the default clock tick would round a wait up by 16 ms.
struct WaitTimer(OwnedHandle);

impl WaitTimer {
    fn new() -> Option<Self> {
        // SAFETY: Null attributes and name are documented inputs; the call retains no pointer.
        let timer = unsafe {
            create_waitable_timer_ex(
                ptr::null(),
                ptr::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS,
            )
        };
        // SAFETY: A non-null result transfers one live handle to this sole owner.
        (!timer.is_null()).then(|| Self(unsafe { OwnedHandle::from_raw_handle(timer) }))
    }

    fn arm(&self, after: Duration) -> io::Result<*mut c_void> {
        // A negative due time is relative, in 100-nanosecond units.
        let due = -i64::try_from(after.as_nanos() / 100)
            .unwrap_or(i64::MAX)
            .max(1);
        let handle = self.0.as_raw_handle();
        // SAFETY: The timer is live, `due` outlives the call, and null routine and argument are
        // documented inputs. The call retains no pointer.
        if unsafe { set_waitable_timer(handle, &due, 0, ptr::null(), ptr::null(), 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(handle)
    }
}

thread_local! {
    static WAIT_TIMER: RefCell<Option<Option<WaitTimer>>> = const { RefCell::new(None) };
}

/// The event set one `wait_any` call admits alongside its deadline source.
pub(super) const MAXIMUM_WAIT_EVENTS: usize = MAXIMUM_WAIT_OBJECTS - 1;

/// Returns when any event is signalled or `timeout` passes. Callers reprobe: a wake is advisory.
pub(super) fn wait_any(events: &[Arc<OwnedHandle>], timeout: Option<Duration>) -> io::Result<()> {
    if events.len() > MAXIMUM_WAIT_EVENTS {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut handles = [ptr::null_mut(); MAXIMUM_WAIT_OBJECTS];
    for (slot, event) in handles.iter_mut().zip(events) {
        *slot = event.as_raw_handle();
    }
    let mut count = events.len();
    let mut milliseconds = INFINITE;
    if let Some(timeout) = timeout {
        let timer = WAIT_TIMER.with(|cell| {
            cell.borrow_mut()
                .get_or_insert_with(WaitTimer::new)
                .as_ref()
                .map(|timer| timer.arm(timeout))
                .transpose()
        })?;
        match timer {
            Some(timer) => {
                handles[count] = timer;
                count += 1;
            }
            None => {
                milliseconds = u32::try_from(timeout.as_millis().saturating_add(1))
                    .unwrap_or(INFINITE - 1)
                    .min(INFINITE - 1);
            }
        }
    }
    if count == 0 && milliseconds == INFINITE {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: `handles[..count]` are live: the caller's `events` borrow and this thread's timer
    // both outlive the call. Waiting for any handle retains no pointer.
    let result =
        unsafe { wait_for_multiple_objects(count as u32, handles.as_ptr(), 0, milliseconds) };
    if result == WAIT_FAILED {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
