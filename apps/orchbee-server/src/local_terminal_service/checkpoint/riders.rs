//! Agent resume and output replay ride on a Windows checkpoint: a save records each terminal's
//! retained Agent session and kept output, a restore resumes the one and replays the other, and a
//! held restore does both for its one Presentation. The Session's settings turn each on. It does
//! not sample directories or publish a checkpoint.

use super::super::{LocalTerminalService, agent_resume::AgentResumeCandidates};
use crate::{
    session_coordinator::{
        PendingHeldRestore, ProcessObservationTarget, RunTableEntry, SessionCoordinator,
    },
    terminal_coordinator::{PendingOperation, WorkerPlacement},
};
use orchbee_server::WorkbenchRestoreTarget;
use orchbee_workbench_protocol::{
    PresentationBinding, TerminalPresentationId, UserSettingsValues, WorkbenchDirectoryPath,
    WorkerId,
};
use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

/// Agent references a restore resumes once the Session runs.
pub(super) type Resumes = AgentResumeCandidates;

/// No Agent reference yet.
pub(super) fn resumes() -> Resumes {
    Resumes::default()
}

/// The kept output one restored Presentation replays before its program starts, if any.
pub(super) type Output = Option<RestoredOutput>;

pub(super) struct RestoredOutput {
    presentation_id: TerminalPresentationId,
    token: u64,
}

/// Earlier output a held Presentation shows when it starts, if any.
pub(in crate::local_terminal_service) type Seed = Option<Vec<u8>>;

/// Whether Agent resume and output replay are on for one save or restore.
#[derive(Clone, Copy)]
pub(in crate::local_terminal_service) struct Riders {
    resume: bool,
    replay: bool,
}

impl Riders {
    /// The Session's current choices, read before the caller takes any lock.
    pub(in crate::local_terminal_service) fn current(service: &LocalTerminalService) -> Self {
        Self {
            resume: service.agent_resume_enabled(),
            replay: service.output_replay_enabled(),
        }
    }

    /// The startup read's choices, which decide a restore once.
    pub(super) fn startup(values: &UserSettingsValues) -> Self {
        Self {
            resume: values.resume_agents_on_restore,
            replay: values.restore_terminal_output,
        }
    }

    /// What Claude Code's run table names for each published Claude Code run, read while no lock
    /// is held. Capture admits an entry only for a run still published as that exact process, so
    /// any change in between gains nothing. Resuming off, or an unreadable coordinator, reads
    /// nothing.
    pub(super) fn entries(self, service: &LocalTerminalService) -> Vec<RunTableEntry> {
        if !self.resume {
            return Vec::new();
        }
        let anchors = match service.coordinator.lock() {
            Ok(coordinator) => coordinator.run_table_anchors(),
            Err(_) => return Vec::new(),
        };
        crate::session_coordinator::read_run_table(&anchors)
    }

    /// Every bound checkpoint member and the Presentation it shows, which a final save captures
    /// output for.
    pub(super) fn presentations(
        self,
        service: &LocalTerminalService,
    ) -> io::Result<Vec<(PresentationBinding, TerminalPresentationId)>> {
        if !self.replay {
            return Ok(Vec::new());
        }
        Ok(service
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_presentations())
    }

    /// The final save's output capture, which spends at most half of what remains before
    /// `deadline`.
    pub(super) fn final_output(
        self,
        presentations: Vec<(PresentationBinding, TerminalPresentationId)>,
        deadline: Instant,
    ) -> FinalOutput {
        FinalOutput {
            replay: self.replay,
            presentations,
            deadline: Instant::now() + deadline.saturating_duration_since(Instant::now()) / 2,
            deadline_noted: false,
        }
    }

    /// The checkpoint of `coordinator`'s Workbench with these directories, recording each
    /// terminal's Agent session and kept output where those are on.
    pub(super) fn checkpoint(
        self,
        service: &LocalTerminalService,
        coordinator: &mut SessionCoordinator,
        worker: &WorkerId,
        directories: &[(PresentationBinding, WorkbenchDirectoryPath)],
        targets: &[ProcessObservationTarget],
        entries: &[RunTableEntry],
    ) -> io::Result<Vec<u8>> {
        let agents = self
            .resume
            .then(|| coordinator.checkpoint_agent_sessions(targets, entries));
        let outputs = service.checkpoint_outputs(self.replay, targets);
        coordinator.checkpoint_with_observations(
            worker,
            directories,
            agents.as_deref(),
            outputs.as_deref(),
        )
    }

    /// Whether a Presentation held for an unavailable directory keeps its Agent reference; one it
    /// does not keep is dropped.
    pub(super) fn hold_agent(
        self,
        coordinator: &mut SessionCoordinator,
        target: &WorkbenchRestoreTarget,
    ) -> bool {
        let agent_resume =
            self.resume && coordinator.has_pending_agent_session(target.presentation_id());
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
                Some(directory) if self.resume => {
                    resumes.push(target.presentation_id().clone(), directory.clone());
                }
                _ => {
                    coordinator.take_pending_agent_session(target.presentation_id());
                }
            }
        }
    }

    /// The kept output a restored Presentation replays, while replay is on.
    pub(super) fn restored_output(self, target: &WorkbenchRestoreTarget) -> Output {
        target
            .restored_output()
            .filter(|_| self.replay)
            .map(|token| RestoredOutput {
                presentation_id: target.presentation_id().clone(),
                token,
            })
    }

    /// The kept output a held Presentation replays when it starts, read outside every gate.
    pub(in crate::local_terminal_service) fn held_seed(
        self,
        service: &LocalTerminalService,
        output: Option<&(TerminalPresentationId, u64)>,
    ) -> Seed {
        output
            .filter(|_| self.replay)
            .and_then(|(presentation, token)| service.restored_output_seed(presentation, *token))
    }

    /// Hands a held Presentation's retained Agent reference to the resume driver once its shell
    /// started where the reference was recorded. It joins the running driver or starts a new run;
    /// only a stopping Server refuses, and the final checkpoint then carries the reference.
    pub(in crate::local_terminal_service) fn resume_held_agent(
        self,
        service: &Arc<LocalTerminalService>,
        coordinator: &SessionCoordinator,
        current: Option<WorkerPlacement>,
        started: Option<(TerminalPresentationId, WorkbenchDirectoryPath)>,
    ) {
        if let Some((presentation, directory)) = started
            && self.resume
            && coordinator.has_pending_agent_session(&presentation)
        {
            let _kept_pending =
                service.resume_retried_agent(coordinator, current, presentation, directory);
        }
    }
}

/// One final save's output capture across its terminals.
pub(super) struct FinalOutput {
    replay: bool,
    presentations: Vec<(PresentationBinding, TerminalPresentationId)>,
    deadline: Instant,
    deadline_noted: bool,
}

impl FinalOutput {
    /// Captures `target`'s output; past the capture deadline each terminal keeps its previous
    /// file and the save goes on.
    pub(super) fn capture(
        &mut self,
        service: &LocalTerminalService,
        target: &ProcessObservationTarget,
    ) -> io::Result<()> {
        if !self.replay {
            return Ok(());
        }
        if Instant::now() >= self.deadline {
            if !self.deadline_noted {
                self.deadline_noted = true;
                eprintln!(
                    "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=final-capture cause=deadline"
                );
            }
            return Ok(());
        }
        let presentation = self
            .presentations
            .iter()
            .find(|(binding, _)| binding == &target.binding)
            .map(|(_, presentation)| presentation);
        let pending = service.capture_final_output(target, presentation);
        service.settle_checkpoint_viewer(pending)
    }
}

/// Attaches a restored launch's kept output, which reaches the Host before the program's first
/// byte; a file that cannot replay here is never attached elsewhere.
pub(super) fn attach_seed(
    service: &LocalTerminalService,
    terminal: &mut PendingOperation,
    output: Output,
) {
    if let Some(output) = output
        && let Some(seed) = service.restored_output_seed(&output.presentation_id, output.token)
        && terminal.set_output_seed(seed).is_err()
    {
        eprintln!(
            "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=seed cause=refused"
        );
    }
}

/// Attaches a held Presentation's kept output to its reserved launch.
pub(in crate::local_terminal_service) fn attach_held_seed(
    pending: &mut PendingHeldRestore,
    seed: Seed,
) {
    if let Some(seed) = seed {
        pending.set_output_seed(seed);
    }
}

/// Captures kept output for the next background save within `budget`, while replay is on.
pub(super) fn capture_background(service: &LocalTerminalService, budget: Duration) {
    if service.output_replay_enabled() {
        service.capture_terminal_output(Instant::now() + budget);
    }
}

/// Removes every kept file a committed save no longer names.
pub(super) fn after_save(service: &LocalTerminalService, bytes: &[u8]) {
    service.terminal_output_after_save(bytes);
}
