//! Agent resume rides on a Windows checkpoint: a save records each terminal's retained Agent
//! session, a restore resumes it where it was recorded, and a held restore resumes its one
//! Presentation once its shell started there. The Session's settings turn it on.

use crate::{
    local_terminal_service::{LocalTerminalService, agent_resume::AgentResumeCandidates},
    session_coordinator::{ProcessObservationTarget, RunTableEntry, SessionCoordinator},
    terminal_coordinator::WorkerPlacement,
};
use orchbee_server::{RetainedAgentSession, WorkbenchRestoreTarget};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalPresentationId, UserSettingsValues, WorkbenchDirectoryPath,
};
use std::sync::Arc;

/// Agent references a restore resumes once the Session runs.
pub(in crate::local_terminal_service::checkpoint) type Resumes = AgentResumeCandidates;

/// No Agent reference yet.
pub(in crate::local_terminal_service::checkpoint) fn resumes() -> Resumes {
    Resumes::default()
}

/// What Claude Code's run table names for each published Claude Code run.
pub(in crate::local_terminal_service::checkpoint) type Entries = Vec<RunTableEntry>;

/// Whether Agent resume is on for one save or restore.
#[derive(Clone, Copy)]
pub(super) struct Resume(bool);

impl Resume {
    /// The Session's current choice, read before the caller takes any lock.
    pub(super) fn current(service: &LocalTerminalService) -> Self {
        Self(service.agent_resume_enabled())
    }

    /// The startup read's choice, which decides a restore once.
    pub(super) fn startup(values: &UserSettingsValues) -> Self {
        Self(values.resume_agents_on_restore)
    }

    /// What Claude Code's run table names for each published Claude Code run, read while no lock
    /// is held. Capture admits an entry only for a run still published as that exact process, so
    /// any change in between gains nothing. Resuming off, or an unreadable coordinator, reads
    /// nothing.
    pub(super) fn entries(self, service: &LocalTerminalService) -> Entries {
        if !self.0 {
            return Vec::new();
        }
        let anchors = match service.coordinator.lock() {
            Ok(coordinator) => coordinator.run_table_anchors(),
            Err(_) => return Vec::new(),
        };
        crate::session_coordinator::read_run_table(&anchors)
    }

    /// Each terminal's Agent session for the checkpoint, or none while resuming is off.
    pub(super) fn agent_sessions(
        self,
        coordinator: &mut SessionCoordinator,
        targets: &[ProcessObservationTarget],
        entries: &[RunTableEntry],
    ) -> Option<Vec<(PresentationBinding, RetainedAgentSession)>> {
        self.0
            .then(|| coordinator.checkpoint_agent_sessions(targets, entries))
    }

    /// Whether a Presentation held for an unavailable directory keeps its Agent reference; one it
    /// does not keep is dropped.
    pub(super) fn hold_agent(
        self,
        coordinator: &mut SessionCoordinator,
        target: &WorkbenchRestoreTarget,
    ) -> bool {
        let agent_resume =
            self.0 && coordinator.has_pending_agent_session(target.presentation_id());
        if !agent_resume {
            coordinator.take_pending_agent_session(target.presentation_id());
        }
        agent_resume
    }

    /// Queues a restored Presentation's Agent reference to resume where it was recorded, or drops
    /// it.
    pub(super) fn restore_agent(
        self,
        coordinator: &mut SessionCoordinator,
        target: &WorkbenchRestoreTarget,
        resumes: &mut Resumes,
    ) {
        if coordinator.has_pending_agent_session(target.presentation_id()) {
            // A reference resumes only where it was recorded.
            match target
                .restored_directory()
                .filter(|p| std::path::Path::new(p.as_str()).is_dir())
            {
                Some(directory) if self.0 => {
                    resumes.push(target.presentation_id().clone(), directory.clone());
                }
                _ => {
                    coordinator.take_pending_agent_session(target.presentation_id());
                }
            }
        }
    }

    /// Hands a held Presentation's retained Agent reference to the resume driver once its shell
    /// started where the reference was recorded. It joins the running driver or starts a new run;
    /// only a stopping Server refuses, and the final checkpoint then carries the reference.
    pub(super) fn resume_held_agent(
        self,
        service: &Arc<LocalTerminalService>,
        coordinator: &SessionCoordinator,
        current: Option<WorkerPlacement>,
        started: Option<(TerminalPresentationId, WorkbenchDirectoryPath)>,
    ) {
        if let Some((presentation, directory)) = started
            && self.0
            && coordinator.has_pending_agent_session(&presentation)
        {
            let _kept_pending =
                service.resume_retried_agent(coordinator, current, presentation, directory);
        }
    }
}
