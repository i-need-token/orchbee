use super::{Failure, PluginCommandIntent, PluginCommandLog, now_ms};
use orchbee_local_site_runtime::{
    WindowsCommandFailure, resolve_windows_program, run_windows_command,
};
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) fn run(
    intent: PluginCommandIntent,
    mut log: PluginCommandLog,
    cancel: &AtomicBool,
) -> PluginCommandLog {
    let mut command = Command::new(resolve_windows_program(
        &intent.argv[0],
        Path::new(&intent.directory),
    ));
    command
        .args(&intent.argv[1..])
        .current_dir(&intent.directory)
        .stdin(Stdio::null());
    // Context belongs to this invocation. Never leak a parent plugin's identifiers.
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("ORCHBEE_PLUGIN_")
        {
            command.env_remove(name);
        }
    }
    for name in [
        "ORCHBEE_WORKSPACE_ID",
        "ORCHBEE_TAB_ID",
        "ORCHBEE_PANE_ID",
        "ORCHBEE_TERMINAL_ID",
        "ORCHBEE_WORKER_ID",
    ] {
        command.env_remove(name);
    }
    command.envs(intent.environment);
    let output = run_windows_command(command, None, || Ok(cancel.load(Ordering::Relaxed)));
    log.finished_ms = Some(now_ms().max(log.started_ms));
    log.exit_code = output.exit_code;
    log.stdout = output.stdout;
    log.stderr = output.stderr;
    log.stdout_truncated = output.stdout_truncated;
    log.stderr_truncated = output.stderr_truncated;
    log.failure = output.failure.map(|reason| match reason {
        WindowsCommandFailure::ExitFailure => Failure::ExitFailure,
        WindowsCommandFailure::SpawnUnavailable => Failure::SpawnUnavailable,
        WindowsCommandFailure::SpawnFailed => Failure::SpawnFailed,
        WindowsCommandFailure::Io | WindowsCommandFailure::Timeout => Failure::Io,
        WindowsCommandFailure::Cancelled => Failure::Cancelled,
        WindowsCommandFailure::PhysicalUnknown => Failure::PhysicalUnknown,
    });
    log
}
