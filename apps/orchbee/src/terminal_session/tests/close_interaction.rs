use super::*;
use crate::tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabCloseReadOutcome, WorkbenchTabCloseReadResult, WorkbenchTabCloseResumeOutcome,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget, Wrap},
};
use std::{convert::Infallible, sync::atomic::AtomicUsize};

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
    dispatch_close_input_at(
        &mut InputDispatcher::default(),
        session,
        view,
        bytes,
        Instant::now(),
    );
}

fn dispatch_close_input_at(
    dispatcher: &mut InputDispatcher,
    session: &mut TerminalSession,
    view: &mut ClientView,
    bytes: &[u8],
    now: Instant,
) {
    assert_eq!(
        dispatch_input(dispatcher, view, bytes, session, now, |_, _| {
            Ok::<_, Infallible>(())
        }),
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
    let (buffer, _) = draw_close_frame(session, view, locale, width, height)?;
    Ok(frame_text(&buffer))
}

fn frame_text(buffer: &Buffer) -> String {
    let mut text = String::new();
    for row in buffer.content.chunks(usize::from(buffer.area.width)) {
        for cell in row {
            text.push_str(cell.symbol());
        }
        text.push('\n');
    }
    text
}

fn compact_display_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn draw_close_frame(
    session: &mut TerminalSession,
    view: &ClientView,
    locale: Locale,
    width: u16,
    height: u16,
) -> Result<(Buffer, FrameHitMap), Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    let mut receipt = NativeDrawReceipt::default();
    let mut hits = HitMapBuilder::new();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            view,
            locale,
            Some(session.view()),
            &crate::tui_theme::Theme::terminal_native(),
            &mut hits,
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
    Ok((terminal.backend().buffer().clone(), hits.finish()))
}

fn displayed_content_rows(buffer: &Buffer, area: Rect) -> Vec<String> {
    (area.y..area.bottom())
        .map(|y| {
            let mut row = String::new();
            let mut x = area.x;
            while x < area.right() {
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                // A wide glyph already represents its continuation cells.
                x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
            }
            row.trim().to_owned()
        })
        .collect()
}

fn close_modal_geometry(
    buffer: &Buffer,
    hits: &FrameHitMap,
    view: &ClientView,
) -> (crate::tui_layout::ShellAreas, Rect, Rect) {
    let shell = crate::tui_layout::project_shell_areas(buffer.area, view.sidebar_surface());
    let modal_scope = crate::tui_layout::overlay_area(&shell);
    assert!(!shell.status.is_empty());
    assert!((shell.status.y..shell.status.bottom()).all(|y| {
        (shell.status.x..shell.status.right())
            .all(|x| matches!(hits.hit_target_at(x, y), Some(HitTarget::OverlaySurface)))
    }));
    let surface: Vec<_> = (modal_scope.y..modal_scope.bottom())
        .flat_map(|y| (modal_scope.x..modal_scope.right()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            !shell.status.contains((*x, *y).into())
                && matches!(hits.hit_target_at(*x, *y), Some(HitTarget::OverlaySurface))
        })
        .collect();
    let left = surface
        .iter()
        .map(|(x, _)| *x)
        .min()
        .expect("painted close modal surface");
    let right = surface.iter().map(|(x, _)| *x).max().unwrap() + 1;
    let top = surface.iter().map(|(_, y)| *y).min().unwrap();
    let bottom = surface.iter().map(|(_, y)| *y).max().unwrap() + 1;
    let outer = Rect::new(left, top, right - left, bottom - top);
    assert_eq!(outer.intersection(modal_scope), outer);
    assert!(outer.intersection(shell.status).is_empty());
    let inner = Rect::new(left + 3, top + 1, outer.width - 6, outer.height - 2);
    (shell, outer, inner)
}

fn assert_modal_contains_text(
    buffer: &Buffer,
    hits: &FrameHitMap,
    view: &ClientView,
    expected: &str,
) {
    let (_, _, inner) = close_modal_geometry(buffer, hits, view);
    let rows = displayed_content_rows(buffer, inner);
    let expected = compact_display_text(expected);
    let complete_text_is_painted = (0..rows.len()).any(|start| {
        let mut candidate = String::new();
        for row in &rows[start..] {
            if row.is_empty() {
                break;
            }
            candidate.push_str(&compact_display_text(row));
            if candidate == expected {
                return true;
            }
            if !expected.starts_with(&candidate) {
                break;
            }
        }
        false
    });
    assert!(
        complete_text_is_painted,
        "complete modal text {expected:?} must occupy consecutive rows inside {inner:?}: {rows:?}"
    );
}

fn assert_complete_close_frame(
    buffer: &Buffer,
    hits: &FrameHitMap,
    token: NativeCloseConfirmationToken,
    locale: Locale,
    view: &ClientView,
) {
    let (shell, outer, inner) = close_modal_geometry(buffer, hits, view);
    // OverlaySurface is also the separately owned status-strip blocker. The
    // shared geometry helper clips modal text to this dialog's exact hit scope.
    let left = outer.x;
    let right = outer.right();
    let top = outer.y;
    let bottom = outer.bottom();
    let title = locale.close_title(NativeCloseConfirmationKind::Close);
    let theme = crate::tui_theme::Theme::terminal_native();
    let mut border = Buffer::empty(outer);
    crate::tui_render::overlay_surface(&theme, ratatui::widgets::Padding::ZERO)
        .title(crate::tui_render::overlay_title(title, outer.width, &theme))
        .render(outer, &mut border);
    let is_close = |x: u16| matches!(hits.hit_target_at(x, top), Some(HitTarget::TabClose { .. }));
    for y in buffer.area.y..buffer.area.bottom() {
        for x in buffer.area.x..buffer.area.right() {
            let point = (x, y).into();
            let hit = hits.hit_target_at(x, y);
            if matches!(hit, Some(HitTarget::OverlaySurface)) {
                assert!(
                    outer.contains(point) || shell.status.contains(point),
                    "no extra surface outside the exact modal and status owners"
                );
            }
            if outer.contains(point) {
                assert!(
                    matches!(
                        hit,
                        Some(HitTarget::OverlaySurface | HitTarget::TabClose { .. })
                    ),
                    "every modal cell keeps its own surface or decision target"
                );
            }
            if let Some(HitTarget::TabClose { token: actual, .. }) = hits.hit_target_at(x, y) {
                assert_eq!(*actual, token);
                assert!(
                    outer.contains((x, y).into()),
                    "no Close hit outside its modal"
                );
            }
            let boundary = x == left || x + 1 == right || y == top || y + 1 == bottom;
            // The frame close keeps one blank on each side between it and the top rule.
            let beside_close =
                y == top && !is_close(x) && (is_close(x + 1) || x > 0 && is_close(x - 1));
            if beside_close {
                assert_eq!(buffer[(x, y)].symbol(), " ", "close gap at {x},{y}");
            } else if outer.contains((x, y).into())
                && boundary
                && !matches!(hits.hit_target_at(x, y), Some(HitTarget::TabClose { .. }))
            {
                assert_eq!(
                    buffer[(x, y)].symbol(),
                    border[(x, y)].symbol(),
                    "complete border at {x},{y}"
                );
            }
        }
    }
    let close: Vec<_> = (left..right)
        .filter(|x| matches!(hits.hit_target_at(*x, top), Some(HitTarget::TabClose { token: actual, confirm: false }) if *actual == token))
        .collect();
    assert_eq!(close.len(), 3);
    assert_eq!(
        displayed_content_rows(buffer, Rect::new(close[0], top, 3, 1)),
        ["[x]"]
    );

    let warning = Paragraph::new(locale.close_warning()).wrap(Wrap { trim: false });
    let warning_area = Rect::new(
        0,
        0,
        inner.width,
        u16::try_from(warning.line_count(inner.width)).unwrap(),
    );
    let mut expected_warning = Buffer::empty(warning_area);
    warning.render(warning_area, &mut expected_warning);
    let mut expected = vec![String::new(), "initial_workspace".to_owned(), String::new()];
    expected.extend(displayed_content_rows(&expected_warning, warning_area));
    expected.extend([
        String::new(),
        format!(
            "[ {}  Esc ]   [ {}  Enter ]",
            locale.tab_close_cancel_label(),
            locale.tab_close_confirm_label()
        ),
    ]);
    assert_eq!(
        displayed_content_rows(buffer, inner),
        expected,
        "{locale:?}: exact complete content rows"
    );
    for confirm in [false, true] {
        assert!((inner.x..inner.right()).any(|x| matches!(
            hits.hit_target_at(x, inner.bottom() - 1),
            Some(HitTarget::TabClose { token: actual, confirm: choice }) if *actual == token && *choice == confirm
        )), "both painted decision buttons keep their exact token");
    }
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
        let (buffer, hits) = draw_close_frame(&mut session, &view, locale, 120, 40)?;
        let rendered = frame_text(&buffer);
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
        assert_modal_contains_text(&buffer, &hits, &view, locale.close_warning());
        assert!(!rendered.contains('\x1b'));
    }
    dispatch_close_input(&mut session, &mut view, b"n");
    drop(session);
    server.finish()?;
    Ok(())
}

#[test]
fn narrow_localized_close_keeps_complete_rows_and_tokens_through_suspend_cancel_and_enter()
-> TestResult {
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
        let mut view = ClientView::new(80);
        let mut dispatcher = InputDispatcher::default();
        let now = Instant::now();
        dispatch_close_input_at(&mut dispatcher, &mut session, &mut view, b"\x02x\r", now);
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        let token = match session.view() {
            TerminalSessionView::Native(native) => native.close_confirmation().unwrap().token(),
        };
        let (buffer, hits) = draw_close_frame(&mut session, &view, locale, 80, 24)?;
        assert_complete_close_frame(&buffer, &hits, token, locale, &view);
        dispatcher.publish_hit_map(hits);
        let (_, hidden_hits) = draw_close_frame(&mut session, &view, locale, 20, 8)?;
        assert!((0..8).all(|y| (0..20).all(|x| !matches!(
            hidden_hits.hit_target_at(x, y),
            Some(HitTarget::TabClose { .. })
        ))));
        dispatcher.publish_hit_map(hidden_hits);
        assert!(matches!(session.view(), TerminalSessionView::Native(native)
            if native.close_confirmation().is_some_and(|prompt| prompt.token() == token && !prompt.interactive())));
        dispatch_close_input_at(&mut dispatcher, &mut session, &mut view, b"\r", now);
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        let (buffer, hits) = draw_close_frame(&mut session, &view, locale, 80, 24)?;
        assert_complete_close_frame(&buffer, &hits, token, locale, &view);
        dispatcher.publish_hit_map(hits);
        // A lone Escape stays pending until the same decoder crosses its idle
        // deadline. Dropping a one-packet dispatcher never delivers Cancel.
        dispatch_close_input_at(&mut dispatcher, &mut session, &mut view, b"\x1b", now);
        assert!(matches!(session.view(), TerminalSessionView::Native(native)
            if native.close_confirmation().is_some_and(|prompt| prompt.token() == token && prompt.interactive())));
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
        let now = now + Duration::from_millis(60);
        dispatcher.synchronize(&mut view, &mut session, now, &mut |_| {
            Ok::<_, Infallible>(())
        })?;
        assert!(
            matches!(session.view(), TerminalSessionView::Native(native) if native.close_confirmation().is_none())
        );
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
        dispatch_close_input_at(&mut dispatcher, &mut session, &mut view, b"\x02x\r", now);
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        let next = match session.view() {
            TerminalSessionView::Native(native) => native.close_confirmation().unwrap().token(),
        };
        assert_ne!(next, token);
        let (buffer, hits) = draw_close_frame(&mut session, &view, locale, 80, 24)?;
        assert_complete_close_frame(&buffer, &hits, next, locale, &view);
        dispatcher.publish_hit_map(hits);
        dispatch_close_input_at(&mut dispatcher, &mut session, &mut view, b"\r", now);
        spin(&mut session, |_| commands.load(Ordering::SeqCst) == 1);
        assert_eq!(commands.load(Ordering::SeqCst), 1);
        assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
        server.finish(session)?;
    }
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
        dispatch_close_input(&mut session, &mut view, b"\x02x\r");
        assert_eq!(commands.load(Ordering::SeqCst), 0);
        let (buffer, hits) = draw_close_frame(&mut session, &view, locale, 120, 32)?;
        assert_modal_contains_text(&buffer, &hits, &view, locale.close_warning());
        dispatch_close_input(&mut session, &mut view, b"\ryy\x02q");
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
fn nondecision_packets_swallowed_and_unpresentable_redraw_suspends_prompt() -> TestResult {
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
    let token = match session.view() {
        TerminalSessionView::Native(view) => view.close_confirmation().unwrap().token(),
    };
    // Geometry loss suspends an admitted prompt. The hidden owner still fences
    // ordinary decisions, but cannot submit until the same token paints again.
    let _ = draw_close(&mut session, &view, Locale::EnUs, 20, 8)?;
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view)
            if view.close_confirmation().is_some_and(|prompt| {
                prompt.token() == token
                    && !prompt.interactive()
                    && prompt.presentation()
                        == crate::tui_presentation::PresentationPhase::Suspended
            })
    ));
    dispatch_close_input(&mut session, &mut view, b"\r");
    assert_eq!(commands.load(Ordering::SeqCst), 0);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
    let _ = draw_close(&mut session, &view, Locale::EnUs, 120, 32)?;
    assert!(matches!(
        session.view(),
        TerminalSessionView::Native(view)
            if view.close_confirmation().is_some_and(|prompt| {
                prompt.token() == token && prompt.interactive()
            })
    ));
    dispatch_close_input(&mut session, &mut view, b"\r");
    spin(&mut session, |_| commands.load(Ordering::SeqCst) == 1);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
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
    dispatch_close_input(&mut session, &mut view, b"\r");
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
    dispatch_close_input(&mut session, &mut view, b"\ryy");
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
        let TerminalSessionView::Native(native) = session.view();
        let status = native
            .close_status()
            .expect("Resume refusal retains the known status");
        assert_eq!(status.status(), NativeCloseStatusKind::CleanupBlocked);
        assert!(status.unavailable());
        assert!(!status.resumable());
        assert!(
            rendered.replace(' ', "").contains(
                &locale
                    .shell()
                    .bound_hint(&view, &locale.native_close_status(&status))
                    .replace(' ', "")
            )
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
        // armed at the loop-entry draw, still armed after the invalidated Enter,
        // still armed while the lone Esc waits out its decode deadline, gone
        // once the wait resolves at the next packet boundary.
        [true, true, true, false],
        "same-iteration resize+Enter must preserve the real current prompt until explicit Esc"
    );
    assert_eq!(commands.load(Ordering::SeqCst), 0);
    assert_eq!(input_bytes.load(Ordering::SeqCst), 0);
    server.finish(session)?;
    Ok(())
}
