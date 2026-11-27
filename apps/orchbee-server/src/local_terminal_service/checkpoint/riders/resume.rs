//! Agent resume rides on a checkpoint: a save records each terminal's retained Agent session, a
//! restore resumes it where it was recorded, in managed PowerShell or cmd on Windows and managed
//! zsh or bash on macOS and Linux, and a held restore resumes its one Presentation once
//! its shell started there. The Session's settings turn it on.

use crate::{
    local_terminal_service::{LocalTerminalService, agent_resume::AgentResumeCandidates},
    session_coordinator::{ProcessObservationTarget, ProcessSourceEntry, SessionCoordinator},
    terminal_coordinator::WorkerPlacement,
};
use orchbee_server::{RetainedAgentSession, WorkbenchRestoreTarget};
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalPresentationId, WorkbenchDirectoryPath,
};
use std::sync::Arc;

/// Agent references a restore resumes once the Session runs.
pub(in crate::local_terminal_service::checkpoint) type Resumes = AgentResumeCandidates;

/// No Agent reference yet.
pub(in crate::local_terminal_service::checkpoint) fn resumes() -> Resumes {
    Resumes::default()
}

/// What each published Agent run's own files name: its Agent's run table or session store.
pub(in crate::local_terminal_service::checkpoint) type Entries = Vec<ProcessSourceEntry>;

/// Whether Agent resume is on for one save or restore.
#[derive(Clone, Copy)]
pub(super) struct Resume(bool);

impl Resume {
    /// Agent resume, on or off.
    pub(super) const fn new(on: bool) -> Self {
        Self(on)
    }

    /// What each published Agent run's own files name, read while no lock is held. Capture admits
    /// an entry only for a run still published as that exact process, so any change in between
    /// gains nothing. Resuming off, or an unreadable coordinator, reads nothing.
    pub(super) fn entries(self, service: &LocalTerminalService) -> Entries {
        if !self.0 {
            return Vec::new();
        }
        let probes = match service.coordinator.lock() {
            Ok(coordinator) => coordinator.process_source_probes(),
            Err(_) => return Vec::new(),
        };
        crate::session_coordinator::read_process_sources(&probes)
    }

    /// Each terminal's Agent session for the checkpoint, or none while resuming is off.
    pub(super) fn agent_sessions(
        self,
        coordinator: &mut SessionCoordinator,
        targets: &[ProcessObservationTarget],
        entries: &[ProcessSourceEntry],
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
    /// it; answers whether it queued one.
    pub(super) fn restore_agent(
        self,
        coordinator: &mut SessionCoordinator,
        target: &WorkbenchRestoreTarget,
        resumes: &mut Resumes,
    ) -> bool {
        if !coordinator.has_pending_agent_session(target.presentation_id()) {
            return false;
        }
        // A reference resumes only where it was recorded.
        match target
            .restored_directory()
            .filter(|p| std::path::Path::new(p.as_str()).is_dir())
        {
            Some(directory) if self.0 => {
                resumes.push(target.presentation_id().clone(), directory.clone());
                true
            }
            _ => {
                coordinator.take_pending_agent_session(target.presentation_id());
                false
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
