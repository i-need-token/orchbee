use super::*;
use crate::site_coordinator::ProcessObservationTarget;
use crate::terminal_coordinator::{BindingViewerTarget, WorkerPlacement};
use crate::worker_registry::WorkerRegistry;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, TerminalId, WorkerGeneration,
    WorkerId,
};
use orchbee_workbench_protocol::{PresentationBinding, SiteId};
use std::num::NonZeroU64;
fn target(n: u128) -> ProcessObservationTarget {
    ProcessObservationTarget {
        site_id: SiteId::parse(&format!("{:032x}", 99)).unwrap(),
        binding: PresentationBinding {
            worker_id: orchbee_workbench_protocol::WorkerId::parse(&format!("{:032x}", 1)).unwrap(),
            terminal_id: orchbee_workbench_protocol::TerminalId::parse(&format!("{n:032x}"))
                .unwrap(),
        },
        target: BindingViewerTarget {
            terminal_id: TerminalId::from_bytes(n.to_be_bytes()).unwrap(),
            worker: WorkerPlacement {
                worker_id: WorkerId::from_bytes([1; 16]).unwrap(),
                generation: WorkerGeneration::new(NonZeroU64::MIN),
            },
            identity: HostTerminalIdentity::new(
                HostCreationId::from_bytes([2; 16]).unwrap(),
                HostEpoch::from_bytes([3; 16]).unwrap(),
                HostTerminalId::from_bytes([4; 16]).unwrap(),
            ),
        },
    }
}
#[test]
fn turns_are_bounded_fair_and_wrap_after_membership_changes() {
    let targets: Vec<_> = (1..=11).map(target).collect();
    assert_eq!(select_turn(&targets, None), (0..8).collect::<Vec<_>>());
    assert_eq!(
        select_turn(&targets, Some(&targets[7].key())),
        [8, 9, 10, 0, 1, 2, 3, 4]
    );
    assert_eq!(
        select_turn(&targets, Some(&target(99).key())),
        (0..8).collect::<Vec<_>>()
    );
    assert_eq!(select_turn(&targets[..2], Some(&targets[0].key())), [1, 0]);
    assert!(select_turn(&[], None).is_empty());
}
#[test]
fn idle_observer_wakes_joins_and_cannot_restart_after_stop() {
    let service = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));
    service.start_process_observer().unwrap();
    assert_eq!(
        service.start_process_observer().unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    service
        .stop_process_observer_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert!(service.process_observer.handle.lock().unwrap().is_none());
    assert_eq!(
        service.start_process_observer().unwrap_err().kind(),
        io::ErrorKind::NotConnected
    );
}
#[test]
fn deadline_keeps_live_handle_until_its_admitted_work_really_finishes() {
    let owner = ProcessObserver::default();
    let (send, receive) = std::sync::mpsc::channel();
    *owner.handle.lock().unwrap() = Some(thread::spawn(move || receive.recv().unwrap()));
    assert_eq!(
        owner.stop_before(Instant::now()).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(
        owner
            .handle
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    );
    send.send(()).unwrap();
    owner
        .stop_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert!(owner.handle.lock().unwrap().is_none());
}
