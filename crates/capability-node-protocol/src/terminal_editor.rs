//! Read-only machine editor selection; resolving argv grants no source or launch authority.
use crate::{HostEpoch, HostTerminalId, LaunchSelection, OperationId, WorkerGeneration, WorkerId};
use std::num::NonZeroU64;

/// Pins capture to an existing read viewer and the exact immutable Host content version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalEditorSource {
    pub attachment_id: OperationId,
    pub host_epoch: HostEpoch,
    pub terminal_id: HostTerminalId,
    pub generation: NonZeroU64,
    pub revision: u64,
}

/// Pins one editor-preference observation to the admitted Worker incarnation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalEditorCommandRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
}

/// Identifies the preference that selected the program without exposing its raw value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalEditorCommandSource {
    Visual,
    Editor,
    Default,
}

/// A closed refusal before file, Host or editor process effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalEditorCommandRefusal {
    WrongWorker,
    StaleWorkerGeneration,
    UnsupportedPlatform,
    InvalidCommand,
    ProgramUnavailable,
}

/// Contains bounded literal argv or a refusal; neither outcome grants launch permission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEditorCommandOutcome {
    Resolved {
        source: TerminalEditorCommandSource,
        selection: LaunchSelection,
    },
    Refused(TerminalEditorCommandRefusal),
}

/// Echoes every request field so the carrier can reject stale or misrouted results.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalEditorCommandResult {
    pub request: TerminalEditorCommandRequest,
    pub outcome: TerminalEditorCommandOutcome,
}

#[cfg(test)]
mod tests;
