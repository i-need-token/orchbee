use super::*;
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
};
use std::sync::atomic::AtomicUsize;

struct CloseTestServer {
    server: FakeServer,
    teardown: Arc<AtomicBool>,
}

impl CloseTestServer {
    fn finish(self, session: TerminalSession) -> io::Result<()> {
        self.teardown.store(true, Ordering::SeqCst);
        drop(session);
        self.server.finish()
    }
}

fn peer_disconnected(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    }) || error.source().is_some_and(peer_disconnected)
}

fn dispatch_close_input(session: &mut TerminalSession, view: &mut ClientView, bytes: &[u8]) {
    assert_eq!(
        dispatch_input(
            &mut InputDispatcher::default(),
            view,
            bytes,
            session,
            Instant::now(),
            |_, _| Ok::<_, std::convert::Infallible>(())
        ),
        Ok(DispatchOutcome::Continue),
    );
}

fn draw_close(
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
        );
    })?;
    if matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_some())
    {
        assert!(
            !terminal.backend().cursor_visible(),
            "Close prompt and fallback must hide the old PTY cursor"
        );
    }
    session.after_close_draw(receipt);
    let mut text = String::new();
    for row in terminal
        .backend()
        .buffer()
        .content
        .chunks(usize::from(width))
    {
        for cell in row {
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    Ok(text)
}

fn interactive_close_server(
    geometry: Geometry,
    commands: Arc<AtomicUsize>,
    input_bytes: Arc<AtomicUsize>,
) -> io::Result<CloseTestServer> {
    status_close_server(geometry, commands, input_bytes, None)
}

fn status_close_server(
    geometry: Geometry,
    commands: Arc<AtomicUsize>,
    input_bytes: Arc<AtomicUsize>,
    resumes: Option<Arc<AtomicUsize>>,
) -> io::Result<CloseTestServer> {
    let teardown = Arc::new(AtomicBool::new(false));
    let server_teardown = Arc::clone(&teardown);
    let server = FakeServer::start(move |mut stream| {
        let result = (|| {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let mut operation: Option<WorkbenchRequestId> = None;
            loop {
                let mut magic = [0; 4];
                if stream.read_exact(&mut magic).is_err() {
                    return Ok(());
                }
                let mut reader = io::Cursor::new(magic).chain(&mut stream);
                if magic == *b"OBWP" {
                    let request = read_workbench_request(&mut reader).map_err(io::Error::other)?;
                    let command = match request {
                        // Publication probes ride the ordinary polls this loop serves.
                        WorkbenchRequest::PublicationRead(intent) => {
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: intent.request_id,
                                    outcome: WorkbenchPublicationOutcome::Current {
                                        revision: intent.known_revision,
                                    },
                                }),
                            )
                            .map_err(io::Error::other)?;
                            continue;
                        }
                        WorkbenchRequest::Command(command) => command,
                        WorkbenchRequest::TabCloseRead(request) if resumes.is_some() => {
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::TabCloseRead(WorkbenchTabCloseReadResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: request.request_id,
                                    outcome: WorkbenchTabCloseReadOutcome::Available {
                                        close_request_id: operation.clone().unwrap(),
                                        terminal_context_id: request.terminal_context_id,
                                        tab_id: request.tab_id,
                                        attempt_generation:
                                            WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                                        status: WorkbenchTabCloseStatus::CleanupBlocked {
                                            reason: WorkbenchTabCloseBlockedReason::HostRefused,
                                            resumable: true,
                                        },
                                    },
                                }),
                            )
                            .map_err(io::Error::other)?;
                            continue;
                        }
                        WorkbenchRequest::TabCloseResume(request) if resumes.is_some() => {
                            assert_eq!(Some(&request.close_request_id), operation.as_ref());
                            assert_eq!(request.expected_attempt_generation.value(), 1);
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: request.request_id,
                                    site_id: request.site_id,
                                    terminal_context_id: request.terminal_context_id,
                                    tab_id: request.tab_id,
                                    close_request_id: request.close_request_id,
                                    outcome: WorkbenchTabCloseResumeOutcome::Denied {},
                                }),
                            )
                            .map_err(io::Error::other)?;
                            resumes.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                        other => {
                            return Err(io::Error::other(format!(
                                "unexpected Workbench operation: {other:?}"
                            )));
                        }
                    };
                    assert!(
                        matches!(command.action, WorkbenchAction::TabClose { ref terminal_context_id, ref tab_id } if terminal_context_id.as_str() == ID && tab_id.as_str() == ID)
                    );
                    let outcome = if resumes.is_some() {
                        operation = Some(command.request_id.clone());
                        WorkbenchCommandOutcome::TabCloseAccepted {
                            close_request_id: command.request_id.clone(),
                            terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                            tab_id: WorkbenchTabId::parse(ID).unwrap(),
                            accepted_revision: command.expected_revision,
                        }
                    } else {
                        WorkbenchCommandOutcome::Denied {}
                    };
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Command(WorkbenchCommandResult {
                            protocol_version: ProtocolVersion,
                            request_id: command.request_id,
                            outcome,
                        }),
                    )
                    .map_err(io::Error::other)?;
                    commands.fetch_add(1, Ordering::SeqCst);
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
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Empty {},
                            )),
                        )
                        .map_err(io::Error::other)?;
                    }
                    ClientMessage::TerminalControl(request) => {
                        let outcome = match request.action() {
                            TerminalControlAction::Input {
                                attachment_id,
                                bytes,
                                ..
                            } => {
                                assert_eq!(attachment_id, &binding_viewer_id()?);
                                input_bytes.fetch_add(bytes.as_bytes().len(), Ordering::SeqCst);
                                TerminalControlOutcome::InputAccepted {}
                            }
                            TerminalControlAction::Resize {
                                attachment_id,
                                geometry,
                                ..
                            } => {
                                assert_eq!(attachment_id, &binding_viewer_id()?);
                                TerminalControlOutcome::Resized {
                                    geometry: *geometry,
                                }
                            }
                            other => {
                                return Err(io::Error::other(format!(
                                    "unexpected control: {other:?}"
                                )));
                            }
                        };
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                outcome,
                            )),
                        )
                        .map_err(io::Error::other)?;
                    }
                    other => {
                        return Err(io::Error::other(format!(
                            "unexpected terminal operation: {other:?}"
                        )));
                    }
                }
            }
        })();
        // The asserted interaction is complete before finish starts. A reply racing the
        // intentional client shutdown may fail; all earlier transport failures remain errors.
        match result {
            Err(error) if server_teardown.load(Ordering::SeqCst) && peer_disconnected(&error) => {
                Ok(())
            }
            result => result,
        }
    })?;
    Ok(CloseTestServer { server, teardown })
}

#[test]
fn actual_close_prompt_fits_a_sanitized_maximum_length_unicode_target() -> TestResult {
    let label = format!("{}\x1b\n", "界".repeat(510));
    let safe = crate::terminal_text::sanitize_terminal_text(&label).into_owned();
    let geometry = fixture_geometry()?;
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    snapshot.terminal_contexts[0].tabs[0].label = label;
    let (poll_sent, poll_seen) = mpsc::sync_channel(0);
    let server = FakeServer::start(move |mut stream| {
        serve_native_controlling_frame_with_snapshot(
            &mut stream,
            geometry,
            WorkbenchSnapshotOutcome::Available { snapshot },
        )?;
        accept_native_frame_feedback(&mut stream)?;
        let _poll = read_client_message(&mut stream).map_err(io::Error::other)?;
        poll_sent.send(()).map_err(io::Error::other)?;
        let mut trailing = Vec::new();
        stream.read_to_end(&mut trailing)?;
        assert!(trailing.is_empty());
        Ok(())
    })?;
    let (mut session, _) = native_session_with_identity(native_client(&server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    poll_seen.recv_timeout(Duration::from_secs(1))?;
    let mut view = ClientView::new(120);
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rendered = draw_close(&mut session, &view, locale, 120, 40)?;
        let compact = rendered.replace([' ', '\n'], "");
        // Wrapped rows contain borders between label segments. Reconstruct only target
        // scalars and their sanitization replacements, preserving their displayed order.
        // The finite modal no longer covers the whole frame — the tab strip shows
        // the same label truncated — so the complete target must appear as one
        // contiguous visible run.
        let visible_target: String = rendered
            .chars()
            .filter(|character| matches!(character, '界' | '\u{fffd}'))
            .collect();
        assert!(visible_target.contains(&safe));
        assert!(compact.contains(&locale.close_warning().replace(' ', "")));
        assert!(!rendered.contains('\x1b'));
    }
    dispatch_close_input(&mut session, &mut view, b"n");
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn raw_close_packet_requires_actual_localized_draw_and_emits_one_command() -> TestResult {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let geometry = fixture_geometry()?;
        let commands = Arc::new(AtomicUsize::new(0));
        let input_bytes = Arc::new(AtomicUsize::new(0));
        let server =
            interactive_close_server(geometry, Arc::clone(&commands), Arc::clone(&input_bytes))?;
        let (mut session, _) =
            native_session_with_identity(native_client(&server.server)?, geometry)?;
        assert!(spin_with_draw(&mut session, |session| session.status()
            == TerminalStatus::Controlling));
        let mut view = ClientView::new(120);
        dispatch_close_input(&mut session, &mut view, b"\x02xy");
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        let rendered = draw_close(&mut session, &view, locale, 120, 32)?;
        assert!(
            rendered
                .replace(' ', "")
                .contains(&locale.close_warning().replace(' ', ""))
        );
        dispatch_close_input(&mut session, &mut view, b"yyy\x02q");
        spin(&mut session, |session| {
            matches!(session.view(), TerminalSessionView::Native(view)
                if view.notice() == Some(NativeSessionNotice::CloseRequestNotApplied))
        });
        let rendered = draw_close(&mut session, &view, locale, 120, 32)?;
        assert!(
            rendered.replace(' ', "").contains(
                &locale
                    .native_notice(NativeSessionNotice::CloseRequestNotApplied)
                    .replace(' ', "")
            )
        );
        assert_eq!(commands.load(Ordering::SeqCst), 1);
        assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
        server.finish(session)?;
    }
    Ok(())
}

#[test]
fn cancel_packets_swallowed_and_unpresentable_redraw_cancels_prompt_with_notice() -> TestResult {
    let geometry = fixture_geometry()?;
    let commands = Arc::new(AtomicUsize::new(0));
    let input_bytes = Arc::new(AtomicUsize::new(0));
    let server =
        interactive_close_server(geometry, Arc::clone(&commands), Arc::clone(&input_bytes))?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut view = ClientView::new(120);
    for cancel in [b"ntrailing".as_slice(), b"\x1b[Atrailing".as_slice()] {
        dispatch_close_input(&mut session, &mut view, b"\x02x");
        let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
        dispatch_close_input(&mut session, &mut view, cancel);
    }
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    // A draw that cannot present the complete prompt cancels it outright with
    // a bounded notice — `y` then belongs to the terminal, not a hidden modal.
    let _ = draw_close(&mut session, &view, Locale::EnUs, 20, 8)?;
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view)
            if view.close_confirmation().is_none()
                && view.notice() == Some(NativeSessionNotice::ConfirmationCancelled)
    ));
    dispatch_close_input(&mut session, &mut view, b"y");
    assert_eq!(commands.load(Ordering::SeqCst), 0);
    spin(&mut session, |_| input_bytes.load(Ordering::SeqCst) == 1);
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    dispatch_close_input(&mut session, &mut view, b"y");
    spin(&mut session, |_| commands.load(Ordering::SeqCst) == 1);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 1);
    server.finish(session)?;
    Ok(())
}

#[test]
fn resume_requires_visible_status_and_the_complete_modal_returns_both_current_tokens() -> TestResult
{
    let geometry = fixture_geometry()?;
    let commands = Arc::new(AtomicUsize::new(0));
    let resumes = Arc::new(AtomicUsize::new(0));
    let input_bytes = Arc::new(AtomicUsize::new(0));
    let server = status_close_server(
        geometry,
        Arc::clone(&commands),
        Arc::clone(&input_bytes),
        Some(Arc::clone(&resumes)),
    )?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    let mut view = ClientView::new(120);
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    dispatch_close_input(&mut session, &mut view, b"y");
    spin(&mut session, |session| {
        matches!(session.view(), TerminalSessionView::Native(view)
        if view.close_status().is_some_and(|status| status.status() == NativeCloseStatusKind::CleanupBlocked))
    });
    let _ = draw_close(&mut session, &view, Locale::EnUs, 30, 12)?;
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_none())
    );
    view.report_backend_unavailable(crate::client_view::BackendAction::CreateTab);
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_none())
    );
    view = ClientView::new(120);
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    assert!(
        matches!(session.view(), TerminalSessionView::Native(view) if view.close_confirmation().is_some_and(|prompt| prompt.kind() == NativeCloseConfirmationKind::Resume))
    );
    // The notice replaces the footer. Only the exact status inside the modal can qualify this draw.
    view.report_backend_unavailable(crate::client_view::BackendAction::CreateTab);
    let rendered = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    assert!(rendered.contains("Close cleanup blocked"));
    dispatch_close_input(&mut session, &mut view, b"yyy");
    spin(&mut session, |session| {
        matches!(session.view(), TerminalSessionView::Native(view)
            if view.notice() == Some(NativeSessionNotice::CloseRequestNotApplied))
    });
    view = ClientView::new(200);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rendered = draw_close(&mut session, &view, locale, 200, 32)?;
        assert!(
            rendered.replace(' ', "").contains(
                &locale
                    .native_notice(NativeSessionNotice::CloseRequestNotApplied)
                    .replace(' ', "")
            )
        );
        let TerminalSessionView::Native(native) = session.view() else {
            unreachable!()
        };
        let status = native
            .close_status()
            .expect("Resume refusal retains the known status");
        assert_eq!(status.status(), NativeCloseStatusKind::CleanupBlocked);
        assert!(status.unavailable());
        assert!(!status.resumable());
        assert!(
            rendered
                .replace(' ', "")
                .contains(&locale.native_close_status(&status).replace(' ', ""))
        );
    }
    dispatch_close_input(&mut session, &mut view, b"\x02x");
    assert!(matches!(session.view(), TerminalSessionView::Native(view)
        if view.close_confirmation().is_none()));
    assert_eq!(commands.load(Ordering::SeqCst), 1);
    assert_eq!(resumes.load(Ordering::SeqCst), 1);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
    server.finish(session)?;
    Ok(())
}

#[test]
fn real_shell_resize_and_confirm_in_one_iteration_keeps_the_native_prompt_unsubmitted() -> TestResult
{
    let geometry = fixture_geometry()?;
    let commands = Arc::new(AtomicUsize::new(0));
    let input_bytes = Arc::new(AtomicUsize::new(0));
    let server =
        interactive_close_server(geometry, Arc::clone(&commands), Arc::clone(&input_bytes))?;
    let (mut session, _) = native_session_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(&mut session, |session| session.status()
        == TerminalStatus::Controlling));
    dispatch_close_input(&mut session, &mut ClientView::new(100), b"\x02x");
    assert_eq!(
        crate::tui_shell::tests::run_native_close_resize_confirmation(&mut session)?,
        [true, true, false],
        "same-iteration resize+y must preserve the real current prompt until explicit Esc"
    );
    assert_eq!(commands.load(Ordering::SeqCst), 0);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
    server.finish(session)?;
    Ok(())
}
