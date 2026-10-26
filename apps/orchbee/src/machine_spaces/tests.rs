use super::*;
use crate::local_client::cancellation::OwnedCancellation;
mod navigation_projection;
use crate::{local_client::tests::server::FakeServer, machine_agents::tests::accept};
use orchbee_workbench_protocol::{
    Directory, MetadataTokenKey, MetadataTokenValue, Project, TerminalContext,
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchRequest, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, WorkbenchTerminalContextId, WorkerId,
    Workspace, WorkspaceGitResult, WorkspaceMetadataResult, read_workbench_request,
    write_workbench_response,
};
use std::{
    io::{self, Read},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn fixture() -> Snapshot {
    let mut snapshot = crate::machine_agents::tests::fixture();
    let directory_id = WorkbenchDirectoryId::parse(&id(80)).unwrap();
    let workspaces: Vec<_> = [81, 91]
        .into_iter()
        .map(|n| Workspace {
            project_id: (n == 81).then(|| WorkbenchProjectId::parse(&id(70)).unwrap()),
            label: "same 工作区".into(),
            primary_directory_id: Some(directory_id.clone()),
            workspace_id: WorkbenchWorkspaceId::parse(&id(n)).unwrap(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(n + 1)).unwrap(),
        })
        .collect();
    snapshot
        .terminal_contexts
        .extend(workspaces.iter().map(|w| TerminalContext {
            terminal_context_id: w.terminal_context_id.clone(),
            tabs: Vec::new(),
            groups: Vec::new(),
        }));
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
        label: "Repo".into(),
    });
    snapshot.navigation.directories = vec![Directory {
        directory_id,
        worker_id: WorkerId::parse(&id(99)).unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
    }];
    snapshot.navigation.workspaces.extend(workspaces);
    snapshot.navigation.workspace_order = snapshot
        .navigation
        .workspaces
        .iter()
        .map(|workspace| workspace.workspace_id.clone())
        .collect();
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&id(71)).unwrap(),
        label: "Empty Repo".into(),
    });
    snapshot
}
fn peer(bad: u8, git_count: Arc<AtomicUsize>) -> io::Result<FakeServer> {
    FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        let mut snapshots = 0;
        let mut metadata = 0;
        loop {
            match read_workbench_request(&mut stream).map_err(io::Error::other)? {
                WorkbenchRequest::SnapshotRead(request) => {
                    snapshots += 1;
                    let mut snapshot = fixture();
                    if bad == 2 && snapshots == 2 {
                        snapshot.revision = WorkbenchRevision::new(2);
                    }
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
                        }),
                    )
                    .map_err(io::Error::other)?;
                    if snapshots == 2 {
                        return Ok(());
                    }
                }
                WorkbenchRequest::WorkspaceMetadata(request) => {
                    metadata += 1;
                    let WorkspaceMetadataAction::Read { workspace_ids } = request.action else {
                        panic!("read only");
                    };
                    assert_eq!(workspace_ids.len(), if metadata == 1 { 3 } else { 0 });
                    let tokens = MetadataTokens::new(
                        [(
                            MetadataTokenKey::parse("branch").unwrap(),
                            MetadataTokenValue::parse("custom branch").unwrap(),
                        )]
                        .into(),
                    )
                    .unwrap();
                    let rows = workspace_ids
                        .into_iter()
                        .map(|workspace_id| WorkspaceMetadataRow {
                            workspace_id,
                            tokens: tokens.clone(),
                        })
                        .collect();
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::WorkspaceMetadata(WorkspaceMetadataResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            session_id: request.session_id,
                            outcome: WorkspaceMetadataOutcome::Read {
                                workbench_revision: WorkbenchRevision::new(if bad == 5 {
                                    2
                                } else {
                                    1
                                }),
                                metadata_revision: WorkbenchRevision::new(
                                    if bad == 1 && metadata == 2 { 3 } else { 2 },
                                ),
                                workspaces: rows,
                            },
                        }),
                    )
                    .map_err(io::Error::other)?;
                    if bad == 5 || (bad == 1 && metadata == 2) {
                        return Ok(());
                    }
                }
                WorkbenchRequest::WorkspaceGit(request) => {
                    git_count.fetch_add(1, Ordering::AcqRel);
                    let outcome = match bad {
                        4 => WorkspaceGitOutcome::Failed {
                            reason: WorkspaceGitFailure::SourceChanged,
                        },
                        7 => WorkspaceGitOutcome::Failed {
                            reason: WorkspaceGitFailure::Unavailable,
                        },
                        _ => WorkspaceGitOutcome::Observed {
                            workbench_revision: WorkbenchRevision::new(1),
                            branch: Some("worktree/测试".into()),
                            divergence: Some(GitDivergence {
                                ahead: 2,
                                behind: 1,
                            }),
                        },
                    };
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::WorkspaceGit(WorkspaceGitResult {
                            protocol_version: ProtocolVersion,
                            request_id: request.request_id,
                            session_id: request.session_id,
                            workspace_id: if bad == 3 {
                                WorkbenchWorkspaceId::parse(&id(100)).unwrap()
                            } else {
                                request.workspace_id
                            },
                            outcome,
                        }),
                    )
                    .map_err(io::Error::other)?;
                    if matches!(bad, 3 | 4) {
                        return Ok(());
                    }
                }
                other => panic!("unexpected or mutating request: {other:?}"),
            }
        }
    })
}
#[test]
fn workspace_checkpoint_is_complete_and_reads_git_for_each_workspace() -> io::Result<()> {
    for (git_requested, bad) in [(true, 0), (false, 0), (true, 7)] {
        let count = Arc::new(AtomicUsize::new(0));
        let peer = peer(bad, count.clone())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        let checkpoint = read(
            &mut client,
            deadline,
            OwnedCancellation::new().unwrap().borrow(),
            git_requested,
        )
        .map_err(|_| io::Error::other("complete checkpoint"))?;
        assert_eq!(checkpoint.rows.len(), 3);
        assert_eq!(
            checkpoint.projects,
            std::sync::Arc::from([
                ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: "Repo".into()
                },
                ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(71)).unwrap(),
                    label: "Empty Repo".into()
                },
            ])
        );
        assert_eq!(checkpoint.git_requested, git_requested);
        assert_eq!(checkpoint.metadata_revision, WorkbenchRevision::new(2));
        assert_ne!(checkpoint.rows[1].target, checkpoint.rows[2].target);
        assert_eq!(
            checkpoint.rows[1].project_id,
            Some(WorkbenchProjectId::parse(&id(70)).unwrap())
        );
        assert_eq!(checkpoint.rows[2].project_id, None);
        assert_eq!(checkpoint.rows[1].label, checkpoint.rows[2].label);
        // Every Workspace has its own live cwd observation, including the one without a
        // canonical directory. Equal canonical directories never coalesce either.
        assert_eq!(
            count.load(Ordering::Acquire),
            3 * usize::from(git_requested)
        );
        for row in &checkpoint.rows[1..] {
            assert_eq!(
                row.tokens.as_map().values().next().unwrap().as_str(),
                "custom branch"
            );
            if git_requested && bad == 0 {
                assert_eq!(
                    row.git.as_ref().unwrap().branch.as_deref(),
                    Some("worktree/测试")
                );
                assert_eq!(
                    row.git.as_ref().unwrap().divergence,
                    Some(GitDivergence {
                        ahead: 2,
                        behind: 1
                    })
                );
            } else {
                assert!(row.git.is_none());
            }
        }
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn workspace_checkpoint_rejects_changed_reports_topology_and_wrong_git_identity() -> io::Result<()>
{
    for bad in 1..=5 {
        let peer = peer(bad, Arc::new(AtomicUsize::new(0)))?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        assert!(
            read(
                &mut client,
                deadline,
                OwnedCancellation::new().unwrap().borrow(),
                true
            )
            .is_err(),
            "accepted torn checkpoint {bad}"
        );
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn workspace_checkpoint_cancellation_retires_inflight_io_and_sends_no_followup() -> io::Result<()> {
    let (send, reading) = mpsc::sync_channel(1);
    let peer = FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("snapshot");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: fixture(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorkspaceMetadata(_)
        ));
        send.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "cancelled client must send no followup"
        );
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    let cancel = Arc::new(OwnedCancellation::new().unwrap());
    let flag = cancel.clone();
    let task = thread::spawn(move || read(&mut client, deadline, flag.borrow(), true));
    reading
        .recv_timeout(Duration::from_secs(2))
        .map_err(io::Error::other)?;
    let start = Instant::now();
    cancel.cancel();
    assert!(task.join().unwrap().is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    peer.finish()?;
    Ok(())
}
