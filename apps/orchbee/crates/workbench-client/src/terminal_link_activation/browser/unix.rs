//! Linux and macOS browser dispatch through the desktop opener (`open` or `xdg-open`). The URL
//! is the opener's only argument and no shell sees it; an accepted request does not transfer
//! browser ownership.
use crate::host_capabilities::unix::{self as host, Exit, Overrun};
use std::{ffi::OsStr, time::Duration};

/// Wait for the opener's verdict. An opener still running then is presenting the page itself, for
/// example a browser started in the foreground, and keeps running.
const OPENER_BUDGET: Duration = Duration::from_secs(2);

pub(super) fn open(url: &str) -> Result<(), ()> {
    let Some(opener) = host::facts().browser.as_ref() else {
        return Err(());
    };
    match host::run(
        &opener.path,
        &[OsStr::new(url)],
        None,
        OPENER_BUDGET,
        Overrun::Release,
    ) {
        Exit::Succeeded | Exit::Overran => Ok(()),
        Exit::Failed => Err(()),
    }
}
