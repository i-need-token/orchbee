#[cfg(unix)]
mod host_connection;
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
