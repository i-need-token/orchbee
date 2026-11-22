//! Agent resume and output replay ride on a checkpoint: a save records each terminal's retained
//! Agent session and kept output, a restore resumes the one and replays the other, and a held
//! restore does both for its one Presentation. The Session's settings turn each on. Output replay
//! rides wherever the Session keeps a checkpoint; Agent resume rides on Windows and macOS, through
//! `resume`, and a Presentation whose Agent resumes replays no output. One setting turns both on.
//! It does not sample directories or publish a checkpoint.

use super::super::LocalTerminalService;
use crate::{
    session_coordinator::{PendingHeldRestore, ProcessObservationTarget, SessionCoordinator},
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

#[cfg(any(windows, target_os = "macos"))]
mod resume;
#[cfg(not(any(windows, target_os = "macos")))]
#[path = "riders/without_resume.rs"]
mod resume;
pub(super) use resume::{Resumes, resumes};

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
    restore: bool,
}

impl Riders {
    /// The Session's current choice, read before the caller takes any lock.
    pub(in crate::local_terminal_service) fn current(service: &LocalTerminalService) -> Self {
        Self {
            restore: service.restores_terminals(),
        }
    }

    /// The startup read's choice, which decides a restore once.
    pub(super) fn startup(values: &UserSettingsValues) -> Self {
        Self {
            restore: values.restore_terminals,
        }
    }

    /// Agent resume, which the same setting turns on.
    const fn resume(self) -> resume::Resume {
        resume::Resume::new(self.restore)
    }

    /// What Agent resume reads for a save while no lock is held.
    pub(super) fn entries(self, service: &LocalTerminalService) -> resume::Entries {
        self.resume().entries(service)
    }

    /// Every bound checkpoint member and the Presentation it shows, which a final save captures
    /// output for.
    pub(super) fn presentations(
        self,
        service: &LocalTerminalService,
    ) -> io::Result<Vec<(PresentationBinding, TerminalPresentationId)>> {
        if !self.restore {
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
            replay: self.restore,
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
        entries: &resume::Entries,
    ) -> io::Result<Vec<u8>> {
        let agents = self.resume().agent_sessions(coordinator, targets, entries);
        let outputs = service.checkpoint_outputs(self.restore, targets);
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
        self.resume().hold_agent(coordinator, target)
    }

    /// Queues a restored Presentation's Agent reference to resume where it was recorded, or drops
    /// it; answers whether it queued one.
    pub(super) fn restore_agent(
        self,
        coordinator: &mut SessionCoordinator,
        target: &WorkbenchRestoreTarget,
        resumes: &mut Resumes,
    ) -> bool {
        self.resume().restore_agent(coordinator, target, resumes)
    }

    /// The kept output a restored Presentation replays, while replay is on. A Presentation whose
    /// Agent resumes replays none: the Agent shows its own conversation.
    pub(super) fn restored_output(self, target: &WorkbenchRestoreTarget, agent: bool) -> Output {
        target
            .restored_output()
            .filter(|_| self.restore && !agent)
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
            .filter(|_| self.restore)
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
        self.resume()
            .resume_held_agent(service, coordinator, current, started);
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
    if service.restores_terminals() {
        service.capture_terminal_output(Instant::now() + budget);
    }
}

/// Removes every kept file a committed save no longer names.
pub(super) fn after_save(service: &LocalTerminalService, bytes: &[u8]) {
    service.terminal_output_after_save(bytes);
}
