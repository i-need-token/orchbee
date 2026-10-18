//! Menu hit authority extends over the pane beside a narrow keyboard-owned sidebar.
use super::*;
use crate::{localization::Locale, terminal_session::ModalPresentation, tui_theme::Theme};

fn open_menu() -> (ClientView, Port, InputDispatcher) {
    let mut view = sidebar_view();
    view.resize(87);
    let mut port = Port::new(MenuAction::QuickWorkspace);
    port.actions = vec![
        MenuAction::QuickWorkspace,
        MenuAction::NewWorktree,
        MenuAction::OpenWorktree,
    ];
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
    assert!(view.machines.sidebar_navigation_active());
    assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
    let crate::machine_picker::SidebarTarget::Project { project_id, .. } = sidebar_project_target()
    else {
        unreachable!()
    };
    assert!(port.open_action_menu(MenuTarget::Create(Some(project_id)), Some((23, 10)), false,));
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    (view, port, dispatcher)
}

fn paint_menu_over_sidebar(
    view: &ClientView,
    port: &mut Port,
    dispatcher: &mut InputDispatcher,
    locale: Locale,
) -> u16 {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(87, 30)).unwrap();
    let sidebar =
        crate::tui_layout::project_shell_areas(Rect::new(0, 0, 87, 30), view.sidebar_surface())
            .sidebar
            .unwrap();
    assert_eq!(sidebar.area.right(), 30);
    let theme = Theme::terminal_native();
    let mut hits = HitMapBuilder::new();
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            // Compose the actual Sidebar first, including its capturing overlay.
            crate::tui_render::render_terminal_shell(frame, view, locale, None, &theme, &mut hits);
            // Native anchored menus use the full viewport above the status line.
            draw = crate::tui_render::render_action_menu_for_test(
                frame,
                Rect::new(0, 0, 87, 29),
                port.menu.view().unwrap(),
                view.bindings(),
                locale,
                &theme,
                &mut hits,
            );
        })
        .unwrap();
    let ModalPresentation::Presented(draw) = draw else {
        panic!("anchored menu must be fully painted at 87 columns");
    };
    let map = hits.finish();
    let menu = port.menu.view().unwrap();
    assert_eq!(
        menu.actions[*draw.result_actions.first().unwrap()],
        MenuAction::QuickWorkspace,
        "the indexed hit belongs to the Quick Workspace action"
    );
    let target = HitTarget::ActionMenuRow {
        token: draw.token,
        row: 0,
    };
    let row_y = (0..29)
        .find(|y| map.hit_target_at(24, *y) == Some(&target))
        .expect("the painted Quick Workspace row overlaps the sidebar edge");
    assert_eq!(map.hit_target_at(34, row_y), Some(&target));
    assert_eq!(
        map.hit_target_at(23, 10),
        Some(&HitTarget::OverlaySurface),
        "the menu frame is not a sidebar backdrop"
    );
    let mut row = String::new();
    let mut column = 0_u16;
    while column < 87 {
        let symbol = terminal.backend().buffer()[(column, row_y)].symbol();
        row.push_str(symbol);
        // A wide glyph's continuation cells are not additional visible spaces.
        column += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
    }
    assert!(
        row.contains(locale.menu_action(&MenuAction::QuickWorkspace)),
        "{row}"
    );
    assert!(port.menu.presented(draw));
    dispatcher.publish_hit_map(map);
    row_y
}

#[test]
fn sidebar_menu_overlap_painted_row_dispatches_once_on_either_side_of_sidebar() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for pointer_x in [None, Some(25), Some(35)] {
            let sample_row_y = {
                let (view, mut port, mut dispatcher) = open_menu();
                paint_menu_over_sidebar(&view, &mut port, &mut dispatcher, locale)
            };
            let sample_report = overlap_report(pointer_x, sample_row_y);
            for split in 0..=sample_report.len() {
                let (mut view, mut port, mut dispatcher) = open_menu();
                let row_y = paint_menu_over_sidebar(&view, &mut port, &mut dispatcher, locale);
                let report = overlap_report(pointer_x, row_y);
                let mut packet = report.to_vec();
                packet.extend_from_slice(b"tail\x1b[200~literal\x1b[201~");
                dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[..split]);
                dispatch_all(&mut dispatcher, &mut view, &mut port, &packet[split..]);
                assert!(!port.menu.active(), "locale={locale:?}, split={split}");
                assert_eq!(
                    *port.calls.borrow(),
                    [Call::Action(ClientAction::RequestQuickWorkspace)],
                    "locale={locale:?}, report={report:?}, split={split}"
                );
                assert!(!view.machines.sidebar_navigation_active());
                assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
                view.settle_project_quick_entry_intent();
                assert!(view.project_quick_entry_pending());
                dispatcher.publish_hit_map(HitMapBuilder::new().finish());
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;35;12m");
                assert!(forwarded_bytes(&port.calls).is_empty());
                assert!(view.complete_project_quick_entry());
                assert_eq!(view.overlay(), None);
            }
        }
    }
}

fn overlap_report(pointer_x: Option<u16>, row_y: u16) -> Vec<u8> {
    pointer_x.map_or_else(
        || b"\r".to_vec(),
        |x| format!("\x1b[<0;{x};{}M\x1b[<0;{x};{}m", row_y + 1, row_y + 1).into_bytes(),
    )
}

#[test]
fn sidebar_menu_overlap_frame_and_backdrop_keep_distinct_capture_semantics() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (mut view, mut port, mut dispatcher) = open_menu();
        paint_menu_over_sidebar(&view, &mut port, &mut dispatcher, locale);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;24;11Mtail");
        assert!(port.menu.active(), "frame press must not dismiss");
        assert!(port.calls.borrow().is_empty());
        assert!(view.machines.sidebar_navigation_active());
        paint_menu_over_sidebar(&view, &mut port, &mut dispatcher, locale);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;24;11m");
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;81;12Mtail");
        assert!(!port.menu.active(), "only the menu backdrop dismisses it");
        assert!(port.calls.borrow().is_empty());
        assert!(view.machines.sidebar_navigation_active());
        assert_eq!(view.overlay(), Some(Overlay::BusinessSidebar));
        assert!(!view.project_quick_entry_pending());
    }
}
