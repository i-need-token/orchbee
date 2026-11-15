//! The Windows window launcher, installed as `desktop\orchbee-desktop.exe` in the release store,
//! the path every shortcut names. It reads the window pointer `active.json` beside it and starts
//! `versions\<active>\<its own file name>` with its own arguments and environment in that version's
//! folder. Asked to relaunch, it first waits for the window that asked; when the active window
//! cannot start or soon ends with a failure, it starts the pointer's previous version once. It
//! moves no pointer, records nothing, and does not own the window it starts.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod decision;
mod localization;
#[cfg(windows)]
mod windows;

use std::process::ExitCode;

#[cfg(windows)]
fn main() -> ExitCode {
    match windows::launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            windows::show(&failure);
            // Exit code `1`: no window started.
            ExitCode::FAILURE
        }
    }
}

/// Exit code `4`: this platform cannot serve the request.
#[cfg(not(windows))]
fn main() -> ExitCode {
    let tag = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|value| !value.is_empty());
    let locale = localization::locale(tag.as_deref().and_then(std::ffi::OsStr::to_str));
    eprintln!("{}", localization::unsupported_platform(locale));
    ExitCode::from(4)
}
