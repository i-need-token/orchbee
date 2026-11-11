use super::*;
use crate::{
    session_coordinator::PreparedTerminalTabCreate,
    terminal_coordinator::{Completion, WorkerPlacement},
    worker_obcn_carrier::{WorkerWireExit, create_worker_carrier},
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, HostCreationId, HostEpoch, HostLaunchAccepted, HostLaunchState,
    HostTerminalId, HostTerminalIdentity, TerminalLaunchMode, TerminalLaunchOutcome,
    TerminalLaunchResult, WorkerGeneration, read_message as read_capability_message,
    write_message as write_capability_message,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_local_worker_protocol::WorkerId as LocalWorkerId;
use orchbee_workbench_protocol::{
    PanelNode, PresentationLaunchProgram, WorkbenchAction, WorkbenchCommandOutcome,
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchTerminalGeometry,
    WorkbenchTerminalOutcome,
};
use std::{
    error::Error,
    num::NonZeroU64,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

fn command(service: &LocalTerminalService, request: u128, revision: u64) -> WorkbenchCommand {
    let snapshot = snapshot(service);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{request:032x}")).unwrap(),
        session_id: snapshot.session_id,
        expected_revision: WorkbenchRevision::new(revision),
        action: WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: snapshot.navigation.workspaces[0]
                .terminal_context_id
                .clone(),
            label: WorkbenchLabel::parse("service tab").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    }
}

fn snapshot(service: &LocalTerminalService) -> orchbee_workbench_protocol::Snapshot {
    let request = WorkbenchRequest::SnapshotRead(orchbee_workbench_protocol::SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 9_999)).unwrap(),
    });
    let response = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap();
    let WorkbenchResponse::Snapshot(result) = response else {
        panic!("Snapshot result expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available Snapshot expected")
    };
    snapshot
}

fn worker() -> WorkerPlacement {
    WorkerPlacement {
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    }
}

#[test]
fn service_replays_new_group_pending_and_final_before_missing_entropy_clock_or_worker() {
    let service =
        LocalTerminalService::workspace_fixture(Arc::new(Mutex::new(WorkerRegistry::new())));
    let mut command = command(&service, 1_001, 0);
    let WorkbenchAction::TabCreate { placement, .. } = &mut command.action else {
        unreachable!()
    };
    *placement = Some(orchbee_workbench_protocol::TabCreationPlacement::NewGroup {});
    let pending = {
        let mut coordinator = service.coordinator.lock().unwrap();
        let PreparedTerminalTabCreate::Dispatch(pending) = coordinator.prepare_terminal_tab_create(
            command.clone(),
            CreateInputs::new(
                Some(RequestId::parse(&format!("{:032x}", 1_002)).unwrap()),
                lease_expiry(),
                Some(worker()),
                LaunchCapture::absent(),
            ),
        ) else {
            panic!("dispatch expected")
        };
        pending
    };

    let replay = service
        .execute_terminal_tab_create_with_candidates(
            command.clone(),
            None,
            None,
            LaunchCapture::absent(),
        )
        .unwrap();
    assert!(matches!(
        replay.outcome,
        WorkbenchCommandOutcome::TabCreatePending { .. }
    ));

    let final_result = service
        .coordinator
        .lock()
        .unwrap()
        .complete_terminal_tab_create(pending, Completion::Unknown, Some(worker()))
        .unwrap()
        .result;
    assert_eq!(
        service
            .execute_terminal_tab_create_with_candidates(
                command,
                None,
                None,
                LaunchCapture::absent()
            )
            .unwrap(),
        final_result
    );
}

#[test]
fn registry_and_session_gate_failures_before_commit_return_server_busy() {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let service = LocalTerminalService::workspace_fixture(Arc::clone(&registry));
    let registry_command = command(&service, 1_051, 0);
    let poison = Arc::clone(&registry);
    assert!(
        thread::spawn(move || {
            let _guard = poison.lock().unwrap();
            panic!("poison Worker registry for tab.create boundary")
        })
        .join()
        .is_err()
    );
    assert!(matches!(
        service
            .execute_terminal_tab_create_with_candidates(
                registry_command,
                None,
                None,
                LaunchCapture::absent()
            )
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServerBusy,
        }
    ));

    let service =
        LocalTerminalService::workspace_fixture(Arc::new(Mutex::new(WorkerRegistry::new())));
    let session_command = command(&service, 1_052, 0);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = service.coordinator.lock().unwrap();
            panic!("poison Session gate for tab.create boundary")
        }))
        .is_err()
    );
    assert!(matches!(
        service
            .execute_terminal_tab_create_with_candidates(
                session_command,
                None,
                None,
                LaunchCapture::absent()
            )
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServerBusy,
        }
    ));
}

#[test]
fn session_gate_loss_after_commit_returns_only_the_close_outcome_unknown_signal() -> TestResult {
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = LocalWorkerId::from_bytes([9; 16]);
    let generation = {
        let mut registry = registry
            .lock()
            .map_err(|_poison| "Worker registry poisoned before admission")?;
        let WorkerAdmission::Accepted { generation } = registry.admit(worker_id, None) else {
            return Err("test Worker was not admitted".into());
        };
        generation
    };
    let carrier = create_worker_carrier(worker_id, generation);
    let owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .ok_or("test Worker carrier was not registered")?;
    let (server, mut worker_peer) = UnixStream::pair()?;
    worker_peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let owner_thread = thread::spawn(move || owner.run(server));

    let service = Arc::new(LocalTerminalService::workspace_fixture(registry));
    let command = command(&service, 1_075, 0);
    let creator_service = Arc::clone(&service);
    let (creator_sender, creator_result) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ignored = creator_sender.send(creator_service.execute_terminal_tab_create(command));
    });
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(_launch) =
        read_capability_message(&mut worker_peer)?
    else {
        return Err("tab.create did not dispatch a terminal Launch".into());
    };

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = service.coordinator.lock().unwrap();
            panic!("poison Session gate after joint commit")
        }))
        .is_err()
    );
    drop(worker_peer);
    assert_eq!(
        creator_result
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_timeout| "creator did not observe the postcommit Session failure")?,
        Err(PostcommitCoordinationError)
    );
    assert_eq!(
        owner_thread
            .join()
            .map_err(|_panic| "Worker owner panicked")?,
        WorkerWireExit::RequestFailed
    );
    Ok(())
}

#[test]
fn silent_worker_timeout_settles_both_owners() -> TestResult {
    const OUTER_DEADLINE: Duration = Duration::from_secs(35);

    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = LocalWorkerId::from_bytes([9; 16]);
    let generation = {
        let mut registry = registry
            .lock()
            .map_err(|_poison| "Worker registry poisoned before admission")?;
        let WorkerAdmission::Accepted { generation } = registry.admit(worker_id, None) else {
            return Err("test Worker was not admitted".into());
        };
        generation
    };
    let carrier = create_worker_carrier(worker_id, generation);
    let owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .ok_or("test Worker carrier was not registered")?;
    let (server, mut worker_peer) = UnixStream::pair()?;
    worker_peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let owner_thread = thread::spawn(move || owner.run(server));

    let service = LocalTerminalService::workspace_fixture(Arc::clone(&registry));
    let service = Arc::new(service);
    let command = command(&service, 1_101, 0);
    let terminal_context_id = match &command.action {
        WorkbenchAction::TabCreate {
            terminal_context_id,
            ..
        } => terminal_context_id.clone(),
        _ => unreachable!("test constructs tab.create"),
    };
    let creator_service = Arc::clone(&service);
    let (creator_sender, creator_result) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let _ignored = creator_sender.send(creator_service.execute_terminal_tab_create(command));
    });

    let CapabilityNodeMessage::AuthorizedTerminalLaunch(_launch) =
        read_capability_message(&mut worker_peer)?
    else {
        return Err("tab.create did not dispatch a terminal Launch".into());
    };

    let result = creator_result
        .recv_timeout(OUTER_DEADLINE)
        .map_err(|_timeout| "tab.create did not settle after the production Worker deadline")??;
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::TabCreated {
            revision: WorkbenchRevision::new(1),
            terminal_context_id,
            tab_id: orchbee_workbench_protocol::WorkbenchTabId::parse(&format!("{:032x}", 5))?,
            panel_id: orchbee_workbench_protocol::WorkbenchPanelId::parse(&format!("{:032x}", 6))?,
            presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(&format!(
                "{:032x}",
                7
            ))?,
            terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
        }
    );
    assert_eq!(
        owner_thread
            .join()
            .map_err(|_panic| "Worker owner panicked")?,
        WorkerWireExit::RequestFailed
    );
    drop(worker_peer);
    Ok(())
}

/// A private configuration directory, removed when the test ends.
struct ConfigurationRoot(std::path::PathBuf);

impl ConfigurationRoot {
    fn new() -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "orchbee-tab-create-shell-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self(root))
    }
}

impl Drop for ConfigurationRoot {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_tab_created_on_a_reporting_worker_publishes_its_system_shell() -> TestResult {
    let configuration = ConfigurationRoot::new()?;
    let registry = Arc::new(Mutex::new(WorkerRegistry::new()));
    let worker_id = LocalWorkerId::from_bytes([9; 16]);
    let generation = {
        let mut registry = registry
            .lock()
            .map_err(|_poison| "Worker registry poisoned before admission")?;
        let reported = PresentationLaunchProgram::parse("pwsh.exe")?;
        let WorkerAdmission::Accepted { generation } = registry.admit(worker_id, Some(reported))
        else {
            return Err("test Worker was not admitted".into());
        };
        generation
    };
    let carrier = create_worker_carrier(worker_id, generation);
    let owner = WorkerRegistry::register_carrier(&registry, generation, carrier)
        .ok_or("test Worker carrier was not registered")?;
    let (server, mut worker_peer) = UnixStream::pair()?;
    worker_peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    let owner_thread = thread::spawn(move || owner.run(server));

    let service = LocalTerminalService::workspace_fixture(Arc::clone(&registry));
    // A missing settings file selects the System shell default profile.
    service.install_configuration(&orchbee_local_runtime::read_user_settings_at(
        Some(&configuration.0.join("config.toml")),
        false,
        |_| None,
    )?)?;
    let service = Arc::new(service);
    let command = command(&service, 1_201, 0);
    let creator_service = Arc::clone(&service);
    let creator = thread::spawn(move || creator_service.execute_terminal_tab_create(command));

    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) =
        read_capability_message(&mut worker_peer)?
    else {
        return Err("tab.create did not dispatch a terminal Launch".into());
    };
    assert!(
        launch.launch().is_none(),
        "a default launch carries no selection"
    );
    let TerminalLaunchMode::Immediate(geometry) = launch.mode() else {
        return Err("tab.create did not start immediately".into());
    };
    let started = TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
        HostTerminalIdentity::new(
            HostCreationId::from_bytes([1; 16])?,
            HostEpoch::from_bytes([2; 16])?,
            HostTerminalId::from_bytes([3; 16])?,
        ),
        HostLaunchState::Started(geometry),
    ));
    write_capability_message(
        &mut worker_peer,
        &CapabilityNodeMessage::TerminalLaunchResult(TerminalLaunchResult::new(
            launch.authorization(),
            started,
        )?),
    )?;
    let result = creator
        .join()
        .map_err(|_panic| "tab.create caller panicked")??;
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::TabCreated {
            terminal_outcome: WorkbenchTerminalOutcome::Started { .. },
            ..
        }
    ));

    let snapshot = snapshot(&service);
    let tab = snapshot.terminal_contexts[0]
        .tabs
        .last()
        .ok_or("the created Tab is missing")?;
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &tab.root
    else {
        return Err("a created Tab holds one Panel".into());
    };
    assert_eq!(
        primary_presentation
            .launch_program
            .as_ref()
            .map(PresentationLaunchProgram::as_str),
        Some("pwsh.exe")
    );

    drop(worker_peer);
    let _exit = owner_thread
        .join()
        .map_err(|_panic| "Worker owner panicked")?;
    Ok(())
}
