//! The reviewed Win32 adapter that derives a standard-rights primary token from this process's own
//! token and starts one child with it, handing the child only this process's standard handles.
//! [`crate::standard_rights`] is its only caller; the crate README's safety contract lists every
//! invariant the unsafe blocks below rely on.
use std::{
    ffi::{OsStr, OsString},
    io,
    mem::{align_of, offset_of, size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle},
    },
    path::Path,
    ptr, slice,
};

use windows::{
    Win32::{
        Foundation::{
            GENERIC_ALL, HANDLE, HANDLE_FLAG_INHERIT, LUID, SetHandleInformation, WAIT_OBJECT_0,
        },
        Security::{
            ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAce,
            CREATE_RESTRICTED_TOKEN_FLAGS, CreateRestrictedToken, CreateWellKnownSid,
            GetTokenInformation, InitializeAcl, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, PSID,
            SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES, SetTokenInformation, TOKEN_ADJUST_DEFAULT,
            TOKEN_ASSIGN_PRIMARY, TOKEN_DEFAULT_DACL, TOKEN_DUPLICATE, TOKEN_GROUPS,
            TOKEN_INFORMATION_CLASS, TOKEN_MANDATORY_LABEL, TOKEN_OWNER, TOKEN_PRIVILEGES,
            TOKEN_QUERY, TokenDefaultDacl, TokenGroups, TokenIntegrityLevel, TokenOwner,
            TokenPrivileges, TokenUser, WELL_KNOWN_SID_TYPE, WinLocalSystemSid, WinMediumLabelSid,
        },
        System::{
            Console::{
                GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
            },
            Threading::{
                CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
                DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
                GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
                LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcessToken, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW,
                UpdateProcThreadAttribute, WaitForSingleObject,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};
use winsafe::{HACCESSTOKEN, guard::CloseHandleGuard};

/// The integrity RID of Medium, `S-1-16-8192`.
const MEDIUM_RID: u32 = 0x2000;
/// `SE_GROUP_ENABLED` and `SE_GROUP_INTEGRITY` from winnt.h. The windows crate keeps them under
/// `Win32_System_SystemServices`, a feature this crate does not enable.
const GROUP_ENABLED: u32 = 0x4;
const GROUP_INTEGRITY: u32 = 0x20;
/// The NT identifier authority, big-endian as a SID stores it.
const NT_AUTHORITY: [u8; 6] = [0, 0, 0, 0, 0, 5];
/// `S-1-5-32-<RID>`: BUILTIN Administrators and the builtin groups UAC filters with it.
const FILTERED_BUILTIN: [u32; 10] = [544, 547, 548, 549, 550, 551, 553, 554, 556, 569];
/// `S-1-5-21-<domain>-<RID>`: the domain groups UAC filters.
const FILTERED_DOMAIN: [u32; 7] = [498, 512, 516, 518, 519, 520, 521];

/// A primary token derived from this process's own token whose re-read showed Medium integrity and
/// no enabled administrator-equivalent group.
pub(crate) struct StandardToken(CloseHandleGuard<HACCESSTOKEN>);

impl StandardToken {
    fn handle(&self) -> HANDLE {
        HANDLE(self.0.ptr())
    }
}

/// The outcome of a derivation whose calls all succeeded.
pub(crate) enum Derived {
    Standard(StandardToken),
    /// The re-read token kept an integrity above Medium or an enabled filtered group.
    StillElevated,
}

/// Steps 1 to 7: opens this process's token, disables every administrator-equivalent group,
/// deletes every privilege outside the kept set, lowers the integrity to Medium, makes the user the
/// owner, sets a default DACL of the user and SYSTEM, and re-reads the result.
pub(crate) fn derive_standard_token() -> io::Result<Derived> {
    let source = open_own_token()?;
    let user = Information::read(handle(&source), TokenUser)?;
    let derived = restrict(&source)?;
    lower(&derived, &user)?;
    if !is_standard(&derived)? {
        return Ok(Derived::StillElevated);
    }
    let raw = derived.into_raw_handle();
    // SAFETY: `raw` is the token handle `derived` owned alone until `into_raw_handle` released it,
    // so the guard becomes its only owner and closes it once.
    Ok(Derived::Standard(StandardToken(unsafe {
        CloseHandleGuard::new(HACCESSTOKEN::from_ptr(raw))
    })))
}

fn open_own_token() -> io::Result<OwnedHandle> {
    let mut token = HANDLE::default();
    // SAFETY: the current-process pseudo handle needs no closing; on success the system writes a
    // new token handle to the live `token`.
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY | TOKEN_ADJUST_DEFAULT,
            &mut token,
        )
    }
    .map_err(os_error)?;
    // SAFETY: the successful call returned a new handle that nothing else in this process owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(token.0) })
}

/// Steps 2 and 3. The SIDs to disable point into `groups`, which outlives the call.
fn restrict(source: &OwnedHandle) -> io::Result<OwnedHandle> {
    let groups = Information::read(handle(source), TokenGroups)?;
    let mut disable = Vec::new();
    for group in groups.groups()? {
        if is_filtered_group(groups.sid(group.Sid)?) {
            disable.push(SID_AND_ATTRIBUTES {
                Sid: group.Sid,
                Attributes: 0,
            });
        }
    }
    let kept = kept_privileges()?;
    let privileges = Information::read(handle(source), TokenPrivileges)?;
    let delete: Vec<LUID_AND_ATTRIBUTES> = privileges
        .privileges()?
        .iter()
        .filter(|privilege| !kept.contains(&privilege.Luid))
        .map(|privilege| LUID_AND_ATTRIBUTES {
            Luid: privilege.Luid,
            Attributes: Default::default(),
        })
        .collect();
    let mut derived = HANDLE::default();
    // SAFETY: `source` is a live token handle; both lists are live slices, and the disable list's
    // SID pointers reach into `groups`, alive until this call returns; the system writes a new
    // handle to `derived`.
    unsafe {
        CreateRestrictedToken(
            handle(source),
            CREATE_RESTRICTED_TOKEN_FLAGS(0),
            non_empty(&disable),
            non_empty(&delete),
            None,
            &mut derived,
        )
    }
    .map_err(os_error)?;
    // SAFETY: the successful call returned a new handle that nothing else in this process owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(derived.0) })
}

fn non_empty<T>(items: &[T]) -> Option<&[T]> {
    (!items.is_empty()).then_some(items)
}

/// The LUIDs of the privileges a standard token keeps, the set UAC leaves a filtered token.
fn kept_privileges() -> io::Result<Vec<LUID>> {
    [
        w!("SeChangeNotifyPrivilege"),
        w!("SeShutdownPrivilege"),
        w!("SeUndockPrivilege"),
        w!("SeIncreaseWorkingSetPrivilege"),
        w!("SeTimeZonePrivilege"),
    ]
    .into_iter()
    .map(|name| {
        let mut luid = LUID::default();
        // SAFETY: `name` is a static NUL-terminated literal; the system writes to the live `luid`.
        unsafe { LookupPrivilegeValueW(PCWSTR::null(), name, &mut luid) }.map_err(os_error)?;
        Ok(luid)
    })
    .collect()
}

/// A label structure followed by the SID it points at, so the length passed with it covers only
/// memory this value owns.
#[repr(C)]
struct LabelBlock {
    label: TOKEN_MANDATORY_LABEL,
    sid: SidStorage,
}

/// Steps 4 to 6 on the restricted token.
fn lower(derived: &OwnedHandle, user: &Information) -> io::Result<()> {
    let token = handle(derived);
    let mut block = LabelBlock {
        label: TOKEN_MANDATORY_LABEL::default(),
        sid: well_known_sid(WinMediumLabelSid)?,
    };
    block.label.Label = SID_AND_ATTRIBUTES {
        Sid: block.sid.psid(),
        Attributes: GROUP_INTEGRITY,
    };
    set_information(token, TokenIntegrityLevel, &block)?;

    let user = user.sid(user.head()?.Sid)?;
    let owner = TOKEN_OWNER {
        Owner: sid_pointer(user),
    };
    set_information(token, TokenOwner, &owner)?;

    let system = well_known_sid(WinLocalSystemSid)?;
    let mut acl = allow_all(&[user, system.bytes()])?;
    let dacl = TOKEN_DEFAULT_DACL {
        DefaultDacl: acl.as_mut_ptr().cast::<ACL>(),
    };
    set_information(token, TokenDefaultDacl, &dacl)
}

/// Sets one token information class from `value`, whose pointers reach only memory the caller
/// keeps alive across this call.
fn set_information<T>(token: HANDLE, class: TOKEN_INFORMATION_CLASS, value: &T) -> io::Result<()> {
    let length = u32::try_from(size_of::<T>()).map_err(|_| invalid())?;
    // SAFETY: `value` is a live T of exactly `length` bytes, padding included, that the system only
    // reads; every pointer inside it reaches memory the caller keeps alive until this call returns.
    unsafe { SetTokenInformation(token, class, ptr::from_ref(value).cast(), length) }
        .map_err(os_error)
}

/// A DWORD-aligned ACL granting GENERIC_ALL to each SID, in storage sized for exactly its entries.
fn allow_all(sids: &[&[u8]]) -> io::Result<Vec<u64>> {
    let entry = size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>();
    let size = size_of::<ACL>() + sids.iter().map(|sid| entry + sid.len()).sum::<usize>();
    let length = u32::try_from(size).map_err(|_| invalid())?;
    let mut storage = vec![0u64; size.div_ceil(size_of::<u64>())];
    let acl = storage.as_mut_ptr().cast::<ACL>();
    // SAFETY: `storage` is writable, 8-aligned and at least `length` bytes, which is a multiple of
    // four because every SID length is.
    unsafe { InitializeAcl(acl, length, ACL_REVISION) }.map_err(os_error)?;
    for sid in sids {
        // SAFETY: `acl` was initialized above with room for this entry; `sid` is a complete SID the
        // caller keeps alive, which the system copies into the ACL without writing to it.
        unsafe { AddAccessAllowedAce(acl, ACL_REVISION, GENERIC_ALL.0, sid_pointer(sid)) }
            .map_err(os_error)?;
    }
    Ok(storage)
}

/// Step 7: Medium integrity and no enabled group from the filtered set.
fn is_standard(derived: &OwnedHandle) -> io::Result<bool> {
    let label = Information::read(handle(derived), TokenIntegrityLevel)?;
    let integrity =
        sub_authorities(label.sid(label.head()?.Sid)?).and_then(|rids| rids.last().copied());
    let groups = Information::read(handle(derived), TokenGroups)?;
    for group in groups.groups()? {
        if group.Attributes & GROUP_ENABLED != 0 && is_filtered_group(groups.sid(group.Sid)?) {
            return Ok(false);
        }
    }
    Ok(integrity == Some(MEDIUM_RID))
}

/// Whether a SID names BUILTIN Administrators or another group UAC filters from an elevated token.
fn is_filtered_group(sid: &[u8]) -> bool {
    if sid.get(2..8) != Some(&NT_AUTHORITY[..]) {
        return false;
    }
    match sub_authorities(sid).as_deref() {
        Some([32, rid]) => FILTERED_BUILTIN.contains(rid),
        Some([21, _, _, _, rid]) => FILTERED_DOMAIN.contains(rid),
        _ => false,
    }
}

/// The sub-authorities of a complete revision-1 SID.
fn sub_authorities(sid: &[u8]) -> Option<Vec<u32>> {
    let (header, rest) = sid.split_at_checked(8)?;
    if header[0] != 1 || rest.len() != usize::from(header[1]) * 4 {
        return None;
    }
    Some(
        rest.chunks_exact(4)
            .map(|rid| u32::from_le_bytes([rid[0], rid[1], rid[2], rid[3]]))
            .collect(),
    )
}

/// A SID pointer for calls that only read the SID.
fn sid_pointer(sid: &[u8]) -> PSID {
    PSID(sid.as_ptr().cast_mut().cast())
}

/// A well-known SID in storage aligned and sized for any SID.
#[repr(C, align(8))]
struct SidStorage([u8; SECURITY_MAX_SID_SIZE as usize]);

impl SidStorage {
    fn psid(&self) -> PSID {
        sid_pointer(&self.0)
    }

    /// The SID's own bytes, without the unused tail of the storage.
    fn bytes(&self) -> &[u8] {
        &self.0[..8 + 4 * usize::from(self.0[1])]
    }
}

fn well_known_sid(kind: WELL_KNOWN_SID_TYPE) -> io::Result<SidStorage> {
    let mut sid = SidStorage([0; SECURITY_MAX_SID_SIZE as usize]);
    let mut size = SECURITY_MAX_SID_SIZE;
    // SAFETY: the storage is writable, 8-aligned and `size` bytes, the largest any SID needs; the
    // kinds used here need no domain SID.
    unsafe { CreateWellKnownSid(kind, None, Some(PSID(sid.0.as_mut_ptr().cast())), &mut size) }
        .map_err(os_error)?;
    Ok(sid)
}

/// One token information block, read into `u64` storage so the TOKEN_* structure at its start and
/// the SIDs the system places after it are aligned. Every view checks its bounds against the
/// length the system reported.
struct Information {
    storage: Vec<u64>,
    length: usize,
}

impl Information {
    fn read(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> io::Result<Self> {
        let mut needed = 0u32;
        // SAFETY: a null buffer of length zero asks only for the size, written to the live `needed`.
        if let Err(error) = unsafe { GetTokenInformation(token, class, None, 0, &mut needed) }
            && needed == 0
        {
            return Err(os_error(error));
        }
        let mut storage = vec![0u64; (needed as usize).div_ceil(size_of::<u64>())];
        let capacity = u32::try_from(storage.len() * size_of::<u64>()).map_err(|_| invalid())?;
        // SAFETY: `storage` is a live, writable, 8-aligned allocation of `capacity` bytes; the system
        // writes at most `capacity` bytes and the length it used to the live `needed`.
        unsafe {
            GetTokenInformation(
                token,
                class,
                Some(storage.as_mut_ptr().cast()),
                capacity,
                &mut needed,
            )
        }
        .map_err(os_error)?;
        let length = needed as usize;
        if length > storage.len() * size_of::<u64>() {
            return Err(invalid());
        }
        Ok(Self { storage, length })
    }

    /// The bytes the system wrote.
    fn bytes(&self) -> &[u8] {
        // SAFETY: `storage` holds at least `length` initialized bytes (checked in `read`); every
        // byte is a valid `u8`, and the slice borrows `self`, so it cannot outlive the storage.
        unsafe { slice::from_raw_parts(self.storage.as_ptr().cast::<u8>(), self.length) }
    }

    /// The number of entries of a count-prefixed array of `entry` bytes at `offset`, checked to fit
    /// inside the block.
    fn count(&self, offset: usize, entry: usize) -> io::Result<usize> {
        let count = self
            .bytes()
            .first_chunk::<4>()
            .map(|count| u32::from_ne_bytes(*count) as usize)
            .ok_or_else(invalid)?;
        let end = count
            .checked_mul(entry)
            .and_then(|size| size.checked_add(offset))
            .ok_or_else(invalid)?;
        if end > self.length {
            return Err(invalid());
        }
        Ok(count)
    }

    /// The entries of a TOKEN_GROUPS block.
    fn groups(&self) -> io::Result<&[SID_AND_ATTRIBUTES]> {
        let offset = offset_of!(TOKEN_GROUPS, Groups);
        let count = self.count(offset, size_of::<SID_AND_ATTRIBUTES>())?;
        const { assert!(align_of::<SID_AND_ATTRIBUTES>() <= align_of::<u64>()) };
        // SAFETY: `count` entries at `offset` lie inside the initialized block (checked by
        // `count`); the storage is 8-aligned and `offset` is the structure's own field offset; the
        // entries are integers and raw pointers, valid for any bits, and the slice borrows `self`.
        Ok(unsafe {
            slice::from_raw_parts(
                self.storage
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<SID_AND_ATTRIBUTES>(),
                count,
            )
        })
    }

    /// The entries of a TOKEN_PRIVILEGES block.
    fn privileges(&self) -> io::Result<&[LUID_AND_ATTRIBUTES]> {
        let offset = offset_of!(TOKEN_PRIVILEGES, Privileges);
        let count = self.count(offset, size_of::<LUID_AND_ATTRIBUTES>())?;
        const { assert!(align_of::<LUID_AND_ATTRIBUTES>() <= align_of::<u64>()) };
        // SAFETY: as in `groups`; LUID_AND_ATTRIBUTES holds only integers.
        Ok(unsafe {
            slice::from_raw_parts(
                self.storage
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<LUID_AND_ATTRIBUTES>(),
                count,
            )
        })
    }

    /// The single SID_AND_ATTRIBUTES that a TOKEN_USER or TOKEN_MANDATORY_LABEL block starts with.
    fn head(&self) -> io::Result<&SID_AND_ATTRIBUTES> {
        if self.length < size_of::<SID_AND_ATTRIBUTES>() {
            return Err(invalid());
        }
        // SAFETY: both structures are exactly one SID_AND_ATTRIBUTES at offset 0; the block holds
        // that many initialized bytes in 8-aligned storage, and the reference borrows `self`.
        Ok(unsafe { &*self.storage.as_ptr().cast::<SID_AND_ATTRIBUTES>() })
    }

    /// The complete SID an entry of this block points at. The system places it inside the block;
    /// a pointer outside it, or a SID running past its end, is refused.
    fn sid(&self, sid: PSID) -> io::Result<&[u8]> {
        let bytes = self.bytes();
        let start = sid
            .0
            .addr()
            .checked_sub(bytes.as_ptr().addr())
            .ok_or_else(invalid)?;
        let count = bytes.get(start + 1).copied().ok_or_else(invalid)?;
        let sid = bytes
            .get(start..start + 8 + 4 * usize::from(count))
            .ok_or_else(invalid)?;
        sub_authorities(sid).map(|_| sid).ok_or_else(invalid)
    }
}

fn handle(owned: &OwnedHandle) -> HANDLE {
    HANDLE(owned.as_raw_handle())
}

/// The standard handles one child receives. Each is a distinct handle this process owns, so the
/// handle list built from them holds no value twice.
pub(crate) struct Stdio {
    input: Option<OwnedHandle>,
    output: Option<OwnedHandle>,
    error: Option<OwnedHandle>,
}

/// Takes this process's three standard handles into owned values and clears the slots, so nothing
/// else here writes to them and closing them after the start leaves the child their only holder.
/// A slot that repeats an earlier one receives its own duplicate.
pub(crate) fn take_standard_handles() -> io::Result<Stdio> {
    let slots = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];
    // SAFETY: GetStdHandle only reads this process's standard-handle slot.
    let raw = slots.map(|slot| {
        unsafe { GetStdHandle(slot) }
            .ok()
            .filter(|h| !h.is_invalid())
    });
    for slot in slots {
        // SAFETY: clearing a slot only changes what later GetStdHandle calls return; ownership of
        // the handle it named moves to the values built below.
        unsafe { SetStdHandle(slot, HANDLE::default()) }.map_err(os_error)?;
    }
    let mut taken: [Option<OwnedHandle>; 3] = [None, None, None];
    for (index, value) in raw.iter().enumerate() {
        let Some(value) = *value else {
            continue;
        };
        let earlier = (0..index)
            .find(|&before| raw[before] == Some(value))
            .and_then(|before| taken[before].as_ref());
        taken[index] = Some(match earlier {
            Some(first) => first.try_clone()?,
            // SAFETY: no slot names this handle any more and no earlier slot took it, so this value
            // becomes its only owner.
            None => unsafe { OwnedHandle::from_raw_handle(value.0) },
        });
    }
    let [input, output, error] = taken;
    Ok(Stdio {
        input,
        output,
        error,
    })
}

/// A started child; dropping it closes the process handle without waiting.
pub(crate) struct StandardChild {
    process: OwnedHandle,
}

/// Steps 8 and 9: starts `"<application>" <arguments>` with `token`, the inherited environment,
/// directory and desktop, no window, and `stdio` as its only inherited handles; the child stays in
/// this process's Job. This process closes its copies of `stdio` and the thread handle before
/// returning.
pub(crate) fn spawn(
    token: &StandardToken,
    application: &Path,
    arguments: &OsStr,
    stdio: Stdio,
) -> io::Result<StandardChild> {
    let application_name = wide(application.as_os_str());
    let mut line = OsString::from("\"");
    line.push(application);
    line.push("\" ");
    line.push(arguments);
    let mut line = wide(&line);
    let standard = [&stdio.input, &stdio.output, &stdio.error]
        .map(|slot| slot.as_ref().map_or(HANDLE::default(), handle));
    let handles: Box<[HANDLE]> = [&stdio.input, &stdio.output, &stdio.error]
        .into_iter()
        .flatten()
        .map(handle)
        .collect();
    for each in &handles {
        // SAFETY: each value is a live handle `stdio` owns until this function returns.
        unsafe { SetHandleInformation(*each, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
            .map_err(os_error)?;
    }
    let inherit = !handles.is_empty();
    let mut attributes = if inherit {
        Some(AttributeList::with_handles(handles)?)
    } else {
        None
    };

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    [
        startup.StartupInfo.hStdInput,
        startup.StartupInfo.hStdOutput,
        startup.StartupInfo.hStdError,
    ] = standard;
    // The inherited environment block is Unicode, and the API requires saying so even when the
    // block is the parent's own.
    let mut flags = CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT;
    startup.StartupInfo.cb = match attributes.as_mut() {
        Some(list) => {
            startup.lpAttributeList = list.as_list();
            flags |= EXTENDED_STARTUPINFO_PRESENT;
            size_of::<STARTUPINFOEXW>()
        }
        None => size_of::<STARTUPINFOW>(),
    } as u32;
    let mut information = PROCESS_INFORMATION::default();
    // SAFETY: the token is a live primary token; both strings are live NUL-terminated buffers and the
    // command line is writable, as the call requires; `startup` is a live STARTUPINFOEXW whose
    // pointer keeps the whole structure in reach, its attribute list stays initialized until
    // `attributes` drops below, and every handle it names is live; the system writes the new
    // handles to the live `information`.
    unsafe {
        CreateProcessAsUserW(
            Some(token.handle()),
            PCWSTR(application_name.as_ptr()),
            Some(PWSTR(line.as_mut_ptr())),
            None,
            None,
            inherit,
            flags,
            None,
            PCWSTR::null(),
            ptr::from_ref(&startup).cast::<STARTUPINFOW>(),
            &mut information,
        )
    }
    .map_err(os_error)?;
    // SAFETY: the successful call returned new process and thread handles nothing else owns.
    let (process, thread) = unsafe {
        (
            OwnedHandle::from_raw_handle(information.hProcess.0),
            OwnedHandle::from_raw_handle(information.hThread.0),
        )
    };
    drop(thread);
    drop(attributes);
    drop(stdio);
    Ok(StandardChild { process })
}

impl StandardChild {
    /// Step 10: waits for the child to exit and returns its exit status.
    pub(crate) fn wait(self) -> io::Result<u32> {
        let process = handle(&self.process);
        // SAFETY: `process` is a live process handle `self` owns.
        if unsafe { WaitForSingleObject(process, INFINITE) } != WAIT_OBJECT_0 {
            return Err(io::Error::last_os_error());
        }
        let mut status = 0u32;
        // SAFETY: as above; the system writes the status to the live `status`.
        unsafe { GetExitCodeProcess(process, &mut status) }.map_err(os_error)?;
        Ok(status)
    }
}

/// An initialized one-attribute list naming the handles a child may inherit. `handles` stays
/// allocated and in place while the list refers to it; drop deletes the list before freeing either.
struct AttributeList {
    storage: Vec<u64>,
    handles: Box<[HANDLE]>,
}

impl AttributeList {
    fn with_handles(handles: Box<[HANDLE]>) -> io::Result<Self> {
        let mut size = 0usize;
        // SAFETY: a null list asks only for the size of a one-attribute list, written to the live
        // `size`; the call reports the expected insufficient-buffer error, judged by `size` below.
        let probe = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut size) };
        if size == 0 {
            return Err(probe.err().map_or_else(invalid, os_error));
        }
        let mut storage = vec![0u64; size.div_ceil(size_of::<u64>())];
        // SAFETY: `storage` is writable, 8-aligned and at least `size` bytes.
        unsafe {
            InitializeProcThreadAttributeList(
                Some(LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast())),
                1,
                None,
                &mut size,
            )
        }
        .map_err(os_error)?;
        let mut list = Self { storage, handles };
        let pointer = list.as_list();
        let value = list.handles.as_ptr();
        let bytes = size_of_val(&*list.handles);
        // SAFETY: the list is initialized; `value` points at `bytes` bytes of distinct live handles
        // in `list.handles`, a heap allocation that stays in place until drop deletes the list.
        unsafe {
            UpdateProcThreadAttribute(
                pointer,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(value.cast()),
                bytes,
                None,
                None,
            )
        }
        .map_err(os_error)?;
        Ok(list)
    }

    fn as_list(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.storage.as_mut_ptr().cast())
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        let list = self.as_list();
        // SAFETY: `with_handles` initialized the list before this value existed, and drop runs once.
        unsafe { DeleteProcThreadAttributeList(list) }
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}

/// The Win32 error a failed call reported, or the whole HRESULT when it carries no Win32 code.
fn os_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0 as u32;
    let win32 = if code & 0xFFFF_0000 == 0x8007_0000 {
        code & 0xFFFF
    } else {
        code
    };
    io::Error::from_raw_os_error(win32 as i32)
}

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
