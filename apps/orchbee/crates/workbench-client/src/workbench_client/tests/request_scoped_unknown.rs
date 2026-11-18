use super::*;
use crate::client_view::TerminalQueryColors;
use crate::terminal_input::{
    RenameAction, SetupAction, SurfaceAction, SurfaceInput, TerminalInputPort,
};
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_render::render_terminal_shell;
use crate::tui_theme::Theme;
use crate::workbench_client::input::RenameEdit;
use crate::workbench_state::workspace_close::WorkspaceCloseStatus;
use crate::workbench_state::workspace_setup::{LocationLabelStatus, SetupEdit, SetupStatus};
use orchbee_workbench_protocol::{
    Snapshot, TerminalContext, WorkbenchTerminalContextId, WorkbenchWorkspaceId, Workspace,
};
use ratatui::{Terminal, backend::TestBackend};
use std::io::Read;
use std::sync::{Arc, Mutex, mpsc};

#[derive(Clone, Copy)]
enum InitUnknownCut {
    Acquire,
    HostColors,
    Resize,
}

fn draw_workbench(
    workbench_client: &mut WorkbenchClient,
    view: &ClientView,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    let mut receipt = DrawReceipt::default();
    let mut hits = HitMapBuilder::new();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            view,
            Locale::EnUs,
            Some(workbench_client.view()),
            &Theme::terminal_native(),
            &mut hits,
        );
    })?;
    workbench_client.after_close_draw(receipt);
    Ok(terminal
        .backend()
        .buffer()
        .content
        .chunks(120)
        .map(|row| {
            let mut line: String = row.iter().map(|cell| cell.symbol()).collect();
            line.push('\n');
            line
        })
        .collect())
}

fn start_multiplexed(
    server: &FakeServer,
    geometry: Geometry,
) -> Result<WorkbenchClient, Box<dyn std::error::Error>> {
    let client = native_client(server)?.into_multiplexed()?;
    let (workbench_client, _) = workbench_client_with_identity(client, geometry)?;
    Ok(workbench_client)
}

fn spin_controlling_settled(workbench_client: &mut WorkbenchClient) {
    assert!(spin_with_draw(workbench_client, |client| {
        client.status() == TerminalStatus::Controlling && client.view().refresh_status().is_none()
    }));
}

fn respond_resized(
    stream: &mut UnixStream,
    geometry: Geometry,
    expected_sequence: u64,
) -> io::Result<()> {
    let ClientMessage::TerminalControl(request) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected Resize"));
    };
    assert!(matches!(
        request.action(),
        TerminalControlAction::Resize {
            attachment_id,
            geometry: actual,
            sequence,
            ..
        } if attachment_id == &binding_viewer_id()?
            && *actual == geometry
            && sequence.get() == expected_sequence
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
            request.request_id().clone(),
            TerminalControlOutcome::Resized { geometry },
        )),
    )
    .map_err(io::Error::other)
}

fn serve_controlling_polls(
    stream: &mut UnixStream,
    geometry: Geometry,
    snapshot: &Snapshot,
    commands: &Arc<Mutex<Vec<WorkbenchAction>>>,
    inputs: &Arc<Mutex<Vec<u8>>>,
    fail_first_command: bool,
) -> io::Result<()> {
    let ClientMessage::TerminalViewer(poll) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected initial Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: binding_viewer_id()?,
        }
    );
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            poll.request_id().clone(),
            TerminalViewerOutcome::Record(
                full_frame_record(1, 1, geometry)
                    .map_err(|error| io::Error::other(error.to_string()))?,
            ),
        )),
    )
    .map_err(io::Error::other)?;
    let ClientMessage::TerminalViewer(feedback) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected FrameApplied"));
    };
    assert!(matches!(
        feedback.action(),
        TerminalViewerAction::FrameApplied { .. }
    ));
    write_server_message(
        stream,
        &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
            feedback.request_id().clone(),
            TerminalViewerOutcome::FeedbackAccepted {},
        )),
    )
    .map_err(io::Error::other)?;

    let mut failed_once = false;
    loop {
        let mut magic = [0; 4];
        if stream.read_exact(&mut magic).is_err() {
            return Ok(());
        }
        let mut reader = io::Cursor::new(magic).chain(&mut *stream);
        if magic == *b"OBWP" {
            let request = read_workbench_request(&mut reader).map_err(io::Error::other)?;
            let action = match request {
                WorkbenchRequest::Command(command) => command.action,
                WorkbenchRequest::SnapshotRead(read_req) => {
                    write_workbench_response(
                        stream,
                        &WorkbenchResponse::Snapshot(
                            orchbee_workbench_protocol::WorkbenchSnapshotResult {
                                protocol_version: ProtocolVersion,
                                request_id: read_req.request_id,
                                outcome: WorkbenchSnapshotOutcome::Available {
                                    snapshot: snapshot.clone(),
                                },
                            },
                        ),
                    )
                    .map_err(io::Error::other)?;
                    continue;
                }
                WorkbenchRequest::PublicationRead(pub_read) => {
                    write_workbench_response(
                        stream,
                        &WorkbenchResponse::Publication(
                            orchbee_workbench_protocol::WorkbenchPublicationResult {
                                protocol_version: ProtocolVersion,
                                request_id: pub_read.request_id,
                                outcome: orchbee_workbench_protocol::WorkbenchPublicationOutcome::Current {
                                    revision: pub_read.known_revision,
                                },
                            },
                        ),
                    )
                    .map_err(io::Error::other)?;
                    continue;
                }
                other => {
                    return Err(io::Error::other(format!(
                        "unexpected workbench request: {other:?}"
                    )));
                }
            };
            commands.lock().unwrap().push(action);
            if fail_first_command && !failed_once {
                failed_once = true;
                stream.outcome_unknown()?;
            }
            continue;
        }
        assert_eq!(magic, *b"OBCS");
        match read_client_message(&mut reader).map_err(io::Error::other)? {
            ClientMessage::TerminalViewer(request)
                if matches!(
                    request.action(),
                    TerminalViewerAction::Poll { attachment_id }
                        if attachment_id == &binding_viewer_id()?
                ) =>
            {
                if write_server_message(
                    stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::Empty {},
                    )),
                )
                .is_err()
                {
                    return Ok(());
                }
            }
            ClientMessage::TerminalControl(request) => match request.action() {
                TerminalControlAction::Input {
                    attachment_id,
                    bytes,
                    ..
                } if attachment_id == &binding_viewer_id()? => {
                    inputs.lock().unwrap().extend(bytes.as_bytes());
                    write_server_message(
                        stream,
                        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                            request.request_id().clone(),
                            TerminalControlOutcome::InputAccepted {},
                        )),
                    )
                    .map_err(io::Error::other)?;
                }
                TerminalControlAction::Resize { .. } => {
                    write_server_message(
                        stream,
                        &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                            request.request_id().clone(),
                            TerminalControlOutcome::Resized { geometry },
                        )),
                    )
                    .map_err(io::Error::other)?;
                }
                other => {
                    return Err(io::Error::other(format!(
                        "unexpected terminal control: {other:?}"
                    )));
                }
            },
            ClientMessage::TerminalViewer(request) => {
                if write_server_message(
                    stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        TerminalViewerOutcome::FeedbackAccepted {},
                    )),
                )
                .is_err()
                {
                    return Ok(());
                }
            }
            ClientMessage::TerminalCreate(_)
            | ClientMessage::TerminalStart(_)
            | ClientMessage::TerminalSend(_)
            | ClientMessage::TerminalLaunchRead(_) => {
                return Err(io::Error::other("unexpected terminal launch message"));
            }
        }
    }
}

fn two_workspaces_snapshot() -> Snapshot {
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    let second_ws = WorkbenchWorkspaceId::parse(CREATED_TAB_ID).unwrap();
    let second_ctx = WorkbenchTerminalContextId::parse(CREATED_TAB_ID).unwrap();
    snapshot.navigation.workspaces.push(Workspace {
        terminal_directory: None,
        automatic_label: false,
        workspace_id: second_ws.clone(),
        project_id: None,
        terminal_context_id: second_ctx.clone(),
        label: "second_workspace".into(),
        primary_directory_id: None,
    });
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: second_ctx,
        tabs: Vec::new(),
        groups: Vec::new(),
    });
    snapshot
}

#[test]
fn init_control_unknown_degrades_to_readonly_over_multiplex_without_retiring() -> TestResult {
    for cut in [
        InitUnknownCut::Acquire,
        InitUnknownCut::HostColors,
        InitUnknownCut::Resize,
    ] {
        let geometry = fixture_geometry()?;
        let (acquire_seen_tx, acquire_seen_rx) = mpsc::sync_channel::<()>(1);
        let (proceed_tx, proceed_rx) = mpsc::sync_channel::<()>(1);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::primary_lane(stream);
            admit(&mut stream)?;
            respond_with_snapshot(&mut stream, available_snapshot(Some(binding())))?;
            stream.answer_probes();
            attach_binding(&mut stream)?;

            let ClientMessage::TerminalControl(acq) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected Acquire"));
            };
            assert_eq!(
                acq.action(),
                &TerminalControlAction::Acquire {
                    attachment_id: binding_viewer_id()?,
                }
            );
            if matches!(cut, InitUnknownCut::Acquire) {
                stream.outcome_unknown()?;
            } else {
                if matches!(cut, InitUnknownCut::HostColors) {
                    acquire_seen_tx.send(()).map_err(io::Error::other)?;
                    proceed_rx.recv().map_err(io::Error::other)?;
                }
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                        acq.request_id().clone(),
                        TerminalControlOutcome::Acquired {
                            controller_id: orchbee_client_server_protocol::ControllerId::parse(ID)
                                .map_err(io::Error::other)?,
                        },
                    )),
                )
                .map_err(io::Error::other)?;

                if matches!(cut, InitUnknownCut::HostColors) {
                    let ClientMessage::TerminalControl(colors) =
                        read_client_message(&mut stream).map_err(io::Error::other)?
                    else {
                        return Err(io::Error::other("expected HostColors"));
                    };
                    assert!(matches!(
                        colors.action(),
                        TerminalControlAction::HostColors { .. }
                    ));
                    stream.outcome_unknown()?;
                } else {
                    let ClientMessage::TerminalControl(resize) =
                        read_client_message(&mut stream).map_err(io::Error::other)?
                    else {
                        return Err(io::Error::other("expected setup Resize"));
                    };
                    assert!(matches!(
                        resize.action(),
                        TerminalControlAction::Resize { .. }
                    ));
                    stream.outcome_unknown()?;
                }
            }

            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected viewer Poll after degradation"));
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    TerminalViewerOutcome::Record(
                        full_frame_record(1, 1, geometry)
                            .map_err(|error| io::Error::other(error.to_string()))?,
                    ),
                )),
            )
            .map_err(io::Error::other)?;
            let ClientMessage::TerminalViewer(feedback) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected FrameApplied"));
            };
            if write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    feedback.request_id().clone(),
                    TerminalViewerOutcome::FeedbackAccepted {},
                )),
            )
            .is_err()
            {
                return Ok(());
            }
            loop {
                match read_client_message(&mut stream) {
                    Ok(ClientMessage::TerminalViewer(request))
                        if matches!(request.action(), TerminalViewerAction::Poll { .. }) =>
                    {
                        if write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Empty {},
                            )),
                        )
                        .is_err()
                        {
                            return Ok(());
                        }
                    }
                    Ok(ClientMessage::TerminalControl(_)) => {
                        return Err(io::Error::other(
                            "unexpected TerminalControl after degrading to ReadOnly",
                        ));
                    }
                    Ok(_) => {}
                    Err(_) => return Ok(()),
                }
            }
        })?;

        let mut workbench_client = start_multiplexed(&server, geometry)?;
        if matches!(cut, InitUnknownCut::HostColors) {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match acquire_seen_rx.try_recv() {
                    Ok(()) => break,
                    Err(mpsc::TryRecvError::Empty) => {
                        assert!(Instant::now() < deadline, "timed out waiting for Acquire");
                        workbench_client.tick();
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("host colors Acquire channel disconnected".into());
                    }
                }
            }
            let ClientBackend::Native(native) = &mut workbench_client.backend;
            native.observe_host_colors(Some(TerminalQueryColors {
                foreground: 0xcccccc,
                background: 0x101010,
                cursor: None,
                palette: [None; 16],
            }));
            proceed_tx
                .send(())
                .map_err(|error| format!("host colors proceed failed: {error:?}"))?;
        }
        assert!(spin_with_draw(&mut workbench_client, |client| {
            client.status() == TerminalStatus::ReadOnly
        }));
        assert!(workbench_client.task.is_some());
        assert!(workbench_client.forward(b"read-only").is_err());
        drop(workbench_client);
        server
            .finish()
            .map_err(|error| format!("init cut {:?} server finish failed: {error:?}", cut as u8))?;
    }
    Ok(())
}

#[test]
fn tab_rename_and_workspace_rename_unknown_persist_until_keypress_without_replay() -> TestResult {
    // Part A: TabRename
    {
        let geometry = fixture_geometry()?;
        let commands = Arc::new(Mutex::new(Vec::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let server_commands = Arc::clone(&commands);
        let server_inputs = Arc::clone(&inputs);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::primary_lane(stream);
            admit(&mut stream)?;
            let WorkbenchSnapshotOutcome::Available { snapshot } =
                available_snapshot(Some(binding()))
            else {
                unreachable!()
            };
            respond_with_snapshot(
                &mut stream,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            )?;
            stream.answer_probes();
            attach_binding(&mut stream)?;
            acquire(&mut stream, &binding_viewer_id()?)?;
            respond_resized(&mut stream, geometry, 1)?;
            serve_controlling_polls(
                &mut stream,
                geometry,
                &snapshot,
                &server_commands,
                &server_inputs,
                true,
            )
        })?;

        let mut workbench_client = start_multiplexed(&server, geometry)?;
        spin_controlling_settled(&mut workbench_client);
        let view = ClientView::new(120);
        let disposition =
            workbench_client.apply_client_action(ClientAction::RequestRenameTab.into());
        draw_workbench(&mut workbench_client, &view)?;
        let token = workbench_client
            .rename_input_context()
            .unwrap_or_else(|| panic!("expected rename context after {disposition:?}"))
            .token();
        assert!(workbench_client.surface_input(SurfaceInput::Rename {
            token,
            edit: RenameEdit::Completeness(true),
        }));
        assert!(workbench_client.surface_input(SurfaceInput::Rename {
            token,
            edit: RenameEdit::Append("Renamed"),
        }));
        workbench_client.surface_action(SurfaceAction::Rename(RenameAction::Submit(token)));

        assert!(spin_with_draw(&mut workbench_client, |client| {
            client.view().notice() == Some(WorkbenchClientNotice::ChangeOutcomeUnknown)
        }));
        assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
        assert!(workbench_client.task.is_some());

        // Notice persists across frames and ticks until a keypress arrives.
        draw_workbench(&mut workbench_client, &view)?;
        workbench_client.tick();
        assert_eq!(
            workbench_client.view().notice(),
            Some(WorkbenchClientNotice::ChangeOutcomeUnknown)
        );

        assert!(workbench_client.forward(b"k").is_ok());
        assert!(workbench_client.view().notice().is_none());
        spin(&mut workbench_client, |_| {
            !inputs.lock().unwrap().is_empty()
        });
        assert_eq!(*inputs.lock().unwrap(), b"k");
        assert_eq!(commands.lock().unwrap().len(), 1);
        drop(workbench_client);
        server.finish()?;
    }

    // Part B: WorkspaceRename via setup
    {
        let geometry = fixture_geometry()?;
        let commands = Arc::new(Mutex::new(Vec::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let server_commands = Arc::clone(&commands);
        let server_inputs = Arc::clone(&inputs);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::primary_lane(stream);
            admit(&mut stream)?;
            let WorkbenchSnapshotOutcome::Available { snapshot } =
                available_snapshot(Some(binding()))
            else {
                unreachable!()
            };
            respond_with_snapshot(
                &mut stream,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            )?;
            stream.answer_probes();
            attach_binding(&mut stream)?;
            acquire(&mut stream, &binding_viewer_id()?)?;
            respond_resized(&mut stream, geometry, 1)?;
            serve_controlling_polls(
                &mut stream,
                geometry,
                &snapshot,
                &server_commands,
                &server_inputs,
                true,
            )
        })?;

        let mut workbench_client = start_multiplexed(&server, geometry)?;
        spin_controlling_settled(&mut workbench_client);
        let view = ClientView::new(120);
        workbench_client.apply_client_action(ClientAction::RequestWorkspaceRename.into());
        let has_draft_before = workbench_client.view().setup_draft().is_some();
        draw_workbench(&mut workbench_client, &view)?;
        let token = workbench_client
            .view()
            .setup_input_context()
            .unwrap_or_else(|| {
                let view = workbench_client.view();
                panic!(
                    "expected setup context: before={has_draft_before} after={} status={:?} phase={:?}",
                    view.setup_draft().is_some(),
                    view.setup_status(),
                    view.phase()
                );
            })
            .token();
        assert!(workbench_client.surface_input(SurfaceInput::Setup {
            token,
            edit: SetupEdit::Completeness(true),
        }));
        assert!(workbench_client.surface_input(SurfaceInput::Setup {
            token,
            edit: SetupEdit::Append("_v2"),
        }));
        workbench_client.surface_action(SurfaceAction::Setup(SetupAction::Submit(token)));

        assert!(spin_with_draw(&mut workbench_client, |client| {
            matches!(
                client.view().setup_status(),
                Some(SetupStatus::LocationLabel {
                    status: LocationLabelStatus::OutcomeUnknown,
                    ..
                })
            )
        }));
        assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
        assert!(workbench_client.task.is_some());

        draw_workbench(&mut workbench_client, &view)?;
        workbench_client.tick();
        assert!(workbench_client.view().setup_status().is_some());

        assert!(workbench_client.forward(b"w").is_ok());
        assert!(workbench_client.view().setup_status().is_none());
        spin(&mut workbench_client, |_| {
            !inputs.lock().unwrap().is_empty()
        });
        assert_eq!(*inputs.lock().unwrap(), b"w");
        assert_eq!(commands.lock().unwrap().len(), 1);
        drop(workbench_client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn tab_and_workspace_close_unknown_keep_tab_interactive_until_keypress() -> TestResult {
    // Part A: TabClose
    {
        let geometry = fixture_geometry()?;
        let commands = Arc::new(Mutex::new(Vec::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let server_commands = Arc::clone(&commands);
        let server_inputs = Arc::clone(&inputs);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::primary_lane(stream);
            admit(&mut stream)?;
            let WorkbenchSnapshotOutcome::Available { snapshot } =
                available_snapshot(Some(binding()))
            else {
                unreachable!()
            };
            respond_with_snapshot(
                &mut stream,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            )?;
            stream.answer_probes();
            attach_binding(&mut stream)?;
            acquire(&mut stream, &binding_viewer_id()?)?;
            respond_resized(&mut stream, geometry, 1)?;
            serve_controlling_polls(
                &mut stream,
                geometry,
                &snapshot,
                &server_commands,
                &server_inputs,
                true,
            )
        })?;

        let mut workbench_client = start_multiplexed(&server, geometry)?;
        spin_controlling_settled(&mut workbench_client);
        let view = ClientView::new(120);
        workbench_client.apply_client_action(ClientAction::RequestCloseTab.into());
        draw_workbench(&mut workbench_client, &view)?;
        workbench_client.apply_client_action(ClientAction::ConfirmClose.into());

        assert!(spin_with_draw(&mut workbench_client, |client| {
            client.view().announced_close_status().is_some()
        }));
        assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
        assert!(workbench_client.task.is_some());

        draw_workbench(&mut workbench_client, &view)?;
        workbench_client.tick();
        assert!(workbench_client.view().announced_close_status().is_some());

        // Shown Tab stays interactive after ambiguous Close, and keypress clears notice.
        assert!(workbench_client.forward(b"t").is_ok());
        assert!(workbench_client.view().announced_close_status().is_none());
        spin(&mut workbench_client, |_| {
            !inputs.lock().unwrap().is_empty()
        });
        assert_eq!(*inputs.lock().unwrap(), b"t");
        assert_eq!(commands.lock().unwrap().len(), 1);
        drop(workbench_client);
        server.finish()?;
    }

    // Part B: WorkspaceClose
    {
        let geometry = fixture_geometry()?;
        let commands = Arc::new(Mutex::new(Vec::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let server_commands = Arc::clone(&commands);
        let server_inputs = Arc::clone(&inputs);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::primary_lane(stream);
            admit(&mut stream)?;
            let snapshot = two_workspaces_snapshot();
            respond_with_snapshot(
                &mut stream,
                WorkbenchSnapshotOutcome::Available {
                    snapshot: snapshot.clone(),
                },
            )?;
            stream.answer_probes();
            attach_binding(&mut stream)?;
            acquire(&mut stream, &binding_viewer_id()?)?;
            respond_resized(&mut stream, geometry, 1)?;
            serve_controlling_polls(
                &mut stream,
                geometry,
                &snapshot,
                &server_commands,
                &server_inputs,
                true,
            )
        })?;

        let mut workbench_client = start_multiplexed(&server, geometry)?;
        spin_controlling_settled(&mut workbench_client);
        let view = ClientView::new(120);
        let second_ws = WorkbenchWorkspaceId::parse(CREATED_TAB_ID).unwrap();
        workbench_client
            .apply_client_action(ClientAction::RequestWorkspaceCloseAt(second_ws).into());

        assert!(spin_with_draw(&mut workbench_client, |client| {
            client
                .view()
                .workspace_close_status()
                .is_some_and(|status| status.status() == WorkspaceCloseStatus::Unknown)
        }));
        assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
        assert!(workbench_client.task.is_some());

        draw_workbench(&mut workbench_client, &view)?;
        workbench_client.tick();
        assert!(workbench_client.view().workspace_close_status().is_some());

        assert!(workbench_client.forward(b"z").is_ok());
        assert!(workbench_client.view().workspace_close_status().is_none());
        spin(&mut workbench_client, |_| {
            !inputs.lock().unwrap().is_empty()
        });
        assert_eq!(*inputs.lock().unwrap(), b"z");
        assert_eq!(commands.lock().unwrap().len(), 1);
        drop(workbench_client);
        server.finish()?;
    }
    Ok(())
}
