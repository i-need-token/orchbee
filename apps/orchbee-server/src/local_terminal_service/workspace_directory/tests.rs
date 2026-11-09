use super::*;
use crate::{
    worker_obcn_carrier::{WorkerWireExit, create_worker_carrier},
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, TerminalCwdValidationResult, encode_message, read_message,
};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_workbench_protocol::{
    Snapshot, SnapshotReadIntent, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome,
};
use std::{
    io::Write,
    net::Shutdown,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

fn snapshot(service: &LocalTerminalService) -> Snapshot {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{:032x}", 999)).unwrap(),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("snapshot response expected")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("available snapshot expected")
    };
    snapshot
}

fn command(service: &LocalTerminalService, id: u128, action: WorkbenchAction) -> WorkbenchCommand {
    let current = snapshot(service);
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
        session_id: current.session_id,
        expected_revision: current.revision,
        action,
    }
}

fn navigation(service: &LocalTerminalService, command: WorkbenchCommand) -> WorkbenchCommandResult {
    let WorkbenchResponse::Command(result) = service
        .execute_workbench(WorkbenchRequest::Command(command).try_into().unwrap(), true)
        .unwrap()
    else {
        panic!("command result expected")
    };
    result
}

fn association(service: &LocalTerminalService) -> WorkbenchCommand {
    let created = navigation(
        service,
        command(
            service,
            1,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("Project").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = created.outcome else {
        panic!("Project expected")
    };
    let created = navigation(
        service,
        command(
            service,
            2,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse("Workspace").unwrap(),
            },
        ),
    );
    let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = created.outcome else {
        panic!("Workspace expected")
    };
    command(
        service,
        3,
        WorkbenchAction::WorkspacePrimaryDirectoryAccept {
            workspace_id,
            path: WorkbenchDirectoryPath::parse("/tmp/workspace/link/../directory").unwrap(),
        },
    )
}

fn service() -> LocalTerminalService {
    LocalTerminalService::workspace_fixture(Arc::new(Mutex::new(WorkerRegistry::new())))
}

struct WorkerPeer {
    peer: UnixStream,
    owner: Option<JoinHandle<WorkerWireExit>>,
}

impl WorkerPeer {
    fn new(service: &LocalTerminalService) -> Self {
        let worker_id = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
        let WorkerAdmission::Accepted { generation } =
            service.workers.lock().unwrap().admit(worker_id, None)
        else {
            panic!("Worker admission expected")
        };
        let carrier = create_worker_carrier(worker_id, generation);
        let owner =
            WorkerRegistry::register_carrier(&service.workers, generation, carrier).unwrap();
        let (server, peer) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        Self {
            peer,
            owner: Some(thread::spawn(move || owner.run(server))),
        }
    }

    fn read(&mut self) -> TerminalCwdValidationRequest {
        let CapabilityNodeMessage::TerminalCwdValidationRequest(request) =
            read_message(&mut self.peer).unwrap()
        else {
            panic!("read-only directory validation expected")
        };
        request
    }

    fn reply(
        &mut self,
        request: TerminalCwdValidationRequest,
        outcome: TerminalCwdValidationOutcome,
    ) {
        self.peer
            .write_all(
                &encode_message(&CapabilityNodeMessage::TerminalCwdValidationResult(
                    TerminalCwdValidationResult { request, outcome },
                ))
                .unwrap(),
            )
            .unwrap();
    }
}

impl Drop for WorkerPeer {
    fn drop(&mut self) {
        let _closed = self.peer.shutdown(Shutdown::Both);
        if let Some(owner) = self.owner.take() {
            let _joined = owner.join();
        }
    }
}

#[test]
fn missing_worker_is_inert_and_cannot_hide_an_exact_retained_acceptance() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    assert_eq!(
        service.execute_workspace_directory(command.clone()),
        unavailable(
            &command,
            WorkbenchCommandUnavailableReason::NoCompatibleWorker
        )
    );
    assert_eq!(snapshot(&service), before);
    let worker = WorkerId::parse(&format!("{:032x}", 9)).unwrap();
    let accepted = service
        .coordinator
        .lock()
        .unwrap()
        .accept_workspace_directory(command.clone(), worker);
    assert!(matches!(
        accepted.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    let settled = snapshot(&service);
    assert_eq!(service.execute_workspace_directory(command), accepted);
    assert_eq!(snapshot(&service), settled);
}

#[test]
fn unavailable_authority_returns_service_unavailable_before_worker_selection() {
    let available = service();
    let command = association(&available);
    let unavailable_service = LocalTerminalService::from_identity(
        Arc::new(Mutex::new(WorkerRegistry::new())),
        Err(std::io::Error::other("test Session entropy unavailable")),
    );
    assert_eq!(
        unavailable_service.execute_workspace_directory(command.clone()),
        unavailable(
            &command,
            WorkbenchCommandUnavailableReason::ServiceUnavailable
        )
    );
}

#[test]
fn exact_worker_validation_commits_path_once_without_touching_initial_workspace() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    let mut peer = WorkerPeer::new(&service);
    let accepted = thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
        let request = peer.read();
        assert_eq!(request.cwd.as_str(), "/tmp/workspace/link/../directory");
        assert_eq!(request.worker_id.as_bytes(), [9; 16]);
        assert_eq!(request.worker_generation.get(), 1);
        // Both gates remain available throughout the external observation.
        assert!(service.workers.try_lock().is_ok());
        assert!(service.coordinator.try_lock().is_ok());
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryAvailable);
        pending.join().unwrap()
    });
    let WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
        revision,
        worker_id,
        directory_id,
    } = &accepted.outcome
    else {
        panic!("directory acceptance expected: {accepted:?}")
    };
    assert_eq!(*revision, WorkbenchRevision::new(3));
    assert_eq!(worker_id.as_str(), "09090909090909090909090909090909");
    let settled = snapshot(&service);
    assert_eq!(settled.terminal_contexts, before.terminal_contexts);
    assert_eq!(
        settled.navigation.workspaces[0].terminal_context_id.clone(),
        before.navigation.workspaces[0].terminal_context_id.clone()
    );
    let navigation = &settled.navigation;
    let WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, .. } = &command.action
    else {
        panic!("directory association expected")
    };
    assert_eq!(
        navigation.directories.len(),
        before.navigation.directories.len() + 1
    );
    assert!(
        navigation
            .directories
            .iter()
            .any(|directory| &directory.directory_id == directory_id)
    );
    assert_eq!(
        navigation
            .workspace(workspace_id)
            .unwrap()
            .primary_directory_id
            .as_ref(),
        Some(directory_id)
    );
    assert_eq!(service.execute_workspace_directory(command), accepted);
    assert_eq!(snapshot(&service), settled);
}

#[test]
fn negative_observations_preserve_snapshot_and_same_command_can_retry_with_fresh_operation() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    let mut peer = WorkerPeer::new(&service);
    let mut previous = None;
    for (outcome, reason) in [
        (
            TerminalCwdValidationOutcome::DirectoryMissing,
            WorkbenchCommandUnavailableReason::DirectoryMissing,
        ),
        (
            TerminalCwdValidationOutcome::NotADirectory,
            WorkbenchCommandUnavailableReason::NotADirectory,
        ),
        (
            TerminalCwdValidationOutcome::DirectoryInaccessible,
            WorkbenchCommandUnavailableReason::DirectoryInaccessible,
        ),
        (
            TerminalCwdValidationOutcome::WorkerUnavailable,
            WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        ),
    ] {
        let result = thread::scope(|scope| {
            let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
            let request = peer.read();
            assert_ne!(previous, Some(request.operation_id));
            previous = Some(request.operation_id);
            peer.reply(request, outcome);
            pending.join().unwrap()
        });
        assert_eq!(result, unavailable(&command, reason));
        assert_eq!(snapshot(&service), before);
    }
    let accepted = thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
        let request = peer.read();
        assert_ne!(previous, Some(request.operation_id));
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryAvailable);
        pending.join().unwrap()
    });
    assert!(
        matches!(accepted.outcome, WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { revision, .. } if revision.value() == 3)
    );
}

#[test]
fn revision_change_during_validation_rejects_without_rebase_or_association() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    let mut peer = WorkerPeer::new(&service);
    let result = thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
        let request = peer.read();
        navigation(
            &service,
            self::command(
                &service,
                4,
                WorkbenchAction::ProjectCreate {
                    label: WorkbenchLabel::parse("Concurrent").unwrap(),
                },
            ),
        );
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryAvailable);
        pending.join().unwrap()
    });
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    let settled = snapshot(&service);
    assert_eq!(settled.revision.value(), 3);
    assert_eq!(
        settled.navigation.directories,
        before.navigation.directories
    );
    let WorkbenchAction::WorkspacePrimaryDirectoryAccept { workspace_id, .. } = &command.action
    else {
        panic!("directory association expected")
    };
    assert!(
        settled
            .navigation
            .workspace(workspace_id)
            .unwrap()
            .primary_directory_id
            .is_none()
    );
}

#[test]
fn concurrent_exact_commit_wins_over_later_negative_validation() {
    let service = service();
    let command = association(&service);
    let mut peer = WorkerPeer::new(&service);
    thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
        let request = peer.read();
        let accepted = service
            .coordinator
            .lock()
            .unwrap()
            .accept_workspace_directory(
                command.clone(),
                WorkerId::parse("09090909090909090909090909090909").unwrap(),
            );
        let settled = snapshot(&service);
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryMissing);
        assert_eq!(pending.join().unwrap(), accepted);
        assert_eq!(snapshot(&service), settled);
    });
}

#[test]
fn mismatched_observation_fails_closed_and_preserves_canonical_state() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    let mut peer = WorkerPeer::new(&service);
    let result = thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_directory(command.clone()));
        let mut request = peer.read();
        request.cwd = AbsoluteCwd::new("/wrong-directory").unwrap();
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryAvailable);
        pending.join().unwrap()
    });
    assert_eq!(
        result,
        unavailable(
            &command,
            WorkbenchCommandUnavailableReason::NoCompatibleWorker
        )
    );
    assert_eq!(snapshot(&service), before);
}

#[test]
fn real_success_from_retired_generation_cannot_configure_replacement_worker() {
    let service = service();
    let command = association(&service);
    let before = snapshot(&service);
    let mut old_peer = WorkerPeer::new(&service);
    let handle = service.workers.lock().unwrap().request_handle().unwrap();
    let worker = placement(Some(&handle)).unwrap();
    let request = TerminalCwdValidationRequest {
        operation_id: OperationId::from_bytes([7; 16]).unwrap(),
        worker_id: worker.worker_id,
        worker_generation: worker.generation,
        cwd: AbsoluteCwd::new("/tmp/workspace/link/../directory").unwrap(),
    };
    let observed = thread::scope(|scope| {
        let pending = scope.spawn(|| handle.validate_terminal_cwd(request.clone()));
        assert_eq!(old_peer.read(), request);
        old_peer.reply(
            request.clone(),
            TerminalCwdValidationOutcome::DirectoryAvailable,
        );
        pending.join().unwrap()
    });
    assert!(
        matches!(&observed, Ok(result) if result.outcome == TerminalCwdValidationOutcome::DirectoryAvailable)
    );
    drop(old_peer);
    let _replacement_peer = WorkerPeer::new(&service);
    let replacement = service.workers.lock().unwrap().request_handle().unwrap();
    assert_eq!(replacement.worker_id(), handle.worker_id());
    assert_eq!(replacement.generation().get(), 2);
    assert_eq!(
        service.complete_workspace_directory(command.clone(), request, observed),
        unavailable(
            &command,
            WorkbenchCommandUnavailableReason::NoCompatibleWorker
        )
    );
    assert_eq!(snapshot(&service), before);
}

fn launch_command(service: &LocalTerminalService) -> WorkbenchCommand {
    let association = association(service);
    let WorkbenchAction::WorkspacePrimaryDirectoryAccept { path, .. } = association.action else {
        unreachable!()
    };
    command(
        service,
        500,
        WorkbenchAction::WorkspaceLaunch {
            automatic_label: false,
            placement: orchbee_workbench_protocol::WorkspacePlacement::Project {
                project_id: snapshot(service).navigation.projects[0].project_id.clone(),
            },
            label: WorkbenchLabel::parse("launch").unwrap(),
            geometry: orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 0, 0)
                .unwrap(),
            launch: None,
            profile: None,
            options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                plugin_pane: None,
                plugin_context: None,
                cwd: Some(path),
                environment: Default::default(),
            }),
        },
    )
}

#[test]
fn workspace_launch_directory_failure_or_stale_probe_never_publishes_partial_resources() {
    for stale in [false, true] {
        let service = service();
        let command = launch_command(&service);
        let mut expected = snapshot(&service);
        let mut peer = WorkerPeer::new(&service);
        let result = thread::scope(|scope| {
            let pending = scope.spawn(|| service.execute_workspace_launch(command.clone()));
            let request = peer.read();
            assert!(service.coordinator.try_lock().is_ok());
            assert!(service.workers.try_lock().is_ok());
            if stale {
                navigation(
                    &service,
                    self::command(
                        &service,
                        501,
                        WorkbenchAction::ProjectCreate {
                            label: WorkbenchLabel::parse("concurrent").unwrap(),
                        },
                    ),
                );
                expected = snapshot(&service);
            }
            peer.reply(
                request,
                if stale {
                    TerminalCwdValidationOutcome::DirectoryAvailable
                } else {
                    TerminalCwdValidationOutcome::DirectoryMissing
                },
            );
            pending.join().unwrap().unwrap()
        });
        if stale {
            assert!(matches!(
                result.outcome,
                WorkbenchCommandOutcome::Rejected {
                    reason: orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch,
                    ..
                }
            ));
        } else {
            assert_eq!(
                result,
                unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::DirectoryMissing
                )
            );
        }
        assert_eq!(snapshot(&service), expected);
    }
}

#[test]
fn workspace_launch_replays_committed_pending_without_probe_and_keeps_resources_after_worker_loss()
{
    let service = service();
    let command = launch_command(&service);
    let before = snapshot(&service);
    let mut peer = WorkerPeer::new(&service);
    let result = thread::scope(|scope| {
        let pending = scope.spawn(|| service.execute_workspace_launch(command.clone()));
        let request = peer.read();
        peer.reply(request, TerminalCwdValidationOutcome::DirectoryAvailable);
        assert!(matches!(
            read_message(&mut peer.peer).unwrap(),
            CapabilityNodeMessage::AuthorizedTerminalLaunch(_)
        ));
        let replay = service.execute_workspace_launch(command.clone()).unwrap();
        assert!(matches!(
            replay.outcome,
            WorkbenchCommandOutcome::WorkspaceLaunchPending { .. }
        ));
        let committed = snapshot(&service);
        assert_eq!(committed.revision.value(), before.revision.value() + 1);
        assert_eq!(
            committed.navigation.workspaces.len(),
            before.navigation.workspaces.len() + 1
        );
        assert_eq!(committed.terminal_contexts.last().unwrap().tabs.len(), 1);
        drop(peer);
        pending.join().unwrap().unwrap()
    });
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::WorkspaceLaunched {
            terminal_outcome:
                orchbee_workbench_protocol::WorkbenchTerminalOutcome::OutcomeUnknown {},
            ..
        }
    ));
    assert_eq!(service.execute_workspace_launch(command).unwrap(), result);
    assert_eq!(
        snapshot(&service).revision.value(),
        before.revision.value() + 1
    );
}
