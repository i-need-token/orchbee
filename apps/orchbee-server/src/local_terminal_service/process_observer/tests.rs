use super::*;
use crate::session_coordinator::ProcessObservationTarget;
use crate::terminal_coordinator::{BindingViewerTarget, WorkerPlacement};
use crate::worker_registry::WorkerRegistry;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, TerminalId, WorkerGeneration,
    WorkerId,
};
use orchbee_workbench_protocol::{PresentationBinding, SessionId};
use std::{
    num::NonZeroU64,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
};

static SETTINGS_SERIAL: AtomicU64 = AtomicU64::new(0);

struct SettingsFixture(PathBuf);
impl SettingsFixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "orchbee-observer-config-{}-{}",
            std::process::id(),
            SETTINGS_SERIAL.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("config.toml")
    }
}
impl Drop for SettingsFixture {
    fn drop(&mut self) {
        let target = self.0.canonicalize().unwrap();
        assert!(target.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-observer-config-")
        );
        std::fs::remove_dir_all(target).unwrap();
    }
}
fn target(n: u128) -> ProcessObservationTarget {
    ProcessObservationTarget {
        session_id: SessionId::parse(&format!("{:032x}", 99)).unwrap(),
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
    assert_eq!(select_turn(&targets, None, 8), (0..8).collect::<Vec<_>>());
    assert_eq!(
        select_turn(&targets, Some(&targets[7].key()), 8),
        [8, 9, 10, 0, 1, 2, 3, 4]
    );
    assert_eq!(
        select_turn(&targets, Some(&target(99).key()), 8),
        (0..8).collect::<Vec<_>>()
    );
    assert_eq!(
        select_turn(&targets[..2], Some(&targets[0].key()), 8),
        [1, 0]
    );
    assert!(select_turn(&[], None, 8).is_empty());
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
fn oversized_initial_publication_does_not_install_manifest_policy_or_schedule() {
    let fixture = SettingsFixture::new();
    std::fs::write(
        fixture.file(),
        super::super::configuration::tests::maximum_accepted_source_with_escaped_profiles(),
    )
    .unwrap();
    let current =
        orchbee_local_runtime::read_user_settings_at(Some(&fixture.file()), true, |_| None)
            .unwrap();
    let startup = current.settings.clone();
    let service = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
        WorkerRegistry::new(),
    ))));

    let error = service
        .process_observer
        .start(&service, startup, current, false)
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        service
            .process_observer
            .wake
            .manifests
            .initialization_for_test(),
        (false, false)
    );
    assert!(!service.configuration.initialized_for_test());
    assert!(service.process_observer.handle.lock().unwrap().is_none());
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
