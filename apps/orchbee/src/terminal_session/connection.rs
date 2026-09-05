use super::{Command, Event, Identities, TerminalIntent, TerminalStatus};
use crate::{
    local_client::{LocalClient, SubmissionError},
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{
    ControlRejectedReason, CreateAcceptedState, CreateOutcome, Geometry, StartMode,
    TerminalControlAction as Control, TerminalControlOutcome as ControlOutcome,
    TerminalControlRequest, TerminalCreateRequest, TerminalViewerAction as Viewer,
    TerminalViewerOutcome as ViewerOutcome, TerminalViewerRequest,
};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use std::{
    num::NonZeroU64,
    sync::mpsc::{Receiver, SyncSender},
};

type Result<T> = std::result::Result<T, TerminalStatus>;
pub(super) fn run(
    mut client: LocalClient,
    intent: TerminalIntent,
    geometry: Geometry,
    mut ids: Identities,
    commands: Receiver<Command>,
    events: &SyncSender<Event>,
) -> Result<()> {
    let attaching = matches!(intent, TerminalIntent::Attach(_));
    let create_id = match intent {
        TerminalIntent::Attach(id) => id,
        TerminalIntent::Create { cwd } => {
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
                } if accepted == geometry => id,
                CreateOutcome::OutcomeUnknown => return Err(TerminalStatus::OutcomeUnknown),
                _ => return Err(TerminalStatus::Unavailable),
            }
        }
    };
    let result = viewer(
        &mut client,
        &mut ids,
        Viewer::Attach {
            terminal_create_request_id: create_id,
        },
    )?;
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
    let result = control(&mut client, &mut ids, Control::Acquire {})?;
    let controller = match result {
        ControlOutcome::Acquired { controller_id } => Some(controller_id),
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy,
        } => None,
        _ => return Err(TerminalStatus::Unavailable),
    };
    let mut sequence = 0_u64;
    if attaching && let Some(controller_id) = controller.clone() {
        if !matches!(
            control(
                &mut client,
                &mut ids,
                Control::Resize {
                    controller_id,
                    sequence: NonZeroU64::MIN,
                    geometry
                }
            )?,
            ControlOutcome::Resized { .. }
        ) {
            return Err(TerminalStatus::Unavailable);
        }
        sequence = 1;
    }
    events
        .send(Event::Opened(projection, controller.is_some()))
        .map_err(|_| TerminalStatus::Unavailable)?;
    while let Ok(command) = commands.recv() {
        match command {
            Command::Poll => {
                let record = match viewer(&mut client, &mut ids, Viewer::Poll {})? {
                    ViewerOutcome::Empty {} => None,
                    ViewerOutcome::Record(record) => Some(record),
                    _ => return Err(TerminalStatus::Unavailable),
                };
                events
                    .send(Event::Polled(record))
                    .map_err(|_| TerminalStatus::Unavailable)?;
            }
            Command::Feedback(feedback) => {
                if viewer(&mut client, &mut ids, feedback)? != (ViewerOutcome::FeedbackAccepted {})
                {
                    return Err(TerminalStatus::Unavailable);
                }
                events
                    .send(Event::FeedbackAccepted)
                    .map_err(|_| TerminalStatus::Unavailable)?;
            }
            Command::Input(_) | Command::Resize(_) => {
                let controller_id = controller.clone().ok_or(TerminalStatus::Unavailable)?;
                sequence = sequence.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
                let sequence = NonZeroU64::new(sequence).ok_or(TerminalStatus::Unavailable)?;
                let action = match command {
                    Command::Input(bytes) => Control::Input {
                        controller_id,
                        sequence,
                        bytes,
                    },
                    Command::Resize(geometry) => Control::Resize {
                        controller_id,
                        sequence,
                        geometry,
                    },
                    _ => unreachable!("control command"),
                };
                if !matches!(
                    control(&mut client, &mut ids, action)?,
                    ControlOutcome::InputAccepted {} | ControlOutcome::Resized { .. }
                ) {
                    return Err(TerminalStatus::Unavailable);
                }
            }
        }
    }
    Ok(())
}
fn viewer(client: &mut LocalClient, ids: &mut Identities, action: Viewer) -> Result<ViewerOutcome> {
    client
        .submit_terminal_viewer(TerminalViewerRequest::new(ids.next()?, action))
        .map(|result| result.into_outcome())
        .map_err(submission)
}
fn control(
    client: &mut LocalClient,
    ids: &mut Identities,
    action: Control,
) -> Result<ControlOutcome> {
    let result = client
        .submit_terminal_control(TerminalControlRequest::new(ids.next()?, action))
        .map_err(submission)?
        .into_outcome();
    if matches!(result, ControlOutcome::OutcomeUnknown {}) {
        return Err(TerminalStatus::OutcomeUnknown);
    }
    Ok(result)
}
fn submission(error: SubmissionError) -> TerminalStatus {
    match error {
        SubmissionError::OutcomeUnknown { .. } => TerminalStatus::OutcomeUnknown,
        _ => TerminalStatus::Unavailable,
    }
}
