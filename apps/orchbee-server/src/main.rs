//! Local Session service composition for Client admission, Workbench authority, Worker coordination
//! and owned checkpoint lifecycle. Domain modules own transitions; Worker and Terminal Host
//! retain physical terminal execution and Clients retain presentation.

#![cfg_attr(
    target_os = "macos",
    expect(
        dead_code,
        reason = "the Apple Silicon development Session omits Windows checkpoint, stop, and Agent-recovery paths"
    )
)]

#[cfg(any(unix, windows))]
mod agent;
mod agent_screen;
#[cfg(any(unix, windows))]
mod agent_service;
#[cfg(any(unix, windows))]
mod client_connection_registry;
mod client_view_registry;
#[cfg(windows)]
mod frame_directory;
#[cfg(any(unix, windows))]
mod local_client_service;
#[cfg(any(unix, windows))]
mod local_terminal_service;
#[cfg(any(unix, windows))]
mod local_worker_service;
#[cfg(windows)]
mod notification_registry;
#[cfg(any(unix, windows))]
mod session_coordinator;
#[cfg(any(windows, test))]
mod session_shutdown;
#[cfg(any(unix, windows))]
mod terminal_coordinator;
#[cfg(windows)]
mod window_title_registry;
#[cfg(any(unix, windows))]
mod workbench_service;
#[cfg(any(unix, windows))]
mod worker_obcn_carrier;
#[cfg(any(unix, windows))]
mod worker_registry;

#[cfg(any(unix, windows))]
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(any(unix, windows))]
    if local_worker_service::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}

#[cfg(all(test, windows))]
mod tests;

#[cfg(test)]
mod test_support;
