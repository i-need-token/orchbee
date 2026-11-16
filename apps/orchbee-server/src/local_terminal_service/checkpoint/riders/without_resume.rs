//! No Agent resume rides on a checkpoint on this platform: a save records no Agent session, a
//! restore resumes nothing, and a held Presentation keeps no Agent reference.

use crate::{
    local_terminal_service::LocalTerminalService,
    session_coordinator::{ProcessObservationTarget, SessionCoordinator},
    terminal_coordinator::WorkerPlacement,
};
use orchbee_server::{RetainedAgentSession, WorkbenchRestoreTarget};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalPresentationId, UserSettingsValues, WorkbenchDirectoryPath,
};
use std::sync::Arc;

/// No Agent reference waits to resume.
pub(in crate::local_terminal_service::checkpoint) struct Resumes;

pub(in crate::local_terminal_service::checkpoint) fn resumes() -> Resumes {
    Resumes
}

/// A save reads nothing for Agent sessions.
pub(in crate::local_terminal_service::checkpoint) struct Entries;

/// Agent resume does not exist here.
#[derive(Clone, Copy)]
pub(super) struct Resume;

impl Resume {
    pub(super) fn current(_service: &LocalTerminalService) -> Self {
        Self
    }

    pub(super) fn startup(_values: &UserSettingsValues) -> Self {
        Self
    }

    pub(super) fn entries(self, _service: &LocalTerminalService) -> Entries {
        Entries
    }

    /// A checkpoint records no Agent session.
    pub(super) fn agent_sessions(
        self,
        _coordinator: &mut SessionCoordinator,
        _targets: &[ProcessObservationTarget],
        _entries: &Entries,
    ) -> Option<Vec<(PresentationBinding, RetainedAgentSession)>> {
        None
    }

    /// A held Presentation keeps no Agent reference.
    pub(super) fn hold_agent(
        self,
        _coordinator: &mut SessionCoordinator,
        _target: &WorkbenchRestoreTarget,
    ) -> bool {
        false
    }

    pub(super) fn restore_agent(
        self,
        _coordinator: &mut SessionCoordinator,
        _target: &WorkbenchRestoreTarget,
        _resumes: &mut Resumes,
    ) {
    }

    pub(super) fn resume_held_agent(
        self,
        _service: &Arc<LocalTerminalService>,
        _coordinator: &SessionCoordinator,
        _current: Option<WorkerPlacement>,
        _started: Option<(TerminalPresentationId, WorkbenchDirectoryPath)>,
    ) {
    }
}
