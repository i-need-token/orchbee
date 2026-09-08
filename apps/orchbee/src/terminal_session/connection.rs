use super::TerminalStatus;
use super::workbench_projection::NativeTerminalBinding;
#[cfg(test)]
use super::{Identities, LegacyCommand, LegacyEvent, LegacyIntent};
use crate::{
    local_client::LocalClient,
    terminal_session::state::{PendingCloseResult, PendingCreateCommit, PendingMoveCommit},
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
#[cfg(target_os = "linux")]
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
    #[cfg(target_os = "linux")]
    unavailable_poll: Option<super::state::retired_membership::UnavailablePoll>,
    pending_create: Option<PendingCreateCommit>,
    pending_move: Option<PendingMoveCommit>,
    pending_close: Option<PendingCloseResult>,
    pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
}

impl NativeWorkerExit {
    #[cfg(target_os = "linux")]
    fn with_unavailable_poll(
        mut self,
        evidence: Option<super::state::retired_membership::UnavailablePoll>,
    ) -> Self {
        self.unavailable_poll = evidence;
        self
    }
    #[cfg(target_os = "linux")]
    pub(super) fn take_unavailable_poll(
        &mut self,
    ) -> Option<super::state::retired_membership::UnavailablePoll> {
        self.unavailable_poll.take()
    }

    const fn new(
        pending_create: Option<PendingCreateCommit>,
        pending_move: Option<PendingMoveCommit>,
        pending_close: Option<PendingCloseResult>,
        pending_rename: Option<super::state::tab_rename::RenameWorkerEvidence>,
    ) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            unavailable_poll: None,
            pending_create,
            pending_move,
            pending_close,
            pending_rename,
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

    pub(super) fn into_pending_results(
        self,
    ) -> (
        Option<PendingCreateCommit>,
        Option<PendingMoveCommit>,
        Option<PendingCloseResult>,
        Option<super::state::tab_rename::RenameWorkerEvidence>,
    ) {
        (
            self.pending_create,
            self.pending_move,
            self.pending_close,
            self.pending_rename,
        )
    }
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
    let result = control(&mut client, &mut ids, Control::Acquire {}, &mut sequence)?;
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
                let record = match viewer(&mut client, &mut ids, Viewer::Poll {})? {
                    ViewerOutcome::Empty {} => None,
                    ViewerOutcome::Record(record) => Some(record),
                    _ => return Err(TerminalStatus::Unavailable),
                };
                events
                    .send(LegacyEvent::Polled(record))
                    .map_err(|_| TerminalStatus::Unavailable)?;
            }
            LegacyCommand::Feedback(feedback) => {
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
                        controller_id,
                        sequence: request_sequence,
                        bytes,
                    },
                    LegacyCommand::Resize(geometry) => Control::Resize {
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
