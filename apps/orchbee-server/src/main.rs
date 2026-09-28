#[cfg(any(unix, windows))]
mod agent;
#[cfg(test)]
mod agent_screen;
#[cfg(any(unix, windows))]
mod agent_service;
#[cfg(any(unix, windows))]
mod client_session_registry;
#[cfg(any(windows, test))]
mod client_view_registry;
#[cfg(any(unix, windows))]
mod local_client_service;
#[cfg(any(unix, windows))]
mod local_site_service;
#[cfg(any(unix, windows))]
mod local_terminal_service;
#[cfg(any(unix, windows))]
mod site_coordinator;
#[cfg(any(windows, test))]
mod site_shutdown;
#[cfg(any(unix, windows))]
mod terminal_coordinator;
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
    if local_site_service::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}

#[cfg(all(test, windows))]
mod test_support;
