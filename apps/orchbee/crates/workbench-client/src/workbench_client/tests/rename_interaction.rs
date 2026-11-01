use super::*;
use crate::terminal_input::InputDispatcher;

fn input(
    dispatcher: &mut InputDispatcher,
    workbench_client: &mut WorkbenchClient,
    view: &mut ClientView,
    bytes: &[u8],
    now: Instant,
) {
    assert_eq!(
        dispatcher
            .dispatch(
                view,
                bytes,
                workbench_client,
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(())
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
}

fn draw(
    workbench_client: &mut WorkbenchClient,
    view: &ClientView,
    locale: Locale,
    width: u16,
    height: u16,
) -> Result<String, Box<dyn std::error::Error>> {
    draw_frame(workbench_client, view, locale, width, height, None)
}

fn draw_frame(
    workbench_client: &mut WorkbenchClient,
    view: &ClientView,
    locale: Locale,
    width: u16,
    height: u16,
    dispatcher: Option<&mut InputDispatcher>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    let mut receipt = DrawReceipt::default();
    let mut hits = crate::tui_hit_map::HitMapBuilder::new();
    terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            view,
            locale,
            Some(workbench_client.view()),
            &crate::tui_theme::Theme::terminal_native(),
            &mut hits,
        )
    })?;
    // A presented text field places the terminal cursor inside the modal; every
    // other surface leaves it hidden.
    assert_eq!(
        terminal.backend().cursor_visible(),
        receipt.editing_cursor_shown()
    );
    if let Some(dispatcher) = dispatcher {
        dispatcher.publish_hit_map(hits.finish());
    }
    workbench_client.after_close_draw(receipt);
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
        let (mut workbench_client, _) =
            workbench_client_with_identity(native_client(&server.server)?, geometry)?;
        assert!(spin_with_draw(
            &mut workbench_client,
            |workbench_client| workbench_client.status() == TerminalStatus::Controlling
        ));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let now = Instant::now();
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            b"\x02,",
            now,
        );
        let token = workbench_client.rename_input_context().unwrap().token();
        assert!(server.commands.lock().unwrap().is_empty());
        let rendered = draw(&mut workbench_client, &view, locale, 120, 32)?;
        // TestBackend's wide-cell continuation slots contain spaces between Chinese glyphs.
        assert!(
            rendered.replace(' ', "").contains("[initial_workspace]"),
            "locale={locale:?}\n{rendered}"
        );
        assert_eq!(
            workbench_client.rename_input_context().unwrap().token(),
            token
        );
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            "\x15雪😀e\u{301}".as_bytes(),
            now,
        );
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            b"\r\n\x02q",
            now,
        );
        spin(&mut workbench_client, |_| {
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
        assert!(workbench_client.rename_input_context().is_none());
        server.finish(workbench_client)?;
    }
    Ok(())
}

#[test]
fn input_fault_survives_valid_edits_until_explicit_complete_backspace() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        now,
    );
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x15\xe2\x08a\x1b[200~B\x1b[201~",
        now,
    );
    assert!({
        let view = workbench_client.view();
        view.rename_draft().unwrap().input_fault()
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\r",
        now,
    );
    assert!(server.commands.lock().unwrap().is_empty());
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x08",
        now,
    );
    assert!({
        let view = workbench_client.view();
        !view.rename_draft().unwrap().input_fault()
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\r",
        now,
    );
    spin(&mut workbench_client, |_| {
        !server.commands.lock().unwrap().is_empty()
    });
    let commands = server.commands.lock().unwrap();
    assert!(
        matches!(&commands[0].action, WorkbenchAction::TabRename {label, ..} if label.as_str() == "a")
    );
    drop(commands);
    assert!(server.inputs.lock().unwrap().is_empty());
    server.finish(workbench_client)?;
    Ok(())
}

#[test]
fn clipped_draw_suspends_draft_and_cancelled_paste_keeps_its_frame_until_the_boundary() -> TestResult
{
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        now,
    );
    let token = workbench_client.rename_input_context().unwrap().token();
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
    assert!({
        let view = workbench_client.view();
        view.rename_draft().is_some_and(|draft| {
            draft.token() == token
                && draft.interactive()
                && draft.presentation() == crate::tui_presentation::PresentationPhase::Admitted
        })
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x15changed\x1b[D",
        now,
    );
    // Geometry loss suspends an admitted draft without losing its identity or
    // editor. While hidden, the owner swallows edits and submission.
    draw(&mut workbench_client, &view, Locale::EnUs, 12, 6)?;
    assert!(server.commands.lock().unwrap().is_empty());
    assert!({
        let view = workbench_client.view();
        view.rename_draft().is_some_and(|draft| {
            draft.token() == token
                && draft.presentation() == crate::tui_presentation::PresentationPhase::Suspended
                && draft
                    .field_window(80)
                    .is_some_and(|field| field.display() == "changed" && field.cursor_column() == 6)
        })
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"blocked\r",
        now,
    );
    assert_eq!(
        workbench_client.rename_input_context().unwrap().token(),
        token
    );
    assert!({
        let view = workbench_client.view();
        view.rename_draft().is_some_and(|draft| {
            draft
                .field_window(80)
                .is_some_and(|field| field.display() == "changed" && field.cursor_column() == 6)
        })
    });
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(server.inputs.lock().unwrap().is_empty());
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
    assert!({
        let view = workbench_client.view();
        view.rename_draft().is_some_and(|draft| {
            draft.token() == token
                && draft.interactive()
                && draft.presentation() == crate::tui_presentation::PresentationPhase::Admitted
                && draft
                    .field_window(80)
                    .is_some_and(|field| field.display() == "changed" && field.cursor_column() == 6)
        })
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x1b",
        now,
    );
    let cancel_boundary = now + Duration::from_millis(100);
    dispatcher.synchronize(
        &mut view,
        &mut workbench_client,
        cancel_boundary,
        &mut |_| Ok::<_, io::Error>(()),
    )?;
    assert!(workbench_client.rename_input_context().is_none());

    // A later packet can arm a fresh draft after explicit local cancellation.
    let later = cancel_boundary + Duration::from_secs(3600);
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        later,
    );
    let token = workbench_client.rename_input_context().unwrap().token();
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x15changed",
        later,
    );
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x1b[200~old",
        later,
    );
    workbench_client.surface_action(crate::terminal_input::SurfaceAction::Rename(
        crate::terminal_input::RenameAction::Cancel(token),
    ));
    let quiet = later + Duration::from_secs(3600);
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02q\r\x1b[201~tail",
        quiet,
    );
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(server.inputs.lock().unwrap().is_empty());
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"new",
        quiet + Duration::from_millis(50),
    );
    spin(&mut workbench_client, |_| {
        !server.inputs.lock().unwrap().is_empty()
    });
    assert_eq!(*server.inputs.lock().unwrap(), b"new");
    server.finish(workbench_client)?;
    Ok(())
}

#[test]
fn maximum_atomic_paste_draws_sanitized_cells_but_submits_exact_original_scalars() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    let label = format!("{}\x1b\n", "界".repeat(510));
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        now,
    );
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 40)?;
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x15\x1b[200~",
        now,
    );
    for byte in label.as_bytes() {
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            std::slice::from_ref(byte),
            now,
        );
    }
    assert!({
        let view = workbench_client.view();
        view.rename_draft()
            .unwrap()
            .field_window(80)
            .unwrap()
            .display()
            .is_empty()
    });
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x1b[201~",
        now,
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rendered = draw(&mut workbench_client, &view, locale, 120, 40)?;
        let displayed: String = rendered
            .chars()
            .filter(|character| matches!(character, '界' | '\u{fffd}'))
            .collect();
        // The field draws only the cursor's window over the projection — a
        // contiguous run of whole graphemes bounded by the inner width —
        // while the raw draft and its exact submission keep the full text.
        let projected = crate::terminal_text::sanitize_terminal_text(&label);
        let tail: String = projected
            .chars()
            .rev()
            .take(10)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        assert!(displayed.contains(&tail));
        assert!(displayed.matches('界').count() < projected.matches('界').count());
        assert!(!rendered.contains('\x1b'));
    }
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\r",
        now,
    );
    spin(&mut workbench_client, |_| {
        !server.commands.lock().unwrap().is_empty()
    });
    let commands = server.commands.lock().unwrap();
    assert!(
        matches!(&commands[0].action, WorkbenchAction::TabRename {label: submitted, ..} if submitted.as_str() == label)
    );
    drop(commands);
    server.finish(workbench_client)?;
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
        let (mut workbench_client, _) =
            workbench_client_with_identity(native_client(&server.server)?, geometry)?;
        assert!(spin_with_draw(
            &mut workbench_client,
            |workbench_client| workbench_client.status() == TerminalStatus::Controlling
        ));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let now = Instant::now();
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            b"\x02,",
            now,
        );
        draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
        if !matches!(response, RenameResponse::Unchanged) {
            input(
                &mut dispatcher,
                &mut workbench_client,
                &mut view,
                "\x15 exact 雪 \u{301}".as_bytes(),
                now,
            );
        }
        input(
            &mut dispatcher,
            &mut workbench_client,
            &mut view,
            b"\r",
            now,
        );
        assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
            let ClientBackend::Native(native) = &workbench_client.backend;
            !server.commands.lock().unwrap().is_empty()
                && (native.rename_receipts_for_test().0.is_none()
                    || workbench_client.task.is_none())
        }));
        let command = server.commands.lock().unwrap()[0].clone();
        assert_eq!(command.expected_revision, WorkbenchRevision::new(1));
        let ClientBackend::Native(native) = &workbench_client.backend;
        let (_, known, uncertain) = native.rename_receipts_for_test();
        match response {
            RenameResponse::Unchanged | RenameResponse::TabUnavailable => {
                assert!(known.is_none() && uncertain.is_none());
                assert_eq!(server.snapshots.load(Ordering::SeqCst), 0);
                assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
            }
            RenameResponse::Unknown => {
                assert!(known.is_none());
                assert_eq!(uncertain.unwrap().request_id(), &command.request_id);
                assert!(workbench_client.task.is_none());
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
                    assert!(workbench_client.task.is_none());
                } else if matches!(response, RenameResponse::Successor) {
                    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
                        let view = workbench_client.view();
                        view.phase() == DisplayPhase::RetryableAttachUnavailable
                    }));
                    assert!(server.detached.load(Ordering::SeqCst));
                    assert!({
                        let view = workbench_client.view();
                        view.active_position() == Some(1)
                    });
                } else if matches!(response, RenameResponse::Removed) {
                    assert!(server.detached.load(Ordering::SeqCst));
                    assert!({
                        let view = workbench_client.view();
                        view.phase() == DisplayPhase::Empty && view.active_position().is_none()
                    });
                } else {
                    assert!(!server.detached.load(Ordering::SeqCst));
                    assert_eq!(workbench_client.status(), TerminalStatus::Controlling);
                }
            }
        }
        if matches!(response, RenameResponse::Recover) {
            assert!({
                let view = workbench_client.view();
                view.phase() == DisplayPhase::RenameProjectionUnavailable
            });
            assert!(workbench_client.rename_input_context().is_none());
            TerminalInputPort::apply_client_action(
                &mut workbench_client,
                ClientAction::OpenWorkbenchNavigator.into(),
            );
            assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
                let view = workbench_client.view();
                view.phase() == DisplayPhase::Ready
            }));
            assert_eq!(server.snapshots.load(Ordering::SeqCst), 2);
            let ClientBackend::Native(native) = &workbench_client.backend;
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
        server.finish(workbench_client)?;
    }
    Ok(())
}

#[test]
fn renamed_removed_target_successor_finishes_setup_feedback_and_accepts_input() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::with_response(geometry, RenameResponse::SuccessorReady)?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        now,
    );
    draw(&mut workbench_client, &view, Locale::EnUs, 120, 32)?;
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x15renamed",
        now,
    );
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\r",
        now,
    );
    assert!(
        spin_with_draw(&mut workbench_client, |workbench_client| {
            server.successor_ready.load(Ordering::SeqCst)
                && workbench_client.status() == TerminalStatus::Controlling
        }),
        "successor must complete setup Poll and accepted draw feedback before Ready"
    );
    assert!(server.detached.load(Ordering::SeqCst));
    assert!({
        let view = workbench_client.view();
        view.active_position() == Some(1)
    });
    assert_eq!(server.snapshots.load(Ordering::SeqCst), 1);
    assert_eq!(server.commands.lock().unwrap().len(), 1);
    let ClientBackend::Native(native) = &workbench_client.backend;
    let (pending, known, unknown) = native.rename_receipts_for_test();
    assert!(pending.is_none() && unknown.is_none());
    assert_eq!(known.unwrap().revision(), WorkbenchRevision::new(2));
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"successor-input",
        now + Duration::from_secs(1),
    );
    spin(&mut workbench_client, |_| {
        !server.inputs.lock().unwrap().is_empty()
    });
    assert_eq!(*server.inputs.lock().unwrap(), b"successor-input");
    server.finish(workbench_client)?;
    Ok(())
}

#[test]
fn actual_navigator_to_rename_entry_dismisses_overlay_and_qualifies_only_the_editor() -> TestResult
{
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let (mut workbench_client, _) =
        workbench_client_with_identity(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let now = Instant::now();
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02w",
        now,
    );
    assert_eq!(
        view.overlay(),
        Some(crate::client_view::Overlay::WorkbenchNavigator)
    );
    assert!(spin_with_draw(&mut workbench_client, |workbench_client| {
        let view = workbench_client.view();
        matches!(view.refresh_status(), Some(RefreshStatus::Success(_)))
    }));
    {
        // Reconcile the completed refresh, then publish the actual navigator draw.
        input(&mut dispatcher, &mut workbench_client, &mut view, b"", now);
        let rendered = draw_frame(
            &mut workbench_client,
            &view,
            Locale::EnUs,
            120,
            32,
            Some(&mut dispatcher),
        )?;
        assert!(rendered.contains(Locale::EnUs.shell().navigator_title));
    }
    // A global prefix action is routed through the Navigator owner and replaces
    // it with the native rename editor instead of leaving the shortcut swallowed.
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\x02,",
        now,
    );
    assert_eq!(view.overlay(), None);
    assert!(server.commands.lock().unwrap().is_empty());
    assert!(server.inputs.lock().unwrap().is_empty());
    let token = workbench_client
        .rename_input_context()
        .expect("Navigator action creates the rename editor capture")
        .token();
    let rendered = draw_frame(
        &mut workbench_client,
        &view,
        Locale::EnUs,
        120,
        32,
        Some(&mut dispatcher),
    )?;
    assert!(
        rendered.replace(' ', "").contains("[initial_workspace]"),
        "{rendered}"
    );
    assert_eq!(
        workbench_client.rename_input_context().unwrap().token(),
        token
    );
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        "\x15雪😀e\u{301}".as_bytes(),
        now,
    );
    input(
        &mut dispatcher,
        &mut workbench_client,
        &mut view,
        b"\r\n\x02q",
        now,
    );
    spin(&mut workbench_client, |_| {
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
    assert!(workbench_client.rename_input_context().is_none());
    server.finish(workbench_client)?;
    Ok(())
}
