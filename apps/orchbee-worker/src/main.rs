#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "default launch resolution is staged for the authorized Host adapter"
    )
)]
#[cfg(unix)]
mod default_interactive_launch;
#[cfg(unix)]
mod host_connection;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the authorized Host-operation kernel is staged before its carrier adapter"
    )
)]
mod terminal_authorization;
#[cfg(unix)]
mod worker_connection;
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
