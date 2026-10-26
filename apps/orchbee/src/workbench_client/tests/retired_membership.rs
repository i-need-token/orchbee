use super::*;
use std::sync::mpsc::{Receiver, SyncSender};

#[test]
fn joined_ordinary_poll_unavailability_allows_only_explicit_snapshot_then_empty() -> TestResult {
    let geometry = fixture_geometry()?;
    let (sent, received) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected ordinary Poll"));
            };
            assert_eq!(
                poll.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            sent.send(poll.request_id().as_str().to_owned())
                .map_err(io::Error::other)?;
            released
                .recv_timeout(Duration::from_secs(2))
                .map_err(io::Error::other)?;
            write_server_message(&mut stream, &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                poll.request_id().clone(), TerminalViewerOutcome::Unavailable {
                    reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
                },
            ))).map_err(io::Error::other)?;
            let mut unexpected = [0; 1];
            assert_eq!(
                stream.read(&mut unexpected)?,
                0,
                "old carrier closes without Detach or another request"
            );
            return Ok(());
        }
        admit(&mut stream)?;
        let WorkbenchSnapshotOutcome::Available { mut snapshot } =
            available_snapshot(Some(binding()))
        else {
            unreachable!()
        };
        snapshot.revision = WorkbenchRevision::new(2);
        snapshot.terminal_contexts[0].tabs.clear();
        let snapshot_id = respond_with_snapshot(
            &mut stream,
            WorkbenchSnapshotOutcome::Available { snapshot },
        )?;
        sent.send(snapshot_id).map_err(io::Error::other)?;
        let mut unexpected = [0; 1];
        assert_eq!(
            stream.read(&mut unexpected)?,
            0,
            "Empty issues no bootstrap, Attach, Create, Start or old cleanup"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let view = ClientView::new(120);
    let poll = arm_uncommitted_panel_close(&mut workbench_client, &received, &release)?;
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    assert!(render_workbench_client(&workbench_client, &view)?.contains("Terminal disconnected"));
    for _ in 0..8 {
        workbench_client.tick();
        workbench_client.after_draw();
    }
    assert!(workbench_client.task.is_none(), "no automatic reconnect");
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(
        workbench_client.task.is_some(),
        "exact joined Poll retirement admits explicit recovery"
    );
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        let view = workbench_client.view();
        view.phase() == DisplayPhase::Empty
    }));
    let snapshot = received.recv_timeout(Duration::from_secs(2))?;
    assert!(
        snapshot > poll,
        "request allocator continues across carriers"
    );
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

/// Joined typed loss on a quiescent Ready focus is pane-local; the old
/// whole-Client retirement origin is reachable only while a structural owner
/// is armed. The server therefore proves it has read and correlated the
/// leased ordinary Poll, then waits until the client reports a real armed
/// panel-close confirmation before answering `TerminalRuntimeUnavailable`.
fn retire_on_poll(
    stream: &mut UnixStream,
    geometry: Geometry,
    sighted: &SyncSender<String>,
    release: &Receiver<()>,
) -> io::Result<()> {
    serve_native_controlling_frame(stream, geometry)?;
    accept_native_frame_feedback(stream)?;
    let ClientMessage::TerminalViewer(poll) =
        read_client_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("ordinary Poll"));
    };
    assert_eq!(
        poll.action(),
        &TerminalViewerAction::Poll {
            attachment_id: binding_viewer_id()?,
        }
    );
    sighted
        .send(poll.request_id().as_str().to_owned())
        .map_err(io::Error::other)?;
    release
        .recv_timeout(Duration::from_secs(2))
        .map_err(io::Error::other)?;
    write_server_message(stream, &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(poll.request_id().clone(), TerminalViewerOutcome::Unavailable { reason: orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable }))).map_err(io::Error::other)?;
    let mut byte = [0; 1];
    assert_eq!(stream.read(&mut byte)?, 0);
    Ok(())
}

/// While the joined ordinary Poll is still leased and unreplied, opens the
/// real panel-close confirmation through `ClientAction::ClosePanel` and proves
/// it admitted with one actual draw — an unpresentable first paint would have
/// refused it, and a submitted close removes it before any Panel request is
/// queued. The admitted confirmation therefore also witnesses that no Panel
/// request was submitted. Returns the correlated Poll request id and releases
/// the server's typed loss.
fn arm_uncommitted_panel_close(
    workbench_client: &mut WorkbenchClient,
    poll_sighted: &Receiver<String>,
    release: &SyncSender<()>,
) -> Result<String, Box<dyn std::error::Error>> {
    let poll = poll_sighted
        .recv_timeout(Duration::from_secs(2))
        .map_err(|error| io::Error::other(format!("ordinary Poll handoff: {error}")))?;
    assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
    workbench_client.apply_client_action(ClientAction::ClosePanel);
    let view = ClientView::new(120);
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    let mut receipt = DrawReceipt::default();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            &view,
            Locale::EnUs,
            Some(workbench_client.view()),
            &crate::tui_theme::Theme::terminal_native(),
            &mut crate::tui_pointer::HitMapBuilder::new(),
        );
    })?;
    workbench_client.after_close_draw(receipt);
    let view = workbench_client.view();
    assert_eq!(view.phase(), DisplayPhase::Ready);
    assert!(
        view.panel_close_confirmation().is_some(),
        "a real draw leaves the uncommitted confirmation armed"
    );
    release.send(())?;
    Ok(poll)
}

#[test]
fn repeated_failed_recovery_joins_before_explicit_retry_and_never_resets_request_ids() -> TestResult
{
    let geometry = fixture_geometry()?;
    let (sent, received) = mpsc::sync_channel(2);
    let (sighted, poll_sighted) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let mut prior = None;
    let server = FakeServer::start_connections(3, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            return retire_on_poll(&mut stream, geometry, &sighted, &released);
        }
        admit(&mut stream)?;
        let outcome = if index == 1 {
            available_snapshot(Some(binding()))
        } else {
            let WorkbenchSnapshotOutcome::Available { mut snapshot } =
                available_snapshot(Some(binding()))
            else {
                unreachable!()
            };
            snapshot.revision = WorkbenchRevision::new(2);
            snapshot.terminal_contexts[0].tabs.clear();
            WorkbenchSnapshotOutcome::Available { snapshot }
        };
        let id = respond_with_snapshot(&mut stream, outcome)?;
        if let Some(prior) = &prior {
            assert!(&id > prior);
        }
        prior = Some(id);
        sent.send(()).map_err(io::Error::other)?;
        let mut byte = [0; 1];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "a present target cannot cause Attach or same-carrier retry"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    arm_uncommitted_panel_close(&mut workbench_client, &poll_sighted, &release)?;
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(workbench_client.task.is_some());
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    received.recv_timeout(Duration::from_secs(2))?;
    assert!({
        let view = workbench_client.view();
        view.phase() == DisplayPhase::Retired
    });
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        let view = workbench_client.view();
        view.phase() == DisplayPhase::Empty
    }));
    received.recv_timeout(Duration::from_secs(2))?;
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn exit_closes_a_recovery_carrier_before_any_late_snapshot_can_install() -> TestResult {
    let geometry = fixture_geometry()?;
    let (sent, received) = mpsc::sync_channel(1);
    let (sighted, poll_sighted) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            return retire_on_poll(&mut stream, geometry, &sighted, &released);
        }
        admit(&mut stream)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        sent.send(()).map_err(io::Error::other)?;
        let mut byte = [0; 1];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "exit cancels the only recovery transport"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    arm_uncommitted_panel_close(&mut workbench_client, &poll_sighted, &release)?;
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    received.recv_timeout(Duration::from_secs(2))?;
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn wrong_id_transport_loss_and_other_unavailability_cannot_open_peer_recovery() -> TestResult {
    for kind in 0..3 {
        let geometry = fixture_geometry()?;
        let server = FakeServer::start(move |mut stream| {
            let mut stream = UnixStream::new(stream);
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("ordinary Poll"));
            };
            if kind == 2 {
                return Ok(());
            }
            let id = if kind == 0 {
                RequestId::parse(ID).unwrap()
            } else {
                poll.request_id().clone()
            };
            let reason = if kind == 0 {
                orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable
            } else {
                orchbee_client_server_protocol::ViewerUnavailableReason::WorkbenchUnavailable
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    id,
                    TerminalViewerOutcome::Unavailable { reason },
                )),
            )
            .map_err(io::Error::other)?;
            let mut byte = [0; 1];
            assert_eq!(stream.read(&mut byte)?, 0);
            Ok(())
        })?;
        let (mut workbench_client, _) =
            workbench_client_with_identity(native_client(&server)?, geometry)?;
        assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
            workbench_client.task.is_none()
        }));
        workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
        assert!(
            workbench_client.task.is_none(),
            "failure kind {kind} does not establish an origin"
        );
        drop(workbench_client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn recovery_installs_neighbor_before_real_attach_and_preserves_it_after_setup_refusal() -> TestResult
{
    recover_neighbor(0)
}

#[test]
fn recovery_successfully_sets_up_the_installed_neighbor_without_bootstrap() -> TestResult {
    recover_neighbor(1)
}

#[test]
fn recovery_setup_poll_unavailability_preserves_candidate_without_granting_another_origin()
-> TestResult {
    recover_neighbor(2)
}

fn recover_neighbor(mode: u8) -> TestResult {
    let geometry = fixture_geometry()?;
    let (sent, received) = mpsc::sync_channel(1);
    let (sighted, poll_sighted) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            return retire_on_poll(&mut stream, geometry, &sighted, &released);
        }
        admit(&mut stream)?;
        let WorkbenchSnapshotOutcome::Available { mut snapshot } =
            available_snapshot(Some(binding()))
        else {
            unreachable!()
        };
        snapshot.revision = WorkbenchRevision::new(2);
        let tab = &mut snapshot.terminal_contexts[0].tabs[0];
        tab.tab_id = WorkbenchTabId::parse(CREATED_TAB_ID).unwrap();
        tab.label = "survivor".into();
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } = &mut tab.root
        else {
            unreachable!()
        };
        *panel_id = WorkbenchPanelId::parse(CREATED_PANEL_ID).unwrap();
        primary_presentation.presentation_id =
            TerminalPresentationId::parse(CREATED_PRESENTATION_ID).unwrap();
        primary_presentation.binding.as_mut().unwrap().terminal_id =
            TerminalId::parse(CREATED_TAB_ID).unwrap();
        respond_with_snapshot(
            &mut stream,
            WorkbenchSnapshotOutcome::Available { snapshot },
        )?;
        let ClientMessage::TerminalViewer(attach) =
            read_client_message(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("exact neighbor Attach"));
        };
        assert!(
            matches!(attach.action(), TerminalViewerAction::AttachBinding { terminal_id, .. } if terminal_id.as_str() == CREATED_TAB_ID)
        );
        let unavailable = || TerminalViewerOutcome::Unavailable {
            reason:
                orchbee_client_server_protocol::ViewerUnavailableReason::TerminalRuntimeUnavailable,
        };
        let attached = binding_attach_outcome()?;
        write_server_message(
            &mut stream,
            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                attach.request_id().clone(),
                if mode == 0 { unavailable() } else { attached },
            )),
        )
        .map_err(io::Error::other)?;
        if mode != 0 {
            acquire(&mut stream, &binding_viewer_id()?)?;
            let ClientMessage::TerminalControl(resize) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("neighbor Resize"));
            };
            assert!(
                matches!(resize.action(), TerminalControlAction::Resize { attachment_id, geometry: requested, .. } if attachment_id == &binding_viewer_id()? && *requested == geometry)
            );
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                    resize.request_id().clone(),
                    TerminalControlOutcome::Resized { geometry },
                )),
            )
            .map_err(io::Error::other)?;
            let ClientMessage::TerminalViewer(poll) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("neighbor setup Poll"));
            };
            assert_eq!(
                poll.action(),
                &TerminalViewerAction::Poll {
                    attachment_id: binding_viewer_id()?,
                }
            );
            let outcome = if mode == 2 {
                unavailable()
            } else {
                TerminalViewerOutcome::Record(
                    full_frame_record(1, 1, geometry)
                        .map_err(|e| io::Error::other(e.to_string()))?,
                )
            };
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    poll.request_id().clone(),
                    outcome,
                )),
            )
            .map_err(io::Error::other)?;
        }
        if mode == 2 {
            let ClientMessage::TerminalViewer(cleanup) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("new setup cleanup"));
            };
            assert_eq!(
                cleanup.action(),
                &TerminalViewerAction::Detach {
                    attachment_id: binding_viewer_id()?,
                }
            );
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    cleanup.request_id().clone(),
                    TerminalViewerOutcome::Detached {},
                )),
            )
            .map_err(io::Error::other)?;
        }
        if mode == 1 {
            // Continue normal feedback/Poll after setup; any bootstrap, mutation or old Detach fails.
            loop {
                let request = match read_client_message(&mut stream) {
                    Ok(ClientMessage::TerminalViewer(request)) => request,
                    Err(_) => break,
                    _ => {
                        return Err(io::Error::other(
                            "only normal viewer traffic after neighbor setup",
                        ));
                    }
                };
                let outcome = match request.action() {
                    TerminalViewerAction::FrameApplied { .. } => {
                        TerminalViewerOutcome::FeedbackAccepted {}
                    }
                    TerminalViewerAction::Poll { attachment_id } => {
                        assert_eq!(attachment_id, &binding_viewer_id()?);
                        TerminalViewerOutcome::Empty {}
                    }
                    _ => return Err(io::Error::other("unexpected viewer action")),
                };
                // Dropping the ready session can close the carrier while its
                // final ordinary Poll reply is in flight. A closed peer ends
                // this fixture; protocol failures remain test failures.
                match write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        outcome,
                    )),
                ) {
                    Ok(()) => {}
                    Err(orchbee_client_server_protocol::WriteMessageError::Io(error))
                        if matches!(
                            error.kind(),
                            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                        ) =>
                    {
                        break;
                    }
                    Err(error) => return Err(io::Error::other(error)),
                }
            }
            return Ok(());
        }
        sent.send(()).map_err(io::Error::other)?;
        let mut byte = [0; 1];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "setup refusal does not issue automatic Start, old Detach or another Snapshot"
        );
        Ok(())
    })?;
    let (mut workbench_client, _old_mailbox) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    arm_uncommitted_panel_close(&mut workbench_client, &poll_sighted, &release)?;
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    if mode == 1 {
        let ok = spin_with_draw(
            &mut workbench_client,
            |workbench_client| matches!(&workbench_client.backend, ClientBackend::Native(native) if native.view().phase() == DisplayPhase::Ready),
        );
        assert!(ok);
    } else {
        assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
            workbench_client.task.is_none()
        }));
        received
            .recv_timeout(Duration::from_secs(2))
            .map_err(|error| {
                io::Error::other(format!("neighbor setup refusal handoff: {error}"))
            })?;
    }
    let ClientBackend::Native(native) = &workbench_client.backend;
    let view = native.view();
    assert_eq!(
        view.phase(),
        if mode == 1 {
            DisplayPhase::Ready
        } else {
            DisplayPhase::Retired
        }
    );
    assert_eq!(view.tab(0).unwrap().tab_label(), "survivor");
    if mode != 1 {
        workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
        assert!(
            workbench_client.task.is_none(),
            "setup failure cannot impersonate ordinary Poll retirement"
        );
    }
    drop(workbench_client);
    server.finish()?;
    Ok(())
}

#[test]
fn cancelled_connect_closes_late_admission_without_placing_snapshot() -> TestResult {
    let geometry = fixture_geometry()?;
    let (arrived, connecting) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let (sighted, poll_sighted) = mpsc::sync_channel(1);
    let (loss_release, loss_released) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        let mut stream = UnixStream::new(stream);
        if index == 0 {
            return retire_on_poll(&mut stream, geometry, &sighted, &loss_released);
        }
        assert!(matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ));
        arrived.send(()).map_err(io::Error::other)?;
        released
            .recv_timeout(Duration::from_secs(2))
            .map_err(io::Error::other)?;
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: NonZeroU64::MIN,
            },
        )
        .map_err(io::Error::other)?;
        let mut byte = [0; 1];
        assert_eq!(
            stream.read(&mut byte)?,
            0,
            "a late admitted connection is closed before Snapshot placement"
        );
        Ok(())
    })?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    arm_uncommitted_panel_close(&mut workbench_client, &poll_sighted, &loss_release)?;
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        workbench_client.task.is_none()
    }));
    workbench_client.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    connecting.recv_timeout(Duration::from_secs(2))?;
    workbench_client.recovery_shutdown.as_ref().unwrap().close();
    release.send(())?;
    drop(workbench_client);
    server.finish()?;
    Ok(())
}
