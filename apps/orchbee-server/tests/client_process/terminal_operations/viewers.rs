use super::*;
use orchbee_capability_node_protocol::{
    TerminalLaunchMode, TerminalViewerAction as WorkerAction,
    TerminalViewerOutcome as WorkerOutcome, TerminalViewerRequest as WorkerRequest,
    TerminalViewerResult as WorkerResult,
};
use orchbee_client_server_protocol::{
    SessionId as ObcsSessionId, TerminalId as ClientTerminalId, TerminalViewerAction as Action,
    TerminalViewerOutcome as Outcome, TerminalViewerRequest, ViewerRejectedReason,
    ViewerSnapshotReason, WorkerId as ClientWorkerId,
};
use orchbee_terminal_protocol::{TerminalFrameRoute, inspect_terminal_frame_route};

static NEXT_VIEWER_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(500);

fn next_viewer_request() -> TestResult<RequestId> {
    id(u128::from(
        NEXT_VIEWER_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ))
}

fn viewer(client: &mut UnixStream, action: Action) -> TestResult<Outcome> {
    let request_id = next_viewer_request()?;
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

fn attach(
    client: &mut UnixStream,
    reference: &RequestId,
) -> TestResult<orchbee_client_server_protocol::ViewerId> {
    let Outcome::Attached { attachment_id, .. } = viewer(
        client,
        Action::Attach {
            terminal_create_request_id: reference.clone(),
        },
    )?
    else {
        return Err("viewer Attach failed".into());
    };
    Ok(attachment_id)
}

fn next_frame(
    client: &mut UnixStream,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
) -> TestResult<TerminalFrameRoute> {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        match viewer(
            client,
            Action::Poll {
                attachment_id: attachment_id.clone(),
            },
        )? {
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
    reference: &RequestId,
    deferred: bool,
) -> TestResult {
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let first_id = attach(first, reference)?;
    let second_id = attach(&mut second, reference)?;
    assert_ne!(first_id, second_id);
    let first_frame = next_frame(first, &first_id)?;
    let second_frame = next_frame(&mut second, &second_id)?;
    assert_eq!(first_frame.attachment_id.to_text(), first_id.as_str());
    assert_eq!(second_frame.attachment_id.to_text(), second_id.as_str());
    assert_eq!(first_frame.host_epoch, second_frame.host_epoch);
    assert_eq!(
        viewer(
            first,
            Action::Poll {
                attachment_id: first_id.clone()
            }
        )?,
        Outcome::Empty {}
    );
    assert_eq!(
        viewer(
            &mut second,
            Action::NeedSnapshot {
                attachment_id: second_id.clone(),
                sequence: second_frame.sequence,
                reason: ViewerSnapshotReason::StoreReset
            }
        )?,
        Outcome::FeedbackAccepted {}
    );
    let replacement = next_frame(&mut second, &second_id)?;
    assert!(replacement.sequence > second_frame.sequence);
    assert_eq!(replacement.base_revision, None);
    let feedback = Action::FrameApplied {
        attachment_id: second_id.clone(),
        sequence: replacement.sequence,
        generation: replacement.generation,
        revision: replacement.revision,
    };
    assert_eq!(
        viewer(
            first,
            Action::FrameApplied {
                attachment_id: first_id.clone(),
                sequence: first_frame
                    .sequence
                    .checked_add(1)
                    .ok_or("frame sequence overflow")?,
                generation: first_frame.generation,
                revision: first_frame.revision
            }
        )?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::FeedbackMismatch
        }
    );
    assert_eq!(viewer(&mut second, feedback)?, Outcome::FeedbackAccepted {});
    let ServerMessage::TerminalStartResult(result) = exchange(
        &mut second,
        &ClientMessage::TerminalStart(TerminalStartRequest::new(
            id(4)?,
            reference.clone(),
            geometry()?,
        )?),
    )?
    else {
        return Err("missing Start result".into());
    };
    assert_eq!(
        result.outcome(),
        if deferred {
            StartOutcome::Accepted {
                geometry: geometry()?,
                transition: StartTransition::AlreadyStarted,
            }
        } else {
            StartOutcome::Rejected {
                reason: StartRejectedReason::TerminalNotAwaitingStart,
            }
        }
    );
    assert_eq!(
        viewer(
            first,
            Action::Detach {
                attachment_id: first_id.clone()
            }
        )?,
        Outcome::Detached {}
    );
    let fresh_id = attach(first, reference)?;
    assert_ne!(fresh_id, first_id);
    let _fresh_frame = next_frame(first, &fresh_id)?;
    controllers::exercise(first, &mut second, runtime, &fresh_id, &second_id)?;
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

fn send_viewer(client: &mut UnixStream, action: Action) -> TestResult<RequestId> {
    let request_id = next_viewer_request()?;
    write_client_message(
        client,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(request_id.clone(), action)),
    )?;
    Ok(request_id)
}

fn receive_viewer(client: &mut UnixStream, request_id: &RequestId) -> TestResult<Outcome> {
    let ServerMessage::TerminalViewerResult(result) = read_server_message(client)? else {
        return Err("missing viewer result".into());
    };
    assert_eq!(result.request_id(), request_id);
    Ok(result.into_outcome())
}

fn different_identity(value: &str) -> String {
    let mut bytes = value.as_bytes().to_vec();
    bytes[0] = if bytes[0] == b'0' { b'1' } else { b'0' };
    String::from_utf8(bytes).unwrap()
}

#[test]
fn canonical_snapshot_binding_attaches_exact_host_without_launch_and_is_session_scoped()
-> TestResult {
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;

    let initial = super::super::tab_create::create_workspace(
        &mut first,
        0x100_000,
        "/tmp",
        Instant::now() + PROCESS_TIMEOUT,
    )?;
    orchbee_workbench_protocol::write_workbench_request(
        &mut first,
        &workspace::tab_request(&initial, 1)?,
    )?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = read_obcn(&mut worker)? else {
        return Err("missing Workspace Launch".into());
    };
    let TerminalLaunchMode::Immediate(launch_geometry) = launch.mode() else {
        return Err("Workspace Launch was not immediate".into());
    };
    let host_identity = identity()?;
    write_obcn(
        &mut worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            launch.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                host_identity,
                HostLaunchState::Started(launch_geometry),
            )),
        )?),
    )?;
    assert!(matches!(
        workspace::tab_outcome(&mut first)?,
        orchbee_workbench_protocol::WorkbenchCommandOutcome::TabCreated { .. }
    ));

    let snapshot = workspace::read_snapshot(&mut first, 700)?;
    let binding = workspace::workspace_binding(&snapshot)?;
    let session_id = ObcsSessionId::parse(snapshot.session_id.as_str())?;
    let worker_id = ClientWorkerId::parse(binding.worker_id.as_str())?;
    let terminal_id = ClientTerminalId::parse(binding.terminal_id.as_str())?;
    assert_eq!(
        worker_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(launch.authorization().worker_id().as_bytes())
        )
    );
    assert_eq!(
        terminal_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(launch.authorization().terminal_id().as_bytes())
        )
    );
    assert_ne!(
        terminal_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(host_identity.terminal_id().as_bytes())
        )
    );
    let exact = Action::AttachBinding {
        session_id: session_id.clone(),
        worker_id: worker_id.clone(),
        terminal_id: terminal_id.clone(),
    };

    for wrong in [
        Action::AttachBinding {
            session_id: ObcsSessionId::parse(&different_identity(session_id.as_str()))?,
            worker_id: worker_id.clone(),
            terminal_id: terminal_id.clone(),
        },
        Action::AttachBinding {
            session_id: session_id.clone(),
            worker_id: ClientWorkerId::parse(&different_identity(worker_id.as_str()))?,
            terminal_id: terminal_id.clone(),
        },
        Action::AttachBinding {
            session_id: session_id.clone(),
            worker_id: worker_id.clone(),
            terminal_id: ClientTerminalId::parse(&different_identity(terminal_id.as_str()))?,
        },
    ] {
        assert_eq!(
            viewer(&mut first, wrong)?,
            Outcome::Rejected {
                reason: ViewerRejectedReason::TerminalUnavailable,
            }
        );
    }

    let first_request = send_viewer(&mut first, exact.clone())?;
    let first_attach = read_viewer(&mut worker)?;
    assert_eq!(first_attach.worker_id, launch.authorization().worker_id());
    assert_eq!(
        first_attach.worker_generation,
        launch.authorization().worker_generation()
    );
    assert_eq!(
        first_attach.action,
        WorkerAction::Attach {
            host_epoch: host_identity.host_epoch(),
            terminal_id: host_identity.terminal_id(),
        }
    );
    reply(&mut worker, first_attach, WorkerOutcome::Attached)?;
    let Outcome::Attached {
        attachment_id: first_attachment,
        host_epoch,
        host_terminal_id,
        ..
    } = receive_viewer(&mut first, &first_request)?
    else {
        return Err("first binding Attach failed".into());
    };
    assert_eq!(
        host_epoch.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(host_identity.host_epoch().as_bytes())
        )
    );

    assert_eq!(
        viewer(&mut first, exact.clone())?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::AlreadyAttached,
        }
    );
    let second_request = send_viewer(&mut second, exact)?;
    let second_attach = read_viewer(&mut worker)?;
    assert_eq!(second_attach.action, first_attach.action);
    assert_ne!(second_attach.attachment_id, first_attach.attachment_id);
    reply(&mut worker, second_attach, WorkerOutcome::Attached)?;
    let Outcome::Attached {
        attachment_id: second_attachment,
        host_epoch: second_epoch,
        host_terminal_id: second_terminal_id,
        ..
    } = receive_viewer(&mut second, &second_request)?
    else {
        return Err("second binding Attach failed".into());
    };
    assert_ne!(first_attachment, second_attachment);
    assert_eq!(host_epoch, second_epoch);
    assert_eq!(host_terminal_id, second_terminal_id);
    assert_eq!(
        host_terminal_id.as_str(),
        format!(
            "{:032x}",
            u128::from_be_bytes(host_identity.terminal_id().as_bytes())
        )
    );

    let first_detach = send_viewer(
        &mut first,
        Action::Detach {
            attachment_id: first_attachment,
        },
    )?;
    let first_cleanup = read_viewer(&mut worker)?;
    assert_eq!(first_cleanup.action, WorkerAction::Detach);
    assert_eq!(first_cleanup.attachment_id, first_attach.attachment_id);
    reply(&mut worker, first_cleanup, WorkerOutcome::Detached)?;
    assert_eq!(
        receive_viewer(&mut first, &first_detach)?,
        Outcome::Detached {}
    );

    let second_detach = send_viewer(
        &mut second,
        Action::Detach {
            attachment_id: second_attachment,
        },
    )?;
    let second_cleanup = read_viewer(&mut worker)?;
    assert_eq!(second_cleanup.action, WorkerAction::Detach);
    assert_eq!(second_cleanup.attachment_id, second_attach.attachment_id);
    reply(&mut worker, second_cleanup, WorkerOutcome::Detached)?;
    assert_eq!(
        receive_viewer(&mut second, &second_detach)?,
        Outcome::Detached {}
    );
    assert_eq!(workspace::read_snapshot(&mut first, 705)?, snapshot);
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
    let ServerMessage::TerminalViewerResult(result) = read_server_message(&mut slow)? else {
        return Err("missing Attach result".into());
    };
    let Outcome::Attached { attachment_id, .. } = result.into_outcome() else {
        return Err("Attach failed".into());
    };
    write_client_message(
        &mut slow,
        &ClientMessage::TerminalViewer(TerminalViewerRequest::new(
            id(3)?,
            Action::Poll { attachment_id },
        )),
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
    let mut payload =
        include_bytes!("../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin").to_vec();
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

#[path = "viewers/multiple.rs"]
mod multiple;

#[path = "viewers/cleanup.rs"]
mod cleanup;
