//! Exact namespace lookup with immediate Win32 error capture and one search-handle owner.

use std::{ffi::c_void, io};
use widestring::U16CString;
use winsafe::{HFINDFILE, WIN32_FIND_DATA, co, guard::FindCloseGuard, prelude::Handle};

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "FindFirstFileW"]
    fn find_first_file(name: *const u16, data: *mut WIN32_FIND_DATA) -> *mut c_void;
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

#[cfg(test)]
#[deny(unsafe_code)]
mod tests;
