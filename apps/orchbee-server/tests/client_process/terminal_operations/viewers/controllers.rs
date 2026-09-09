use super::*;
use orchbee_client_server_protocol::{
    ControlInput, ControlRejectedReason, ControllerId, TerminalControlAction as ControlAction,
    TerminalControlOutcome as ControlOutcome, TerminalControlRequest,
};
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) fn control(
    client: &mut UnixStream,
    action: ControlAction,
) -> TestResult<ControlOutcome> {
    static NEXT: AtomicU64 = AtomicU64::new(10_000);
    let request_id = id(u128::from(NEXT.fetch_add(1, Ordering::Relaxed)))?;
    let ServerMessage::TerminalControlResult(result) = exchange(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(request_id.clone(), action)),
    )?
    else {
        return Err("missing control result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    Ok(result.into_outcome())
}
pub(super) fn acquire(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
) -> TestResult<ControllerId> {
    let ControlOutcome::Acquired { controller_id } = control(
        client,
        ControlAction::Acquire {
            attachment_id: attachment_id.clone(),
        },
    )?
    else {
        return Err("controller acquisition failed".into());
    };
    Ok(controller_id)
}
pub(super) fn input(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    grant: &ControllerId,
    sequence: u64,
    bytes: Vec<u8>,
) -> TestResult<ControlOutcome> {
    control(
        client,
        ControlAction::Input {
            attachment_id: attachment_id.clone(),
            controller_id: grant.clone(),
            sequence: NonZeroU64::new(sequence).ok_or("zero sequence")?,
            bytes: ControlInput::new(bytes)?,
        },
    )
}
pub(super) fn observed(path: &Path, suffix: &str) -> TestResult<String> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match std::fs::read_to_string(path) {
            Ok(value) if value.ends_with(suffix) => return Ok(value),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if Instant::now() >= deadline {
            return Err("shell did not publish the input witness".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
}
pub(super) fn exercise(
    first: &mut UnixStream,
    second: &mut UnixStream,
    runtime: &Path,
    first_attachment: &orchbee_client_server_protocol::ViewerId,
    second_attachment: &orchbee_client_server_protocol::ViewerId,
) -> TestResult {
    let grant = acquire(first, first_attachment)?;
    assert_eq!(
        control(
            second,
            ControlAction::Acquire {
                attachment_id: second_attachment.clone()
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy
        }
    );
    let output = runtime.join("controller-witness");
    let quoted = format!("'{}'", output.display().to_string().replace('\'', "'\\''"));
    assert_eq!(
        input(
            first,
            first_attachment,
            &grant,
            1,
            format!("printf 'start\\n' > {quoted}\n").into_bytes()
        )?,
        ControlOutcome::InputAccepted {}
    );
    observed(&output, "start\n")?;
    let forbidden = format!("printf 'bad\\n' >> {quoted}\n").into_bytes();
    assert_eq!(
        input(second, second_attachment, &grant, 1, forbidden.clone())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    assert_eq!(
        input(first, first_attachment, &grant, 1, forbidden)?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::SequenceMismatch
        }
    );
    let resized = Geometry::new(100, 31, 8, 16)?;
    assert_eq!(
        control(
            second,
            ControlAction::Resize {
                attachment_id: second_attachment.clone(),
                controller_id: grant.clone(),
                sequence: NonZeroU64::MIN,
                geometry: resized
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    assert_eq!(
        control(
            first,
            ControlAction::Resize {
                attachment_id: first_attachment.clone(),
                controller_id: grant.clone(),
                sequence: NonZeroU64::new(2).ok_or("zero sequence")?,
                geometry: resized
            }
        )?,
        ControlOutcome::Resized { geometry: resized }
    );
    assert_eq!(
        input(
            first,
            first_attachment,
            &grant,
            3,
            format!("stty size >> {quoted}; printf 'end\\n' >> {quoted}\n").into_bytes()
        )?,
        ControlOutcome::InputAccepted {}
    );
    let content = observed(&output, "end\n")?;
    assert_eq!(
        content.split_whitespace().collect::<Vec<_>>(),
        vec!["start", "31", "100", "end"]
    );
    assert_eq!(
        viewer(
            first,
            Action::Detach {
                attachment_id: first_attachment.clone()
            }
        )?,
        Outcome::Detached {}
    );
    assert_eq!(
        input(second, second_attachment, &grant, 4, b"invalid\n".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    let replacement = acquire(second, second_attachment)?;
    assert_ne!(replacement, grant);
    assert_eq!(
        input(second, second_attachment, &grant, 5, b"invalid\n".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    assert_eq!(
        input(
            second,
            second_attachment,
            &replacement,
            1,
            format!("printf 'second\\n' >> {quoted}\n").into_bytes()
        )?,
        ControlOutcome::InputAccepted {}
    );
    assert!(observed(&output, "second\n")?.ends_with("end\nsecond\n"));
    Ok(())
}

pub(super) fn send_control(
    client: &mut UnixStream,
    action: ControlAction,
) -> TestResult<RequestId> {
    static NEXT: AtomicU64 = AtomicU64::new(20_000);
    let request_id = id(u128::from(NEXT.fetch_add(1, Ordering::Relaxed)))?;
    write_client_message(
        client,
        &ClientMessage::TerminalControl(TerminalControlRequest::new(request_id.clone(), action)),
    )?;
    Ok(request_id)
}
pub(super) fn read_control(
    worker: &mut UnixStream,
) -> TestResult<orchbee_capability_node_protocol::TerminalControlRequest> {
    let CapabilityNodeMessage::TerminalControlRequest(request) = read_obcn(worker)? else {
        return Err("missing Worker control request".into());
    };
    Ok(request)
}
pub(super) fn reply_control(
    worker: &mut UnixStream,
    request: &orchbee_capability_node_protocol::TerminalControlRequest,
    outcome: orchbee_capability_node_protocol::TerminalControlOutcome,
) -> TestResult {
    write_obcn(
        worker,
        &CapabilityNodeMessage::TerminalControlResult(
            orchbee_capability_node_protocol::TerminalControlResult {
                request_id: request.request_id,
                viewer_id: request.viewer_id,
                controller_id: request.controller_id,
                outcome,
            },
        ),
    )?;
    Ok(())
}
fn attach_peer(
    client: &mut UnixStream,
    worker: &mut UnixStream,
    request_id: u128,
) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    write_client_message(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(request_id)?,
            Action::Attach {
                terminal_create_request_id: id(1)?,
            },
        )),
    )?;
    let request = read_viewer(worker)?;
    reply(worker, request, WorkerOutcome::Attached)?;
    let ServerMessage::TerminalViewerResult(result) = read_server_message(client)? else {
        return Err("missing Attach result".into());
    };
    assert_eq!(result.request_id(), &id(request_id)?);
    let Outcome::Attached { attachment_id, .. } = result.into_outcome() else {
        return Err("Attach failed".into());
    };
    Ok(attachment_id)
}

#[test]
fn abandoned_acquisition_keeps_reservation_until_exact_controller_cleanup_finishes() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalControlAction as WorkerControl, TerminalControlOutcome as ControlResult,
    };
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut first, &create(1)?)?;
    accept_launch(&mut worker)?;
    let _created = read_server_message(&mut first)?;
    let first_attachment = attach_peer(&mut first, &mut worker, 2)?;
    let second_attachment = attach_peer(&mut second, &mut worker, 3)?;
    send_control(
        &mut first,
        ControlAction::Acquire {
            attachment_id: first_attachment.clone(),
        },
    )?;
    let acquiring = read_control(&mut worker)?;
    assert_eq!(acquiring.action, WorkerControl::Acquire);
    assert_eq!(
        control(
            &mut second,
            ControlAction::Acquire {
                attachment_id: second_attachment.clone()
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy
        }
    );
    drop(first);
    reply_control(&mut worker, &acquiring, ControlResult::Acquired)?;
    let cleanup = read_control(&mut worker)?;
    assert_eq!(cleanup.action, WorkerControl::Release);
    assert_eq!(cleanup.controller_id, acquiring.controller_id);
    assert_eq!(cleanup.viewer_id, acquiring.viewer_id);
    assert_eq!(
        control(
            &mut second,
            ControlAction::Acquire {
                attachment_id: second_attachment.clone()
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ControllerBusy
        }
    );
    reply_control(&mut worker, &cleanup, ControlResult::Released)?;
    let detached = read_viewer(&mut worker)?;
    assert_eq!(detached.action, WorkerAction::Detach);
    reply(&mut worker, detached, WorkerOutcome::Detached)?;
    send_control(
        &mut second,
        ControlAction::Acquire {
            attachment_id: second_attachment.clone(),
        },
    )?;
    let replacement = read_control(&mut worker)?;
    assert_ne!(replacement.controller_id, acquiring.controller_id);
    assert_ne!(replacement.viewer_id, acquiring.viewer_id);
    reply_control(&mut worker, &replacement, ControlResult::Acquired)?;
    assert!(
        matches!(read_server_message(&mut second)?,ServerMessage::TerminalControlResult(result) if matches!(result.outcome(),ControlOutcome::Acquired {..}))
    );
    Ok(())
}

#[test]
fn lost_worker_after_input_delivery_reports_unknown_and_retires_the_grant() -> TestResult {
    use orchbee_capability_node_protocol::{
        TerminalControlAction as WorkerControl, TerminalControlOutcome as ControlResult,
    };
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut client, &create(1)?)?;
    accept_launch(&mut worker)?;
    let _created = read_server_message(&mut client)?;
    let attachment_id = attach_peer(&mut client, &mut worker, 2)?;
    send_control(
        &mut client,
        ControlAction::Acquire {
            attachment_id: attachment_id.clone(),
        },
    )?;
    let acquiring = read_control(&mut worker)?;
    reply_control(&mut worker, &acquiring, ControlResult::Acquired)?;
    let ServerMessage::TerminalControlResult(result) = read_server_message(&mut client)? else {
        return Err("missing acquisition".into());
    };
    let ControlOutcome::Acquired { controller_id } = result.into_outcome() else {
        return Err("acquisition rejected".into());
    };
    let request_id = send_control(
        &mut client,
        ControlAction::Input {
            attachment_id: attachment_id.clone(),
            controller_id: controller_id.clone(),
            sequence: NonZeroU64::MIN,
            bytes: ControlInput::new(b"effect\n".to_vec())?,
        },
    )?;
    let input = read_control(&mut worker)?;
    assert!(matches!(input.action, WorkerControl::Input { .. }));
    drop(worker);
    assert!(
        matches!(read_server_message(&mut client)?,ServerMessage::TerminalControlResult(result) if result.request_id()==&request_id && result.outcome()==&ControlOutcome::OutcomeUnknown {})
    );
    assert_eq!(
        control(
            &mut client,
            ControlAction::Input {
                attachment_id,
                controller_id,
                sequence: NonZeroU64::new(2).ok_or("zero")?,
                bytes: ControlInput::new(b"next\n".to_vec())?
            }
        )?,
        ControlOutcome::Unavailable {
            reason:
                orchbee_client_server_protocol::ControlUnavailableReason::TerminalRuntimeUnavailable
        }
    );
    Ok(())
}
