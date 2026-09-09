#[cfg(unix)]
mod default_interactive_launch;
#[cfg(unix)]
mod host_connection;
#[cfg(any(unix, test))]
mod terminal_authorization;
mod terminal_control_operation;
#[cfg(unix)]
mod terminal_cwd_validation;
#[cfg(any(unix, test))]
mod terminal_host_launch_adapter;
#[cfg(unix)]
mod terminal_host_operation;
#[cfg(unix)]
mod terminal_lifecycle_operation;
#[cfg(unix)]
mod terminal_operations;
#[cfg(unix)]
mod terminal_viewer_operation;
#[cfg(unix)]
mod worker_connection;
#[cfg(unix)]
mod worker_identity;
#[cfg(unix)]
mod worker_options;

#[cfg(unix)]
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(unix)]
    if worker_connection::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}
