//! Windows system-browser adapter for native link activation; it reports only whether the shell
//! accepted one request and never claims ownership of the resulting browser process.

use winsafe::prelude::Handle;

/// Stateless port that submits one URL to the registered Windows shell handler.
pub(super) struct SystemBrowser;

impl SystemBrowser {
    /// Submit the URL once and report whether the Windows shell accepted that request.
    pub(super) fn open(&mut self, url: &str) -> Result<(), ()> {
        winsafe::HWND::NULL
            .ShellExecute("open", url, None, None, winsafe::co::SW::SHOWNORMAL)
            .map_err(|_| ())
    }
}
