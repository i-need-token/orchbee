//! Shared local-startup verdicts; raw causes and execution stay with activation.
//! This vocabulary is independent of CLI output, TUI selection and localized copy.

/// The single local-startup verdict vocabulary shared below activation and its UI adapters.
/// The activation core itself only ever emits `Cohort`, `Runtime`, `SocketPathTooLong`,
/// `RecoveryRequired`, one of the handshake refusals, `Admission`, or `TimedOut`;
/// `Terminal`, `TerminalUnavailable`, `OutcomeUnknown`, and `Interface` are
/// produced exclusively by the TUI adapter's own terminal stages and live
/// in this one enum so both callers classify the same verdict type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupFailure {
    AlreadyInSession,
    Terminal,
    Cohort,
    Runtime,
    SocketPathTooLong,
    RecoveryRequired,
    /// The running Session supports only newer Clients.
    ClientTooOld(Box<RefusedVersions>),
    /// The running Session is older than this Client supports, or predates the handshake.
    SessionTooOld(Box<RefusedVersions>),
    /// The running Session belongs to another environment.
    OtherEnvironment,
    /// The running Session is another development build.
    DevelopmentBuildMismatch,
    Admission,
    TimedOut,
    TerminalUnavailable,
    OutcomeUnknown,
    Interface,
    Configuration,
}

/// The product versions a generation refusal names; a peer from before the handshake states no
/// version, so the Session's may be unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefusedVersions {
    pub client: String,
    pub session: Option<String>,
}
