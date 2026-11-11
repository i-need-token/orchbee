mod organization;
mod restoration;
use super::LocalTerminalService;
use crate::{
    worker_obcn_carrier::{WorkerWireExit, create_worker_carrier},
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    AbsoluteCwd, CapabilityNodeMessage, GitReference, WorktreeCheckout, WorktreeMutation,
    WorktreeMutationCompletion as Completion, WorktreeMutationEffect, WorktreeMutationError,
    WorktreeMutationFailure, WorktreeMutationJobAction, WorktreeMutationOutcome,
    WorktreeMutationPollOutcome, WorktreeMutationRequest, WorktreeMutationResult,
    WorktreeMutationStage, WorktreeMutationStartOutcome, read_message, write_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProjectId,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchSnapshotOutcome,
    WorkbenchTerminalGeometry, WorkbenchWorkspaceId, WorkbenchWorktreeAction as Action,
    WorkbenchWorktreeOutcome as Outcome, WorkbenchWorktreeRequest as Request, WorkerId,
    WorktreeGitError, WorktreeIntent, WorktreeOperationFailure as Failure,
    WorktreeOperationStatus as Status, WorktreeSelector, WorktreeSource,
    WorktreeWorkspaceDisposition,
};
use std::{
    net::Shutdown,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}
struct Fixture {
    service: Arc<LocalTerminalService>,
    peer: LocalStream,
    wire: Option<JoinHandle<WorkerWireExit>>,
    project: WorkbenchProjectId,
    workspace: WorkbenchWorkspaceId,
}
impl Fixture {
    fn new() -> Self {
        Self::with_close_owner(true)
    }
    fn with_close_owner(start_owner: bool) -> Self {
        // The scenario's Project Workspace is the Session's only Workspace.
        let service = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
            WorkerRegistry::new(),
        ))));
        let worker = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
        let WorkerAdmission::Accepted { generation } =
            service.workers.lock().unwrap().admit(worker, None)
        else {
            panic!("Worker admission")
        };
        let owner = WorkerRegistry::register_carrier(
            &service.workers,
            generation,
            create_worker_carrier(worker, generation),
        )
        .unwrap();
        let (server, peer) = LocalStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let wire = Some(thread::spawn(move || owner.run(server)));
        let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = navigation(
            &service,
            1,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("Project").unwrap(),
            },
        ) else {
            panic!("project")
        };
        let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = navigation(
            &service,
            2,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse("Original").unwrap(),
            },
        ) else {
            panic!("workspace")
        };
        let snapshot = snapshot(&service);
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(3),
            session_id: snapshot.session_id,
            expected_revision: snapshot.revision,
            action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id: workspace_id.clone(),
                path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
            },
        };
        let result = service
            .coordinator
            .lock()
            .unwrap()
            .accept_workspace_directory(command, WorkerId::parse(&"09".repeat(16)).unwrap());
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
        ));
        if start_owner {
            service.start_close_maintenance().unwrap();
        }
        Self {
            service,
            peer,
            wire,
            project: project_id,
            workspace: workspace_id,
        }
    }
    fn apply(&self, action: Action) -> Outcome {
        self.service
            .execute_worktree_operation(Request {
                protocol_version: ProtocolVersion,
                request_id: id(1000),
                session_id: snapshot(&self.service).session_id,
                action,
            })
            .outcome
    }
    fn start(&self, intent: WorktreeIntent) -> Outcome {
        self.apply(Action::Start {
            operation_id: id(40),
            project_id: match &intent {
                WorktreeIntent::Create {
                    source: WorktreeSource::Directory { .. },
                    ..
                }
                | WorktreeIntent::Open {
                    source: WorktreeSource::Directory { .. },
                    ..
                } => Some(self.project.clone()),
                _ => None,
            },
            intent,
        })
    }
    fn read(&self) -> Outcome {
        self.apply(Action::Read {
            operation_id: id(40),
        })
    }
    fn receive(&mut self) -> WorktreeMutationRequest {
        let CapabilityNodeMessage::WorktreeMutationRequest(request) =
            read_message(&mut self.peer).unwrap()
        else {
            panic!("only Git requests allowed")
        };
        request
    }
    fn reply(&mut self, request: &WorktreeMutationRequest, outcome: WorktreeMutationOutcome) {
        let job_id = match &request.action {
            WorktreeMutationJobAction::Start(_) => request.operation_id,
            WorktreeMutationJobAction::Poll { job_id }
            | WorktreeMutationJobAction::Cancel { job_id } => *job_id,
        };
        write_message(
            &mut self.peer,
            &CapabilityNodeMessage::WorktreeMutationResult(WorktreeMutationResult {
                operation_id: request.operation_id,
                job_id,
                outcome,
            }),
        )
        .unwrap();
    }
    fn finish_job(&mut self, completion: Completion) -> WorktreeMutationRequest {
        let started = self.receive();
        assert!(matches!(
            started.action,
            WorktreeMutationJobAction::Start(_)
        ));
        self.reply(
            &started,
            WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
        );
        let poll = self.receive();
        assert!(
            matches!(poll.action, WorktreeMutationJobAction::Poll { job_id } if job_id == started.operation_id)
        );
        self.reply(
            &poll,
            WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(completion)),
        );
        started
    }
    fn settled(&self) -> Status {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let Outcome::Available { status, .. } = self.read() else {
                panic!("retained status")
            };
            if !matches!(*status, Status::Pending { .. }) {
                return *status;
            }
            assert!(Instant::now() < deadline, "operation did not settle");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _groups = self
            .service
            .stop_workspace_groups_before(Instant::now() + Duration::from_secs(10));
        let _stopped = self
            .service
            .stop_worktrees_before(Instant::now() + Duration::from_secs(10));
        let _closed = self
            .service
            .stop_close_before(Instant::now() + Duration::from_secs(10));
        let _disconnected = self.peer.shutdown(Shutdown::Both);
        if let Some(wire) = self.wire.take() {
            let _joined = wire.join();
        }
    }
}
fn snapshot(service: &LocalTerminalService) -> Snapshot {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: id(999),
    });
    let WorkbenchResponse::Snapshot(result) = service
        .execute_workbench(request.try_into().unwrap(), false)
        .unwrap()
    else {
        panic!("snapshot")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = result.outcome else {
        panic!("snapshot available")
    };
    snapshot
}
fn navigation(
    service: &LocalTerminalService,
    value: u128,
    action: WorkbenchAction,
) -> WorkbenchCommandOutcome {
    let snapshot = snapshot(service);
    let request = WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: id(value),
        session_id: snapshot.session_id,
        expected_revision: snapshot.revision,
        action,
    });
    let WorkbenchResponse::Command(result) = service
        .execute_workbench(request.try_into().unwrap(), true)
        .unwrap()
    else {
        panic!("command")
    };
    result.outcome
}
fn checkout(dirty: bool) -> Completion {
    Completion::Checkout(
        WorktreeCheckout::new(
            AbsoluteCwd::new("C:/repo").unwrap(),
            AbsoluteCwd::new("C:/repo/.git").unwrap(),
            AbsoluteCwd::new("C:/w").unwrap(),
            Some(GitReference::new("topic").unwrap()),
            dirty,
        )
        .unwrap(),
    )
}

#[test]
fn absent_checkout_ends_partial_create_recovery_without_replaying_add() {
    let mut fixture = Fixture::new();
    fixture.start(WorktreeIntent::Create {
        source: WorktreeSource::Workspace {
            workspace_id: fixture.workspace.clone(),
        },
        branch: Some(orchbee_workbench_protocol::WorktreeReference::parse("topic").unwrap()),
        base: None,
        path: Some(WorkbenchDirectoryPath::parse("C:/new-checkout").unwrap()),
        label: None,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
        agent_start: None,
    });
    fixture.finish_job(checkout(false));
    let create = fixture.finish_job(Completion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::GitFailed(Some(128)),
            b"fatal: checkout failed".to_vec(),
        )
        .unwrap(),
    ));
    assert!(
        matches!(create.action, WorktreeMutationJobAction::Start(intent)
        if matches!(intent.mutation(), WorktreeMutation::Create { .. }))
    );
    assert!(matches!(
        fixture.settled(),
        Status::Failed {
            resumable: true,
            ..
        }
    ));
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt: 1
        }),
        Outcome::Available { attempt: 2, .. }
    ));
    let observe = fixture.finish_job(Completion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Select,
            WorktreeMutationEffect::None,
            WorktreeMutationError::CheckoutNotFound,
            vec![],
        )
        .unwrap(),
    ));
    assert!(
        matches!(observe.action, WorktreeMutationJobAction::Start(intent)
        if matches!(intent.mutation(), WorktreeMutation::Open(_)))
    );
    assert!(matches!(
        fixture.settled(),
        Status::Failed {
            resumable: false,
            reason: Failure::Git {
                error: WorktreeGitError::CheckoutNotFound,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt: 2
        }),
        Outcome::Available { attempt: 2, .. }
    ));
}
#[test]
fn detached_open_reuses_and_renames_workspace_without_terminal_launch_or_locks_across_ipc() {
    let mut fixture = Fixture::new();
    let intent = WorktreeIntent::Open {
        source: WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        },
        selector: WorktreeSelector::Path {
            path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
        },
        label: Some(WorkbenchLabel::parse("Review").unwrap()),
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
    };
    assert!(matches!(
        fixture.start(intent.clone()),
        Outcome::Available { .. }
    ));
    let start = fixture.receive(); // Start has returned while the Worker is still blocked here.
    assert!(fixture.service.workers.try_lock().is_ok());
    assert!(fixture.service.coordinator.try_lock().is_ok());
    assert!(matches!(fixture.start(intent), Outcome::Available { .. }));
    assert!(
        matches!(fixture.read(), Outcome::Available { status, .. } if matches!(*status, Status::Pending { .. }))
    );
    fixture.reply(
        &start,
        WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
    );
    let poll = fixture.receive();
    fixture.reply(
        &poll,
        WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(checkout(false))),
    );
    let opened = fixture.finish_job(checkout(false));
    assert!(
        matches!(opened.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::Open(_)))
    );
    assert!(
        matches!(fixture.settled(), Status::Opened { workspace_id, reused: true, terminal_outcome: None, .. } if workspace_id == fixture.workspace)
    );
    let snapshot = snapshot(&fixture.service);
    assert_eq!(snapshot.navigation.workspaces.len(), 1);
    assert_eq!(snapshot.navigation.workspaces[0].label.as_str(), "Review");
}
#[test]
fn dirty_remove_is_rejected_before_workspace_close() {
    let mut fixture = Fixture::new();
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: false,
        trust_repository: false,
    });
    let start = fixture.finish_job(checkout(true));
    assert!(
        matches!(start.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::InspectRemove { .. }))
    );
    assert!(
        matches!(fixture.settled(), Status::Failed { reason: Failure::Git { error: WorktreeGitError::Dirty, .. }, progress, .. }
        if matches!(progress.workspace, WorktreeWorkspaceDisposition::Open { .. }))
    );
    assert_eq!(snapshot(&fixture.service).navigation.workspaces.len(), 1);
}
#[test]
fn failed_git_remove_keeps_workspace_structure_and_resume_uses_repository_root() {
    let mut fixture = Fixture::new();
    let before = snapshot(&fixture.service);
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: true,
        trust_repository: false,
    });
    fixture.finish_job(checkout(true));
    let failed = Completion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::GitFailed(Some(128)),
            b"locked".to_vec(),
        )
        .unwrap(),
    );
    let remove = fixture.finish_job(failed);
    assert!(
        matches!(remove.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::Remove { force: true, .. }) && intent.source().as_str() == "C:/repo")
    );
    fixture.finish_job(checkout(true));
    assert!(
        matches!(fixture.settled(), Status::Failed { progress, resumable: true, .. } if matches!(progress.workspace, WorktreeWorkspaceDisposition::Open { .. }) && progress.restoration.as_ref().is_some_and(|receipt| receipt.total == 0 && receipt.failure.is_none()))
    );
    assert_eq!(snapshot(&fixture.service), before);
    assert!(matches!(
        fixture.apply(Action::Resume {
            operation_id: id(40),
            expected_attempt: 1
        }),
        Outcome::Available { attempt: 2, .. }
    ));
    let inspect = fixture.finish_job(checkout(false));
    assert!(
        matches!(inspect.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::InspectRemove { .. }) && intent.source().as_str() == "C:/repo")
    );
    fixture.finish_job(Completion::Removed {
        path: AbsoluteCwd::new("C:/w").unwrap(),
    });
    let settled = fixture.settled();
    assert!(
        matches!(&settled, Status::Removed { workspace_id, .. } if *workspace_id == fixture.workspace),
        "{settled:?}"
    );
}

#[test]
fn failed_remove_can_close_explicitly_without_repeating_git_and_pending_close_is_busy() {
    let mut fixture = Fixture::new();
    let before = snapshot(&fixture.service);
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: true,
        trust_repository: false,
    });
    fixture.finish_job(checkout(false));
    let remove = fixture.receive();
    assert!(matches!(remove.action, WorktreeMutationJobAction::Start(_)));
    let close = |request| {
        let state = snapshot(&fixture.service);
        fixture
            .service
            .execute_workspace_close(WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: id(request),
                session_id: state.session_id,
                expected_revision: state.revision,
                action: WorkbenchAction::WorkspaceClose {
                    workspace_id: fixture.workspace.clone(),
                },
            })
            .unwrap()
    };
    assert!(matches!(
        close(900).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: orchbee_workbench_protocol::WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    fixture.reply(
        &remove,
        WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
    );
    let poll = fixture.receive();
    fixture.reply(
        &poll,
        WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Completed(Completion::Failed(
            WorktreeMutationFailure::new(
                WorktreeMutationStage::Mutate,
                WorktreeMutationEffect::MayHaveChanged,
                WorktreeMutationError::GitFailed(Some(128)),
                b"locked".to_vec(),
            )
            .unwrap(),
        ))),
    );
    fixture.finish_job(checkout(false));
    let failed = fixture.settled();
    assert!(matches!(failed, Status::Failed { .. }));
    assert_eq!(snapshot(&fixture.service), before);
    let state = snapshot(&fixture.service);
    assert!(matches!(
        fixture
            .service
            .execute_workspace_close(WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: id(901),
                session_id: state.session_id,
                expected_revision: state.revision,
                action: WorkbenchAction::WorkspaceClose {
                    workspace_id: fixture.workspace.clone()
                }
            })
            .unwrap()
            .outcome,
        WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !snapshot(&fixture.service).navigation.workspaces.is_empty() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fixture.settled(),
        failed,
        "manual Close does not rewrite the Git failure receipt"
    );
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(40)))
        .unwrap();
    assert!(
        read_message(&mut fixture.peer).is_err(),
        "manual Close must not submit Git"
    );
}

#[test]
fn mismatched_removed_path_is_unknown_and_keeps_the_workspace() {
    let mut fixture = Fixture::new();
    let before = snapshot(&fixture.service);
    fixture.start(WorktreeIntent::Remove {
        workspace_id: fixture.workspace.clone(),
        force: true,
        trust_repository: false,
    });
    fixture.finish_job(checkout(false));
    fixture.finish_job(Completion::Removed {
        path: AbsoluteCwd::new("C:/other").unwrap(),
    });
    assert!(matches!(
        fixture.settled(),
        Status::Failed {
            reason: Failure::PhysicalUnknown {},
            resumable: false,
            ..
        }
    ));
    assert_eq!(snapshot(&fixture.service), before);
}

#[test]
fn session_stop_joins_the_retained_submitter_after_cancelling_its_job() {
    let mut fixture = Fixture::new();
    fixture.start(WorktreeIntent::Open {
        source: WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
        },
        selector: WorktreeSelector::Path {
            path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
        },
        label: None,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
    });
    let start = fixture.receive();
    fixture.reply(
        &start,
        WorktreeMutationOutcome::Start(WorktreeMutationStartOutcome::Accepted),
    );
    let service = Arc::clone(&fixture.service);
    thread::scope(|scope| {
        let stop =
            scope.spawn(|| service.stop_worktrees_before(Instant::now() + Duration::from_secs(5)));
        loop {
            let request = fixture.receive();
            match request.action {
                WorktreeMutationJobAction::Poll { .. } => fixture.reply(
                    &request,
                    WorktreeMutationOutcome::Poll(WorktreeMutationPollOutcome::Pending),
                ),
                WorktreeMutationJobAction::Cancel { .. } => {
                    fixture.reply(&request, WorktreeMutationOutcome::Cancel(orchbee_capability_node_protocol::WorktreeMutationCancelOutcome::Registered));
                    break;
                }
                WorktreeMutationJobAction::Start(_) => panic!("stop must never start a second job"),
            }
        }
        stop.join().unwrap().unwrap();
    });
    assert!(matches!(
        fixture.settled(),
        Status::Failed {
            reason: Failure::SessionStopping {},
            resumable: false,
            ..
        }
    ));
    assert!(!fixture.service.worktree_maintenance.available());
}

#[test]
fn observed_source_open_then_group_close_retains_tombstones_without_git_deletion() {
    use orchbee_workbench_protocol::{
        WorkbenchWorkspaceGroupCloseRequest, WorkspaceGroupCloseAction as GroupAction,
        WorkspaceGroupCloseOutcome as GroupOutcome, WorkspaceGroupCloseStatus as GroupStatus,
    };
    let mut fixture = Fixture::new();
    let WorkbenchCommandOutcome::WorkspaceCreated {
        workspace_id: main, ..
    } = navigation(
        &fixture.service,
        50,
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(fixture.project.clone()),
            label: WorkbenchLabel::parse("Main source").unwrap(),
        },
    )
    else {
        panic!("main");
    };
    let head = snapshot(&fixture.service);
    let result = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .accept_workspace_directory(
            WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id: id(51),
                session_id: head.session_id.clone(),
                expected_revision: head.revision,
                action: WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                    workspace_id: main.clone(),
                    path: WorkbenchDirectoryPath::parse("C:/repo/subdir").unwrap(),
                },
            },
            WorkerId::parse(&"09".repeat(16)).unwrap(),
        );
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
    ));
    fixture.start(WorktreeIntent::Open {
        source: WorktreeSource::Workspace {
            workspace_id: main.clone(),
        },
        selector: WorktreeSelector::Path {
            path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
        },
        label: None,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        trust_repository: false,
    });
    let inspection = fixture.finish_job(Completion::Checkout(
        WorktreeCheckout::new(
            AbsoluteCwd::new("C:/repo").unwrap(),
            AbsoluteCwd::new("C:/repo/.git").unwrap(),
            AbsoluteCwd::new("C:/repo").unwrap(),
            Some(GitReference::new("main").unwrap()),
            false,
        )
        .unwrap(),
    ));
    assert!(
        matches!(inspection.action, WorktreeMutationJobAction::Start(intent) if matches!(intent.mutation(), WorktreeMutation::InspectSource))
    );
    fixture.finish_job(checkout(false));
    assert!(matches!(
        fixture.settled(),
        Status::Opened { reused: true, .. }
    ));
    let read = || WorkbenchWorkspaceGroupCloseRequest {
        protocol_version: ProtocolVersion,
        request_id: id(81),
        session_id: head.session_id.clone(),
        action: GroupAction::Read {
            operation_id: id(80),
        },
    };
    assert_eq!(
        fixture
            .service
            .execute_workspace_group_close(read())
            .outcome,
        GroupOutcome::NotFound {}
    );
    assert!(
        !fixture.service.group_close_maintenance.available(),
        "read must not start a submitter"
    );
    let start = WorkbenchWorkspaceGroupCloseRequest {
        protocol_version: ProtocolVersion,
        request_id: id(82),
        session_id: head.session_id.clone(),
        action: GroupAction::Start {
            operation_id: id(80),
            project_id: fixture.project.clone(),
        },
    };
    assert!(matches!(
        fixture
            .service
            .execute_workspace_group_close(start.clone())
            .outcome,
        GroupOutcome::Available { .. }
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    let closed = loop {
        let result = fixture
            .service
            .execute_workspace_group_close(read())
            .outcome;
        if !matches!(
            result,
            GroupOutcome::Available {
                status: GroupStatus::Pending {},
                ..
            }
        ) {
            break result;
        }
        assert!(
            Instant::now() < deadline,
            "group continuation did not settle"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let GroupOutcome::Available {
        status: GroupStatus::Closed {},
        members,
        ..
    } = &closed
    else {
        panic!("closed: {closed:?}");
    };
    assert_eq!(
        members
            .iter()
            .map(|member| &member.workspace_id)
            .collect::<Vec<_>>(),
        vec![&fixture.workspace, &main]
    );
    assert!(
        members
            .iter()
            .all(|member| member.close.as_ref().unwrap().is_closed())
    );
    assert!(snapshot(&fixture.service).navigation.workspaces.is_empty());
    assert_eq!(
        fixture.service.execute_workspace_group_close(start).outcome,
        closed
    );
    fixture
        .service
        .stop_workspace_groups_before(deadline)
        .unwrap();
    assert!(!fixture.service.group_close_maintenance.available());
    assert_eq!(
        fixture
            .service
            .execute_workspace_group_close(read())
            .outcome,
        closed
    );
    fixture
        .peer
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    assert!(
        read_message(&mut fixture.peer).is_err(),
        "group Close must not submit Git deletion"
    );
}

#[cfg(windows)]
#[test]
fn group_owner_resumes_admission_failure_but_refuses_stop_or_removed_source() {
    use orchbee_workbench_protocol::{
        WorkbenchWorkspaceGroupCloseRequest, WorkspaceGroupCloseAction as GroupAction,
        WorkspaceGroupCloseFailure as GroupFailure, WorkspaceGroupCloseOutcome as GroupOutcome,
        WorkspaceGroupCloseStatus as GroupStatus,
    };
    use std::sync::atomic::AtomicBool;
    fn apply(fixture: &Fixture, action: GroupAction) -> GroupOutcome {
        let head = snapshot(&fixture.service);
        let workers = fixture.service.workers.lock().unwrap();
        let handle = workers.request_handle().unwrap();
        let mut coordinator = fixture.service.coordinator.lock().unwrap();
        let authority = coordinator.worktree_authority().unwrap();
        let grant = orchbee_server::WorkbenchGrant::read_only(head.session_id.clone())
            .with_terminal_lifecycle();
        authority
            .execute_workspace_group_close(
                Some(&grant),
                WorkbenchWorkspaceGroupCloseRequest {
                    protocol_version: ProtocolVersion,
                    request_id: id(91),
                    session_id: head.session_id,
                    action,
                },
                Some((
                    WorkerId::parse(&"09".repeat(16)).unwrap(),
                    handle.generation(),
                )),
                true,
            )
            .outcome
    }
    for scenario in 0..3 {
        let stopping = scenario == 1;
        let removed = scenario == 2;
        let mut fixture = Fixture::with_close_owner(removed);
        assert!(matches!(
            apply(
                &fixture,
                GroupAction::Start {
                    operation_id: id(90),
                    project_id: fixture.project.clone(),
                }
            ),
            GroupOutcome::Available { .. }
        ));
        if removed {
            let head = snapshot(&fixture.service);
            let response = fixture
                .service
                .execute_workspace_close(WorkbenchCommand {
                    protocol_version: ProtocolVersion,
                    request_id: id(92),
                    session_id: head.session_id,
                    expected_revision: head.revision,
                    action: WorkbenchAction::WorkspaceClose {
                        workspace_id: fixture.workspace.clone(),
                    },
                })
                .unwrap();
            assert!(matches!(
                response.outcome,
                WorkbenchCommandOutcome::WorkspaceCloseAccepted { .. }
            ));
            let deadline = Instant::now() + Duration::from_secs(5);
            while !snapshot(&fixture.service).navigation.workspaces.is_empty() {
                assert!(Instant::now() < deadline, "external Close must settle");
                thread::sleep(Duration::from_millis(10));
            }
        }
        fixture
            .service
            .workspace_group_close_turn(&AtomicBool::new(stopping));
        let failed = apply(
            &fixture,
            GroupAction::Read {
                operation_id: id(90),
            },
        );
        let GroupOutcome::Available {
            status: GroupStatus::Failed { reason, resumable },
            members,
            ..
        } = failed
        else {
            panic!("failure");
        };
        assert_eq!(
            reason,
            if stopping {
                GroupFailure::SessionStopping
            } else if removed {
                GroupFailure::SourceChanged
            } else {
                GroupFailure::ChildAdmission
            }
        );
        assert_eq!(resumable, scenario == 0);
        assert!(members.iter().all(|member| member.close.is_none()));
        assert_eq!(
            snapshot(&fixture.service).navigation.workspaces.len(),
            usize::from(!removed)
        );
        if scenario == 0 {
            fixture.service.start_close_maintenance().unwrap();
            assert!(matches!(
                apply(
                    &fixture,
                    GroupAction::Resume {
                        operation_id: id(90),
                        expected_attempt: 1
                    }
                ),
                GroupOutcome::Available { attempt: 2, .. }
            ));
            fixture
                .service
                .workspace_group_close_turn(&AtomicBool::new(false));
            assert!(matches!(
                apply(
                    &fixture,
                    GroupAction::Read {
                        operation_id: id(90)
                    }
                ),
                GroupOutcome::Available {
                    attempt: 2,
                    status: GroupStatus::Closed {},
                    ..
                }
            ));
            assert!(snapshot(&fixture.service).navigation.workspaces.is_empty());
        }
        fixture
            .peer
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        assert!(
            read_message(&mut fixture.peer).is_err(),
            "empty Workspace closure cannot issue Git or terminal effects"
        );
    }
}
