//! Whether one fresh read of a restored terminal makes its resume worth trying now, later or never:
//! the shared prompt readiness rule in the retained directory, its refusals read as resume's
//! give-up reasons. Only managed PowerShell and marked cmd have a resume line, so a managed zsh or
//! bash the rule would try skips as unsupported; a Windows Session never launches one.
use super::Reason;
use crate::local_terminal_service::prompt_readiness::{self, MarkedShell, Refusal, ResumeFacts};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

/// What one read admits for a resume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Readiness {
    /// Worth one gated send of the resume line for this shell now.
    Try(MarkedShell),
    /// The read cannot decide yet.
    Wait,
    /// The terminal must never receive this resume.
    Skip(Reason),
}

/// Decides one read for a resume that must find `retained`.
pub(super) fn readiness(facts: &ResumeFacts, retained: &WorkbenchDirectoryPath) -> Readiness {
    match prompt_readiness::readiness(facts, Some(retained)) {
        prompt_readiness::Readiness::Try(
            shell @ (MarkedShell::ManagedPowerShell | MarkedShell::Cmd),
        ) => Readiness::Try(shell),
        prompt_readiness::Readiness::Try(MarkedShell::ManagedZsh | MarkedShell::ManagedBash) => {
            Readiness::Skip(Reason::UnsupportedShell)
        }
        prompt_readiness::Readiness::Wait => Readiness::Wait,
        prompt_readiness::Readiness::Refuse(refusal) => Readiness::Skip(match refusal {
            Refusal::NotIdle => Reason::NotIdle,
            Refusal::UnsupportedShell => Reason::UnsupportedShell,
            Refusal::TerminalTooOld => Reason::TerminalTooOld,
            Refusal::DirectoryMismatch => Reason::DirectoryMismatch,
        }),
    }
}
