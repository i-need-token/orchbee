//! Shared local-startup verdicts; raw causes and execution stay with activation.
//! This vocabulary is independent of CLI output, TUI selection and localized copy.

/// The single local-startup verdict vocabulary shared below activation and its UI adapters.
/// The activation core itself only ever emits `Cohort`, `Runtime`, `SocketPathTooLong`,
/// `RecoveryRequired`, `Incompatible`, `Admission`, or `TimedOut`;
/// `Terminal`, `TerminalUnavailable`, `OutcomeUnknown`, and `Interface` are
/// produced exclusively by the TUI adapter's own terminal stages and live
/// in this one enum so both callers classify the same verdict type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupFailure {
    AlreadyInSession,
    Terminal,
    Cohort,
    Runtime,
    SocketPathTooLong,
    RecoveryRequired,
    Incompatible,
    Admission,
    TimedOut,
    TerminalUnavailable,
    OutcomeUnknown,
    Interface,
    Configuration,
}
