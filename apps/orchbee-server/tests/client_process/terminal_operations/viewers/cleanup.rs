use super::controllers::{control, input, read_control, reply_control, send_control};
use super::*;
use orchbee_capability_node_protocol::{
    TerminalControlAction as WorkerControl, TerminalControlOutcome as WorkerControlOutcome,
    TerminalControlRequest as WorkerControlRequest,
};
use orchbee_client_server_protocol::{
    ControlInput, ControlRejectedReason, ControllerId, TerminalControlAction as ControlAction,
    TerminalControlOutcome as ControlOutcome, ViewerId,
};
use std::num::NonZeroU64;

fn create_peer(
    client: &mut UnixStream,
    worker: &mut UnixStream,
    reference: u128,
    terminal: u8,
) -> TestResult {
    write_client_message(client, &create(reference)?)?;
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = read_obcn(worker)? else {
        return Err("missing terminal launch".into());
    };
    write_obcn(
        worker,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            request.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                HostTerminalIdentity::new(
                    HostCreationId::from_bytes([terminal; 16])?,
                    HostEpoch::from_bytes([4; 16])?,
                    HostTerminalId::from_bytes([terminal; 16])?,
                ),
                HostLaunchState::AwaitingStart,
            )),
        )?),
    )?;
    assert!(
        matches!(read_server_message(client)?, ServerMessage::TerminalCreateResult(result)
        if result.request_id() == &id(reference)? && result.outcome() == CreateOutcome::Accepted { state: CreateAcceptedState::AwaitingStart })
    );
    Ok(())
}

fn attach_peer(
    client: &mut UnixStream,
    worker: &mut UnixStream,
    reference: u128,
) -> TestResult<(ViewerId, WorkerRequest)> {
    let request_id = send_viewer(
        client,
        Action::Attach {
            terminal_create_request_id: id(reference)?,
        },
    )?;
    let forwarded = read_viewer(worker)?;
    assert!(matches!(forwarded.action, WorkerAction::Attach { .. }));
    reply(worker, forwarded, WorkerOutcome::Attached)?;
    let Outcome::Attached { attachment_id, .. } = receive_viewer(client, &request_id)? else {
        return Err("missing attached identity".into());
    };
    Ok((attachment_id, forwarded))
}

fn acquire_peer(
    client: &mut UnixStream,
    worker: &mut UnixStream,
    attachment: &ViewerId,
) -> TestResult<(ControllerId, WorkerControlRequest)> {
    let request_id = send_control(
        client,
        ControlAction::Acquire {
            attachment_id: attachment.clone(),
        },
    )?;
    let forwarded = read_control(worker)?;
    assert_eq!(forwarded.action, WorkerControl::Acquire);
    reply_control(worker, &forwarded, WorkerControlOutcome::Acquired)?;
    let ServerMessage::TerminalControlResult(result) = read_server_message(client)? else {
        return Err("missing acquisition result".into());
    };
    assert_eq!(result.request_id(), &request_id);
    let ControlOutcome::Acquired { controller_id } = result.into_outcome() else {
        return Err("controller acquisition rejected".into());
    };
    Ok((controller_id, forwarded))
}

#[test]
fn two_viewer_session_rejects_unowned_routes_locally_and_cleans_each_exact_pair_once() -> TestResult
{
    let server = RunningServer::start()?;
    let mut worker = connect_worker(server.worker_endpoint(), FIRST_WORKER)?;
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut foreign, _) = connect_bounded_client(server.client_endpoint())?;
    create_peer(&mut client, &mut worker, 1, 5)?;
    create_peer(&mut client, &mut worker, 2, 6)?;
    let (a, attached_a) = attach_peer(&mut client, &mut worker, 1)?;
    let (b, attached_b) = attach_peer(&mut client, &mut worker, 2)?;
    assert_ne!(a, b);
    assert_ne!(attached_a.action, attached_b.action);
    let (grant_a, acquired_a) = acquire_peer(&mut client, &mut worker, &a)?;
    let (_grant_b, acquired_b) = acquire_peer(&mut client, &mut worker, &b)?;
    assert_eq!(acquired_a.viewer_id, attached_a.attachment_id);
    assert_eq!(acquired_b.viewer_id, attached_b.attachment_id);
    assert_ne!(acquired_a.controller_id, acquired_b.controller_id);

    assert_eq!(
        viewer(
            &mut foreign,
            Action::Poll {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Rejected {
            reason: ViewerRejectedReason::TerminalUnavailable
        }
    );
    assert_eq!(
        viewer(
            &mut foreign,
            Action::Detach {
                attachment_id: a.clone()
            }
        )?,
        Outcome::Detached {}
    );
    assert_eq!(
        control(
            &mut foreign,
            ControlAction::Acquire {
                attachment_id: a.clone()
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ViewerUnavailable
        }
    );
    assert_eq!(
        input(&mut foreign, &a, &grant_a, 1, b"foreign".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::ViewerUnavailable
        }
    );
    assert_eq!(
        input(&mut client, &b, &grant_a, 1, b"wrong-grant".to_vec())?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    assert_eq!(
        control(
            &mut client,
            ControlAction::Resize {
                attachment_id: b.clone(),
                controller_id: grant_a.clone(),
                sequence: NonZeroU64::MIN,
                geometry: geometry()?,
            }
        )?,
        ControlOutcome::Rejected {
            reason: ControlRejectedReason::NotController
        }
    );
    // A positive marker is the next Worker request; no rejection above may forward work.
    let marker = send_control(
        &mut client,
        ControlAction::Input {
            attachment_id: a,
            controller_id: grant_a,
            sequence: NonZeroU64::MIN,
            bytes: ControlInput::new(b"marker".to_vec())?,
        },
    )?;
    let forwarded = read_control(&mut worker)?;
    assert_eq!(forwarded.viewer_id, acquired_a.viewer_id);
    assert_eq!(forwarded.controller_id, acquired_a.controller_id);
    assert!(
        matches!(&forwarded.action, WorkerControl::Input { sequence, bytes }
        if *sequence == NonZeroU64::MIN && bytes.as_bytes() == b"marker")
    );
    reply_control(&mut worker, &forwarded, WorkerControlOutcome::InputAccepted)?;
    assert!(
        matches!(read_server_message(&mut client)?, ServerMessage::TerminalControlResult(result)
        if result.request_id() == &marker && result.outcome() == &ControlOutcome::InputAccepted {})
    );
    drop(client);

    let expected = [acquired_a, acquired_b];
    let mut released = [false; 2];
    let mut detached = [false; 2];
    for _ in 0..4 {
        match read_obcn(&mut worker)? {
            CapabilityNodeMessage::TerminalControlRequest(request) => {
                assert_eq!(request.action, WorkerControl::Release);
                let index = expected
                    .iter()
                    .position(|pair| pair.viewer_id == request.viewer_id)
                    .ok_or("cleanup released an unrelated viewer")?;
                assert!(!released[index], "controller released twice");
                assert_eq!(request.controller_id, expected[index].controller_id);
                assert_eq!(request.worker_id, expected[index].worker_id);
                assert_eq!(request.worker_generation, expected[index].worker_generation);
                released[index] = true;
                reply_control(&mut worker, &request, WorkerControlOutcome::Released)?;
            }
            CapabilityNodeMessage::TerminalViewerRequest(request) => {
                assert_eq!(request.action, WorkerAction::Detach);
                let index = expected
                    .iter()
                    .position(|pair| pair.viewer_id == request.attachment_id)
                    .ok_or("cleanup detached an unrelated viewer")?;
                assert!(released[index], "Detach preceded controller Release");
                assert!(!detached[index], "viewer detached twice");
                assert_eq!(request.worker_id, expected[index].worker_id);
                assert_eq!(request.worker_generation, expected[index].worker_generation);
                detached[index] = true;
                reply(&mut worker, request, WorkerOutcome::Detached)?;
            }
            other => {
                return Err(format!("session cleanup performed unrelated work: {other:?}").into());
            }
        }
    }
    assert_eq!(released, [true; 2]);
    assert_eq!(detached, [true; 2]);
    // Existing terminal references remain usable and the next Worker operation is Attach,
    // bounding the cleanup trace without relying on an idle socket timeout.
    let (replacement_a, new_a) = attach_peer(&mut foreign, &mut worker, 1)?;
    let (replacement_b, new_b) = attach_peer(&mut foreign, &mut worker, 2)?;
    assert_eq!(new_a.action, attached_a.action);
    assert_eq!(new_b.action, attached_b.action);
    assert_ne!(new_a.attachment_id, attached_a.attachment_id);
    assert_ne!(new_b.attachment_id, attached_b.attachment_id);
    let (_, new_grant_a) = acquire_peer(&mut foreign, &mut worker, &replacement_a)?;
    let (_, new_grant_b) = acquire_peer(&mut foreign, &mut worker, &replacement_b)?;
    assert_ne!(new_grant_a.controller_id, expected[0].controller_id);
    assert_ne!(new_grant_b.controller_id, expected[1].controller_id);
    Ok(())
}
