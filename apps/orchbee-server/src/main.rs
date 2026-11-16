//! Local Session service composition for Client admission, Workbench authority, Worker coordination
//! and owned checkpoint lifecycle. Domain modules own transitions; Worker and Terminal Host
//! retain physical terminal execution and Clients retain presentation.

#[cfg(any(unix, windows))]
mod agent;
#[cfg(any(unix, windows))]
mod agent_catalog;
#[cfg(any(unix, windows))]
mod agent_screen;
#[cfg(any(unix, windows))]
mod agent_service;
// Only Windows keeps a release store yet, so only a Windows Server schedules automatic updates.
#[cfg(windows)]
mod automatic_updates;
#[cfg(any(unix, windows))]
mod client_connection_registry;
#[cfg(any(unix, windows))]
mod client_view_registry;
#[cfg(any(unix, windows))]
mod fault_records;
#[cfg(any(unix, windows))]
mod frame_directory;
// The rule names no platform type; only the Windows Session wires it yet.
#[cfg(any(windows, test))]
mod idle_move;
#[cfg(any(unix, windows))]
mod local_client_service;
#[cfg(any(unix, windows))]
mod local_terminal_service;
#[cfg(any(unix, windows))]
mod local_worker_service;
#[cfg(any(unix, windows))]
#[cfg(windows)]
mod localization;
#[cfg(any(unix, windows))]
mod notification_registry;
#[cfg(any(unix, windows))]
mod session_coordinator;
#[cfg(any(unix, windows))]
mod session_shutdown;
#[cfg(any(unix, windows))]
mod terminal_coordinator;
#[cfg(any(unix, windows))]
#[cfg(windows)]
mod terminal_output;
#[cfg(any(unix, windows))]
mod terminal_titles;
#[cfg(any(unix, windows))]
mod viewer_ready;
#[cfg(any(unix, windows))]
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
    // A root whose layout this build cannot read is refused before anything in it is read, the
    // log included; the Client that started this process names the refusal to the person.
    if orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
        .is_ok_and(|root| root.admit_layout().is_err())
    {
        return ExitCode::FAILURE;
    }
    #[cfg(any(unix, windows))]
    orchbee_local_runtime::install_diagnostic_log(orchbee_local_runtime::LogRole::Server);
    // A panic ends only its own thread, so its site is the only trace a silent partial outage leaves.
    #[cfg(any(unix, windows))]
    fault_records::install_panic_hook();
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
