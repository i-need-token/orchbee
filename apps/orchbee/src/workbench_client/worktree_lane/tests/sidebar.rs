use super::*;
use crate::workbench_client::primary_directory::tests::process_record;
use crate::workbench_client::state::worktree_launch::addressed::{
    SidebarWorktreeTarget, tests::fixture,
};
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult, read_client_message, write_server_message,
};

fn viewer_reply(
    stream: &mut LocalStream,
    action: TerminalViewerAction,
    outcome: TerminalViewerOutcome,
) -> io::Result<()> {
    let ClientMessage::TerminalViewer(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        panic!("read-only Viewer action");
    };
    assert_eq!(request.action(), &action);
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            request.request_id().clone(),
            outcome,
        )),
    )
    .map_err(io::Error::other)
}
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

fn attach_reply(
    stream: &mut LocalStream,
    addressed: &AddressedWorktreeSource,
) -> io::Result<orchbee_client_server_protocol::ViewerId> {
    let primary = addressed.primary.as_ref().unwrap();
    let viewer = orchbee_client_server_protocol::ViewerId::parse(&"01".repeat(16)).unwrap();
    viewer_reply(
        stream,
        TerminalViewerAction::AttachBinding {
            site_id: orchbee_client_server_protocol::SiteId::parse(
                addressed.cache.site_id().as_str(),
            )
            .unwrap(),
            worker_id: orchbee_client_server_protocol::WorkerId::parse(
                primary.worker_id().as_str(),
            )
            .unwrap(),
            terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                primary.terminal_id().as_str(),
            )
            .unwrap(),
        },
        TerminalViewerOutcome::Attached {
            attachment_id: viewer.clone(),
            host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&"04".repeat(16)).unwrap(),
            host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                &"02".repeat(16),
            )
            .unwrap(),
        },
    )?;
    Ok(viewer)
}

#[test]
fn changed_first_root_at_start_is_no_effect_and_never_queries_the_replacement_repository() {
    use crate::workbench_client::state::worktree_launch::{Launcher, tests::sidebar_fixture};
    let canonical = fixture();
    let (origin, sidebar) = sidebar_fixture();
    let addressed = sidebar.addressed.clone();
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
        let viewer = attach_reply(&mut stream, &addressed)?;
        viewer_reply(
            &mut stream,
            TerminalViewerAction::ReadProcessInfo {
                attachment_id: viewer.clone(),
            },
            TerminalViewerOutcome::Record(
                process_record(Some("C:/changed-root"), false).map_err(io::Error::other)?,
            ),
        )?;
        viewer_reply(
            &mut stream,
            TerminalViewerAction::Detach {
                attachment_id: viewer,
            },
            TerminalViewerOutcome::Detached {},
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
fn unexpected_attach_reply_retires_the_auxiliary_client_without_borrowing_a_controller() {
    let canonical = fixture();
    let addressed = AddressedWorktreeSource::capture(
        &canonical,
        &SidebarWorktreeTarget::Project(canonical.navigation.projects[0].project_id.clone()),
    )
    .unwrap();
    let expected = addressed.clone();
    let (released_tx, released_rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        snapshot_reply(&mut stream, canonical.clone())?;
        let primary = expected.primary.as_ref().unwrap();
        viewer_reply(
            &mut stream,
            TerminalViewerAction::AttachBinding {
                site_id: orchbee_client_server_protocol::SiteId::parse(
                    expected.cache.site_id().as_str(),
                )
                .unwrap(),
                worker_id: orchbee_client_server_protocol::WorkerId::parse(
                    primary.worker_id().as_str(),
                )
                .unwrap(),
                terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                    primary.terminal_id().as_str(),
                )
                .unwrap(),
            },
            TerminalViewerOutcome::Detached {},
        )?;
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "unknown attach ownership ends this Client Connection before any further request"
        );
        released_tx.send(()).unwrap();
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::SidebarQuery {
        episode: 10,
        source: Box::new(addressed)
    })));
    assert!(matches!(
        take(&lane),
        Reply::SidebarQuery {
            episode: 10,
            result: Err(WorktreeNotice::SourceUnavailable)
        }
    ));
    released_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(
        lane.idle(),
        "Client retirement does not turn a read-only failure into a lane panic"
    );
    assert!(lane.close());
    server.finish().unwrap();
}

#[test]
fn addressed_lane_samples_first_root_around_git_and_always_releases_its_own_viewer() {
    for fault in 0..4 {
        let canonical = fixture();
        let addressed = AddressedWorktreeSource::capture(
            &canonical,
            &SidebarWorktreeTarget::Project(canonical.navigation.projects[0].project_id.clone()),
        )
        .unwrap();
        let expected = addressed.clone();
        let server_snapshot = canonical.clone();
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            handshake(&mut stream, index as u64 + 1)?;
            if index == 0 {
                return Ok(());
            }
            snapshot_reply(&mut stream, server_snapshot.clone())?;
            let primary = expected.primary.as_ref().unwrap();
            let viewer = orchbee_client_server_protocol::ViewerId::parse(&"01".repeat(16)).unwrap();
            viewer_reply(
                &mut stream,
                TerminalViewerAction::AttachBinding {
                    site_id: orchbee_client_server_protocol::SiteId::parse(
                        expected.cache.site_id().as_str(),
                    )
                    .unwrap(),
                    worker_id: orchbee_client_server_protocol::WorkerId::parse(
                        primary.worker_id().as_str(),
                    )
                    .unwrap(),
                    terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                        primary.terminal_id().as_str(),
                    )
                    .unwrap(),
                },
                TerminalViewerOutcome::Attached {
                    attachment_id: viewer.clone(),
                    host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&"04".repeat(16))
                        .unwrap(),
                    host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                        &"02".repeat(16),
                    )
                    .unwrap(),
                },
            )?;
            viewer_reply(
                &mut stream,
                TerminalViewerAction::ReadProcessInfo {
                    attachment_id: viewer.clone(),
                },
                TerminalViewerOutcome::Record(
                    process_record(
                        Some(if fault == 3 { "relative" } else { "C:/repo" }),
                        fault == 2,
                    )
                    .map_err(io::Error::other)?,
                ),
            )?;
            if fault < 2 {
                let WorkbenchRequest::WorktreeQuery(request) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    panic!("Git query after admitted root sample");
                };
                assert_eq!(
                    request.source,
                    WorktreeSource::Directory {
                        path: WorkbenchDirectoryPath::parse("C:/repo").unwrap()
                    }
                );
                assert_ne!(expected.cache.directory().unwrap().path.as_str(), "C:/repo");
                let discovery = discovery();
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id,
                        outcome: WorkbenchWorktreeQueryOutcome::Found {
                            site_id: expected.cache.site_id().clone(),
                            workbench_revision: expected.cache.revision(),
                            worker_id: primary.worker_id().clone(),
                            source: discovery.source,
                            entries: discovery.entries,
                        },
                    }),
                )
                .map_err(io::Error::other)?;
                snapshot_reply(&mut stream, server_snapshot.clone())?;
                viewer_reply(
                    &mut stream,
                    TerminalViewerAction::ReadProcessInfo {
                        attachment_id: viewer.clone(),
                    },
                    TerminalViewerOutcome::Record(
                        process_record(
                            Some(if fault == 1 {
                                "C:/changed-root"
                            } else {
                                "C:/repo"
                            }),
                            false,
                        )
                        .map_err(io::Error::other)?,
                    ),
                )?;
            }
            viewer_reply(
                &mut stream,
                TerminalViewerAction::Detach {
                    attachment_id: viewer,
                },
                TerminalViewerOutcome::Detached {},
            )?;
            assert_eq!(
                stream.read(&mut [0])?,
                0,
                "no Control, process start, fallback query or retained Viewer"
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
        match fault {
            0 => {
                let result = result.unwrap();
                assert!(result.valid(&canonical));
                assert_eq!(
                    result.sample,
                    crate::workbench_client::primary_directory::RootDirectory::Available(Some(
                        WorkbenchDirectoryPath::parse("C:/repo").unwrap()
                    ))
                );
            }
            1 => assert_eq!(result.unwrap_err(), WorktreeNotice::SourceChanged),
            _ => assert_eq!(result.unwrap_err(), WorktreeNotice::SourceUnavailable),
        }
        assert!(lane.close());
        server.finish().unwrap();
    }
}

#[test]
fn non_git_first_project_member_never_queries_a_later_repository_or_starts_an_operation() {
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
                    reason: WorktreeQueryFailure::NotFound {},
                },
            }),
        )
        .map_err(io::Error::other)?;
        let mut byte = [0];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "no fallback query, Viewer, or Start after a non-Git first member"
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
            result: Err(WorktreeNotice::ListingUnavailable)
        }
    ));
    assert!(lane.close());
    server.finish().unwrap();
}

#[test]
fn source_membership_drift_before_sampling_refuses_without_query_or_viewer() {
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
            "a changed first member retires before any Viewer or Git query"
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
