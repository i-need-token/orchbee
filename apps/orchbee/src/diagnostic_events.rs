//! The Client's diagnostic log records (ORC-397): the TUI's start and end, the outcomes of the
//! verbs that start or relay to the local Session, and the link helper's token rights and Session
//! starts. Every other command writes none, so reads and the offline commands create no directory.
//! Records carry only tokens, counts and exit statuses.
use crate::local_activation::StartupFailure;
use orchbee_local_runtime::{LogLevel, LogRole, LogValue, install_diagnostic_log, log_event};

const VERSION: LogValue = LogValue::Token(env!("CARGO_PKG_VERSION"));

pub(crate) fn tui_started() {
    install_diagnostic_log(LogRole::Client);
    log_event(LogLevel::Info, "tui-started", &[("version", VERSION)]);
}

pub(crate) fn tui_exited(outcome: &Result<(), StartupFailure>) {
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

/// Starts the link helper's log and records the rights its token passed the gate with:
/// `link-rights` with `level` (`low` or `medium`), `admin` (`deny-only` or `absent`) and `elevated`
/// (`TokenIsElevated`, 0 or 1, recorded only). The helper's `ensure-session` then writes what the
/// shared activation records for a start: `session-start-source` and, for a Server it starts,
/// `server-job`.
#[cfg(windows)]
pub(crate) fn link_started(rights: Option<orchbee_local_runtime::TokenRights>) {
    use orchbee_local_runtime::{Administrators, IntegrityLevel};
    install_diagnostic_log(LogRole::Client);
    let Some(rights) = rights else {
        return;
    };
    let level = match rights.level {
        IntegrityLevel::Low => "low",
        IntegrityLevel::Medium => "medium",
        IntegrityLevel::High => "high",
    };
    let admin = match rights.administrators {
        Administrators::Enabled => "enabled",
        Administrators::DenyOnly => "deny-only",
        Administrators::Absent => "absent",
    };
    log_event(
        LogLevel::Info,
        "link-rights",
        &[
            ("level", LogValue::Token(level)),
            ("admin", LogValue::Token(admin)),
            ("elevated", LogValue::Count(u64::from(rights.elevated))),
        ],
    );
}

/// Records why an elevated link helper refused to relaunch with standard rights: `link-rights`
/// with `outcome=refused` and `reason` `derive`, `still-elevated` or `start`. With
/// [`link_child_failed`], these are the elevated process's only writes.
#[cfg(windows)]
pub(crate) fn link_rights_refused(failure: orchbee_local_runtime::RelaunchFailure) {
    use orchbee_local_runtime::RelaunchFailure;
    install_diagnostic_log(LogRole::Client);
    let reason = match failure {
        RelaunchFailure::Derive => "derive",
        RelaunchFailure::StillElevated => "still-elevated",
        RelaunchFailure::Start => "start",
    };
    log_event(
        LogLevel::Error,
        "link-rights",
        &[
            ("outcome", LogValue::Token("refused")),
            ("reason", LogValue::Token(reason)),
        ],
    );
}

/// Records a relaunched link helper's exit status that no exit code can relay, one above 255:
/// `link-rights` with `outcome=child-failed` and `status`.
#[cfg(windows)]
pub(crate) fn link_child_failed(status: u32) {
    install_diagnostic_log(LogRole::Client);
    log_event(
        LogLevel::Error,
        "link-rights",
        &[
            ("outcome", LogValue::Token("child-failed")),
            ("status", LogValue::Count(u64::from(status))),
        ],
    );
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

const fn startup_failure(failure: &StartupFailure) -> &'static str {
    match failure {
        StartupFailure::AlreadyInSession => "already-in-session",
        StartupFailure::Terminal => "terminal",
        StartupFailure::Cohort => "cohort",
        StartupFailure::Runtime | StartupFailure::Refused(_) => "runtime",
        StartupFailure::SocketPathTooLong => "socket-path-too-long",
        StartupFailure::RecoveryRequired => "recovery-required",
        StartupFailure::ClientTooOld(_) => "client-too-old",
        StartupFailure::SessionTooOld(_) => "session-too-old",
        StartupFailure::OtherEnvironment => "other-environment",
        StartupFailure::DevelopmentBuildMismatch => "development-build-mismatch",
        StartupFailure::ElevatedSession => "admission",
        StartupFailure::Admission => "admission",
        StartupFailure::TimedOut => "timed-out",
        StartupFailure::TerminalUnavailable => "terminal-unavailable",
        StartupFailure::OutcomeUnknown => "outcome-unknown",
        StartupFailure::Interface => "interface",
        StartupFailure::Configuration => "configuration",
    }
}
