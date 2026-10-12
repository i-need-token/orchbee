//! Real renderer maps drive bounded wheel work and complete pointer gestures.
use super::*;
use crate::{localization::Locale, terminal_session::ModalPresentation, tui_theme::Theme};

#[test]
fn hover_then_one_click_activates_the_painted_row_across_highlight_redraws() {
    for repaint in [0, 1, 2] {
        let mut port = Port::new(MenuAction::Detach);
        port.actions = vec![MenuAction::Detach, MenuAction::Help];
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOQ");
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, Locale::EnUs);
        let y = y + 1;
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<35;{x};{y}M").as_bytes(),
        );
        assert_eq!(port.menu.view().unwrap().selected, 1);
        if repaint == 1 {
            paint(&mut dispatcher, &mut port, Locale::EnUs);
        }
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<0;{x};{y}M").as_bytes(),
        );
        assert!(port.menu.active(), "press cannot execute");
        if repaint == 2 {
            paint(&mut dispatcher, &mut port, Locale::EnUs);
        }
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

fn paint(
    dispatcher: &mut InputDispatcher,
    port: &mut Port,
    locale: Locale,
) -> (u16, u16, usize, (u16, u16)) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(50, 10)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            draw = crate::tui_render::render_action_menu_for_test(
                frame,
                frame.area(),
                port.menu.view().unwrap(),
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
    let close = (0..10)
        .find_map(|y| {
            (0..50).find_map(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::ActionMenuControl { back: false, .. })
                )
                .then_some((x + 1, y + 1))
            })
        })
        .unwrap();
    port.menu.draw(Some(draw));
    dispatcher.publish_hit_map(map);
    (cell.0 + 1, cell.1 + 1, draw.rows, close)
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
        let (x, y, capacity, _) = paint(&mut dispatcher, &mut port, locale);
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
            port.menu.view().unwrap().selected,
            capacity,
            "one flush is one viewport"
        );
        let (x, y, _, _) = paint(&mut dispatcher, &mut port, locale);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            format!("\x1b[<64;{x};{y}M").as_bytes(),
        );
        assert_eq!(
            port.menu.view().unwrap().selected,
            capacity - 3,
            "the first reverse notch survives"
        );
        paint(&mut dispatcher, &mut port, locale);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            down.repeat(20).as_bytes(),
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[H");
        paint(&mut dispatcher, &mut port, locale);
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
    let cell = (0..40)
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
        format!("\x1b[<0;{};{}Mtail", cell.0, cell.1).as_bytes(),
    );
    assert!(port.menu.active());
    assert_eq!(port.target, Some(MenuTarget::Global));
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}m", cell.0, cell.1).as_bytes(),
    );
    let (_, _, _, cell) = paint(&mut dispatcher, &mut port, Locale::EnUs);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", cell.0, cell.1).as_bytes(),
    );
    assert!(port.menu.active());
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", cell.0, cell.1).as_bytes(),
    );
    assert!(!port.menu.active());
    assert!(port.calls.borrow().is_empty());
}

fn help_map(
    dispatcher: &mut InputDispatcher,
    view: &ClientView,
    locale: Locale,
) -> crate::tui_pointer::FrameHitMap {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 16)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                view,
                locale,
                None,
                &Theme::one_light(),
                &mut hits,
            );
        })
        .unwrap();
    let map = hits.finish();
    dispatcher.publish_hit_map(map.clone());
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
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    let receipt = crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        None,
                        &Theme::one_light(),
                        &mut hits,
                    );
                    assert!(receipt.help_page().is_none(), "no complete help page fits");
                })
                .unwrap();
            let map = hits.finish();
            let (x, y) = (0..height)
                .find_map(|y| {
                    (0..width).find_map(|x| {
                        matches!(
                            map.hit_target_at(x, y),
                            Some(HitTarget::Help {
                                action: crate::tui_pointer::HelpControl::Close,
                                ..
                            })
                        )
                        .then_some((x, y))
                    })
                })
                .expect("a visible exit survives refused help");
            assert!(matches!(
                terminal.backend().buffer()[(x, y)].symbol(),
                "[" | "x"
            ));
            dispatcher.publish_hit_map(map);
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
        let map = help_map(&mut dispatcher, &view, locale);
        let wheel = (0..16)
            .find_map(|y| {
                (0..80).find_map(|x| {
                    matches!(
                        map.hit_target_at(x, y),
                        Some(HitTarget::ListScroll(
                            crate::tui_pointer::ListScrollTarget::Help { .. }
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
        help_map(&mut dispatcher, &view, locale);
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
        let map = help_map(&mut dispatcher, &view, locale);
        for action in [
            crate::tui_pointer::HelpControl::Next,
            crate::tui_pointer::HelpControl::Close,
        ] {
            let current = if action == crate::tui_pointer::HelpControl::Next {
                map.clone()
            } else {
                help_map(&mut dispatcher, &view, locale)
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
