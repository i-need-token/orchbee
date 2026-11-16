//! No Agent resume or output replay rides on a checkpoint on this platform: a save records no
//! Agent session and names no kept output, and a restore resumes and replays nothing. It does not
//! sample directories or publish a checkpoint.

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

/// No Agent reference waits to resume.
pub(super) struct Resumes;

pub(super) fn resumes() -> Resumes {
    Resumes
}

/// No restored Presentation replays kept output.
pub(super) struct Output;

/// No held Presentation shows earlier output.
pub(in crate::local_terminal_service) struct Seed;

/// A save reads nothing for Agent sessions.
pub(super) struct Entries;

/// A final save captures no Presentation's output.
pub(super) struct Presentations;

/// Neither Agent resume nor output replay exists here.
#[derive(Clone, Copy)]
pub(in crate::local_terminal_service) struct Riders;

impl Riders {
    pub(in crate::local_terminal_service) fn current(_service: &LocalTerminalService) -> Self {
        Self
    }

    pub(super) fn startup(_values: &UserSettingsValues) -> Self {
        Self
    }

    pub(super) fn entries(self, _service: &LocalTerminalService) -> Entries {
        Entries
    }

    pub(super) fn presentations(
        self,
        _service: &LocalTerminalService,
    ) -> io::Result<Presentations> {
        Ok(Presentations)
    }

    pub(super) fn final_output(
        self,
        _presentations: Presentations,
        _deadline: Instant,
    ) -> FinalOutput {
        FinalOutput
    }

    /// The checkpoint of `coordinator`'s Workbench with these directories alone.
    pub(super) fn checkpoint(
        self,
        _service: &LocalTerminalService,
        coordinator: &mut SessionCoordinator,
        worker: &WorkerId,
        directories: &[(PresentationBinding, WorkbenchDirectoryPath)],
        _targets: &[ProcessObservationTarget],
        _entries: &Entries,
    ) -> io::Result<Vec<u8>> {
        coordinator.checkpoint_with_observations(worker, directories, None, None)
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

    pub(super) fn restored_output(self, _target: &WorkbenchRestoreTarget) -> Output {
        Output
    }

    pub(in crate::local_terminal_service) fn held_seed(
        self,
        _service: &LocalTerminalService,
        _output: Option<&(TerminalPresentationId, u64)>,
    ) -> Seed {
        Seed
    }

    pub(in crate::local_terminal_service) fn resume_held_agent(
        self,
        _service: &Arc<LocalTerminalService>,
        _coordinator: &SessionCoordinator,
        _current: Option<WorkerPlacement>,
        _started: Option<(TerminalPresentationId, WorkbenchDirectoryPath)>,
    ) {
    }
}

/// A final save captures no output.
pub(super) struct FinalOutput;

impl FinalOutput {
    pub(super) fn capture(
        &mut self,
        _service: &LocalTerminalService,
        _target: &ProcessObservationTarget,
    ) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn attach_seed(
    _service: &LocalTerminalService,
    _terminal: &mut PendingOperation,
    _output: Output,
) {
}

pub(in crate::local_terminal_service) fn attach_held_seed(
    _pending: &mut PendingHeldRestore,
    _seed: Seed,
) {
}

pub(super) fn capture_background(_service: &LocalTerminalService, _budget: Duration) {}

pub(super) fn after_save(_service: &LocalTerminalService, _bytes: &[u8]) {}
