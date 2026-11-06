use super::*;
use orchbee_local_ipc::LocalStream as UnixStream;

#[test]
fn controller_cleanup_preserves_replacements_and_requires_both_transfer_receipts() {
    use crate::{worker_obcn_carrier::create_worker_carrier, worker_registry::WorkerRegistry};
    use orchbee_capability_node_protocol::{
        CapabilityNodeMessage, TerminalControlResult, WorkerGeneration,
        WorkerId as CapabilityWorkerId, read_message, write_message,
    };
    use std::{
        num::NonZeroU64,
        sync::{Arc, Mutex},
        thread,
    };

    let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([7; 16]);
    let old_id = OperationId::from_bytes([8; 16]).unwrap();
    let new_id = OperationId::from_bytes([9; 16]).unwrap();
    for (current_owner, has_previous, previous_refused, armed) in [
        (None, false, false, true),
        (Some(new_id), false, false, true),
        (Some(old_id), false, false, true),
        (Some(old_id), true, false, false),
        (Some(old_id), true, false, true),
        (Some(old_id), true, true, true),
    ] {
        let service = LocalTerminalService::from_identity(
            Arc::new(Mutex::new(WorkerRegistry::new())),
            Err(std::io::ErrorKind::PermissionDenied.into()),
        );
        let target = ControlTarget {
            worker: worker_id.as_bytes(),
            generation: 1,
            host_epoch: [2; 16],
            terminal: [3; 16],
        };
        if let Some(id) = current_owner {
            assert!(
                reserve(
                    &mut service.controllers.lock().unwrap(),
                    target,
                    reservation(id),
                    false
                )
                .is_ok()
            );
        }
        let sibling = ControlTarget {
            terminal: [4; 16],
            ..target
        };
        assert!(
            reserve(
                &mut service.controllers.lock().unwrap(),
                sibling,
                reservation(new_id),
                false
            )
            .is_ok()
        );
        let cleanup = WorkerRequest {
            request_id: OperationId::from_bytes([10; 16]).unwrap(),
            controller_id: old_id,
            viewer_id: OperationId::from_bytes([11; 16]).unwrap(),
            worker_id: CapabilityWorkerId::from_bytes(worker_id.as_bytes()).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action: WorkerAction::Release,
        };
        let previous = has_previous.then(|| WorkerRequest {
            request_id: OperationId::from_bytes([14; 16]).unwrap(),
            controller_id: OperationId::from_bytes([13; 16]).unwrap(),
            viewer_id: OperationId::from_bytes([15; 16]).unwrap(),
            ..cleanup.clone()
        });
        let previous_id = OperationId::from_bytes([14; 16]).unwrap();
        let marker = WorkerRequest {
            request_id: OperationId::from_bytes([12; 16]).unwrap(),
            ..cleanup.clone()
        };
        let marker_id = marker.request_id;
        let carrier = create_worker_carrier(worker_id, NonZeroU64::MIN);
        let (handle, shutdown, owner) = carrier.into_parts();
        let (server, mut peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let owner_thread = thread::spawn(move || owner.run(server));
        let peer_thread = thread::spawn(move || {
            let mut observed = Vec::new();
            loop {
                let CapabilityNodeMessage::TerminalControlRequest(request) =
                    read_message(&mut peer).unwrap()
                else {
                    panic!("expected control request")
                };
                observed.push(request.request_id);
                write_message(
                    &mut peer,
                    &CapabilityNodeMessage::TerminalControlResult(TerminalControlResult {
                        request_id: request.request_id,
                        viewer_id: request.viewer_id,
                        controller_id: request.controller_id,
                        outcome: if request.request_id == previous_id && previous_refused {
                            WorkerOutcome::Rejected(ControlRejection::ControllerBusy)
                        } else {
                            WorkerOutcome::Released
                        },
                    }),
                )
                .unwrap();
                if request.request_id == marker_id {
                    return observed;
                }
            }
        });
        let cleanup_id = cleanup.request_id;
        drop(ControllerLease {
            geometry: None,
            service: &service,
            handle: handle.clone(),
            target: BindingViewerTarget {
                terminal_id: orchbee_capability_node_protocol::TerminalId::from_bytes([1; 16])
                    .unwrap(),
                worker: crate::terminal_coordinator::WorkerPlacement {
                    worker_id: CapabilityWorkerId::from_bytes(target.worker).unwrap(),
                    generation: WorkerGeneration::new(NonZeroU64::MIN),
                },
                identity: orchbee_capability_node_protocol::HostTerminalIdentity::new(
                    orchbee_capability_node_protocol::HostCreationId::from_bytes([1; 16]).unwrap(),
                    orchbee_capability_node_protocol::HostEpoch::from_bytes(target.host_epoch)
                        .unwrap(),
                    orchbee_capability_node_protocol::HostTerminalId::from_bytes(target.terminal)
                        .unwrap(),
                ),
            },
            cleanup,
            armed,
            superseded: previous,
            sequence: 0,
        });
        assert!(
            matches!(handle.request(WorkerOperationRequest::Control(marker)),
            Ok(WorkerOperationResult::Control(result)) if result.outcome == WorkerOutcome::Released)
        );
        let observed = peer_thread.join().unwrap();
        drop(shutdown);
        let _exit = owner_thread.join().unwrap();
        let mut expected = Vec::new();
        if current_owner == Some(old_id) {
            if has_previous {
                expected.push(previous_id);
            }
            if armed {
                expected.push(cleanup_id);
            }
        }
        expected.push(marker_id);
        assert_eq!(observed, expected, "reservation {current_owner:?}");
        let owners = service.controllers.lock().unwrap();
        assert_eq!(
            owners.get(&target).map(|owner| owner.controller_id),
            current_owner.filter(|id| *id != old_id || previous_refused)
        );
        assert_eq!(owners.get(&sibling), Some(&reservation(new_id)));
    }
}

#[test]
fn reservation_excludes_competitors_and_stale_cleanup_cannot_release_a_new_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let mut owners = BTreeMap::new();
    let target = ControlTarget {
        worker: [1; 16],
        generation: 1,
        host_epoch: [2; 16],
        terminal: [3; 16],
    };
    let first = OperationId::from_bytes([4; 16])?;
    let next = OperationId::from_bytes([5; 16])?;
    assert!(reserve(&mut owners, target, reservation(first), false).is_ok());
    assert!(!reserve(&mut owners, target, reservation(next), false).is_ok());
    remove_exact(&mut owners, target, next);
    assert_eq!(owners.get(&target), Some(&reservation(first)));
    remove_exact(&mut owners, target, first);
    assert!(reserve(&mut owners, target, reservation(next), false).is_ok());
    remove_exact(&mut owners, target, first);
    assert_eq!(owners.get(&target), Some(&reservation(next)));
    assert!(
        reserve(
            &mut owners,
            ControlTarget {
                terminal: [6; 16],
                ..target
            },
            reservation(first),
            false
        )
        .is_ok()
    );
    Ok(())
}

fn reservation(controller_id: OperationId) -> ControllerReservation {
    ControllerReservation {
        controller_id,
        viewer_id: OperationId::from_bytes([11; 16]).unwrap(),
        phase: ReservationPhase::Active,
    }
}

#[test]
fn takeover_fences_incumbent_and_pending_reservations_cannot_be_stolen() {
    let target = ControlTarget {
        worker: [1; 16],
        generation: 1,
        host_epoch: [2; 16],
        terminal: [3; 16],
    };
    let old = reservation(OperationId::from_bytes([4; 16]).unwrap());
    let new = ControllerReservation {
        controller_id: OperationId::from_bytes([5; 16]).unwrap(),
        viewer_id: OperationId::from_bytes([6; 16]).unwrap(),
        phase: ReservationPhase::Pending,
    };
    let mut owners = BTreeMap::new();
    assert_eq!(reserve(&mut owners, target, old, false), Ok(None));
    assert_eq!(reserve(&mut owners, target, new, false), Err(()));
    assert_eq!(reserve(&mut owners, target, new, true), Ok(Some(old)));
    assert_eq!(owners.get(&target), Some(&new));
    assert_eq!(reserve(&mut owners, target, old, true), Err(()));
    remove_exact(&mut owners, target, old.controller_id);
    assert_eq!(owners.get(&target), Some(&new));
    remove_exact(&mut owners, target, new.controller_id);
    assert_eq!(reserve(&mut owners, target, new, true), Ok(None));
}

#[test]
fn focus_and_mouse_reports_do_not_acknowledge_a_completed_turn() {
    for bytes in [
        b"\x1b[I".as_slice(),
        b"\x1b[O",
        b"\x1b[I\x1b[O",
        b"\x1b[<0;4;9M",
        b"\x1b[<0;4;9m",
        b"\x1b[M !!",
    ] {
        let action = Action::Input {
            attachment_id: orchbee_client_server_protocol::ViewerId::parse(&"1".repeat(32))
                .unwrap(),
            controller_id: orchbee_client_server_protocol::ControllerId::parse(&"2".repeat(32))
                .unwrap(),
            sequence: std::num::NonZeroU64::new(1).unwrap(),
            bytes: orchbee_client_server_protocol::ControlInput::new(bytes.to_vec()).unwrap(),
        };
        assert!(
            !acknowledges_input(&action),
            "report {bytes:?} is not typing"
        );
    }
}

#[test]
fn report_batches_preserve_done_but_real_keys_text_and_mixed_input_acknowledge() {
    for bytes in [
        b"\x1b[<64;4;9M\x1b[O".as_slice(),
        b"\x1b[32;4;9M",
        b"\x1b[M \xc3\xa9",
        b"\x9bI",
        b"\x9b<0;4;9m",
        b"\x1b[M \xc4\x80\xc4\x81",
    ] {
        assert!(!contains_typed_input(bytes), "report {bytes:?}");
    }
    for bytes in [
        b"x".as_slice(),
        b"\r",
        b"\x1b",
        b"\x1b[A",
        b"\x1b[1;5C",
        b"\x1b[Ihi\x1b[O",
        b"\x1b[<0;4;9Mx",
        b"\x1b[200~paste\x1b[201~",
    ] {
        assert!(contains_typed_input(bytes), "typed input {bytes:?}");
    }
    for (kind, expected) in [
        (
            orchbee_client_server_protocol::SemanticInputKind::Focus,
            false,
        ),
        (
            orchbee_client_server_protocol::SemanticInputKind::Mouse,
            false,
        ),
        (orchbee_client_server_protocol::SemanticInputKind::Key, true),
        (
            orchbee_client_server_protocol::SemanticInputKind::Text,
            true,
        ),
    ] {
        assert_eq!(
            acknowledges_input(&Action::SemanticInput {
                attachment_id: orchbee_client_server_protocol::ViewerId::parse(&"1".repeat(32))
                    .unwrap(),
                controller_id: orchbee_client_server_protocol::ControllerId::parse(&"2".repeat(32))
                    .unwrap(),
                sequence: std::num::NonZeroU64::new(1).unwrap(),
                input_kind: kind,
                payload: orchbee_client_server_protocol::SemanticInputPayload::new(vec![1])
                    .unwrap(),
            }),
            expected
        );
    }
}
