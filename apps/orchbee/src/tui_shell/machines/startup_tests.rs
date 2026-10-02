//! Actual native admission and candidate bootstrap through the machine UI owner.
use super::{
    tests::{dimensions, profile, setup},
    *,
};
use crate::{
    local_client::test_server::FakeServer,
    terminal_session::{NativeContextLocation, NativeContextTarget},
};
use orchbee_client_server_protocol::{
    ClientMessage, OpenOutcome, RequestId, ServerMessage, TerminalOpenResult, read_client_message,
    write_server_message,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::*;
use std::{io::Read, num::NonZeroU64};

fn id(n: u8) -> String {
    format!("{n:032x}")
}
fn snapshot(empty: bool) -> Snapshot {
    let standalone = WorkbenchTerminalContextId::parse(&id(1)).unwrap();
    let workspace = WorkbenchTerminalContextId::parse(&id(2)).unwrap();
    let tab = Tab {
        automatic_label: false,
        group_id: None,
        label: "destination".into(),
        pinned: false,
        tab_id: WorkbenchTabId::parse(&id(3)).unwrap(),
        root: PanelNode::Panel {
            right_click: Default::default(),
            child_presentations: Vec::new(),
            panel_id: WorkbenchPanelId::parse(&id(4)).unwrap(),
            primary_presentation: Presentation {
                custom_label: None,
                label: "unbound".into(),
                presentation_id: TerminalPresentationId::parse(&id(5)).unwrap(),
                binding: None,
            },
        },
    };
    Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(6)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            workspace_order: Vec::new(),
            standalone_terminal_context_id: standalone.clone(),
            projects: vec![Project {
                project_id: WorkbenchProjectId::parse(&id(7)).unwrap(),
                directories: Vec::new(),
                label: "project".into(),
                workspaces: vec![Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&id(8)).unwrap(),
                    label: "workspace".into(),
                    primary_directory_id: None,
                    terminal_context_id: workspace.clone(),
                }],
            }],
        },
        terminal_contexts: vec![
            TerminalContext {
                terminal_context_id: standalone,
                tabs: vec![tab.clone()],
                groups: Vec::new(),
            },
            TerminalContext {
                terminal_context_id: workspace,
                tabs: if empty {
                    Vec::new()
                } else {
                    let mut other = tab;
                    other.tab_id = WorkbenchTabId::parse(&id(9)).unwrap();
                    other.root = PanelNode::Panel {
                        right_click: Default::default(),
                        child_presentations: Vec::new(),
                        panel_id: WorkbenchPanelId::parse(&id(10)).unwrap(),
                        primary_presentation: Presentation {
                            custom_label: None,
                            label: "workspace".into(),
                            presentation_id: TerminalPresentationId::parse(&id(11)).unwrap(),
                            binding: None,
                        },
                    };
                    vec![other]
                },
                groups: Vec::new(),
            },
        ],
    }
}
fn target(snapshot: &Snapshot) -> NativeContextTarget {
    let project = &snapshot.navigation.projects[0];
    let workspace = &project.workspaces[0];
    NativeContextTarget {
        site_id: snapshot.site_id.clone(),
        location: NativeContextLocation::Workspace {
            project_id: project.project_id.clone(),
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
    }
}
fn peer(snapshot: Snapshot, standalone: bool) -> io::Result<FakeServer> {
    FakeServer::start(move |mut stream| {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if standalone {
            let ClientMessage::TerminalOpen(open) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                panic!("standalone startup must open its own context");
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalOpenResult(TerminalOpenResult::new(
                    open.request_id().clone(),
                    OpenOutcome::Opened {
                        terminal_create_request_id: RequestId::parse(&id(12)).unwrap(),
                    },
                )),
            )
            .map_err(io::Error::other)?;
        }
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("context startup must read before terminal effects");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "empty/unbound or refused startup must not create/attach another terminal"
        );
        Ok(())
    })
}
fn attempt(server: &FakeServer, context: Option<NativeContextTarget>) -> Attempt {
    let endpoint = server.endpoint().to_path_buf();
    Attempt {
        profile: Some(profile()),
        context: context.map(Into::into),
        cancel: Arc::new(AtomicU8::new(0)),
        task: Some(thread::spawn(move || {
            LocalClient::connect_before(
                &endpoint,
                "test-cohort",
                Instant::now() + Duration::from_secs(3),
            )
            .map_err(io::Error::other)
        })),
    }
}
#[test]
fn machine_candidate_supplies_startup_deadline_and_commits_drawable_standalone() -> io::Result<()> {
    let server = peer(snapshot(false), true)?;
    let (mut manager, mut view, mut surface) = setup();
    view.machines.show();
    manager.attempt = Some(attempt(&server, None));
    let deadline = Instant::now() + Duration::from_secs(3);
    while view.machines.current == Choice::Local {
        manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        assert!(view.machines.error.is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        active_context(surface.active()).unwrap().location,
        NativeContextLocation::Standalone(_)
    ));
    drop(manager);
    drop(surface);
    server.finish()
}
#[test]
fn machine_workspace_candidate_starts_exact_context_including_empty_without_opening_standalone()
-> io::Result<()> {
    for empty in [true, false] {
        let snapshot = snapshot(empty);
        let target = target(&snapshot);
        let server = peer(snapshot, false)?;
        let (mut manager, mut view, mut surface) = setup();
        view.machines.show();
        manager.attempt = Some(attempt(&server, Some(target.clone())));
        let deadline = Instant::now() + Duration::from_secs(3);
        while view.machines.current == Choice::Local {
            manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
            assert!(view.machines.error.is_none());
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(active_context(surface.active()), Some(target.clone()));
        assert_eq!(manager.cached[0].choice(), Choice::Local);
        assert!(!view.machines.open);
        assert!(surface.active().reusable_machine());
        assert!(manager.restore(&Choice::Local, &mut surface, &mut view, dimensions(), true));
        assert!(manager.restore(
            &Choice::Remote(profile().id),
            &mut surface,
            &mut view,
            dimensions(),
            true
        ));
        assert_eq!(active_context(surface.active()), Some(target));
        drop(manager);
        drop(surface);
        server.finish()?;
    }
    Ok(())
}
#[test]
fn machine_workspace_missing_site_or_membership_refuses_without_fallback() -> io::Result<()> {
    for changed_site in [true, false] {
        let snapshot = snapshot(false);
        let mut target = target(&snapshot);
        if changed_site {
            target.site_id = SiteId::parse(&id(99)).unwrap();
        } else if let NativeContextLocation::Workspace { workspace_id, .. } = &mut target.location {
            *workspace_id = WorkbenchWorkspaceId::parse(&id(99)).unwrap();
        }
        let server = peer(snapshot, false)?;
        let (mut manager, mut view, mut surface) = setup();
        view.machines.show();
        manager.attempt = Some(attempt(&server, Some(target)));
        let deadline = Instant::now() + Duration::from_secs(3);
        while view.machines.error.is_none() {
            assert!(
                !manager
                    .tick(&mut surface, &mut view, dimensions(), Instant::now())
                    .switched
            );
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(view.machines.current, Choice::Local);
        assert!(manager.cached.is_empty());
        drop(manager);
        drop(surface);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn machine_local_workspace_selection_reuses_local_origin_and_replaces_only_after_drawable()
-> io::Result<()> {
    let snapshot = snapshot(true);
    let destination = target(&snapshot);
    let mut retained = Vec::with_capacity(1);
    let server = FakeServer::start_optional_connections(4, move |index, mut stream| {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        if index == 0 {
            retained.push(stream);
            return Ok(());
        }
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("context lane must begin with a snapshot");
        };
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            }),
        )
        .map_err(io::Error::other)?;
        // Agent and Workspace sidebars have independent read consumers.
        // Both and the destination start with a snapshot. Refuse each
        // observation without affecting destination admission or treating
        // an unavailable read as an empty successful checkpoint.
        match read_workbench_request(&mut stream) {
            Ok(WorkbenchRequest::AgentView(AgentViewRequest {
                action: AgentViewAction::Read {},
                ..
            })) => {}
            Ok(WorkbenchRequest::WorkspaceMetadata(WorkspaceMetadataRequest {
                action: WorkspaceMetadataAction::Read { workspace_ids },
                ..
            })) => assert_eq!(
                workspace_ids,
                vec![
                    snapshot.navigation.projects[0].workspaces[0]
                        .workspace_id
                        .clone()
                ]
            ),
            Err(WorkbenchWireError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {}
            other => panic!("unexpected post-snapshot request: {other:?}"),
        }
        Ok(())
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let client = LocalClient::connect_before(server.endpoint(), "test-cohort", deadline)
        .map_err(io::Error::other)?;
    // An admitted local origin is sufficient to reopen a native lane. Keep the
    // source inert so native attention observers do not open unrelated lanes.
    // Origin and destination are required; the two sidebar consumers may
    // cancel before admission when selection replaces their source. Keep the
    // peer and original connection alive until the actual consumers finish.
    struct SourceSurface(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    );
    impl crate::terminal_input::TerminalInputPort for SourceSurface {
        fn forward(
            &mut self,
            _bytes: &[u8],
        ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
            Ok(())
        }
    }
    impl TerminalSurface for SourceSurface {
        fn machine_source(
            &self,
        ) -> Option<(
            crate::local_client::LocalClientOrigin,
            crate::local_client::LocalOperationClock,
        )> {
            Some((self.0.clone(), self.1.clone()))
        }
    }
    let (mut manager, mut view, mut surface) = setup();
    drop(surface.replace(Box::new(SourceSurface(
        client.origin(),
        client.operation_clock(),
    ))));
    view.machines.show();
    view.machines.request = Some(crate::machine_picker::Selection {
        machine: Choice::Local,
        context: Some(destination.clone().into()),
    });
    assert!(
        !manager
            .tick(&mut surface, &mut view, dimensions(), Instant::now())
            .switched
    );
    assert_eq!(active_context(surface.active()), None);
    loop {
        let change = manager.tick(&mut surface, &mut view, dimensions(), Instant::now());
        if change.switched {
            break;
        }
        assert!(view.machines.error.is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(active_context(surface.active()), Some(destination));
    assert_eq!(view.machines.current, Choice::Local);
    assert!(manager.cached.is_empty());
    drop(manager);
    drop(surface);
    server.finish()
}

#[test]
fn machine_workspace_escape_cancels_pending_native_read_and_retains_current_surface()
-> io::Result<()> {
    let (sent, received) = std::sync::mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut stream| {
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .map_err(io::Error::other)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        sent.send(()).map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })?;
    let (mut manager, mut view, mut surface) = setup();
    view.machines.show();
    manager.attempt = Some(attempt(&server, Some(target(&snapshot(true)))));
    let deadline = Instant::now() + Duration::from_secs(3);
    while received.try_recv().is_err() {
        assert!(
            !manager
                .tick(&mut surface, &mut view, dimensions(), Instant::now())
                .switched
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    view.machines.open = false;
    assert!(
        !manager
            .tick(&mut surface, &mut view, dimensions(), Instant::now())
            .switched
    );
    assert!(manager.candidate.is_none());
    assert_eq!(view.machines.current, Choice::Local);
    drop(manager);
    drop(surface);
    server.finish()
}
