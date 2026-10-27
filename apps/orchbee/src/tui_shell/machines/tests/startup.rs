//! Actual native admission and candidate bootstrap through the machine UI owner.
use super::super::*;
use super::{dimensions, profile, setup};
use crate::{
    local_client::test_support::{multiplex_peer::Peer, server::FakeServer},
    workbench_client::{ClientContextLocation, ClientContextTarget},
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
                launch_failure: None,
            },
        },
    };
    Snapshot {
        protocol_version: ProtocolVersion,
        session_id: SessionId::parse(&id(6)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: {
            let fixture_context_id = standalone.clone();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspace_order: std::iter::once(fixture_workspace_id.clone())
                    .chain(vec![WorkbenchWorkspaceId::parse(&id(8)).unwrap()])
                    .collect(),
                projects: vec![Project {
                    project_id: WorkbenchProjectId::parse(&id(7)).unwrap(),
                    label: "project".into(),
                }],
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(vec![Workspace {
                    project_id: Some(WorkbenchProjectId::parse(&id(7)).unwrap()),
                    workspace_id: WorkbenchWorkspaceId::parse(&id(8)).unwrap(),
                    label: "workspace".into(),
                    primary_directory_id: None,
                    terminal_context_id: workspace.clone(),
                }])
                .collect(),
            }
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
                            launch_failure: None,
                        },
                    };
                    vec![other]
                },
                groups: Vec::new(),
            },
        ],
    }
}
fn target(snapshot: &Snapshot) -> ClientContextTarget {
    let workspace = &snapshot.navigation.workspaces[1];
    ClientContextTarget {
        session_id: snapshot.session_id.clone(),
        location: ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
    }
}
fn peer(snapshot: Snapshot) -> io::Result<FakeServer> {
    FakeServer::start(move |stream| {
        let mut stream = Peer::primary_lane(stream);
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
        stream.answer_probes();
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "empty/unbound or refused startup must not create/attach another terminal"
        );
        Ok(())
    })
}
fn attempt(server: &FakeServer, context: Option<ClientContextTarget>) -> Attempt {
    let endpoint = server.endpoint().to_path_buf();
    Attempt {
        profile: Some(profile()),
        context: context.map(Into::into),
        cancel: Arc::new(OwnedCancellation::new().unwrap()),
        task: Some(thread::spawn(move || {
            LocalClient::connect_before(
                &endpoint,
                "test-cohort",
                Instant::now() + Duration::from_secs(3),
            )
            .map(PreparedSelection::Connection)
            .map_err(io::Error::other)
        })),
    }
}
#[test]
fn machine_candidate_supplies_startup_deadline_and_commits_drawable_standalone() -> io::Result<()> {
    let server = peer(snapshot(false))?;
    let (mut manager, mut view, mut surface) = setup();
    view.machines.show();
    manager.selection_scope = view.machines.selection_scope();
    manager.attempt = Some(attempt(&server, None));
    let deadline = Instant::now() + Duration::from_secs(3);
    while view.machines.current == Choice::Local {
        manager.tick(
            &mut surface,
            &mut view,
            dimensions(),
            Instant::now(),
            &mut |_| {},
        );
        assert!(view.machines.error.is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        active_context(surface.active()).unwrap().location,
        ClientContextLocation::Workspace { .. }
    ));
    drop(manager);
    drop(surface);
    server.finish()
}
#[test]
fn machine_workspace_candidate_starts_exact_context_including_empty_without_opening_standalone()
-> io::Result<()> {
    for (empty, direct) in [(true, false), (false, false), (true, true), (false, true)] {
        let snapshot = snapshot(empty);
        let target = target(&snapshot);
        let server = peer(snapshot)?;
        let (mut manager, mut view, mut surface) = setup();
        if direct {
            view.machines.observe_agent_sidebar(true);
            view.machines.observe(&[(
                profile().id,
                crate::machine_discovery::Observation::Online(crate::machine_discovery::Overview {
                    session: target.session_id.clone(),
                    revision: WorkbenchRevision::new(1),
                    projects: 1,
                    workspaces: 1,
                    tabs: usize::from(!empty),
                    labels: Vec::new(),
                    contexts: vec![Context {
                        target: target.clone(),
                        project_id: None,
                        project: None,
                        workspace: Some("workspace".into()),
                    }]
                    .into(),
                }),
            )]);
            assert!(
                view.machines
                    .activate_sidebar_workspace(&crate::machine_picker::Selection {
                        machine: Choice::Remote(profile().id),
                        context: Some(target.clone().into()),
                    })
            );
            view.machines.request.take();
        } else {
            view.machines.show();
        }
        manager.selection_scope = view.machines.selection_scope();
        manager.attempt = Some(attempt(&server, Some(target.clone())));
        let deadline = Instant::now() + Duration::from_secs(3);
        while view.machines.current == Choice::Local {
            manager.tick(
                &mut surface,
                &mut view,
                dimensions(),
                Instant::now(),
                &mut |_| {},
            );
            assert!(view.machines.error.is_none());
            assert_eq!(
                view.machines.open,
                !direct && view.machines.current == Choice::Local
            );
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(active_context(surface.active()), Some(target.clone()));
        assert_eq!(manager.cached[0].choice(), Choice::Local);
        assert!(!view.machines.open);
        assert!(surface.active().reusable_machine());
        assert!(manager.restore(
            &Choice::Local,
            &mut surface,
            &mut view,
            dimensions(),
            true,
            &mut |_| {},
        ));
        assert!(manager.restore(
            &Choice::Remote(profile().id),
            &mut surface,
            &mut view,
            dimensions(),
            true,
            &mut |_| {},
        ));
        assert_eq!(active_context(surface.active()), Some(target));
        drop(manager);
        drop(surface);
        server.finish()?;
    }
    Ok(())
}
#[test]
fn machine_workspace_missing_session_or_membership_refuses_without_fallback() -> io::Result<()> {
    for changed_session in [true, false] {
        let snapshot = snapshot(false);
        let mut target = target(&snapshot);
        if changed_session {
            target.session_id = SessionId::parse(&id(99)).unwrap();
        } else if let ClientContextLocation::Workspace { workspace_id, .. } = &mut target.location {
            *workspace_id = WorkbenchWorkspaceId::parse(&id(99)).unwrap();
        }
        let server = peer(snapshot)?;
        let (mut manager, mut view, mut surface) = setup();
        view.machines.show();
        manager.selection_scope = view.machines.selection_scope();
        manager.attempt = Some(attempt(&server, Some(target)));
        let deadline = Instant::now() + Duration::from_secs(3);
        while view.machines.error.is_none() {
            assert!(
                !manager
                    .tick(
                        &mut surface,
                        &mut view,
                        dimensions(),
                        Instant::now(),
                        &mut |_| {},
                    )
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
    use orchbee_agent_observation_protocol::{
        AGENT_MAGIC, AgentChangesRequest, AgentRequest, read_agent_request,
    };

    #[derive(Debug, PartialEq, Eq)]
    enum DecodedAttention {
        Semantic,
        Physical,
    }

    let snapshot = snapshot(true);
    let destination = target(&snapshot);
    let retained = Arc::new(std::sync::Mutex::new(Vec::with_capacity(1)));
    let handler_retained = Arc::clone(&retained);
    let (decoded_attention, attention_receipts) = std::sync::mpsc::sync_channel(2);
    // Origin, destination, Agent snapshot, Agent watch, and both native attention lanes.
    let server = FakeServer::start_optional_connections(6, move |index, mut stream| {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        assert!(matches!(
            read_message(&mut stream)
                .inspect_err(|error| {
                    eprintln!("machine-local-workspace lane={index} stage=hello-read: {error:?}");
                })
                .map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::new(1).unwrap(),
            },
        )
        .inspect_err(|error| {
            eprintln!("machine-local-workspace lane={index} stage=accepted-write: {error:?}");
        })
        .map_err(io::Error::other)?;
        if index == 0 {
            handler_retained
                .lock()
                .map_err(|_| io::Error::other("retained origin lock poisoned"))?
                .push(stream);
            return Ok(());
        }
        // Every later connection is one multiplexed carrier. Its logical streams are dispatched
        // by request; observations stay pending until teardown closes the carrier.
        loop {
            let (stream_id, mode, request) = match read_multiplex_message(&mut stream) {
                Ok(MultiplexMessage::StreamOpen {
                    stream_id,
                    mode,
                    request,
                    ..
                }) => (stream_id, mode, request),
                Ok(
                    MultiplexMessage::LaneOpen { .. }
                    | MultiplexMessage::LaneClose { .. }
                    | MultiplexMessage::StreamClose { .. },
                ) => continue,
                Ok(MultiplexMessage::Disconnect { .. }) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                other => {
                    eprintln!("machine-local-workspace lane={index} stage=carrier-read: {other:?}");
                    return Err(io::Error::other("unexpected carrier message"));
                }
            };
            if request.starts_with(&AGENT_MAGIC) {
                let request = read_agent_request(&mut io::Cursor::new(&request))
                    .inspect_err(|error| {
                        eprintln!(
                            "machine-local-workspace lane={index} stage=attention-read: {error:?}"
                        );
                    })
                    .map_err(io::Error::other)?;
                assert!(matches!(
                    request,
                    AgentRequest::Changes(AgentChangesRequest::Read { cursor: None, .. })
                ));
                decoded_attention
                    .send(DecodedAttention::Semantic)
                    .map_err(io::Error::other)?;
                continue;
            }
            let request = read_workbench_request(&mut io::Cursor::new(&request))
                .inspect_err(|error| {
                    eprintln!("machine-local-workspace lane={index} stage=request-read: {error:?}");
                })
                .map_err(io::Error::other)?;
            match request {
                WorkbenchRequest::SnapshotRead(read) => {
                    let mut record = Vec::new();
                    write_workbench_response(
                        &mut record,
                        &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                            protocol_version: ProtocolVersion,
                            request_id: read.request_id,
                            outcome: WorkbenchSnapshotOutcome::Available {
                                snapshot: snapshot.clone(),
                            },
                        }),
                    )
                    .map_err(io::Error::other)?;
                    write_multiplex_message(
                        &mut stream,
                        &MultiplexMessage::StreamFrame {
                            stream_id,
                            sequence: NonZeroU64::MIN,
                            kind: if mode == StreamMode::Request {
                                StreamFrameKind::Result
                            } else {
                                StreamFrameKind::Snapshot
                            },
                            record,
                        },
                    )
                    .inspect_err(|error| {
                        eprintln!(
                            "machine-local-workspace lane={index} stage=snapshot-write: {error:?}"
                        );
                    })?;
                }
                // A periodic refresh finds the destination unchanged.
                WorkbenchRequest::PublicationRead(read) => {
                    let mut record = Vec::new();
                    write_workbench_response(
                        &mut record,
                        &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                            protocol_version: ProtocolVersion,
                            request_id: read.request_id,
                            outcome: WorkbenchPublicationOutcome::Current {
                                revision: snapshot.revision,
                            },
                        }),
                    )
                    .map_err(io::Error::other)?;
                    write_multiplex_message(
                        &mut stream,
                        &MultiplexMessage::StreamFrame {
                            stream_id,
                            sequence: NonZeroU64::MIN,
                            kind: StreamFrameKind::Result,
                            record,
                        },
                    )?;
                }
                // Native physical attention reads the same Session's presentation states.
                WorkbenchRequest::PanelAgent(PanelAgentRequest {
                    session_id,
                    action: PanelAgentAction::States { presentation_ids },
                    ..
                }) => {
                    assert_eq!(session_id, snapshot.session_id);
                    assert_eq!(
                        presentation_ids,
                        vec![TerminalPresentationId::parse(&id(5)).unwrap()]
                    );
                    decoded_attention
                        .send(DecodedAttention::Physical)
                        .map_err(io::Error::other)?;
                }
                // The Windows Tab strip observes the attached machine's Agents while the Sidebar
                // is hidden.
                WorkbenchRequest::AgentView(AgentViewRequest {
                    action: AgentViewAction::Read {},
                    ..
                }) => {}
                WorkbenchRequest::Events(SemanticEventsRequest {
                    session_id,
                    action: SemanticEventsAction::Subscribe { .. },
                    ..
                }) => assert_eq!(session_id, snapshot.session_id),
                WorkbenchRequest::WorkspaceMetadata(WorkspaceMetadataRequest {
                    action: WorkspaceMetadataAction::Read { workspace_ids },
                    ..
                }) => assert_eq!(
                    workspace_ids,
                    vec![snapshot.navigation.workspaces[1].workspace_id.clone()]
                ),
                // Settings and Panel metadata are read-only observations of the destination.
                WorkbenchRequest::Settings(SettingsRequest {
                    session_id,
                    operation: SettingsOperation::Open {},
                    ..
                }) => assert_eq!(session_id, snapshot.session_id),
                WorkbenchRequest::PanelMetadata(PanelMetadataRequest {
                    action:
                        PanelMetadataAction::Read { .. } | PanelMetadataAction::ReadPresentations { .. },
                    ..
                }) => {}
                other => panic!("unexpected request on the destination carrier: {other:?}"),
            }
        }
    })?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let client = LocalClient::connect_before(server.endpoint(), "test-cohort", deadline)
        .inspect_err(|error| {
            eprintln!("machine-local-workspace lane=0 stage=origin-connect: {error:?}");
        })
        .map_err(io::Error::other)?;
    // An admitted local origin is sufficient to reopen a native lane. Keep the
    // source inert; the replacement native UI owns both attention readers,
    // independently of sidebar visibility. Preserve the peer and original
    // connection until every consumer finishes.
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
    assert!(!view.machines.wants_agents());
    assert!(!view.machines.wants_spaces());
    view.machines.request = Some(crate::machine_picker::Selection {
        machine: Choice::Local,
        context: Some(destination.clone().into()),
    });
    assert!(
        !manager
            .tick(
                &mut surface,
                &mut view,
                dimensions(),
                Instant::now(),
                &mut |_| {},
            )
            .switched
    );
    assert_eq!(active_context(surface.active()), None);
    loop {
        let change = manager.tick(
            &mut surface,
            &mut view,
            dimensions(),
            Instant::now(),
            &mut |_| {},
        );
        if change.switched {
            break;
        }
        let now = Instant::now();
        let error = view.machines.error;
        let candidate = manager.candidate.as_ref().map(|candidate| {
            (
                candidate.session.status(),
                candidate.deadline.saturating_duration_since(now),
            )
        });
        assert!(
            error.is_none(),
            "machine error={error:?}; test budget={:?}; phase queued={} attempt={} candidate={candidate:?}; active={:?}; destination={destination:?}",
            deadline.saturating_duration_since(now),
            manager.queued.is_some(),
            manager.attempt.is_some(),
            active_context(surface.active()),
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(active_context(surface.active()), Some(destination));
    assert_eq!(view.machines.current, Choice::Local);
    assert!(manager.cached.is_empty());
    eprintln!("machine-local-workspace stage=switched-before-teardown");
    // Each receipt proves a complete decoded request whose only remaining peer
    // operation is an EOF read. It does not assert that the kernel is blocking.
    let first_attention = attention_receipts
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(io::Error::other)?;
    let second_attention = attention_receipts
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(io::Error::other)?;
    assert_ne!(
        first_attention, second_attention,
        "each native attention request kind must be decoded exactly once"
    );
    drop(manager);
    drop(surface);
    drop(client);
    retained
        .lock()
        .map_err(|_| io::Error::other("retained origin lock poisoned"))?
        .clear();
    server.finish().inspect_err(|error| {
        eprintln!("machine-local-workspace stage=finish: {error:?}");
    })
}

#[test]
fn machine_workspace_escape_cancels_pending_native_read_and_retains_current_surface()
-> io::Result<()> {
    let (sent, received) = std::sync::mpsc::sync_channel(1);
    let server = FakeServer::start(move |stream| {
        let mut stream = Peer::primary_lane(stream);
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
    manager.selection_scope = view.machines.selection_scope();
    manager.attempt = Some(attempt(&server, Some(target(&snapshot(true)))));
    let deadline = Instant::now() + Duration::from_secs(3);
    while received.try_recv().is_err() {
        assert!(
            !manager
                .tick(
                    &mut surface,
                    &mut view,
                    dimensions(),
                    Instant::now(),
                    &mut |_| {},
                )
                .switched
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    view.machines.open = false;
    assert!(
        !manager
            .tick(
                &mut surface,
                &mut view,
                dimensions(),
                Instant::now(),
                &mut |_| {},
            )
            .switched
    );
    assert!(manager.candidate.is_none());
    assert_eq!(view.machines.current, Choice::Local);
    drop(manager);
    drop(surface);
    server.finish()
}
