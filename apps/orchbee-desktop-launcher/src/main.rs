//! The window launcher. On Windows it is installed as `desktop\orchbee-desktop.exe` in the release
//! store, the path every shortcut names. It reads the window pointer `active.json` beside it and
//! starts `versions\<active>\<its own file name>` with its own arguments and environment in that
//! version's folder. Asked to relaunch, it first waits for the window that asked; when the active
//! window cannot start or soon ends with a failure, it starts the pointer's previous version once.
//!
//! On macOS the application bundle is the entry, and the launcher runs only to relaunch, from the
//! cohort of a window version's bundle in the store: once the asking window has ended, it replaces
//! the application that window ran from with the active window version and starts it, and puts the
//! replaced application back when the new one fails soon after. Neither moves a pointer, records
//! anything, or owns the window it starts.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(any(windows, target_os = "macos"))]
mod decision;
#[cfg(not(target_os = "macos"))]
mod localization;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod replacement;
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

/// Exit code `1`: the request is no relaunch the launcher serves, or no window started.
#[cfg(target_os = "macos")]
fn main() -> ExitCode {
    macos::launch().map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS)
}

/// Exit code `4`: this platform cannot serve the request.
#[cfg(not(any(windows, target_os = "macos")))]
fn main() -> ExitCode {
    let tag = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|value| !value.is_empty());
    let locale = localization::locale(tag.as_deref().and_then(std::ffi::OsStr::to_str));
    eprintln!("{}", localization::unsupported_platform(locale));
    ExitCode::from(4)
}
