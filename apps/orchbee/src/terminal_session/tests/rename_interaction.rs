use super::*;
use crate::terminal_input::InputDispatcher;
use std::sync::Mutex;

struct RenameServer {
    server: FakeServer,
    commands: Arc<Mutex<Vec<orchbee_workbench_protocol::WorkbenchCommand>>>,
    inputs: Arc<Mutex<Vec<u8>>>,
    teardown: Arc<AtomicBool>,
    snapshots: Arc<AtomicU8>,
    detached: Arc<AtomicBool>,
    successor_ready: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
enum RenameResponse {
    Denied,
    Unchanged,
    Changed,
    Recover,
    Removed,
    Successor,
    SuccessorReady,
    WrongLabel,
    Unknown,
    LostSnapshot,
    TabUnavailable,
}

impl RenameServer {
    fn start(geometry: Geometry) -> io::Result<Self> {
        Self::with_response(geometry, RenameResponse::Denied)
    }

    fn with_response(geometry: Geometry, response: RenameResponse) -> io::Result<Self> {
        let commands = Arc::new(Mutex::new(Vec::<
            orchbee_workbench_protocol::WorkbenchCommand,
        >::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let teardown = Arc::new(AtomicBool::new(false));
        let recorded = Arc::clone(&commands);
        let forwarded = Arc::clone(&inputs);
        let stopping = Arc::clone(&teardown);
        let snapshots = Arc::new(AtomicU8::new(0));
        let snapshot_count = Arc::clone(&snapshots);
        let detached = Arc::new(AtomicBool::new(false));
        let saw_detach = Arc::clone(&detached);
        let successor_ready = Arc::new(AtomicBool::new(false));
        let saw_successor_feedback = Arc::clone(&successor_ready);
        let server = FakeServer::start(move |mut stream| {
            let result = (|| {
                serve_native_controlling_frame(&mut stream, geometry)?;
                accept_native_frame_feedback(&mut stream)?;
                let mut successor_stage = 0;
                loop {
                    let mut magic = [0; 4];
                    if stream.read_exact(&mut magic).is_err() {
                        return Ok(());
                    }
                    let mut reader = io::Cursor::new(magic).chain(&mut stream);
                    if magic == *b"OBWP" {
                        let request =
                            read_workbench_request(&mut reader).map_err(io::Error::other)?;
                        let command = match request {
                            WorkbenchRequest::TabCloseRead(_)
                            | WorkbenchRequest::TabCloseResume(_) => {
                                return Err(io::Error::other(
                                    "Rename unexpectedly issued a Close request",
                                ));
                            }
                            WorkbenchRequest::Command(command) => command,
                            WorkbenchRequest::SnapshotRead(request) => {
                                let Some(outcome) =
                                    next_rename_snapshot(&recorded, &snapshot_count, response)
                                else {
                                    return Ok(());
                                };
                                write_workbench_response(
                                    &mut stream,
                                    &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                                        protocol_version: ProtocolVersion,
                                        request_id: request.request_id,
                                        outcome,
                                    }),
                                )
                                .map_err(io::Error::other)?;
                                continue;
                            }
                            WorkbenchRequest::PublicationRead(request) => {
                                let Some(outcome) =
                                    next_rename_snapshot(&recorded, &snapshot_count, response)
                                else {
                                    return Ok(());
                                };
                                write_workbench_response(
                                    &mut stream,
                                    &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                        protocol_version: ProtocolVersion,
                                        request_id: request.request_id,
                                        outcome: publication_outcome_for(
                                            outcome,
                                            request.known_revision,
                                        ),
                                    }),
                                )
                                .map_err(io::Error::other)?;
                                continue;
                            }
                        };
                        assert!(
                            matches!(&command.action, WorkbenchAction::TabRename {terminal_context_id, tab_id, ..}
                            if terminal_context_id.as_str() == ID && tab_id.as_str() == ID)
                        );
                        recorded.lock().unwrap().push(command.clone());
                        if matches!(response, RenameResponse::Unknown) {
                            return Ok(());
                        }
                        let outcome = match response {
                            RenameResponse::Denied => WorkbenchCommandOutcome::Denied {},
                            RenameResponse::Unchanged => WorkbenchCommandOutcome::Unchanged { revision: command.expected_revision },
                            RenameResponse::TabUnavailable => WorkbenchCommandOutcome::Rejected { revision: command.expected_revision, reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabUnavailable },
                            _ => WorkbenchCommandOutcome::Renamed { revision: WorkbenchRevision::new(2) },
                        };
                        write_workbench_response(
                            &mut stream,
                            &WorkbenchResponse::Command(WorkbenchCommandResult {
                                protocol_version: ProtocolVersion,
                                request_id: command.request_id.clone(),
                                outcome,
                            }),
                        )
                        .map_err(io::Error::other)?;
                        continue;
                    }
                    assert_eq!(magic, *b"OBCS");
                    let response = match read_client_message(&mut reader)
                        .map_err(io::Error::other)?
                    {
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::Detach { attachment_id }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            assert!(matches!(
                                response,
                                RenameResponse::Removed
                                    | RenameResponse::Successor
                                    | RenameResponse::SuccessorReady
                            ));
                            assert!(!saw_detach.swap(true, Ordering::SeqCst));
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Detached {},
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::AttachBinding { .. }
                            ) =>
                        {
                            assert!(matches!(
                                response,
                                RenameResponse::Successor | RenameResponse::SuccessorReady
                            ));
                            assert!(saw_detach.load(Ordering::SeqCst));
                            assert!(
                                matches!(request.action(), TerminalViewerAction::AttachBinding { terminal_id, .. } if terminal_id.as_str() == CREATED_TAB_ID)
                            );
                            let outcome = if matches!(response, RenameResponse::SuccessorReady) {
                                assert_eq!(successor_stage, 0);
                                successor_stage = 1;
                                binding_attach_outcome()?
                            } else {
                                TerminalViewerOutcome::Rejected { reason: orchbee_client_server_protocol::ViewerRejectedReason::TerminalUnavailable }
                            };
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::Poll { attachment_id }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            let outcome = if successor_stage == 3 {
                                successor_stage = 4;
                                TerminalViewerOutcome::Record(
                                    full_frame_record(1, 1, geometry)
                                        .map_err(|error| io::Error::other(error.to_string()))?,
                                )
                            } else {
                                TerminalViewerOutcome::Empty {}
                            };
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::FrameApplied { attachment_id, .. }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            assert_eq!(successor_stage, 4);
                            successor_stage = 5;
                            saw_successor_feedback.store(true, Ordering::SeqCst);
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::FeedbackAccepted {},
                            ))
                        }
                        ClientMessage::TerminalControl(request) => {
                            let outcome = match request.action() {
                                TerminalControlAction::Acquire { attachment_id } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    assert_eq!(successor_stage, 1);
                                    successor_stage = 2;
                                    TerminalControlOutcome::Acquired {
                                        controller_id:
                                            orchbee_client_server_protocol::ControllerId::parse(ID)
                                                .map_err(io::Error::other)?,
                                    }
                                }
                                TerminalControlAction::Input {
                                    attachment_id,
                                    bytes,
                                    ..
                                } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    forwarded.lock().unwrap().extend(bytes.as_bytes());
                                    TerminalControlOutcome::InputAccepted {}
                                }
                                TerminalControlAction::Resize {
                                    attachment_id,
                                    geometry,
                                    ..
                                } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    if successor_stage == 2 {
                                        successor_stage = 3;
                                    }
                                    TerminalControlOutcome::Resized {
                                        geometry: *geometry,
                                    }
                                }
                                TerminalControlAction::Paste { .. } => {
                                    TerminalControlOutcome::Pasted {
                                        accepted_bytes:
                                            orchbee_client_server_protocol::PasteAcceptedBytes::new(
                                                1,
                                            )
                                            .map_err(io::Error::other)?,
                                    }
                                }
                            };
                            ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        other => {
                            return Err(io::Error::other(format!(
                                "unexpected terminal request {other:?}"
                            )));
                        }
                    };
                    write_server_message(&mut stream, &response).map_err(io::Error::other)?;
                }
            })();
            match result {
                Err(error) if stopping.load(Ordering::SeqCst) && disconnected(&error) => Ok(()),
                other => other,
            }
        })?;
        Ok(Self {
            server,
            commands,
            inputs,
            teardown,
            snapshots,
            detached,
            successor_ready,
        })
    }

    fn finish(self, session: TerminalSession) -> io::Result<()> {
        self.teardown.store(true, Ordering::SeqCst);
        drop(session);
        self.server.finish()
    }
}

fn disconnected(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    }) || error.source().is_some_and(disconnected)
}

fn input(
    dispatcher: &mut InputDispatcher,
    session: &mut TerminalSession,
    view: &mut ClientView,
    bytes: &[u8],
    now: Instant,
) {
    assert_eq!(
        dispatcher
            .dispatch(
                view,
                bytes,
                session,
                now,
                |_, _| Ok::<_, io::Error>(()),
                &mut |_| Ok(())
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
}

fn draw(
    session: &mut TerminalSession,
    view: &ClientView,
    locale: Locale,
    width: u16,
    height: u16,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    let mut receipt = NativeDrawReceipt::default();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            view,
            locale,
            Some(session.view()),
            &crate::tui_theme::Theme::terminal_native(),
        )
    })?;
    assert!(!terminal.backend().cursor_visible());
    session.after_close_draw(receipt);
    Ok(terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(width))
        .map(|row| {
            let mut line: String = row.iter().map(|cell| cell.symbol()).collect();
            line.push('\n');
            line
        })
        .collect())
}

#[test]
fn actual_localized_draw_and_exact_token_enter_submit_one_unicode_rename() -> TestResult {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let geometry = fixture_geometry()?;
        let server = RenameServer::start(geometry)?;
        let (mut session, _) =
            native_session_with_identity(native_client(&server.server)?, geometry)?;
        assert!(spin_with_draw(&mut session, |session| session.status()
            == TerminalStatus::Controlling));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let now = Instant::now();
        input(
            &mut dispatcher,
            &mut session,
            &mut view,
            "\x02,\x15雪😀e\u{301}\r".as_bytes(),
            now,
        );
        let token = session.rename_input_context().unwrap().token();
        assert!(server.commands.lock().unwrap().is_empty());
        let rendered = draw(&mut session, &view, locale, 120, 32)?;
        // TestBackend's wide-cell continuation slots contain spaces between Chinese glyphs.
        assert!(
            rendered
                .replace(' ', "")
                .contains(&locale.rename_title().replace(' ', "")),
            "locale={locale:?}\n{rendered}"
        );
        assert!(
            rendered
                .replace(' ', "")
                .contains(&locale.rename_warning().replace(' ', ""))
        );
        assert_eq!(session.rename_input_context().unwrap().token(), token);
        input(&mut dispatcher, &mut session, &mut view, b"\r\n\x02q", now);
        spin(&mut session, |_| {
            !server.commands.lock().unwrap().is_empty()
        });
        let commands = server.commands.lock().unwrap();
        assert_eq!(commands.len(), 1);
        let WorkbenchAction::TabRename { label, .. } = &commands[0].action else {
            unreachable!()
        };
        assert_eq!(label.as_str(), "雪😀e\u{301}");
        drop(commands);
        assert!(server.inputs.lock().unwrap().is_empty());
        assert!(session.rename_input_context().is_none());
        server.finish(session)?;
    }
    Ok(())
}

#[test]
fn input_fault_survives_valid_edits_until_explicit_complete_backspace() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15\xe2\x08a\x1b[200~B\x1b[201~",
        now,
    );
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.rename_draft().unwrap().input_fault())
    );
    draw(&mut session, &view, Locale::EnUs, 120, 32)?;
    input(&mut dispatcher, &mut session, &mut view, b"\r", now);
    assert!(server.commands.lock().unwrap().is_empty());
    input(&mut dispatcher, &mut session, &mut view, b"\x08", now);
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if !view.rename_draft().unwrap().input_fault())
    );
    input(&mut dispatcher, &mut session, &mut view, b"\r", now);
    spin(&mut session, |_| {
        !server.commands.lock().unwrap().is_empty()
    });
    let commands = server.commands.lock().unwrap();
    assert!(
        matches!(&commands[0].action, WorkbenchAction::TabRename {label, ..} if label.as_str() == "a")
    );
    drop(commands);
    assert!(server.inputs.lock().unwrap().is_empty());
    server.finish(session)?;
    Ok(())
}

#[test]
fn clipped_draw_revokes_token_and_cancelled_paste_keeps_its_frame_until_quiet() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15changed",
        now,
    );
    session.rename_input_context().unwrap();
    draw(&mut session, &view, Locale::EnUs, 120, 32)?;
    // A draw that cannot present the complete draft cancels it outright with a
    // bounded notice — no hidden capture survives to keep swallowing keys.
    draw(&mut session, &view, Locale::EnUs, 12, 6)?;
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(session.rename_input_context().is_none());
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view)
            if view.notice()
                == Some(crate::terminal_session::NativeSessionNotice::DraftCancelled)
    ));
    // The released decoder still owns the frame until input goes quiet — a
    // re-arm attempt inside the kept frame is swallowed wholesale.
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15changed",
        now,
    );
    assert!(session.rename_input_context().is_none());
    assert!(server.inputs.lock().unwrap().is_empty());
    // Past the quiet window the same packet re-arms a fresh draft.
    let later = now + Duration::from_secs(3600);
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15changed",
        later,
    );
    let token = session.rename_input_context().unwrap().token();
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x1b[200~old",
        later,
    );
    session.cancel_rename_draft(token);
    let quiet = later + Duration::from_secs(3600);
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02q\r\x1b[201~tail",
        quiet,
    );
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(server.inputs.lock().unwrap().is_empty());
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"new",
        quiet + Duration::from_millis(50),
    );
    spin(&mut session, |_| !server.inputs.lock().unwrap().is_empty());
    assert_eq!(*server.inputs.lock().unwrap(), b"new");
    server.finish(session)?;
    Ok(())
}

#[test]
fn actual_loop_same_terminal_columns_outer_resize_revokes_before_enter_dispatch() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let leases = crate::tui_shell::tests::run_native_rename_resize(&mut session)?;
    assert_eq!(leases.iter().filter(|enabled| **enabled).count(), 1);
    assert_eq!(leases.last(), Some(&false));
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(server.inputs.lock().unwrap().is_empty());
    assert!(session.rename_input_context().is_none());
    server.finish(session)?;
    Ok(())
}

#[test]
fn maximum_atomic_paste_draws_sanitized_cells_but_submits_exact_original_scalars() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    let label = format!("{}\x1b\n", "界".repeat(510));
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15\x1b[200~",
        now,
    );
    for byte in label.as_bytes() {
        input(
            &mut dispatcher,
            &mut session,
            &mut view,
            std::slice::from_ref(byte),
            now,
        );
    }
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.rename_draft().unwrap().text().is_empty())
    );
    input(&mut dispatcher, &mut session, &mut view, b"\x1b[201~", now);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rendered = draw(&mut session, &view, locale, 120, 40)?;
        let displayed: String = rendered
            .chars()
            .filter(|character| matches!(character, '界' | '\u{fffd}'))
            .collect();
        // The finite modal no longer covers the whole frame — the panel
        // viewport contributes its own fixture scalars — so the complete
        // sanitized draft must appear as one contiguous visible run.
        assert!(displayed.contains(crate::terminal_text::sanitize_terminal_text(&label).as_ref()));
        assert!(!rendered.contains('\x1b'));
    }
    input(&mut dispatcher, &mut session, &mut view, b"\r", now);
    spin(&mut session, |_| {
        !server.commands.lock().unwrap().is_empty()
    });
    let commands = server.commands.lock().unwrap();
    assert!(
        matches!(&commands[0].action, WorkbenchAction::TabRename {label: submitted, ..} if submitted.as_str() == label)
    );
    drop(commands);
    server.finish(session)?;
    Ok(())
}

#[test]
fn actual_rename_outcomes_install_recover_or_retire_without_replay() -> TestResult {
    for response in [
        RenameResponse::Unchanged,
        RenameResponse::Changed,
        RenameResponse::Recover,
        RenameResponse::Removed,
        RenameResponse::Successor,
        RenameResponse::WrongLabel,
        RenameResponse::Unknown,
        RenameResponse::LostSnapshot,
        RenameResponse::TabUnavailable,
    ] {
        let geometry = fixture_geometry()?;
        let server = RenameServer::with_response(geometry, response)?;
        let (mut session, _) =
            native_session_with_identity(native_client(&server.server)?, geometry)?;
        assert!(spin_with_draw(&mut session, |session| session.status()
            == TerminalStatus::Controlling));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let now = Instant::now();
        input(&mut dispatcher, &mut session, &mut view, b"\x02,", now);
        if !matches!(response, RenameResponse::Unchanged) {
            input(
                &mut dispatcher,
                &mut session,
                &mut view,
                "\x15 exact 雪 \u{301}".as_bytes(),
                now,
            );
        }
        draw(&mut session, &view, Locale::EnUs, 120, 32)?;
        input(&mut dispatcher, &mut session, &mut view, b"\r", now);
        assert!(spin_with_draw(&mut session, |session| {
            let SessionBackend::Native(native) = &session.backend else {
                unreachable!()
            };
            !server.commands.lock().unwrap().is_empty()
                && (native.rename_receipts_for_test().0.is_none() || session.task.is_none())
        }));
        let command = server.commands.lock().unwrap()[0].clone();
        assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
        let SessionBackend::Native(native) = &session.backend else {
            unreachable!()
        };
        let (_, known, uncertain) = native.rename_receipts_for_test();
        match response {
            RenameResponse::Unchanged | RenameResponse::TabUnavailable => {
                assert!(known.is_none() && uncertain.is_none());
                assert_eq!(server.snapshots.load(Ordering::SeqCst), 0);
                assert_eq!(session.status(), TerminalStatus::Controlling);
            }
            RenameResponse::Unknown => {
                assert!(known.is_none());
                assert_eq!(uncertain.unwrap().request_id(), &command.request_id);
                assert!(session.task.is_none());
                assert_eq!(server.snapshots.load(Ordering::SeqCst), 0);
            }
            _ => {
                let known = known.unwrap();
                assert_eq!(known.operation().request_id(), &command.request_id);
                assert_eq!(known.revision(), WorkbenchRevision::new(2));
                assert_eq!(server.snapshots.load(Ordering::SeqCst), 1);
                assert!(uncertain.is_none());
                if matches!(
                    response,
                    RenameResponse::WrongLabel | RenameResponse::LostSnapshot
                ) {
                    assert!(session.task.is_none());
                } else if matches!(response, RenameResponse::Successor) {
                    assert!(spin_with_draw(
                        &mut session,
                        |session| matches!(session.view(), TerminalSessionView::Native(view) if view.phase() == NativeDisplayPhase::RetryableAttachUnavailable)
                    ));
                    assert!(server.detached.load(Ordering::SeqCst));
                    assert!(
                        matches!(session.view(), TerminalSessionView::Native(view) if view.active_position() == Some(1))
                    );
                } else if matches!(response, RenameResponse::Removed) {
                    assert!(server.detached.load(Ordering::SeqCst));
                    assert!(
                        matches!(session.view(), TerminalSessionView::Native(view) if view.phase() == NativeDisplayPhase::Empty && view.active_position().is_none())
                    );
                } else {
                    assert!(!server.detached.load(Ordering::SeqCst));
                    assert_eq!(session.status(), TerminalStatus::Controlling);
                }
            }
        }
        if matches!(response, RenameResponse::Recover) {
            assert!(
                matches!(session.view(), TerminalSessionView::Native(view) if view.phase() == NativeDisplayPhase::RenameProjectionUnavailable)
            );
            assert!(session.rename_input_context().is_none());
            TerminalInputPort::apply_client_action(
                &mut session,
                ClientAction::OpenWorkbenchNavigator,
            );
            assert!(spin_with_draw(
                &mut session,
                |session| matches!(session.view(), TerminalSessionView::Native(view) if view.phase() == NativeDisplayPhase::Ready)
            ));
            assert_eq!(server.snapshots.load(Ordering::SeqCst), 2);
            let SessionBackend::Native(native) = &session.backend else {
                unreachable!()
            };
            assert_eq!(
                native
                    .rename_receipts_for_test()
                    .1
                    .unwrap()
                    .operation()
                    .request_id(),
                &command.request_id
            );
        }
        assert_eq!(server.commands.lock().unwrap().len(), 1);
        server.finish(session)?;
    }
    Ok(())
}

#[test]
fn renamed_removed_target_successor_finishes_setup_feedback_and_accepts_input() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::with_response(geometry, RenameResponse::SuccessorReady)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"\x02,\x15renamed",
        now,
    );
    draw(&mut session, &view, Locale::EnUs, 120, 32)?;
    input(&mut dispatcher, &mut session, &mut view, b"\r", now);
    assert!(
        spin_with_draw(&mut session, |session| {
            server.successor_ready.load(Ordering::SeqCst)
                && session.status() == TerminalStatus::Controlling
        }),
        "successor must complete setup Poll and accepted draw feedback before Ready"
    );
    assert!(server.detached.load(Ordering::SeqCst));
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.active_position() == Some(1))
    );
    assert_eq!(server.snapshots.load(Ordering::SeqCst), 1);
    assert_eq!(server.commands.lock().unwrap().len(), 1);
    let SessionBackend::Native(native) = &session.backend else {
        unreachable!()
    };
    let (pending, known, unknown) = native.rename_receipts_for_test();
    assert!(pending.is_none() && unknown.is_none());
    assert_eq!(known.unwrap().revision(), WorkbenchRevision::new(2));
    input(
        &mut dispatcher,
        &mut session,
        &mut view,
        b"successor-input",
        now + Duration::from_secs(1),
    );
    spin(&mut session, |_| !server.inputs.lock().unwrap().is_empty());
    assert_eq!(*server.inputs.lock().unwrap(), b"successor-input");
    server.finish(session)?;
    Ok(())
}

#[test]
fn actual_navigator_to_rename_entry_dismisses_overlay_and_qualifies_only_the_editor() -> TestResult
{
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(&mut dispatcher, &mut session, &mut view, b"\x02w", now);
    assert_eq!(
        view.overlay(),
        Some(crate::client_view::Overlay::WorkbenchNavigator)
    );
    assert!(spin_with_draw(
        &mut session,
        |session| matches!(session.view(), TerminalSessionView::Native(view) if matches!(view.refresh_status(), Some(NativeRefreshStatus::Success(_))))
    ));
    input(&mut dispatcher, &mut session, &mut view, b"\x02,\r", now);
    assert_eq!(view.overlay(), None);
    assert!(server.commands.lock().unwrap().is_empty());
    let rendered = draw(&mut session, &view, Locale::EnUs, 120, 32)?;
    assert!(rendered.contains(Locale::EnUs.rename_title()));
    input(&mut dispatcher, &mut session, &mut view, b"\r", now);
    spin(&mut session, |_| {
        !server.commands.lock().unwrap().is_empty()
    });
    assert_eq!(server.commands.lock().unwrap().len(), 1);
    server.finish(session)?;
    Ok(())
}

/// Computes the scripted outcome for whichever Workbench read arrived. The first read before any
/// rename command is the bootstrap snapshot; later reads walk the ordinal script. `None` tells the
/// caller to drop the connection (`LostSnapshot`).
fn next_rename_snapshot(
    recorded: &Arc<Mutex<Vec<orchbee_workbench_protocol::WorkbenchCommand>>>,
    snapshot_count: &Arc<AtomicU8>,
    response: RenameResponse,
) -> Option<WorkbenchSnapshotOutcome> {
    if recorded.lock().unwrap().is_empty() {
        return Some(available_snapshot(Some(binding())));
    }
    let ordinal = snapshot_count.fetch_add(1, Ordering::SeqCst);
    if matches!(response, RenameResponse::LostSnapshot) {
        return None;
    }
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    snapshot.revision = WorkbenchRevision::new(if ordinal == 0 { 2 } else { 3 });
    if matches!(
        response,
        RenameResponse::Successor | RenameResponse::SuccessorReady
    ) {
        snapshot = super::move_refinements::two_tabs();
        snapshot.revision = WorkbenchRevision::new(3);
        snapshot.terminal_contexts[0].tabs.remove(0);
    } else if matches!(response, RenameResponse::Removed) {
        snapshot.revision = WorkbenchRevision::new(3);
        snapshot.terminal_contexts[0].tabs.clear();
    } else if let WorkbenchAction::TabRename { label, .. } =
        &recorded.lock().unwrap().last().unwrap().action
    {
        snapshot.terminal_contexts[0].tabs[0].label =
            if matches!(response, RenameResponse::WrongLabel) {
                "other".to_owned()
            } else {
                label.as_str().to_owned()
            };
    }
    Some(
        if matches!(response, RenameResponse::Recover) && ordinal == 0 {
            WorkbenchSnapshotOutcome::Denied {}
        } else {
            WorkbenchSnapshotOutcome::Available { snapshot }
        },
    )
}
