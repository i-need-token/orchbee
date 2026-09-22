use super::TerminalStatus;
use super::workbench_projection::NativeTerminalBinding;
#[cfg(test)]
use super::{Identities, LegacyCommand, LegacyEvent, LegacyIntent};
use crate::{
    local_client::LocalClient,
    terminal_session::state::{
        PendingCloseResult, PendingCreateCommit, PendingMoveCommit, PendingPanelCommit,
    },
};
#[cfg(test)]
use crate::{local_client::SubmissionError, terminal_projection::TerminalProjection};
#[cfg(test)]
use orchbee_client_server_protocol::{
    ControlRejectedReason, CreateAcceptedState, CreateOutcome, StartMode,
    TerminalControlAction as Control, TerminalControlOutcome as ControlOutcome,
    TerminalCreateRequest, TerminalViewerOutcome as ViewerOutcome,
};
use orchbee_client_server_protocol::{
    Geometry, SiteId as ObcsSiteId, TerminalId as ObcsTerminalId, TerminalViewerAction as Viewer,
    WorkerId as ObcsWorkerId,
};
#[cfg(test)]
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::SiteId as WorkbenchSiteId;
use std::sync::Arc;
#[cfg(test)]
use std::{
    num::NonZeroU64,
    sync::mpsc::{Receiver, SyncSender},
};
#[cfg(test)]
mod backpressure;
mod native;
#[cfg(test)]
use backpressure::{control, viewer};
#[cfg(any(target_os = "linux", windows))]
pub(super) use native::{RecoveryShutdown, run_status_recovery};

use super::mailbox::CommandMailbox;

type Result<T> = std::result::Result<T, TerminalStatus>;

pub(super) fn run_native(
    client: LocalClient,
    initial_geometry: Geometry,
    mailbox: Arc<CommandMailbox>,
) -> NativeWorkerExit {
    native::run(client, initial_geometry, mailbox)
}

pub(super) struct NativeWorkerExit {
    #[cfg(any(target_os = "linux", windows))]
    unavailable_poll: Option<super::state::retired_membership::UnavailablePoll>,
    pending_create: Option<PendingCreateCommit>,
    pending_move: Option<PendingMoveCommit>,
    pending_panel: Option<PendingPanelCommit>,
    pending_close: Option<PendingCloseResult>,
    pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
    pending_setup: Option<super::state::workspace_setup::SetupWorkerEvidence>,
}

impl NativeWorkerExit {
    fn with_setup(
        mut self,
        evidence: Option<super::state::workspace_setup::SetupWorkerEvidence>,
    ) -> Self {
        self.pending_setup = evidence;
        self
    }
    pub(super) fn take_setup_result(
        &mut self,
    ) -> Option<super::state::workspace_setup::SetupWorkerEvidence> {
        self.pending_setup.take()
    }
    #[cfg(any(target_os = "linux", windows))]
    fn with_unavailable_poll(
        mut self,
        evidence: Option<super::state::retired_membership::UnavailablePoll>,
    ) -> Self {
        self.unavailable_poll = evidence;
        self
    }
    #[cfg(any(target_os = "linux", windows))]
    pub(super) fn take_unavailable_poll(
        &mut self,
    ) -> Option<super::state::retired_membership::UnavailablePoll> {
        self.unavailable_poll.take()
    }

    const fn new(
        pending_create: Option<PendingCreateCommit>,
        pending_move: Option<PendingMoveCommit>,
        pending_panel: Option<PendingPanelCommit>,
        pending_close: Option<PendingCloseResult>,
        pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
    ) -> Self {
        Self {
            #[cfg(any(target_os = "linux", windows))]
            unavailable_poll: None,
            pending_create,
            pending_move,
            pending_panel,
            pending_close,
            pending_rename,
            pending_setup: None,
        }
    }

    #[cfg(test)]
    pub(super) fn into_pending_create(self) -> Option<PendingCreateCommit> {
        self.pending_create
    }

    #[cfg(test)]
    pub(super) fn into_pending_commits(
        self,
    ) -> (Option<PendingCreateCommit>, Option<PendingMoveCommit>) {
        (self.pending_create, self.pending_move)
    }

    #[cfg(test)]
    pub(super) fn into_pending_close(self) -> Option<PendingCloseResult> {
        self.pending_close
    }

    pub(super) fn into_pending_results(self) -> PendingWorkerResults {
        PendingWorkerResults {
            create: self.pending_create,
            moved: self.pending_move,
            panel: self.pending_panel,
            close: self.pending_close,
            renamed: self.pending_rename,
        }
    }
}

pub(super) struct PendingWorkerResults {
    pub(super) create: Option<PendingCreateCommit>,
    pub(super) moved: Option<PendingMoveCommit>,
    pub(super) panel: Option<PendingPanelCommit>,
    pub(super) close: Option<PendingCloseResult>,
    pub(super) renamed: Option<super::state::tab_rename::RenameWorkerEvidence>,
}

#[cfg(test)]
pub(super) fn run_legacy(
    mut client: LocalClient,
    intent: LegacyIntent,
    geometry: Geometry,
    mut ids: Identities,
    commands: Receiver<LegacyCommand>,
    events: &SyncSender<LegacyEvent>,
) -> Result<()> {
    let attaching = matches!(intent, LegacyIntent::Attach(_));
    let attach = match intent {
        LegacyIntent::Attach(id) => Viewer::Attach {
            terminal_create_request_id: id,
        },
        LegacyIntent::Create { cwd } => {
            let id = ids.next()?;
            let result = client
                .submit_terminal_create(TerminalCreateRequest::new(
                    id.clone(),
                    None,
                    cwd,
                    StartMode::Immediate { geometry },
                    None,
                ))
                .map_err(submission)?;
            match result.outcome() {
                CreateOutcome::Accepted {
                    state: CreateAcceptedState::Started { geometry: accepted },
                } if accepted == geometry => {}
                CreateOutcome::OutcomeUnknown => return Err(TerminalStatus::OutcomeUnknown),
                _ => return Err(TerminalStatus::Unavailable),
            }
            Viewer::Attach {
                terminal_create_request_id: id,
            }
        }
    };
    let result = viewer(&mut client, &mut ids, attach)?;
    let ViewerOutcome::Attached {
        attachment_id,
        host_epoch,
    } = result
    else {
        return Err(TerminalStatus::Unavailable);
    };
    let projection = TerminalProjection::new(
        AttachmentId::parse(attachment_id.as_str()).map_err(|_| TerminalStatus::Unavailable)?,
        HostEpoch::parse(host_epoch.as_str()).map_err(|_| TerminalStatus::Unavailable)?,
    );
    let mut sequence = 0_u64;
    let result = control(
        &mut client,
        &mut ids,
        Control::Acquire {
            attachment_id: attachment_id.clone(),
        },
        &mut sequence,
    )?;
    let controller = match result {
        ControlOutcome::Acquired { controller_id } => Some(controller_id),
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy,
        } => None,
        _ => return Err(TerminalStatus::Unavailable),
    };
    if attaching
        && let Some(controller_id) = controller.clone()
        && !matches!(
            control(
                &mut client,
                &mut ids,
                Control::Resize {
                    attachment_id: attachment_id.clone(),
                    controller_id,
                    sequence: NonZeroU64::MIN,
                    geometry
                },
                &mut sequence,
            )?,
            ControlOutcome::Resized { .. }
        )
    {
        return Err(TerminalStatus::Unavailable);
    }
    events
        .send(LegacyEvent::Opened(projection, controller.is_some()))
        .map_err(|_| TerminalStatus::Unavailable)?;
    while let Ok(command) = commands.recv() {
        match command {
            LegacyCommand::Poll => {
                let record = match viewer(
                    &mut client,
                    &mut ids,
                    Viewer::Poll {
                        attachment_id: attachment_id.clone(),
                    },
                )? {
                    ViewerOutcome::Empty {} => None,
                    ViewerOutcome::Record(record) => Some(record),
                    _ => return Err(TerminalStatus::Unavailable),
                };
                events
                    .send(LegacyEvent::Polled(record))
                    .map_err(|_| TerminalStatus::Unavailable)?;
            }
            LegacyCommand::Feedback(feedback) => {
                if feedback.attachment_id() != Some(&attachment_id) {
                    return Err(TerminalStatus::Unavailable);
                }
                if viewer(&mut client, &mut ids, feedback)? != (ViewerOutcome::FeedbackAccepted {})
                {
                    return Err(TerminalStatus::Unavailable);
                }
                events
                    .send(LegacyEvent::FeedbackAccepted)
                    .map_err(|_| TerminalStatus::Unavailable)?;
            }
            LegacyCommand::Input(_) | LegacyCommand::Resize(_) => {
                let controller_id = controller.clone().ok_or(TerminalStatus::Unavailable)?;
                sequence = sequence.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
                let request_sequence =
                    NonZeroU64::new(sequence).ok_or(TerminalStatus::Unavailable)?;
                let action = match command {
                    LegacyCommand::Input(bytes) => Control::Input {
                        attachment_id: attachment_id.clone(),
                        controller_id,
                        sequence: request_sequence,
                        bytes,
                    },
                    LegacyCommand::Resize(geometry) => Control::Resize {
                        attachment_id: attachment_id.clone(),
                        controller_id,
                        sequence: request_sequence,
                        geometry,
                    },
                    _ => unreachable!("control command"),
                };
                if !matches!(
                    control(&mut client, &mut ids, action, &mut sequence)?,
                    ControlOutcome::InputAccepted {} | ControlOutcome::Resized { .. }
                ) {
                    return Err(TerminalStatus::Unavailable);
                }
            }
        }
    }
    Ok(())
}

fn attach_binding(site_id: &WorkbenchSiteId, binding: &NativeTerminalBinding) -> Result<Viewer> {
    Ok(Viewer::AttachBinding {
        site_id: ObcsSiteId::parse(site_id.as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
        worker_id: ObcsWorkerId::parse(binding.worker_id().as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
        terminal_id: ObcsTerminalId::parse(binding.terminal_id().as_str())
            .map_err(|_error| TerminalStatus::Unavailable)?,
    })
}

#[cfg(test)]
fn submission(error: SubmissionError) -> TerminalStatus {
    match error {
        SubmissionError::OutcomeUnknown { .. } => TerminalStatus::OutcomeUnknown,
        _ => TerminalStatus::Unavailable,
    }
}
