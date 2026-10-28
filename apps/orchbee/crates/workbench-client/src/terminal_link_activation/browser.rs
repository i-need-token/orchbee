//! Registered browser capability; link resolution and plugin routing remain platform independent.
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "browser/unix.rs"]
mod platform;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Unsupported,
    Failed,
}

pub(super) struct SystemBrowser;
impl SystemBrowser {
    pub(super) fn open(&mut self, url: &str) -> Result<(), Error> {
        crate::tui_capabilities::Capability::Browser
            .require()
            .map_err(|_| Error::Unsupported)?;
        platform::open(url).map_err(|_| Error::Failed)
    }
}
