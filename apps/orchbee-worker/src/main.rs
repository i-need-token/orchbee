#[cfg(any(unix, windows))]
mod agent_drain;
#[cfg(any(unix, windows))]
mod default_interactive_launch;
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
mod terminal_viewer_operation;
#[cfg(windows)]
mod windows_command_child;
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
    #[cfg(any(unix, windows))]
    if worker_connection::run(env::args_os().skip(1), env!("CARGO_PKG_VERSION")).is_ok() {
        return ExitCode::SUCCESS;
    }

    ExitCode::FAILURE
}

#[cfg(all(test, windows))]
mod test_support;
