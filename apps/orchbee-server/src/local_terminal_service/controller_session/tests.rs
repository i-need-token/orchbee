use super::*;

#[test]
fn dropping_a_revoked_controller_is_local_and_preserves_the_current_reservation() {
    use crate::{worker_obcn_carrier::create_worker_carrier, worker_registry::WorkerRegistry};
    use orchbee_capability_node_protocol::{
        CapabilityNodeMessage, TerminalControlResult, WorkerGeneration,
        WorkerId as CapabilityWorkerId, read_message, write_message,
    };
    use std::{
        num::NonZeroU64,
        os::unix::net::UnixStream,
        sync::{Arc, Mutex},
        thread,
    };

    let worker_id = orchbee_local_site_protocol::WorkerId::from_bytes([7; 16]);
    let old_id = OperationId::from_bytes([8; 16]).unwrap();
    let new_id = OperationId::from_bytes([9; 16]).unwrap();
    for current_owner in [None, Some(new_id), Some(old_id)] {
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
            assert!(reserve(
                &mut service.controllers.lock().unwrap(),
                target,
                id
            ));
        }
        let sibling = ControlTarget {
            terminal: [4; 16],
            ..target
        };
        assert!(reserve(
            &mut service.controllers.lock().unwrap(),
            sibling,
            new_id
        ));
        let cleanup = WorkerRequest {
            request_id: OperationId::from_bytes([10; 16]).unwrap(),
            controller_id: old_id,
            viewer_id: OperationId::from_bytes([11; 16]).unwrap(),
            worker_id: CapabilityWorkerId::from_bytes(worker_id.as_bytes()).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action: WorkerAction::Release,
        };
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
                        outcome: WorkerOutcome::Released,
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
            armed: true,
            sequence: 0,
        });
        assert!(
            matches!(handle.request(WorkerOperationRequest::Control(marker)),
            Ok(WorkerOperationResult::Control(result)) if result.outcome == WorkerOutcome::Released)
        );
        let observed = peer_thread.join().unwrap();
        drop(shutdown);
        let _exit = owner_thread.join().unwrap();
        let expected = if current_owner == Some(old_id) {
            vec![cleanup_id, marker_id]
        } else {
            vec![marker_id]
        };
        assert_eq!(observed, expected, "reservation {current_owner:?}");
        let owners = service.controllers.lock().unwrap();
        assert_eq!(
            owners.get(&target).copied(),
            current_owner.filter(|id| *id != old_id)
        );
        assert_eq!(owners.get(&sibling), Some(&new_id));
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
    assert!(reserve(&mut owners, target, first));
    assert!(!reserve(&mut owners, target, next));
    remove_exact(&mut owners, target, next);
    assert_eq!(owners.get(&target), Some(&first));
    remove_exact(&mut owners, target, first);
    assert!(reserve(&mut owners, target, next));
    remove_exact(&mut owners, target, first);
    assert_eq!(owners.get(&target), Some(&next));
    assert!(reserve(
        &mut owners,
        ControlTarget {
            terminal: [6; 16],
            ..target
        },
        first
    ));
    Ok(())
}
