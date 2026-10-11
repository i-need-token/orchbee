use super::{Identities, LegacyCommand, LegacyEvent, LegacyIntent, TerminalStatus};
use crate::{
    local_client::{LocalClient, SubmissionError},
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{
    ControlRejectedReason, CreateAcceptedState, CreateOutcome, Geometry, StartMode,
    TerminalControlAction as Control, TerminalControlOutcome as ControlOutcome,
    TerminalCreateRequest, TerminalViewerAction as Viewer, TerminalViewerOutcome as ViewerOutcome,
};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use std::{
    num::NonZeroU64,
    sync::mpsc::{Receiver, SyncSender},
};

mod backpressure;
use backpressure::{control, viewer};

type Result<T> = std::result::Result<T, TerminalStatus>;

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
        host_terminal_id,
    } = result
    else {
        return Err(TerminalStatus::Unavailable);
    };
    let projection = TerminalProjection::new(
        AttachmentId::parse(attachment_id.as_str()).map_err(|_| TerminalStatus::Unavailable)?,
        HostEpoch::parse(host_epoch.as_str()).map_err(|_| TerminalStatus::Unavailable)?,
    )
    .with_host_terminal(host_terminal_id);
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
        .send(LegacyEvent::Opened(
            Box::new(projection),
            controller.is_some(),
        ))
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

fn submission(error: SubmissionError) -> TerminalStatus {
    match error {
        SubmissionError::OutcomeUnknown { .. } => TerminalStatus::OutcomeUnknown,
        _ => TerminalStatus::Unavailable,
    }
}
