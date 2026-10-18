//! Real renderer maps drive bounded wheel work and complete pointer gestures.
use super::*;
use crate::{localization::Locale, terminal_session::ModalPresentation, tui_theme::Theme};

#[test]
fn hover_moves_only_the_menu_candidate_and_one_click_activates_the_painted_row() {
    for repaint in [0, 1, 2] {
        let mut port = Port::new(MenuAction::Detach);
        port.actions = vec![MenuAction::Detach, MenuAction::Help];
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        let y = y + 1;
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<35;{x};{y}M").as_bytes(),
        );
        assert_eq!(port.menu.view().unwrap().selected, 1);
        assert!(port.calls.borrow().is_empty(), "hover cannot execute");
        if repaint == 1 {
            paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}M").as_bytes(),
        );
        assert!(port.menu.active(), "press cannot execute");
        if repaint == 2 {
            paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        }
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<_, ()>(())
            })
            .unwrap();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}mtail").as_bytes(),
        );
        assert!(!port.menu.active());
        assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
        assert_eq!(
            *port.calls.borrow(),
            [Call::Action(ClientAction::OpenShortcutHelp)]
        );
    }
}

#[test]
fn hover_admission_leave_and_keyboard_navigation_preserve_menu_ownership() {
    let mut port = Port::new(MenuAction::Detach);
    port.actions = vec![MenuAction::Detach, MenuAction::Help, MenuAction::Reload];
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(50);
    dispatcher.set_pointer_capture(&mut view, &mut port, false);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    let motion = |code, row| format!("\x1b[<{code};{x};{row}M");
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        motion(35, y + 1).as_bytes(),
    );
    assert_eq!(
        port.menu.view().unwrap().selected,
        0,
        "keyboard menu does not re-arm released capture"
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        dispatcher.pointer_capture,
        PointerCaptureRequest::MenuMotion
    );
    paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    for code in [39, 43, 51, 32] {
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            motion(code, y + 1).as_bytes(),
        );
        assert_eq!(
            port.menu.view().unwrap().selected,
            0,
            "modified or held motion is inert"
        );
    }
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("{}{}", motion(35, y + 1), motion(35, y + 2)).as_bytes(),
    );
    assert_eq!(
        port.menu.view().unwrap().selected,
        2,
        "last pre-paint motion wins"
    );
    assert!(port.calls.borrow().is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<35;1;1M");
    assert_eq!(
        port.menu.view().unwrap().selected,
        2,
        "leaving retains the candidate"
    );
    paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[A");
    assert_eq!(
        port.menu.view().unwrap().selected,
        1,
        "arrows resume from hover"
    );
    assert!(port.calls.borrow().is_empty());
    paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    assert!(!port.menu.active());
    assert!(
        port.calls.borrow().is_empty(),
        "cancellation retains the underlying session"
    );
}

#[test]
fn captured_menu_press_loses_authority_on_owner_source_geometry_or_transport_retirement() {
    #[derive(Debug)]
    enum Retirement {
        CoveringOwner,
        Source,
        Geometry,
        Capture,
        Focus,
    }
    for retirement in [
        Retirement::CoveringOwner,
        Retirement::Source,
        Retirement::Geometry,
        Retirement::Capture,
        Retirement::Focus,
    ] {
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}M").as_bytes(),
        );
        paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        match retirement {
            Retirement::CoveringOwner => {
                port.settings_capture = true;
                dispatcher
                    .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                port.settings_capture = false;
            }
            Retirement::Source => {
                port.menu.cancel();
                assert!(port.menu.open_for(
                    MenuTarget::Create(None),
                    String::new(),
                    vec![MenuAction::Help],
                    None,
                ));
            }
            Retirement::Geometry => dispatcher.invalidate_hit_map(&mut port),
            Retirement::Capture => dispatcher.set_pointer_capture(&mut view, &mut port, false),
            Retirement::Focus => {
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
            }
        }
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<_, ()>(())
            })
            .unwrap();
        if port.menu.active() {
            paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        } else {
            dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}mtail").as_bytes(),
        );
        assert_eq!(
            port.menu.active(),
            !matches!(retirement, Retirement::Capture),
            "{retirement:?} cannot activate an old press"
        );
        assert!(
            port.calls.borrow().is_empty(),
            "{retirement:?} retains its packet"
        );
    }
}

#[test]
fn hover_and_keyboard_bursts_require_a_draw_before_their_candidate_can_execute() {
    let mut port = Port::new(MenuAction::Help);
    port.actions = vec![MenuAction::Detach, MenuAction::Reload, MenuAction::Help];
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(50);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
    let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<35;{x};{}M\r", y + 2).as_bytes(),
    );
    assert_eq!(port.menu.view().unwrap().selected, 2);
    assert_eq!(port.menu.confirmed(), None);
    assert!(
        port.calls.borrow().is_empty(),
        "Enter cannot accept an unpainted hover"
    );

    paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<35;{x};{}M\x1b[B\r", y + 1).as_bytes(),
    );
    assert_eq!(
        port.menu.view().unwrap().selected,
        2,
        "arrows follow the hover in the same packet"
    );
    assert_eq!(port.menu.confirmed(), None);
    assert!(
        port.calls.borrow().is_empty(),
        "keyboard navigation still requires its new draw"
    );

    paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    assert!(!port.menu.active());
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    assert_eq!(
        *port.calls.borrow(),
        [Call::Action(ClientAction::OpenShortcutHelp)]
    );
}

fn paint(
    dispatcher: &mut InputDispatcher,
    port: &mut Port,
    bindings: &crate::client_view::Bindings,
    locale: Locale,
) -> (u16, u16, usize, Option<(u16, u16)>) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(50, 10)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            draw = crate::tui_render::render_action_menu_for_test(
                frame,
                frame.area(),
                port.menu.view().unwrap(),
                bindings,
                locale,
                &Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = draw else {
        panic!("menu fits");
    };
    let map = hits.finish();
    let cell = (0..10)
        .find_map(|y| {
            (0..50).find_map(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::ActionMenuRow { .. })
                )
                .then_some((x, y))
            })
        })
        .unwrap();
    let close = (0..10).find_map(|y| {
        (0..50).find_map(|x| {
            matches!(
                map.hit_target_at(x, y),
                Some(HitTarget::ActionMenuControl { back: false, .. })
            )
            .then_some((x + 1, y + 1))
        })
    });
    assert_eq!(close.is_none(), port.menu.view().unwrap().context_menu);
    let visible_rows = draw.visible_rows;
    assert!(port.menu.presented(draw));
    dispatcher.publish_hit_map(map);
    (cell.0 + 1, cell.1 + 1, visible_rows, close)
}

#[test]
fn menu_wheel_is_regional_bounded_reversible_and_cancelled_by_keyboard() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut port = Port::new(MenuAction::Help);
        port.actions = vec![MenuAction::Help; 40];
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        let (x, y, capacity, _) = paint(&mut dispatcher, &mut port, view.bindings(), locale);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<65;1;1M");
        assert_eq!(
            port.menu.view().unwrap().selected,
            0,
            "outside wheel stays inert"
        );
        let down = format!("\x1b[<65;{x};{y}M");
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            down.repeat(20).as_bytes(),
        );
        assert_eq!(
            port.menu.view().unwrap().first,
            capacity,
            "one flush is one viewport"
        );
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), locale);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<64;{x};{y}M").as_bytes(),
        );
        assert_eq!(
            port.menu.view().unwrap().first,
            capacity - 3,
            "the first reverse notch survives"
        );
        paint(&mut dispatcher, &mut port, view.bindings(), locale);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            down.repeat(20).as_bytes(),
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[H");
        paint(&mut dispatcher, &mut port, view.bindings(), locale);
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<_, io::Error>(())
            })
            .unwrap();
        assert_eq!(
            port.menu.view().unwrap().selected,
            0,
            "keyboard abandons queued wheel work"
        );
        assert!(port.calls.borrow().is_empty());
    }
}

#[test]
fn global_menu_button_and_close_follow_painted_cells_and_own_the_tail() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let map = rendered_sidebar_map(&view);
    let global_cell = (0..40)
        .find_map(|y| {
            (0..120).find_map(|x| {
                (map.hit_target_at(x, y) == Some(&HitTarget::MenuButton)).then_some((x + 1, y + 1))
            })
        })
        .unwrap();
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", global_cell.0, global_cell.1).as_bytes(),
    );
    assert!(
        !port.menu.active(),
        "press only arms the global Menu control"
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", global_cell.0, global_cell.1).as_bytes(),
    );
    assert!(port.menu.active());
    assert_eq!(port.target, Some(MenuTarget::Global));
    let (_, _, _, close_cell) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    let close_cell = close_cell.expect("global menu retains its painted close control");
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", close_cell.0, close_cell.1).as_bytes(),
    );
    assert!(port.menu.active());
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", close_cell.0, close_cell.1).as_bytes(),
    );
    assert!(!port.menu.active());
    assert!(port.calls.borrow().is_empty());

    dispatcher.publish_hit_map(rendered_sidebar_map(&view));
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", global_cell.0, global_cell.1).as_bytes(),
    );
    assert!(port.open_action_menu(MenuTarget::Create(None), None, true));
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", global_cell.0, global_cell.1).as_bytes(),
    );
    assert!(port.menu.active());
    assert_eq!(port.target, Some(MenuTarget::Create(None)));
    assert!(port.calls.borrow().is_empty());
}

fn help_map(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    locale: Locale,
) -> crate::tui_pointer::FrameHitMap {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 16)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    let theme = Theme::terminal_native();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(frame, view, locale, None, &theme, &mut hits);
        })
        .unwrap();
    if locale == Locale::EnUs && view.help_page() == 0 {
        crate::tui_render::test_capture::capture_screen(
            &mut terminal,
            "alignment-help-presented-en-us-80x16-terminal-native",
            locale,
            &theme,
        );
    }
    let map = hits.finish();
    let source = dispatcher.publish_hit_map_with_help(map.clone());
    view.settle_help_draw(source);
    map
}

#[test]
fn unpresentable_help_keeps_a_painted_mouse_exit_without_losing_the_underlying_mode() {
    for (width, height) in [(39, 14), (40, 10), (19, 4), (5, 2)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut port = Port::new(MenuAction::Help);
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(width);
            for action in [
                ClientAction::EnterResizeMode,
                ClientAction::OpenShortcutHelp,
            ] {
                view.commit(
                    view.decide_action(action),
                    SessionActionDisposition::KeepNavigator,
                );
            }
            assert!(view.resize_mode());
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            // Production settles the current input owner before drawing the
            // frame whose hit map it publishes. Mirror that boundary here so
            // the first dispatch does not correctly retire this fresh map as
            // belonging to the preceding ownerless frame.
            dispatcher
                .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                    Ok::<(), io::Error>(())
                })
                .unwrap();
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            let theme = Theme::one_light();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame, &view, locale, None, &theme, &mut hits,
                    );
                })
                .unwrap();
            if (width, height, locale) == (39, 14, Locale::EnUs) {
                crate::tui_render::test_capture::capture_screen(
                    &mut terminal,
                    "alignment-help-refused-en-us-39x14-one-light",
                    locale,
                    &theme,
                );
            }
            let map = hits.finish();
            let mut close_cells = Vec::new();
            for y in 0..height {
                for x in 0..width {
                    if matches!(
                        map.hit_target_at(x, y),
                        Some(HitTarget::Help {
                            action: crate::tui_pointer::HelpControl::Close,
                            ..
                        })
                    ) {
                        close_cells.push((x, y));
                    }
                }
            }
            if crate::tui_layout::ViewportTier::from_size(width, height)
                == crate::tui_layout::ViewportTier::TooSmall
            {
                assert!(
                    close_cells.is_empty(),
                    "TooSmall never invents a clipped Help close at {width}x{height}"
                );
                let source = dispatcher.publish_hit_map_with_help(map);
                assert_eq!(source, None);
                view.settle_help_draw(source);
                assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
                assert!(
                    view.resize_mode(),
                    "TooSmall retains the actual input owner"
                );

                // With no complete pointer control, the retained resize owner
                // still provides its real local Help toggle as the exit route.
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"?");
                assert_eq!(view.overlay(), None);
                assert!(view.resize_mode());
                assert_eq!(
                    *port.calls.borrow(),
                    [Call::Action(ClientAction::CloseShortcutHelp)]
                );
                assert!(forwarded_bytes(&port.calls).is_empty());
                continue;
            }
            assert_eq!(
                close_cells.len(),
                3,
                "Minimal refusal publishes one complete three-cell close"
            );
            let (x, y) = close_cells[0];
            assert_eq!(close_cells, [(x, y), (x + 1, y), (x + 2, y)]);
            let close_glyph = close_cells
                .iter()
                .map(|&(x, y)| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<Vec<_>>()
                .concat();
            assert_eq!(close_glyph, "[x]");
            let source = dispatcher.publish_hit_map_with_help(map);
            assert!(matches!(
                source,
                Some(crate::client_view::help::HelpDrawSource::Refused { .. })
            ));
            view.settle_help_draw(source);
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                format!(
                    "\x1b[<0;{};{}Mtail\x1b[<0;{};{}m",
                    x + 1,
                    y + 1,
                    x + 1,
                    y + 1
                )
                .as_bytes(),
            );
            assert_eq!(view.overlay(), None);
            assert!(
                view.resize_mode(),
                "closing help retains the underlying owner"
            );
            assert!(forwarded_bytes(&port.calls).is_empty());
        }
    }
}

#[test]
fn help_mouse_pages_and_close_capture_only_their_own_surface_and_packet() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut port = Port::new(MenuAction::Help);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(80);
        view.commit(
            view.decide_action(ClientAction::OpenShortcutHelp),
            SessionActionDisposition::KeepNavigator,
        );
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        let map = help_map(&mut dispatcher, &mut view, locale);
        let wheel = (0..16)
            .find_map(|y| {
                (0..80).find_map(|x| {
                    matches!(
                        map.hit_target_at(x, y),
                        Some(HitTarget::ListScroll(
                            crate::tui_pointer::ListScrollTarget::Help(_)
                        ))
                    )
                    .then_some((x + 1, y + 1))
                })
            })
            .unwrap();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<65;1;1M");
        assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
        assert_eq!(view.help_page(), 0);
        let down = format!("\x1b[<65;{};{}M", wheel.0, wheel.1);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            down.repeat(12).as_bytes(),
        );
        assert_eq!(
            view.help_page(),
            1,
            "paginated help advances at most one page per frame"
        );
        help_map(&mut dispatcher, &mut view, locale);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<64;{};{}M", wheel.0, wheel.1).as_bytes(),
        );
        assert_eq!(
            view.help_page(),
            0,
            "reversal abandons the queued later pages"
        );
        let map = help_map(&mut dispatcher, &mut view, locale);
        for action in [
            crate::tui_pointer::HelpControl::Next,
            crate::tui_pointer::HelpControl::Close,
        ] {
            let current = if action == crate::tui_pointer::HelpControl::Next {
                map.clone()
            } else {
                help_map(&mut dispatcher, &mut view, locale)
            };
            let cell = (0..16).find_map(|y| (0..80).find_map(|x| matches!(current.hit_target_at(x, y), Some(HitTarget::Help { action: actual, .. }) if *actual == action).then_some((x + 1, y + 1)))).unwrap();
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                format!(
                    "\x1b[<0;{};{}Mtail\x1b[<0;{};{}m",
                    cell.0, cell.1, cell.0, cell.1
                )
                .as_bytes(),
            );
            assert!(forwarded_bytes(&port.calls).is_empty());
        }
        assert_eq!(view.overlay(), None);
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
        assert_eq!(forwarded_bytes(&port.calls), b"fresh");
    }
}

#[test]
fn help_close_activates_on_release_and_fences_that_packet_once() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    view.commit(
        view.decide_action(ClientAction::OpenShortcutHelp),
        SessionActionDisposition::KeepNavigator,
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let map = help_map(&mut dispatcher, &mut view, Locale::EnUs);
    let close = (0..16)
        .find_map(|y| {
            (0..80).find_map(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::Help {
                        action: crate::tui_pointer::HelpControl::Close,
                        ..
                    })
                )
                .then_some((x + 1, y + 1))
            })
        })
        .unwrap();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", close.0, close.1).as_bytes(),
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    port.calls.borrow_mut().clear();

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!(
            "\x1b[<0;{};{}mtail\x1b[200~paste\x1b[201~\x1b[",
            close.0, close.1
        )
        .as_bytes(),
    );
    assert_eq!(view.overlay(), None);
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"fresh");
    assert_eq!(forwarded_bytes(&port.calls), b"fresh");
    port.calls.borrow_mut().clear();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}morphan", close.0, close.1).as_bytes(),
    );
    assert_eq!(forwarded_bytes(&port.calls), b"orphan");
}

#[test]
fn replaced_help_episode_consumes_the_old_release_without_closing_the_new_surface() {
    let mut port = Port::new(MenuAction::Help);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(80);
    view.commit(
        view.decide_action(ClientAction::OpenShortcutHelp),
        SessionActionDisposition::KeepNavigator,
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let map = help_map(&mut dispatcher, &mut view, Locale::EnUs);
    let close = (0..16)
        .find_map(|y| {
            (0..80).find_map(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::Help {
                        action: crate::tui_pointer::HelpControl::Close,
                        ..
                    })
                )
                .then_some((x + 1, y + 1))
            })
        })
        .unwrap();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", close.0, close.1).as_bytes(),
    );
    view.commit(
        view.decide_action(ClientAction::OpenShortcutHelp),
        SessionActionDisposition::KeepNavigator,
    );
    assert_eq!(view.overlay(), None);
    view.commit(
        view.decide_action(ClientAction::OpenShortcutHelp),
        SessionActionDisposition::KeepNavigator,
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    help_map(&mut dispatcher, &mut view, Locale::EnUs);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", close.0, close.1).as_bytes(),
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn cancelled_menu_painted_press_settles_its_parallel_release_debt_once() {
    for retire_with_new_press in [false, true] {
        let mut port = Port::new(MenuAction::Detach);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}M").as_bytes(),
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;50;10M");
        assert!(port.menu.active());

        if retire_with_new_press {
            // Middle press cannot select or dismiss this menu, but it proves
            // the cancelled left release can no longer arrive.
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<1;50;10Mtail");
        } else {
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                format!("\x1b[<0;{x};{y}mtail").as_bytes(),
            );
        }
        assert!(port.menu.active());
        assert!(port.calls.borrow().is_empty());
        assert_eq!(
            dispatcher.menu_release, None,
            "the PaintedPress and menu packet debt settle together"
        );

        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
        assert!(!port.menu.active());
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        port.calls.borrow_mut().clear();
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}morphan").as_bytes(),
        );
        assert_eq!(
            forwarded_bytes(&port.calls),
            b"orphan",
            "new press={retire_with_new_press}"
        );
    }
}
