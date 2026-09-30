use super::super::*;
use crate::site_coordinator::PreparedTerminalTabCreate;
use crate::terminal_coordinator::{Completion, WorkerPlacement};
use crate::worker_obcn_carrier::create_worker_carrier;
use crate::worker_registry::{WorkerAdmission, WorkerRegistry};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostLaunchAccepted, HostLaunchState, HostTerminalId,
    HostTerminalIdentity, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchResult,
    WorkerGeneration,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTerminalGeometry,
};
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};
use std::thread;

#[path = "worker.rs"]
mod worker;
pub(super) use worker::Effect;

pub(super) struct Fixture {
    pub(super) service: Arc<LocalTerminalService>,
    targets: Vec<WorkbenchCommand>,
    pub(super) effects: Arc<Mutex<Vec<Effect>>>,
    pub(super) detach_refusals: Arc<std::sync::atomic::AtomicUsize>,
    pub(super) popup_fault: Arc<std::sync::atomic::AtomicU8>,
    stop: Mutex<UnixStream>,
    marker_sequence: std::sync::atomic::AtomicU64,
    owner: Mutex<Option<thread::JoinHandle<crate::worker_obcn_carrier::WorkerWireExit>>>,
    peer: Mutex<Option<thread::JoinHandle<()>>>,
}

pub(super) fn client_id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}
fn workbench_id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}
pub(super) fn read_snapshot(
    service: &LocalTerminalService,
) -> orchbee_workbench_protocol::Snapshot {
    let request = WorkbenchRequest::SnapshotRead(orchbee_workbench_protocol::SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(9000),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("snapshot result")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("snapshot available")
    };
    snapshot
}
fn placement_value() -> WorkerPlacement {
    WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    }
}
impl Fixture {
    pub(super) fn new() -> Self {
        Self::with_plugin_panes(false)
    }
    pub(super) fn with_plugin_panes(plugin: bool) -> Self {
        let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
        let worker_id = orchbee_local_site_protocol::WorkerId::from_bytes([9; 16]);
        let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id)
        else {
            panic!("worker admission")
        };
        let owner = WorkerRegistry::register_carrier(
            &registry,
            generation,
            create_worker_carrier(worker_id, generation),
        )
        .unwrap();
        let (server, peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let stop = server.try_clone().unwrap();
        let owner = thread::spawn(move || owner.run(server));
        let effects = Arc::new(Mutex::new(Vec::new()));
        let observation = Arc::clone(&effects);
        let detach_refusals = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let refusals = Arc::clone(&detach_refusals);
        let popup_fault = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let faults = Arc::clone(&popup_fault);
        let peer = thread::spawn(move || worker::run(peer, observation, refusals, faults));
        let service = Arc::new(LocalTerminalService::from_identity(
            registry,
            Ok(orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 1)).unwrap()),
        ));
        let mut fixture = Self {
            service,
            targets: Vec::new(),
            effects,
            detach_refusals,
            popup_fault,
            stop: Mutex::new(stop),
            marker_sequence: std::sync::atomic::AtomicU64::new(1),
            owner: Mutex::new(Some(owner)),
            peer: Mutex::new(Some(peer)),
        };
        for target in 0..10 {
            fixture
                .targets
                .push(add_terminal(&fixture.service, target, plugin));
        }
        fixture
    }
    pub(super) fn attach(&self, target: usize, binding: bool) -> Action {
        if !binding {
            return Action::Attach {
                terminal_create_request_id: client_id(11 + target as u128 * 100),
            };
        }
        let snapshot = read_snapshot(&self.service);
        let target_id = match &self.targets[target].action {
            WorkbenchAction::TabClose { tab_id, .. } => tab_id,
            _ => unreachable!(),
        };
        let tab = snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find(|tab| &tab.tab_id == target_id);
        // Removed targets retain a well-formed but unexposed binding selector.
        let terminal_id = tab
            .and_then(|tab| match &tab.root {
                orchbee_workbench_protocol::PanelNode::Panel {
                    primary_presentation,
                    ..
                } => primary_presentation.binding.as_ref(),
                _ => None,
            })
            .map(|binding| binding.terminal_id.as_str().to_owned())
            .unwrap_or_else(|| format!("{:032x}", 90000 + target));
        Action::AttachBinding {
            site_id: ClientSiteId::parse(snapshot.site_id.as_str()).unwrap(),
            worker_id: ClientWorkerId::parse(&format!("{:032x}", u128::from_be_bytes([9; 16])))
                .unwrap(),
            terminal_id: ClientTerminalId::parse(&terminal_id).unwrap(),
        }
    }
    pub(super) fn dispose(&self, target: usize) {
        let mut command = self.targets[target].clone();
        command.expected_revision = read_snapshot(&self.service).revision;
        let result = self
            .service
            .coordinator
            .lock()
            .unwrap()
            .execute_terminal_tab_close(
                command,
                Some(placement_value()),
                true,
                std::time::Instant::now(),
            )
            .unwrap();
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::TabCloseAccepted { .. }
        ));
        let mut cursor = None;
        for _ in 0..6 {
            self.service.close_maintenance_turn(&mut cursor).unwrap();
        }
    }
    pub(super) fn barrier(&self) -> Result<Vec<Effect>, String> {
        let handle = self
            .service
            .workers
            .lock()
            .unwrap()
            .request_handle()
            .ok_or("marker has no active Worker")?;
        let current = super::super::super::placement(Some(&handle)).unwrap();
        let sequence = self
            .marker_sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut marker = [250; 16];
        marker[8..].copy_from_slice(&sequence.to_be_bytes());
        let request = WorkerRequest {
            request_id: OperationId::from_bytes(marker).unwrap(),
            attachment_id: OperationId::from_bytes([249; 16]).unwrap(),
            worker_id: current.worker_id,
            worker_generation: current.generation,
            action: WorkerAction::Poll,
        };
        // The marker takes the blocking high-lane admission path so its response
        // is a strict cut for earlier high-lane and waiter predecessors — which
        // is what these fixtures enqueue (cleanup detaches, session-drop work).
        // Earlier low-lane requests may still be queued when it dispatches;
        // ordinary low-lane producers here complete synchronously beforehand.
        match handle.blocking_admit_for_test(WorkerOperationRequest::Viewer(request)) {
            Ok(WorkerOperationResult::Viewer(result)) if result.outcome == WorkerOutcome::Empty => {
                Ok(self.effects.lock().unwrap().clone())
            }
            other => Err(format!(
                "marker={sequence}, generation={}, result={other:?}, owner_finished={}, peer_finished={}, effects={:?}",
                current.generation.get(),
                self.owner
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_none_or(thread::JoinHandle::is_finished),
                self.peer
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_none_or(thread::JoinHandle::is_finished),
                self.effects.lock().unwrap()
            )),
        }
    }
    pub(super) fn replace_worker(&self) {
        self.stop
            .lock()
            .unwrap()
            .shutdown(std::net::Shutdown::Both)
            .unwrap();
        if let Some(owner) = self.owner.lock().unwrap().take() {
            let _ = owner.join();
        }
        if let Some(peer) = self.peer.lock().unwrap().take() {
            peer.join().unwrap();
        }
        let registry = &self.service.workers;
        let worker_id = orchbee_local_site_protocol::WorkerId::from_bytes([9; 16]);
        let WorkerAdmission::Accepted { generation } = registry.lock().unwrap().admit(worker_id)
        else {
            panic!("replacement admission")
        };
        assert_eq!(
            generation.get(),
            2,
            "replacement must advance the actual registry generation"
        );
        let owner = WorkerRegistry::register_carrier(
            registry,
            generation,
            create_worker_carrier(worker_id, generation),
        )
        .unwrap();
        let (server, peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        *self.stop.lock().unwrap() = server.try_clone().unwrap();
        *self.owner.lock().unwrap() = Some(thread::spawn(move || owner.run(server)));
        let effects = Arc::clone(&self.effects);
        let refusals = Arc::clone(&self.detach_refusals);
        let faults = Arc::clone(&self.popup_fault);
        *self.peer.lock().unwrap() = Some(thread::spawn(move || {
            worker::run(peer, effects, refusals, faults)
        }));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.service.stop_close_maintenance();
        let _ = self
            .stop
            .get_mut()
            .unwrap()
            .shutdown(std::net::Shutdown::Both);
        if let Some(owner) = self.owner.get_mut().unwrap().take() {
            let _ = owner.join();
        }
        if let Some(peer) = self.peer.get_mut().unwrap().take() {
            peer.join().unwrap();
        }
    }
}
fn add_terminal(service: &LocalTerminalService, index: u8, plugin: bool) -> WorkbenchCommand {
    let snapshot = read_snapshot(service);
    let create = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(10 + u128::from(index) * 100),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabCreate {
            automatic_label: false,
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
            label: WorkbenchLabel::parse("viewer routing").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                plugin_pane: plugin.then(|| {
                    orchbee_workbench_protocol::PluginPaneSelector::new(
                        "tools".into(),
                        "dashboard".into(),
                    )
                    .unwrap()
                }),
                ..Default::default()
            }),
        },
    };
    let mut site = service.coordinator.lock().unwrap();
    let PreparedTerminalTabCreate::Dispatch(pending) = site.prepare_terminal_tab_create(
        create,
        Some(client_id(11 + u128::from(index) * 100)),
        super::super::super::lease_expiry(),
        Some(placement_value()),
    ) else {
        panic!("create pending")
    };
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = pending.command() else {
        panic!("launch")
    };
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        panic!("geometry")
    };
    let identity = HostTerminalIdentity::new(
        HostCreationId::from_bytes([3 + index; 16]).unwrap(),
        orchbee_capability_node_protocol::HostEpoch::from_bytes([4; 16]).unwrap(),
        HostTerminalId::from_bytes([5 + index; 16]).unwrap(),
    );
    let result = TerminalLaunchResult::new(
        launch.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            identity,
            HostLaunchState::Started(geometry),
        )),
    )
    .unwrap();
    site.complete_terminal_tab_create(pending, Completion::Launch(result), Some(placement_value()))
        .unwrap();
    drop(site);
    let snapshot = read_snapshot(service);
    let tab_id = snapshot.terminal_contexts[0]
        .tabs
        .last()
        .unwrap()
        .tab_id
        .clone();
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(20 + u128::from(index) * 100),
        site_id: snapshot.site_id,
        expected_revision: snapshot.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: snapshot.navigation.standalone_terminal_context_id,
            tab_id,
        },
    }
}
