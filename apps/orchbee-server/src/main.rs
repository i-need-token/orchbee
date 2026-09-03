#[cfg(unix)]
mod client_session_registry;
#[cfg(unix)]
mod local_client_service;
#[cfg(unix)]
mod local_site_service;
#[cfg(unix)]
mod worker_registry;

#[cfg(unix)]
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(unix)]
    if local_site_service::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}
