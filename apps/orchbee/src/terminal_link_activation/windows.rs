//! Windows shell browser dispatch; an accepted request does not transfer browser ownership.
use winsafe::prelude::Handle;
pub(super) fn open(url: &str) -> Result<(), ()> {
    winsafe::HWND::NULL
        .ShellExecute("open", url, None, None, winsafe::co::SW::SHOWNORMAL)
        .map_err(|_| ())
}
