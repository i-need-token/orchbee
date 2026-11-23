//! Runs one frozen plugin command with bounded captured output, cancellation, and a sanitized
//! Orchbee context, mapping physical process outcomes into its retained log. It does not
//! select a command or own the job slot and receipt history.

use super::{PluginCommandIntent, PluginCommandLog};
use std::{process::Command, sync::atomic::AtomicBool};

#[cfg(windows)]
pub(super) fn run(
    intent: PluginCommandIntent,
    mut log: PluginCommandLog,
    cancel: &AtomicBool,
) -> PluginCommandLog {
    use super::{Failure, now_ms};
    use orchbee_local_runtime::{CommandFailure, resolve_windows_program, run_windows_command};
    use std::{path::Path, process::Stdio, sync::atomic::Ordering};
    let mut command = Command::new(resolve_windows_program(
        &intent.argv[0],
        Path::new(&intent.directory),
    ));
    command
        .args(&intent.argv[1..])
        .current_dir(&intent.directory)
        .stdin(Stdio::null());
    clear_context(&mut command);
    command.envs(intent.environment);
    let output = run_windows_command(command, None, || Ok(cancel.load(Ordering::Relaxed)));
    log.finished_ms = Some(now_ms().max(log.started_ms));
    log.exit_code = output.exit_code;
    log.stdout = output.stdout;
    log.stderr = output.stderr;
    log.stdout_truncated = output.stdout_truncated;
    log.stderr_truncated = output.stderr_truncated;
    log.failure = output.failure.map(|reason| match reason {
        CommandFailure::ExitFailure => Failure::ExitFailure,
        CommandFailure::SpawnUnavailable => Failure::SpawnUnavailable,
        CommandFailure::SpawnFailed => Failure::SpawnFailed,
        CommandFailure::Io | CommandFailure::Timeout => Failure::Io,
        CommandFailure::Cancelled => Failure::Cancelled,
        CommandFailure::PhysicalUnknown => Failure::PhysicalUnknown,
    });
    log
}

/// Plugin processes run only where plugin stores exist, on Windows. The plugin-job operation
/// refuses them elsewhere before they reach this owner; this adapter keeps any other caller's
/// outcome typed instead of starting a process.
#[cfg(not(windows))]
pub(super) fn run(
    _intent: PluginCommandIntent,
    log: PluginCommandLog,
    _cancel: &AtomicBool,
) -> PluginCommandLog {
    super::finish_failure(log, super::Failure::SpawnUnavailable)
}

pub(super) fn clear_context(command: &mut Command) {
    for name in context_removals() {
        command.env_remove(name);
    }
}

/// The variables a command must not inherit. Context belongs to this invocation: never leak a
/// parent plugin's identifiers.
pub(super) fn context_removals() -> Vec<std::ffi::OsString> {
    let plugin = std::env::vars_os().map(|(name, _)| name).filter(|name| {
        name.to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("ORCHBEE_PLUGIN_")
    });
    let fixed = [
        "ORCHBEE_WORKSPACE_ID",
        "ORCHBEE_TAB_ID",
        "ORCHBEE_PANEL_ID",
        "ORCHBEE_TERMINAL_ID",
        "ORCHBEE_WORKER_ID",
        // A Server started by an announced TUI inherits its launcher's relaunch variable.
        orchbee_local_runtime::RELAUNCH_PROTOCOL_VARIABLE,
    ]
    .map(std::ffi::OsString::from);
    plugin.chain(fixed).collect()
}
