use super::*;
use crate::workbench_state::worktree_launch::addressed::SidebarWorktreeTarget;
use crate::workbench_state::worktree_launch::addressed::tests::fixture;

fn snapshot_reply(stream: &mut LocalStream, snapshot: Snapshot) -> io::Result<()> {
    let WorkbenchRequest::SnapshotRead(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("one scope snapshot");
    };
    write_workbench_response(
        stream,
        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        }),
    )
    .map_err(io::Error::other)
}

/// Answers the lane's `workbench.current-directory` read, which must name the first Workspace.
fn directory_reply(
    stream: &mut LocalStream,
    workspace_id: &WorkbenchWorkspaceId,
    outcome: CurrentDirectoryOutcome,
) -> io::Result<()> {
    let WorkbenchRequest::CurrentDirectory(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("the Server's current-directory answer, not a terminal attachment");
    };
    assert_eq!(&request.workspace_id, workspace_id);
    write_workbench_response(
        stream,
        &WorkbenchResponse::CurrentDirectory(CurrentDirectoryResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            workspace_id: request.workspace_id,
            panel: request.panel,
            outcome,
        }),
    )
    .map_err(io::Error::other)
}

#[test]
fn changed_first_root_at_start_is_no_effect_and_never_queries_the_replacement_repository() {
    use crate::workbench_state::worktree_launch::Launcher;
    use crate::workbench_state::worktree_launch::tests::sidebar_fixture;
    let canonical = fixture();
    let (origin, sidebar) = sidebar_fixture();
    let first = sidebar.addressed.first_workspace().unwrap().clone();
    let mut launcher = Launcher::default();
    assert!(launcher.open_sidebar(
        origin.clone(),
        sidebar,
        WorktreeMode::Open,
        Some(&WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap())
    ));
    let token = launcher.token().unwrap();
    launcher.draw(Some(token));
    let plan = launcher.confirm(token, id(90), &origin).unwrap();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        snapshot_reply(&mut stream, canonical.clone())?;
        directory_reply(
            &mut stream,
            &first,
            CurrentDirectoryOutcome::Observed {
                directory: WorkbenchDirectoryPath::parse("C:/changed-root").unwrap(),
            },
        )?;
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "changed source must not query another repository or send Start"
        );
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::Start(Box::new(plan)))));
    assert!(matches!(
        take(&lane),
        Reply::Operation {
            exchange: Exchange::Start,
            result: Err(false),
            snapshot: None,
            ..
        }
    ));
    assert!(lane.close());
    server.finish().unwrap();
}

#[test]
fn sidebar_rows_read_git_where_the_server_answers_without_attaching_to_a_terminal() {
    use orchbee_workbench_protocol::CurrentDirectoryFailure;
    let nested = WorkbenchDirectoryPath::parse("C:/first-repository/nested").unwrap();
    for case in 0..3 {
        let mut canonical = fixture();
        let workspace = canonical.navigation.workspaces[1].workspace_id.clone();
        // The location line still shows the Workspace's own directory; the terminal has moved on.
        canonical.navigation.workspaces[1].terminal_directory =
            Some(WorkbenchDirectoryPath::parse("C:/first-repository").unwrap());
        let target = if case == 2 {
            SidebarWorktreeTarget::Project(canonical.navigation.projects[0].project_id.clone())
        } else {
            SidebarWorktreeTarget::Workspace(workspace.clone())
        };
        let addressed = AddressedWorktreeSource::capture(&canonical, &target).unwrap();
        assert_eq!(addressed.first_workspace(), Some(&workspace));
        let directory = addressed.cache.directory().unwrap().clone();
        let (outcome, source) = match case {
            0 => (
                CurrentDirectoryOutcome::Observed {
                    directory: nested.clone(),
                },
                Some(WorktreeSource::WorkspaceDirectory {
                    workspace_id: workspace.clone(),
                    path: nested.clone(),
                }),
            ),
            // Without an answer a Workspace reads its own directory, and a Project reads nothing.
            1 => (
                CurrentDirectoryOutcome::Failed {
                    reason: CurrentDirectoryFailure::Unavailable,
                },
                Some(WorktreeSource::Workspace {
                    workspace_id: workspace.clone(),
                }),
            ),
            _ => (
                CurrentDirectoryOutcome::Failed {
                    reason: CurrentDirectoryFailure::NotFound,
                },
                None,
            ),
        };
        // Git lists the first member's checkout, which the Server associates with it.
        let member = &canonical.navigation.workspaces[1];
        let mut entries = discovery().entries;
        entries[0].checkout.path = WorktreeEntryText::Utf8(directory.path.as_str().into());
        entries[0].workspaces.push(WorktreeWorkspaceAssociation {
            workspace_id: workspace.clone(),
            project_id: member.project_id.clone(),
            label: WorkbenchLabel::parse(&member.label).unwrap(),
        });
        let expected = addressed.clone();
        let server_snapshot = canonical.clone();
        let queried = source.clone();
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            handshake(&mut stream, index as u64 + 1)?;
            if index == 0 {
                return Ok(());
            }
            snapshot_reply(&mut stream, server_snapshot.clone())?;
            directory_reply(&mut stream, &workspace, outcome.clone())?;
            if let Some(source) = queried.clone() {
                let WorkbenchRequest::WorktreeQuery(request) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("Git lists the repository where the Server answered");
                };
                assert_eq!(request.source, source);
                let requested_path = match &source {
                    WorktreeSource::WorkspaceDirectory { path, .. } => path.clone(),
                    _ => directory.path.clone(),
                };
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        outcome: WorkbenchWorktreeQueryOutcome::Found {
                            session_id: expected.cache.session_id().clone(),
                            workbench_revision: expected.cache.revision(),
                            worker_id: directory.worker_id.clone(),
                            source: WorktreeQuerySource {
                                requested_path,
                                repository_root: directory.path.clone(),
                                common_directory: WorkbenchDirectoryPath::parse(
                                    "C:/first-repository/.git",
                                )
                                .unwrap(),
                                checkout_path: directory.path.clone(),
                                workspace_id: Some(workspace.clone()),
                            },
                            entries: entries.clone(),
                        },
                    }),
                )
                .map_err(io::Error::other)?;
                snapshot_reply(&mut stream, server_snapshot.clone())?;
            }
            assert_eq!(
                stream.read(&mut [0])?,
                0,
                "no terminal attachment, fallback query or Start"
            );
            Ok(())
        })
        .unwrap();
        let mut lane = lane(&server);
        assert!(lane.submit(job(Action::SidebarQuery {
            episode: 9,
            source: Box::new(addressed)
        })));
        let Reply::SidebarQuery { episode: 9, result } = take(&lane) else {
            panic!("exact menu episode");
        };
        match source {
            Some(source) => {
                let result = result.unwrap();
                assert!(result.valid(&canonical));
                assert_eq!(result.source, source);
            }
            None => assert_eq!(result.unwrap_err(), WorktreeNotice::SourceUnavailable),
        }
        assert!(lane.close());
        server.finish().unwrap();
    }
}

#[test]
fn non_git_first_project_member_never_queries_a_later_repository_or_starts_an_operation() {
    // Git's probe refusal names its reason; other listing failures stay generic.
    for (reason, notice) in [
        (
            WorktreeQueryFailure::GitFailed {
                stage: WorktreeListStage::Probe,
                status: Some(128),
            },
            WorktreeNotice::NotRepository,
        ),
        (
            WorktreeQueryFailure::NotFound {},
            WorktreeNotice::ListingUnavailable,
        ),
    ] {
        let mut canonical = fixture();
        canonical.terminal_contexts[1].tabs.clear();
        let expected = canonical.clone();
        let addressed = AddressedWorktreeSource::capture(
            &canonical,
            &SidebarWorktreeTarget::Project(canonical.navigation.projects[0].project_id.clone()),
        )
        .unwrap();
        let expected_workspace = canonical.navigation.workspaces[1].workspace_id.clone();
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            handshake(&mut stream, index as u64 + 1)?;
            if index == 0 {
                return Ok(());
            }
            let WorkbenchRequest::SnapshotRead(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("first prove exact addressed membership");
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: expected.clone(),
                    },
                }),
            )
            .map_err(io::Error::other)?;
            // Without a terminal the Server answers the member's own directory.
            directory_reply(
                &mut stream,
                &expected_workspace,
                CurrentDirectoryOutcome::Observed {
                    directory: WorkbenchDirectoryPath::parse("C:/first-repository").unwrap(),
                },
            )?;
            let WorkbenchRequest::WorktreeQuery(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("only addressed first member discovery");
            };
            assert_eq!(
                request.source,
                WorktreeSource::Workspace {
                    workspace_id: expected_workspace.clone()
                }
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchWorktreeQueryOutcome::Failed {
                        reason: reason.clone(),
                    },
                }),
            )
            .map_err(io::Error::other)?;
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "no fallback query or Start after a non-Git first member"
            );
            Ok(())
        })
        .unwrap();
        let mut lane = lane(&server);
        assert!(lane.submit(job(Action::SidebarQuery {
            episode: 7,
            source: Box::new(addressed)
        })));
        assert!(matches!(
            take(&lane),
            Reply::SidebarQuery {
                episode: 7,
                result: Err(actual)
            } if actual == notice
        ));
        assert!(lane.close());
        server.finish().unwrap();
    }
}

#[test]
fn source_membership_drift_refuses_before_any_directory_read_or_query() {
    let canonical = fixture();
    let addressed = AddressedWorktreeSource::capture(
        &canonical,
        &SidebarWorktreeTarget::Project(canonical.navigation.projects[0].project_id.clone()),
    )
    .unwrap();
    let mut changed = canonical;
    changed.revision = WorkbenchRevision::new(changed.revision.value() + 1);
    changed.navigation.workspace_order.reverse();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("one scope read");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: changed.clone(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "a changed first member retires before any directory read or Git query"
        );
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::SidebarQuery {
        episode: 8,
        source: Box::new(addressed)
    })));
    assert!(matches!(
        take(&lane),
        Reply::SidebarQuery {
            episode: 8,
            result: Err(WorktreeNotice::SourceChanged)
        }
    ));
    assert!(lane.close());
    server.finish().unwrap();
}
