use super::*;
use orchbee_capability_node_protocol::{
    TerminalViewerAction as WorkerAction, TerminalViewerOutcome as WorkerOutcome,
    TerminalViewerRequest as WorkerRequest, TerminalViewerResult as WorkerResult,
};
use orchbee_client_server_protocol::{
    TerminalViewerAction as Action, TerminalViewerOutcome as Outcome, TerminalViewerRequest,
    ViewerRejectedReason, ViewerSnapshotReason,
};
use orchbee_terminal_protocol::{TerminalFrameRoute, inspect_terminal_frame_route};

fn viewer(client: &mut UnixStream, action: Action) -> TestResult<Outcome> {
    static NEXT_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(500);
    let request_id = id(u128::from(
        NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ))?;
    let ServerMessage::TerminalViewerResult(result) = exchange(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(request_id.clone(), action)),
    )?
    else {
        return Err("missing viewer result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    Ok(result.into_outcome())
}

fn attach(client: &mut UnixStream) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    let Outcome::Attached { attachment_id, .. } = viewer(
        client,
        Action::Attach {
            terminal_create_request_id: id(1)?,
        },
    )?
    else {
        return Err("viewer Attach failed".into());
    };
    Ok(attachment_id)
}

fn next_frame(client: &mut UnixStream) -> TestResult<TerminalFrameRoute> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match viewer(client, Action::Poll {})? {
            Outcome::Record(record) => {
                let envelope = decode_transport_envelope(&record)?;
                let route = inspect_terminal_frame_route(envelope.payload)?;
                assert_eq!(envelope.host_epoch, Some(route.host_epoch));
                return Ok(route);
            }
            Outcome::Empty {} if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5))
            }
            other => return Err(format!("frame was not delivered: {other:?}").into()),
        }
    }
}

pub(super) fn exercise_real_viewers(
    server: &RunningServer,
    first: &mut UnixStream,
    runtime: &Path,
) -> TestResult {
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let first_id = attach(first)?;
    let second_id = attach(&mut second)?;
    assert_ne!(first_id, second_id);
    let first_frame = next_frame(first)?;
    let second_frame = next_frame(&mut second)?;
    assert_eq!(first_frame.attachment_id.to_text(), first_id.as_str());
    assert_eq!(second_frame.attachment_id.to_text(), second_id.as_str());
    assert_eq!(first_frame.host_epoch, second_frame.host_epoch);
    assert_eq!(viewer(first, Action::Poll {})?, Outcome::Empty {});
    assert_eq!(
        viewer(
            &mut second,
            Action::NeedSnapshot {
                sequence: second_frame.sequence,
                reason: ViewerSnapshotReason::StoreReset
            }
        )?,
        Outcome::FeedbackAccepted {}
    );
    let replacement = next_frame(&mut second)?;
    assert!(replacement.sequence > second_frame.sequence);
    assert_eq!(replacement.base_revision, None);
    let feedback = Action::FrameApplied {
        sequence: replacement.sequence,
        generation: replacement.generation,
        revision: replacement.revision,
    };
    assert_eq!(
        viewer(first, feedback.clone())?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::FeedbackMismatch
        }
    );
    assert_eq!(viewer(&mut second, feedback)?, Outcome::FeedbackAccepted {});
    assert!(
        matches!(exchange(&mut second, &start(4, 1)?)?, ServerMessage::TerminalStartResult(result) if result.outcome() == StartOutcome::Accepted { geometry: geometry()?, transition: StartTransition::AlreadyStarted })
    );
    assert_eq!(viewer(first, Action::Detach {})?, Outcome::Detached {});
    assert_ne!(attach(first)?, first_id);
    let _fresh_frame = next_frame(first)?;
    controllers::exercise(first, &mut second, runtime)?;
    // The second handler's Drop owns its Detach; the terminal remains owned by Host.
    drop(second);
    Ok(())
}

fn accept_launch(worker: &mut UnixStream) -> TestResult {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = read_obcn(worker)? else {
        return Err("missing Launch".into());
    };
    write_obcn(
        worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            request.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity()?,
                HostLaunchState::AwaitingStart,
            )),
        )?),
    )?;
    Ok(())
}

fn read_viewer(worker: &mut UnixStream) -> TestResult<WorkerRequest> {
    let CapabilityNodeMessage::TerminalViewerRequest(request) = read_obcn(worker)? else {
        return Err("missing viewer operation".into());
    };
    Ok(request)
}

fn reply(worker: &mut UnixStream, request: WorkerRequest, outcome: WorkerOutcome) -> TestResult {
    write_obcn(
        worker,
        &CapabilityNodeMessage::TerminalViewerResult(WorkerResult {
            request_id: request.request_id,
            attachment_id: request.attachment_id,
            outcome,
        }),
    )?;
    Ok(())
}

#[test]
fn client_loss_during_attach_detaches_committed_identity_and_preserves_worker() -> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut client, &create(1)?)?;
    accept_launch(&mut worker)?;
    let _created = read_server_message(&mut client)?;
    write_client_message(
        &mut client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(2)?,
            Action::Attach {
                terminal_create_request_id: id(1)?,
            },
        )),
    )?;
    let attached = read_viewer(&mut worker)?;
    assert!(matches!(attached.action, WorkerAction::Attach { .. }));
    drop(client);
    reply(&mut worker, attached, WorkerOutcome::Attached)?;
    let cleanup = read_viewer(&mut worker)?;
    assert_eq!(cleanup.action, WorkerAction::Detach);
    assert_eq!(cleanup.attachment_id, attached.attachment_id);
    assert_ne!(cleanup.request_id, attached.request_id);
    reply(&mut worker, cleanup, WorkerOutcome::Detached)?;
    let (mut replacement, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut replacement, &create(3)?)?;
    accept_launch(&mut worker)?;
    assert!(
        matches!(read_server_message(&mut replacement)?, ServerMessage::TerminalCreateResult(result) if matches!(result.outcome(), CreateOutcome::Accepted { .. }))
    );
    Ok(())
}

#[test]
fn blocked_client_output_releases_worker_before_publication_and_times_out_only_its_session()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut slow, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut slow, &create(1)?)?;
    accept_launch(&mut worker)?;
    let _created = read_server_message(&mut slow)?;
    write_client_message(
        &mut slow,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(2)?,
            Action::Attach {
                terminal_create_request_id: id(1)?,
            },
        )),
    )?;
    let attached = read_viewer(&mut worker)?;
    reply(&mut worker, attached, WorkerOutcome::Attached)?;
    let _attached = read_server_message(&mut slow)?;
    write_client_message(
        &mut slow,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(id(3)?, Action::Poll {})),
    )?;
    let polled = read_viewer(&mut worker)?;
    reply(
        &mut worker,
        polled,
        WorkerOutcome::Record(large_record(attached)?),
    )?;
    // Never read the large response. A separate Client still drives the same Worker.
    let (mut active, _) = connect_bounded_client(server.client_endpoint())?;
    write_client_message(&mut active, &create(4)?)?;
    accept_launch(&mut worker)?;
    assert!(
        matches!(read_server_message(&mut active)?, ServerMessage::TerminalCreateResult(result) if matches!(result.outcome(), CreateOutcome::Accepted { .. }))
    );
    let cleanup = read_viewer(&mut worker)?;
    assert_eq!(cleanup.action, WorkerAction::Detach);
    assert_eq!(cleanup.attachment_id, attached.attachment_id);
    reply(&mut worker, cleanup, WorkerOutcome::Detached)?;
    write_client_message(&mut active, &create(5)?)?;
    accept_launch(&mut worker)?;
    let _still_active = read_server_message(&mut active)?;
    Ok(())
}

fn large_record(request: WorkerRequest) -> TestResult<Vec<u8>> {
    // This fixture exercises opaque routing and socket pressure, not a presentation decoder.
    let mut payload = include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    )
    .to_vec();
    payload.resize(4 * 1024 * 1024, 0);
    let length = u32::try_from(payload.len())?;
    payload[8..12].copy_from_slice(&length.to_le_bytes());
    payload[6] = 1;
    payload[16..32].copy_from_slice(&identity()?.host_epoch().as_bytes());
    payload[32..48].copy_from_slice(&request.attachment_id.as_bytes());
    payload[48..56].copy_from_slice(&1_u64.to_le_bytes());
    payload[56..64].fill(255);
    payload[64..72].copy_from_slice(&1_u64.to_le_bytes());
    payload[72..80].copy_from_slice(&1_u64.to_le_bytes());
    Ok(encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(orchbee_terminal_protocol::HostEpoch::from_bytes(
            identity()?.host_epoch().as_bytes(),
        )?),
        kind: 41,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::TerminalFrame,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })?)
}

#[path = "viewers/controllers.rs"]
mod controllers;
