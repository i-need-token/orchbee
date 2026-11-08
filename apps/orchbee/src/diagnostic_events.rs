//! The Client's diagnostic log records (ORC-397): the TUI's start and end, and the outcomes of the
//! verbs that start or relay to the local Session. Every other command writes none, so reads and
//! the offline commands create no directory. Records carry only tokens, counts and exit statuses.
use crate::local_activation::StartupFailure;
use orchbee_local_runtime::{LogLevel, LogRole, LogValue, install_diagnostic_log, log_event};

const VERSION: LogValue = LogValue::Token(env!("CARGO_PKG_VERSION"));

pub(crate) fn tui_started() {
    install_diagnostic_log(LogRole::Client);
    log_event(LogLevel::Info, "tui-started", &[("version", VERSION)]);
}

pub(crate) fn tui_exited(outcome: Result<(), StartupFailure>) {
    let (level, outcome) = match outcome {
        Ok(()) => (LogLevel::Info, "ok"),
        Err(failure) => (LogLevel::Error, startup_failure(failure)),
    };
    log_event(
        level,
        "tui-exited",
        &[("outcome", LogValue::Token(outcome))],
    );
}

/// Records one activation verb's start before it runs.
pub(crate) fn activation_started(event: &'static str) {
    install_diagnostic_log(LogRole::Client);
    log_event(LogLevel::Info, event, &[("version", VERSION)]);
}

/// Records the exit status an activation verb answered.
pub(crate) fn activation_finished(event: &'static str, status: u8) {
    let level = if status == 0 {
        LogLevel::Info
    } else {
        LogLevel::Error
    };
    log_event(
        level,
        event,
        &[("status", LogValue::Count(u64::from(status)))],
    );
}

const fn startup_failure(failure: StartupFailure) -> &'static str {
    match failure {
        StartupFailure::AlreadyInSession => "already-in-session",
        StartupFailure::Terminal => "terminal",
        StartupFailure::Cohort => "cohort",
        StartupFailure::Runtime => "runtime",
        StartupFailure::SocketPathTooLong => "socket-path-too-long",
        StartupFailure::RecoveryRequired => "recovery-required",
        StartupFailure::Incompatible => "incompatible",
        StartupFailure::Admission => "admission",
        StartupFailure::TimedOut => "timed-out",
        StartupFailure::TerminalUnavailable => "terminal-unavailable",
        StartupFailure::OutcomeUnknown => "outcome-unknown",
        StartupFailure::Interface => "interface",
        StartupFailure::Configuration => "configuration",
    }
}
