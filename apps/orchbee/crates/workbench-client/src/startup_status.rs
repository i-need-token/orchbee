//! Shared local-startup verdicts; raw causes and execution stay with activation.
//! This vocabulary is independent of CLI output, TUI selection and localized copy.

/// The single local-startup verdict vocabulary shared below activation and its UI adapters.
/// The activation core itself only ever emits `Cohort`, `Runtime`, `Refused`,
/// `SocketPathTooLong`, `RecoveryRequired`, `RecoveryBlocked`, `ServerNotStarted`, one of the
/// handshake refusals, `ElevatedSession`, `Admission`, or `TimedOut`;
/// `Terminal`, `TerminalUnavailable`, `OutcomeUnknown`, and `Interface` are
/// produced exclusively by the TUI adapter's own terminal stages and live
/// in this one enum so both callers classify the same verdict type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupFailure {
    AlreadyInSession,
    Terminal,
    Cohort,
    Runtime,
    /// A data or socket folder, or a file in one, failed the ownership, permission, link, or
    /// file-lock rules; the refusal names the item at fault.
    Refused(Box<orchbee_local_runtime::AdmissionRefusal>),
    SocketPathTooLong,
    RecoveryRequired,
    /// A process an earlier start registered is still running, so the Session cannot recover;
    /// ending it, or restarting the computer, lets the next start recover by itself.
    RecoveryBlocked(orchbee_local_runtime::RecoveryBlocker),
    /// The system refused to create the Server process. Nothing ran, so the next start recovers by
    /// itself once the cause is gone.
    ServerNotStarted(ServerSpawnFailure),
    /// The running Session supports only newer Clients.
    ClientTooOld(Box<RefusedVersions>),
    /// The running Session is older than this Client supports, or predates the handshake.
    SessionTooOld(Box<RefusedVersions>),
    /// The running Session belongs to another environment.
    OtherEnvironment,
    /// The running Session is another development build.
    DevelopmentBuildMismatch,
    /// The Session runs as administrator and refused this standard Client (Windows).
    ElevatedSession,
    Admission,
    TimedOut,
    TerminalUnavailable,
    OutcomeUnknown,
    Interface,
    Configuration,
}

/// What the system answered when it refused to create the Server process: the error's kind and,
/// when it stated one, its operating-system error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerSpawnFailure {
    pub kind: std::io::ErrorKind,
    pub os_error: Option<i32>,
}

/// The product versions a generation refusal names; a peer from before the handshake states no
/// version, so the Session's may be unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedVersions {
    pub client: String,
    pub session: Option<String>,
}
