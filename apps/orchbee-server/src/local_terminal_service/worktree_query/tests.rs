use super::*;
use crate::{
    worker_obcn_carrier::{WorkerWireExit, create_worker_carrier},
    worker_registry::{WorkerAdmission, WorkerRegistry},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, WorktreeJobAction, WorktreeJobOutcome, WorktreeJobPollOutcome,
    WorktreeJobRequest, WorktreeJobResult, WorktreeJobStartOutcome, read_message, write_message,
};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{
    Snapshot, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchRequest, WorkbenchRequestId, WorkbenchWorkspaceId,
};
use std::{
    net::Shutdown,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}
struct Fixture {
    service: Arc<LocalTerminalService>,
    peer: LocalStream,
    wire: Option<JoinHandle<WorkerWireExit>>,
}
impl Fixture {
    fn new() -> Self {
        let service = Arc::new(LocalTerminalService::workspace_fixture(Arc::new(
            Mutex::new(WorkerRegistry::new()),
        )));
        let worker = orchbee_local_worker_protocol::WorkerId::from_bytes([9; 16]);
        let WorkerAdmission::Accepted { generation } =
            service.workers.lock().unwrap().admit(worker)
        else {
            panic!("admit")
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
        service.start_close_maintenance().unwrap();
        Self {
            service,
            peer,
            wire,
        }
    }
    fn snapshot(&self) -> Snapshot {
        self.service
            .coordinator
            .lock()
            .unwrap()
            .worktree_authority()
            .unwrap()
            .snapshot()
            .clone()
    }
    fn command(&self, value: u128, action: WorkbenchAction) -> WorkbenchCommand {
        let snapshot = self.snapshot();
        WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: id(value),
            session_id: snapshot.session_id,
            expected_revision: snapshot.revision,
            action,
        }
    }
    fn navigate(&self, value: u128, action: WorkbenchAction) -> WorkbenchCommandOutcome {
        let request = WorkbenchRequest::Command(self.command(value, action));
        let WorkbenchResponse::Command(result) = self
            .service
            .execute_workbench(request.try_into().unwrap(), true)
            .unwrap()
        else {
            panic!("navigation")
        };
        result.outcome
    }
    fn project(&self) -> WorkbenchProjectId {
        let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = self.navigate(
            1,
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse("Repo").unwrap(),
            },
        ) else {
            panic!("project")
        };
        project_id
    }
    fn workspace(
        &self,
        project: &WorkbenchProjectId,
        value: u128,
        path: &str,
    ) -> WorkbenchWorkspaceId {
        let WorkbenchCommandOutcome::WorkspaceCreated { workspace_id, .. } = self.navigate(
            value,
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project.clone()),
                label: WorkbenchLabel::parse(&format!("Workspace {value}")).unwrap(),
            },
        ) else {
            panic!("workspace")
        };
        let command = self.command(
            value + 1,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept {
                workspace_id: workspace_id.clone(),
                path: WorkbenchDirectoryPath::parse(path).unwrap(),
            },
        );
        let result = self
            .service
            .coordinator
            .lock()
            .unwrap()
            .accept_workspace_directory(command, WorkerId::parse(&"09".repeat(16)).unwrap());
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted { .. }
        ));
        workspace_id
    }
    fn request(&self, source: WorktreeSource) -> WorkbenchWorktreeQuery {
        WorkbenchWorktreeQuery {
            protocol_version: ProtocolVersion,
            request_id: id(1000),
            session_id: self.snapshot().session_id,
            source,
            trust_repository: true,
        }
    }
    fn spawn(&self, source: WorktreeSource) -> JoinHandle<WorkbenchWorktreeQueryResult> {
        let request = self.request(source);
        let service = Arc::clone(&self.service);
        thread::spawn(move || service.execute_worktree_query(request))
    }
    fn receive(&mut self) -> WorktreeJobRequest {
        let CapabilityNodeMessage::WorktreeJobRequest(request) =
            read_message(&mut self.peer).unwrap()
        else {
            panic!("read-only discovery request")
        };
        request
    }
    fn reply(&mut self, request: &WorktreeJobRequest, outcome: WorktreeJobOutcome) {
        let job_id = match request.action {
            WorktreeJobAction::Start { .. } | WorktreeJobAction::StartStatus { .. } => {
                request.operation_id
            }
            WorktreeJobAction::Poll { job_id } | WorktreeJobAction::Cancel { job_id } => job_id,
        };
        write_message(
            &mut self.peer,
            &CapabilityNodeMessage::WorktreeJobResult(WorktreeJobResult {
                operation_id: request.operation_id,
                job_id,
                outcome,
            }),
        )
        .unwrap();
    }
    fn start(&mut self, path: &str) -> WorktreeJobRequest {
        let request = self.receive();
        assert_eq!(
            request.action,
            WorktreeJobAction::Start {
                directory: path.as_bytes().to_vec(),
                trust_repository: Some(true)
            }
        );
        // The read may wait for IPC only after both gates have been released.
        assert!(self.service.workers.try_lock().is_ok());
        assert!(self.service.coordinator.try_lock().is_ok());
        self.reply(
            &request,
            WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted),
        );
        let poll = self.receive();
        assert!(
            matches!(poll.action, WorktreeJobAction::Poll { job_id } if job_id == request.operation_id)
        );
        poll
    }
    fn complete(&mut self, request: &WorktreeJobRequest) {
        self.reply(request, WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::source_completed(
            b"C:/r/.git\n".to_vec(), b"C:/w\n".to_vec(),
            format!("worktree C:/r\0HEAD {}\0branch refs/heads/main\0\0worktree C:/w\0HEAD {}\0branch refs/heads/feature\0\0", "a".repeat(40), "b".repeat(40)).into_bytes(), vec![],
        ).unwrap()));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _stopped = self
            .service
            .stop_close_before(Instant::now() + Duration::from_secs(5));
        let _disconnected = self.peer.shutdown(Shutdown::Both);
        if let Some(wire) = self.wire.take() {
            let _joined = wire.join();
        }
    }
}

#[test]
fn directory_source_needs_no_project_and_never_holds_gates_during_git() {
    let mut fixture = Fixture::new();
    assert!(fixture.snapshot().navigation.projects.is_empty());
    let read = fixture.spawn(WorktreeSource::Directory {
        path: WorkbenchDirectoryPath::parse("C:/w/sub").unwrap(),
    });
    let poll = fixture.start("C:/w/sub");
    fixture.complete(&poll);
    let result = read.join().unwrap();
    assert!(result.validate().is_ok());
    let Outcome::Found {
        source, entries, ..
    } = result.outcome
    else {
        panic!("found")
    };
    assert_eq!(source.checkout_path.as_str(), "C:/w");
    assert_eq!(source.workspace_id, None);
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.workspaces.is_empty()));
    assert!(fixture.snapshot().navigation.projects.is_empty());
}

#[test]
fn workspace_subdirectory_observation_joins_all_known_current_labels() {
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let source = fixture.workspace(&project, 10, "C:/w/sub");
    let other = fixture.workspace(&project, 20, "C:/w");
    let read = fixture.spawn(WorktreeSource::Workspace {
        workspace_id: source.clone(),
    });
    let poll = fixture.start("C:/w/sub");
    fixture.navigate(
        30,
        WorkbenchAction::WorkspaceRename {
            workspace_id: other.clone(),
            label: WorkbenchLabel::parse("Fresh label").unwrap(),
        },
    );
    fixture.complete(&poll);
    let Outcome::Found {
        entries,
        workbench_revision,
        ..
    } = read.join().unwrap().outcome
    else {
        panic!("found")
    };
    assert_eq!(workbench_revision, fixture.snapshot().revision);
    assert_eq!(entries[1].workspaces.len(), 2);
    assert!(
        entries[1]
            .workspaces
            .iter()
            .any(|item| item.workspace_id == source)
    );
    assert!(
        entries[1]
            .workspaces
            .iter()
            .any(|item| item.workspace_id == other && item.label.as_str() == "Fresh label")
    );
}

#[test]
fn closed_source_cannot_publish_the_stale_git_observation() {
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let workspace = fixture.workspace(&project, 10, "C:/w");
    let read = fixture.spawn(WorktreeSource::Workspace {
        workspace_id: workspace.clone(),
    });
    let poll = fixture.start("C:/w");
    let command = fixture.command(
        30,
        WorkbenchAction::WorkspaceClose {
            workspace_id: workspace.clone(),
        },
    );
    fixture.service.execute_workspace_close(command).unwrap();
    fixture.complete(&poll);
    assert_eq!(
        read.join().unwrap().outcome,
        Outcome::Failed {
            reason: Failure::SourceChanged {}
        }
    );
}

#[test]
fn wrong_session_malformed_result_and_lost_worker_never_report_found() {
    let mut fixture = Fixture::new();
    let source = WorktreeSource::Directory {
        path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
    };
    let mut denied = fixture.request(source.clone());
    denied.session_id = orchbee_workbench_protocol::SessionId::parse(&"aa".repeat(16)).unwrap();
    assert_eq!(
        fixture.service.execute_worktree_query(denied).outcome,
        Outcome::Failed {
            reason: Failure::Denied {}
        }
    );
    let read = fixture.spawn(source.clone());
    let poll = fixture.start("C:/w");
    fixture.reply(
        &poll,
        WorktreeJobOutcome::Poll(
            WorktreeJobPollOutcome::completed(vec![], vec![], vec![]).unwrap(),
        ),
    );
    assert_eq!(
        read.join().unwrap().outcome,
        Outcome::Failed {
            reason: Failure::MalformedListing {}
        }
    );
    let read = fixture.spawn(source);
    let _poll = fixture.start("C:/w");
    fixture.peer.shutdown(Shutdown::Both).unwrap();
    assert!(matches!(
        read.join().unwrap().outcome,
        Outcome::Failed {
            reason: Failure::TransportLoss {} | Failure::Unavailable { .. }
        }
    ));
}

fn spawn_workspace_git(
    fixture: &Fixture,
    workspace_id: WorkbenchWorkspaceId,
) -> JoinHandle<orchbee_workbench_protocol::WorkspaceGitResult> {
    let snapshot = fixture.snapshot();
    let service = fixture.service.clone();
    thread::spawn(move || {
        service.execute_workspace_git(orchbee_workbench_protocol::WorkspaceGitRequest {
            protocol_version: ProtocolVersion,
            request_id: id(900),
            session_id: snapshot.session_id,
            workspace_id,
        })
    })
}
fn status_poll(fixture: &mut Fixture, path: &str) -> WorktreeJobRequest {
    let start = fixture.receive();
    assert_eq!(
        start.action,
        WorktreeJobAction::StartStatus {
            directory: path.as_bytes().to_vec()
        }
    );
    assert!(fixture.service.coordinator.try_lock().is_ok());
    assert!(fixture.service.workers.try_lock().is_ok());
    fixture.reply(
        &start,
        WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted),
    );
    let poll = fixture.receive();
    assert!(matches!(poll.action,WorktreeJobAction::Poll{job_id} if job_id==start.operation_id));
    poll
}

fn complete_status(fixture: &mut Fixture, poll: &WorktreeJobRequest) {
    fixture.reply(
        poll,
        WorktreeJobOutcome::Poll(
            WorktreeJobPollOutcome::status_completed(
                b"# branch.oid (initial)\n# branch.head main\n".to_vec(),
                vec![],
            )
            .unwrap(),
        ),
    );
}

fn assert_status_is_not_admitted(fixture: &Fixture, workspace: WorkbenchWorkspaceId) {
    use orchbee_workbench_protocol::{
        WorkspaceGitFailure, WorkspaceGitOutcome, WorkspaceGitRequest,
    };
    let service = Arc::clone(&fixture.service);
    let session_id = fixture.snapshot().session_id;
    let (sent, received) = std::sync::mpsc::sync_channel(1);
    let read = thread::spawn(move || {
        sent.send(service.execute_workspace_git(WorkspaceGitRequest {
            protocol_version: ProtocolVersion,
            request_id: id(960),
            session_id,
            workspace_id: workspace,
        }))
        .unwrap();
    });
    // The peer deliberately has not answered the active reader. This return
    // proves there was no Worker admission or queued background lifetime.
    let result = received
        .recv_timeout(Duration::from_secs(2))
        .expect("background status queued behind a discovery owner");
    assert_eq!(
        result.outcome,
        WorkspaceGitOutcome::Failed {
            reason: WorkspaceGitFailure::Unavailable
        }
    );
    read.join().unwrap();
}

#[test]
fn six_sidebar_status_readers_cannot_steal_workspace_or_directory_query_receipts() {
    for workspace_source in [true, false] {
        let mut fixture = Fixture::new();
        let project = fixture.project();
        let workspaces: Vec<_> = (1..=6)
            .map(|i| fixture.workspace(&project, 10 * i, "C:/w"))
            .collect();
        let before = fixture.snapshot();
        let source = if workspace_source {
            WorktreeSource::Workspace {
                workspace_id: workspaces[0].clone(),
            }
        } else {
            WorktreeSource::Directory {
                path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
            }
        };
        let read = fixture.spawn(source);
        let poll = fixture.start("C:/w");
        fixture.reply(
            &poll,
            WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::Pending),
        );
        // The Worker may finish immediately after Pending. Its single receipt
        // must remain owned throughout the server's interval before the next poll.
        for workspace in &workspaces {
            assert_status_is_not_admitted(&fixture, workspace.clone());
        }
        let settled = fixture.receive();
        assert_eq!(settled.action, poll.action);
        fixture.complete(&settled);
        assert!(matches!(
            read.join().unwrap().outcome,
            Outcome::Found { .. }
        ));
        assert_eq!(fixture.snapshot(), before);
        let status = spawn_workspace_git(&fixture, workspaces[0].clone());
        let poll = status_poll(&mut fixture, "C:/w");
        complete_status(&mut fixture, &poll);
        assert!(matches!(
            status.join().unwrap().outcome,
            orchbee_workbench_protocol::WorkspaceGitOutcome::Observed { .. }
        ));
    }
}

#[test]
fn committed_directory_list_shares_discovery_ownership_with_query_and_status() {
    use orchbee_workbench_protocol::WorkbenchWorktreeListRead;
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let workspace = fixture.workspace(&project, 10, "C:/w");
    let before = fixture.snapshot();
    let directory_id = before.navigation.workspaces[0]
        .primary_directory_id
        .clone()
        .unwrap();
    let request = WorkbenchWorktreeListRead {
        protocol_version: ProtocolVersion,
        request_id: id(970),
        directory_id: directory_id.clone(),
    };
    let service = Arc::clone(&fixture.service);
    let list = thread::spawn(move || service.execute_worktree_list(request).unwrap());
    let start = fixture.receive();
    assert_eq!(
        start.action,
        WorktreeJobAction::Start {
            directory: b"C:/w".to_vec(),
            trust_repository: None,
        }
    );
    fixture.reply(
        &start,
        WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted),
    );
    let poll = fixture.receive();
    assert!(
        matches!(poll.action, WorktreeJobAction::Poll { job_id } if job_id == start.operation_id)
    );
    let query = fixture.spawn(WorktreeSource::Directory {
        path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
    });
    assert_status_is_not_admitted(&fixture, workspace);
    fixture.reply(
        &poll,
        WorktreeJobOutcome::Poll(
            WorktreeJobPollOutcome::completed(
                b"C:/r/.git\n".to_vec(),
                format!(
                    "worktree C:/w\0HEAD {}\0branch refs/heads/main\0\0",
                    "a".repeat(40)
                )
                .into_bytes(),
                vec![],
            )
            .unwrap(),
        ),
    );
    assert!(
        matches!(list.join().unwrap().outcome, WorkbenchWorktreeListOutcome::Found { directory_id: ref found, .. } if found == &directory_id)
    );
    let poll = fixture.start("C:/w");
    fixture.complete(&poll);
    assert!(matches!(
        query.join().unwrap().outcome,
        Outcome::Found { .. }
    ));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn foreground_query_waits_for_active_status_without_holding_session_or_registry_gates() {
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let workspace = fixture.workspace(&project, 10, "C:/w");
    let status = spawn_workspace_git(&fixture, workspace.clone());
    let poll = status_poll(&mut fixture, "C:/w");
    let query = fixture.spawn(WorktreeSource::Workspace {
        workspace_id: workspace.clone(),
    });
    for _ in 0..6 {
        assert_status_is_not_admitted(&fixture, workspace.clone());
    }
    assert!(fixture.service.coordinator.try_lock().is_ok());
    assert!(fixture.service.workers.try_lock().is_ok());
    complete_status(&mut fixture, &poll);
    assert!(matches!(
        status.join().unwrap().outcome,
        orchbee_workbench_protocol::WorkspaceGitOutcome::Observed { .. }
    ));
    let poll = fixture.start("C:/w");
    fixture.complete(&poll);
    assert!(matches!(
        query.join().unwrap().outcome,
        Outcome::Found { .. }
    ));
}

#[test]
fn foreground_admission_timeout_does_not_dispatch_or_release_the_active_status() {
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let workspace = fixture.workspace(&project, 10, "C:/w");
    let status = spawn_workspace_git(&fixture, workspace.clone());
    let poll = status_poll(&mut fixture, "C:/w");
    let request = fixture.request(WorktreeSource::Workspace {
        workspace_id: workspace.clone(),
    });
    let result = fixture
        .service
        .execute_worktree_query_until(request, Instant::now() + Duration::from_millis(50));
    assert_eq!(
        result.outcome,
        Outcome::Failed {
            reason: Failure::Timeout {}
        }
    );
    assert_status_is_not_admitted(&fixture, workspace.clone());
    complete_status(&mut fixture, &poll);
    assert!(matches!(
        status.join().unwrap().outcome,
        orchbee_workbench_protocol::WorkspaceGitOutcome::Observed { .. }
    ));
    // The very next wire message must belong to the new read, never the waiter
    // whose deadline elapsed before admission.
    let next = fixture.spawn(WorktreeSource::Workspace {
        workspace_id: workspace,
    });
    let poll = fixture.start("C:/w");
    fixture.complete(&poll);
    assert!(matches!(
        next.join().unwrap().outcome,
        Outcome::Found { .. }
    ));
}

#[test]
fn query_deadline_cancels_the_exact_admitted_job_and_only_definite_busy_starts_retry() {
    use orchbee_capability_node_protocol::WorktreeJobCancelOutcome;
    for stall_start in [true, false] {
        let mut fixture = Fixture::new();
        let source = WorktreeSource::Directory {
            path: WorkbenchDirectoryPath::parse("C:/w").unwrap(),
        };
        let request = fixture.request(source.clone());
        let service = Arc::clone(&fixture.service);
        let began = Instant::now();
        let deadline = began + Duration::from_millis(300);
        let read = thread::spawn(move || service.execute_worktree_query_until(request, deadline));
        let start = fixture.receive();
        assert!(matches!(
            start.action,
            WorktreeJobAction::Start {
                trust_repository: Some(true),
                ..
            }
        ));
        let stalled = if stall_start {
            start.clone()
        } else {
            fixture.reply(
                &start,
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted),
            );
            fixture.receive()
        };
        assert_eq!(
            read.join().unwrap().outcome,
            Outcome::Failed {
                reason: Failure::Timeout {}
            }
        );
        assert!(began.elapsed() < Duration::from_secs(2));
        fixture.reply(
            &stalled,
            if stall_start {
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted)
            } else {
                WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::Pending)
            },
        );
        let cancel = fixture.receive();
        assert!(
            matches!(cancel.action, WorktreeJobAction::Cancel { job_id } if job_id == start.operation_id)
        );
        fixture.reply(
            &cancel,
            WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::Registered),
        );
        let next = fixture.spawn(source);
        let mut last_id = cancel.operation_id;
        // Model physical cancellation still settling. Busy is the only reply
        // that permits a fresh Start; every attempt retains fresh exact IDs.
        for _ in 0..2 {
            let retry = fixture.receive();
            assert!(matches!(
                retry.action,
                WorktreeJobAction::Start {
                    trust_repository: Some(true),
                    ..
                }
            ));
            assert_ne!(retry.operation_id, last_id);
            assert_ne!(retry.operation_id, start.operation_id);
            last_id = retry.operation_id;
            fixture.reply(
                &retry,
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Busy),
            );
        }
        let poll = fixture.start("C:/w");
        assert!(
            matches!(poll.action, WorktreeJobAction::Poll { job_id } if job_id != start.operation_id && job_id != last_id)
        );
        fixture.complete(&poll);
        assert!(matches!(
            next.join().unwrap().outcome,
            Outcome::Found { .. }
        ));
    }
}
#[test]
fn workspace_git_status_is_read_only_and_rejects_topology_changes_after_worker_settlement() {
    use orchbee_workbench_protocol::{
        WorkspaceGitFailure as GitFailure, WorkspaceGitOutcome as GitOutcome,
    };
    for changed in [false, true] {
        let mut fixture = Fixture::new();
        let project = fixture.project();
        let workspace = fixture.workspace(&project, 10, "C:/w/sub");
        let before = fixture.snapshot();
        let read = spawn_workspace_git(&fixture, workspace.clone());
        let poll = status_poll(&mut fixture, "C:/w/sub");
        if changed {
            fixture.navigate(
                40,
                WorkbenchAction::WorkspaceRename {
                    workspace_id: workspace,
                    label: WorkbenchLabel::parse("renamed").unwrap(),
                },
            );
        }
        fixture.reply(&poll,WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::status_completed(format!("# branch.oid {}\n# branch.head feature\n# branch.upstream main\n# branch.ab +2 -3\n","a".repeat(40)).into_bytes(),vec![]).unwrap()));
        let result = read.join().unwrap();
        assert!(result.validate().is_ok());
        if changed {
            assert_eq!(
                result.outcome,
                GitOutcome::Failed {
                    reason: GitFailure::SourceChanged
                }
            );
        } else {
            assert!(
                matches!(result.outcome,GitOutcome::Observed {workbench_revision,branch:Some(ref b),divergence:Some(ref d)} if workbench_revision==before.revision && b=="feature" && d.ahead==2 && d.behind==3)
            );
            assert_eq!(fixture.snapshot(), before);
        }
    }
}
#[test]
fn workspace_git_status_denies_wrong_session_and_fences_lost_worker_without_retrying_start() {
    use orchbee_workbench_protocol::{
        WorkspaceGitFailure as GitFailure, WorkspaceGitOutcome as GitOutcome, WorkspaceGitRequest,
    };
    let mut fixture = Fixture::new();
    let project = fixture.project();
    let workspace = fixture.workspace(&project, 10, "C:/w");
    let denied = fixture.service.execute_workspace_git(WorkspaceGitRequest {
        protocol_version: ProtocolVersion,
        request_id: id(910),
        session_id: orchbee_workbench_protocol::SessionId::parse(&"aa".repeat(16)).unwrap(),
        workspace_id: workspace.clone(),
    });
    assert_eq!(
        denied.outcome,
        GitOutcome::Failed {
            reason: GitFailure::Denied
        }
    );
    let read = spawn_workspace_git(&fixture, workspace);
    let _poll = status_poll(&mut fixture, "C:/w");
    fixture.peer.shutdown(Shutdown::Both).unwrap();
    assert_eq!(
        read.join().unwrap().outcome,
        GitOutcome::Failed {
            reason: GitFailure::Unavailable
        }
    );
}

#[test]
fn workspace_git_deadline_returns_before_start_or_poll_reply_and_late_cleanup_keeps_carrier_live() {
    use orchbee_capability_node_protocol::WorktreeJobCancelOutcome;
    use orchbee_workbench_protocol::{
        WorkspaceGitFailure as GitFailure, WorkspaceGitOutcome as GitOutcome, WorkspaceGitRequest,
    };
    for stall_start in [true, false] {
        let mut fixture = Fixture::new();
        let project = fixture.project();
        let workspace = fixture.workspace(&project, 10, "C:/w");
        let request = WorkspaceGitRequest {
            protocol_version: ProtocolVersion,
            request_id: id(950),
            session_id: fixture.snapshot().session_id,
            workspace_id: workspace.clone(),
        };
        let service = fixture.service.clone();
        let began = Instant::now();
        let deadline = began + Duration::from_millis(500);
        let read = thread::spawn(move || service.execute_workspace_git_until(request, deadline));
        let start = fixture.receive();
        assert!(matches!(
            start.action,
            WorktreeJobAction::StartStatus { .. }
        ));
        let stalled = if stall_start {
            start.clone()
        } else {
            fixture.reply(
                &start,
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted),
            );
            fixture.receive()
        };
        // No Worker reply is sent until the real handler has returned. Drop
        // cannot wait for start acceptance, poll settlement or cancel delivery.
        assert_eq!(
            read.join().unwrap().outcome,
            GitOutcome::Failed {
                reason: GitFailure::Timeout
            }
        );
        assert!(began.elapsed() < Duration::from_secs(2));
        fixture.reply(
            &stalled,
            if stall_start {
                WorktreeJobOutcome::Start(WorktreeJobStartOutcome::Accepted)
            } else {
                WorktreeJobOutcome::Poll(WorktreeJobPollOutcome::Pending)
            },
        );
        let cancel = fixture.receive();
        assert!(
            matches!(cancel.action, WorktreeJobAction::Cancel { job_id } if job_id == start.operation_id)
        );
        fixture.reply(
            &cancel,
            WorktreeJobOutcome::Cancel(WorktreeJobCancelOutcome::Registered),
        );
        // Checked late start/poll/cancel replies were consumed. A new real read
        // on the same carrier works, without retrying the timed-out start.
        let next = spawn_workspace_git(&fixture, workspace);
        let poll = status_poll(&mut fixture, "C:/w");
        fixture.reply(
            &poll,
            WorktreeJobOutcome::Poll(
                WorktreeJobPollOutcome::status_completed(
                    b"# branch.oid (initial)\n# branch.head next\n".to_vec(),
                    vec![],
                )
                .unwrap(),
            ),
        );
        assert!(
            matches!(next.join().unwrap().outcome, GitOutcome::Observed { branch: Some(ref b), .. } if b == "next")
        );
    }
}
