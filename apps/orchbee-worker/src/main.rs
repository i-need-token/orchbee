//! Worker process composition for one admitted Server connection and its authorized Host and job
//! operations. Runtime modules retain child ownership and bounded settlement; this process does
//! not choose product authority, own Workbench state or execute PTY/VT behavior itself.

#[cfg(any(unix, windows))]
mod agent_drain;
#[cfg(any(unix, windows))]
mod agent_integration_jobs;
mod agent_probe_jobs;
#[cfg(any(unix, windows))]
mod default_interactive_launch;
#[cfg(any(unix, windows))]
mod diagnostic_events;
#[cfg(windows)]
mod editor_documents;
#[cfg(windows)]
mod git_repository_windows;
#[cfg(any(unix, windows))]
mod host_connection;
#[cfg(any(unix, windows))]
mod plugin_job_operation;
#[cfg(windows)]
mod plugin_jobs;
#[cfg(any(unix, windows, test))]
mod terminal_authorization;
mod terminal_control_operation;
#[cfg(any(unix, windows))]
mod terminal_cwd_validation;
#[cfg(any(unix, windows))]
mod terminal_editor_operation;
#[cfg(any(unix, windows, test))]
mod terminal_host_launch_adapter;
#[cfg(any(unix, windows))]
mod terminal_host_operation;
#[cfg(any(unix, windows))]
mod terminal_lifecycle_operation;
#[cfg(any(unix, windows))]
mod terminal_operations;
#[cfg(any(unix, windows))]
mod terminal_send_operation;
#[cfg(any(unix, windows))]
mod terminal_theme;
#[cfg(any(unix, windows))]
mod terminal_viewer_operation;
#[cfg(any(unix, windows))]
mod worker_connection;
#[cfg(any(unix, windows))]
mod worker_identity;
#[cfg(any(unix, windows))]
mod worker_options;
#[cfg(any(unix, windows))]
mod worktree_discovery_job;
#[cfg(any(unix, windows))]
mod worktree_job_operation;
#[cfg(windows)]
mod worktree_mutation_job;
#[cfg(any(unix, windows))]
mod worktree_mutation_operation;
#[cfg(windows)]
mod worktree_mutation_windows;

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
    orchbee_local_runtime::install_diagnostic_log(orchbee_local_runtime::LogRole::Worker);
    #[cfg(any(unix, windows))]
    if worker_connection::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}

#[cfg(all(test, windows))]
mod tests;
