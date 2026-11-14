//! Real renderer maps drive bounded wheel work and complete pointer gestures.
use super::*;
use crate::{localization::Locale, tui_presentation::ModalPresentation, tui_theme::Theme};

#[test]
fn hover_moves_only_the_menu_candidate_and_one_click_activates_the_painted_row() {
    for repaint in [0, 1, 2] {
        let mut port = Port::new(MenuAction::Detach);
        port.actions = vec![MenuAction::Detach, MenuAction::Machines];
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
        let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        let (x, y) = painted.row(&MenuAction::Machines);
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
        assert_eq!(
            *port.calls.borrow(),
            [Call::Action(ClientAction::OpenMachines)]
        );
    }
}

/// A press and its release on a row are one click when an availability refresh between them
/// leaves that row the same enabled action in the same place, whether the refreshed menu was
/// painted before the release or not. A refresh that disables or moves the row, or a change that
/// replaces the rows' presentation, cancels the click.
#[test]
fn a_click_survives_an_availability_refresh_that_keeps_its_row() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Change {
        OtherRowDisabled,
        RowDisabled,
        RowMoved,
        RowsReplaced,
    }
    let busy = crate::action_menu::MenuUnavailable::OperationBusy;
    for change in [
        Change::OtherRowDisabled,
        Change::RowDisabled,
        Change::RowMoved,
        Change::RowsReplaced,
    ] {
        for repaint in [false, true] {
            if change == Change::RowMoved && !repaint {
                continue;
            }
            let mut port = Port::new(MenuAction::Machines);
            port.actions = vec![MenuAction::Machines, MenuAction::Detach];
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(50);
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
            let area = Rect::new(0, 0, 48, 10);
            let painted = paint_in(
                &mut dispatcher,
                &mut port,
                view.bindings(),
                Locale::EnUs,
                area,
            );
            let (x, y) = painted.row(&MenuAction::Machines);
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                format!("\x1b[<0;{x};{y}M").as_bytes(),
            );
            match change {
                Change::OtherRowDisabled | Change::RowMoved => {
                    assert!(port.menu.refresh_availability(|action| {
                        (action == MenuAction::Detach).then_some(busy)
                    }));
                }
                Change::RowDisabled => {
                    assert!(port.menu.refresh_availability(|action| {
                        (action == MenuAction::Machines).then_some(busy)
                    }));
                }
                Change::RowsReplaced => {
                    port.menu.set_labels(|action| {
                        (action == MenuAction::Machines).then(|| "Help target".into())
                    });
                }
            }
            if repaint {
                let area = if change == Change::RowMoved {
                    Rect::new(2, 0, 48, 10)
                } else {
                    area
                };
                paint_in(
                    &mut dispatcher,
                    &mut port,
                    view.bindings(),
                    Locale::EnUs,
                    area,
                );
            }
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                format!("\x1b[<0;{x};{y}mtail").as_bytes(),
            );
            let clicked = change == Change::OtherRowDisabled;
            assert_eq!(
                port.menu.active(),
                !clicked,
                "{change:?}, repainted {repaint}"
            );
            let expected: &[Call] = if clicked {
                &[Call::Action(ClientAction::OpenMachines)]
            } else {
                &[]
            };
            assert_eq!(
                *port.calls.borrow(),
                expected,
                "{change:?}, repainted {repaint}: one activation at most, and the packet stays fenced"
            );
        }
    }
}

#[test]
fn hover_admission_leave_and_keyboard_navigation_preserve_menu_ownership() {
    let mut port = Port::new(MenuAction::Detach);
    port.actions = vec![
        MenuAction::Detach,
        MenuAction::Machines,
        MenuAction::Settings,
    ];
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(50);
    dispatcher.set_pointer_capture(&mut view, &mut port, false);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
    let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    let help_point = painted.row(&MenuAction::Machines);
    let motion = |code, (x, row)| format!("\x1b[<{code};{x};{row}M");
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        motion(35, help_point).as_bytes(),
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
    let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::ZhCn);
    let help_point = painted.row(&MenuAction::Machines);
    for code in [39, 43, 51, 32] {
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            motion(code, help_point).as_bytes(),
        );
        assert_eq!(
            port.menu.view().unwrap().selected,
            0,
            "modified or held motion is inert"
        );
    }
    let settings_point = painted.row(&MenuAction::Settings);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("{}{}", motion(35, help_point), motion(35, settings_point)).as_bytes(),
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
        let mut port = Port::new(MenuAction::Machines);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
        let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        let (x, y) = painted.row_at(0);
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
                    vec![MenuAction::Machines],
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
    let mut port = Port::new(MenuAction::Machines);
    port.actions = vec![
        MenuAction::Detach,
        MenuAction::Settings,
        MenuAction::Machines,
    ];
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(50);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
    let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    let (help_x, help_y) = painted.row(&MenuAction::Machines);

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<35;{help_x};{help_y}M\r").as_bytes(),
    );
    assert_eq!(port.menu.view().unwrap().selected, 2);
    assert_eq!(port.menu.confirmed(), None);
    assert!(
        port.calls.borrow().is_empty(),
        "Enter cannot accept an unpainted hover"
    );

    let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    let (settings_x, settings_y) = painted.row(&MenuAction::Settings);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<35;{settings_x};{settings_y}M\x1b[B\r").as_bytes(),
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
    assert_eq!(
        *port.calls.borrow(),
        [Call::Action(ClientAction::OpenMachines)]
    );
}

fn paint(
    dispatcher: &mut InputDispatcher,
    port: &mut Port,
    bindings: &crate::client_view::Bindings,
    locale: Locale,
) -> PaintedActionMenu {
    paint_in(dispatcher, port, bindings, locale, Rect::new(0, 0, 50, 10))
}

/// Paints the menu within `area` of a 50 by 10 frame.
fn paint_in(
    dispatcher: &mut InputDispatcher,
    port: &mut Port,
    bindings: &crate::client_view::Bindings,
    locale: Locale,
    area: Rect,
) -> PaintedActionMenu {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(50, 10)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    let mut draw = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            draw = crate::tui_render::render_action_menu_for_test(
                frame,
                area,
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
    let actions = port.menu.view().unwrap().actions.to_vec();
    let mut rows = vec![None; actions.len()];
    for (position, action_index) in draw.result_actions.iter().copied().enumerate() {
        rows[action_index] = (0..10).find_map(|y| {
            (0..50).find_map(|x| {
                matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::ActionMenuRow { token, row })
                        if *token == draw.token && *row == position
                )
                .then_some((x + 1, y + 1))
            })
        });
    }
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
    PaintedActionMenu {
        actions,
        rows,
        visible_rows,
        close,
    }
}

struct PaintedActionMenu {
    actions: Vec<MenuAction>,
    rows: Vec<Option<(u16, u16)>>,
    visible_rows: usize,
    close: Option<(u16, u16)>,
}

impl PaintedActionMenu {
    fn row(&self, action: &MenuAction) -> (u16, u16) {
        let index = self
            .actions
            .iter()
            .position(|candidate| candidate == action)
            .expect("the requested action belongs to the menu");
        self.row_at(index)
    }

    fn row_at(&self, index: usize) -> (u16, u16) {
        self.rows
            .get(index)
            .and_then(|point| *point)
            .expect("the requested action row is painted")
    }

    fn first_visible(&self) -> (u16, u16) {
        self.rows
            .iter()
            .find_map(|point| *point)
            .expect("the menu has a painted action row")
    }
}

#[test]
fn menu_wheel_is_regional_bounded_reversible_and_cancelled_by_keyboard() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut port = Port::new(MenuAction::Machines);
        port.actions = vec![MenuAction::Machines; 40];
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
        let painted = paint(&mut dispatcher, &mut port, view.bindings(), locale);
        let (x, y) = painted.first_visible();
        let capacity = painted.visible_rows;
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
        let painted = paint(&mut dispatcher, &mut port, view.bindings(), locale);
        let (x, y) = painted.first_visible();
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
fn global_menu_has_no_top_button_and_keyboard_open_retains_its_close_control() {
    let mut port = Port::new(MenuAction::Machines);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let map = rendered_sidebar_map(&view);
    assert!(
        (0..40).all(|y| (0..120).all(|x| map.hit_target_at(x, y) != Some(&HitTarget::MenuButton)))
    );
    dispatcher.publish_hit_map(map);
    assert!(port.open_action_menu(MenuTarget::Global, None, false));
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
    let close = painted.close.unwrap();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}M", close.0, close.1).as_bytes(),
    );
    assert!(port.menu.active());
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("\x1b[<0;{};{}mtail", close.0, close.1).as_bytes(),
    );
    assert!(!port.menu.active());
    assert!(port.calls.borrow().is_empty());
}

#[test]
fn cancelled_menu_painted_press_settles_its_parallel_release_debt_once() {
    for retire_with_new_press in [false, true] {
        let mut port = Port::new(MenuAction::Detach);
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(50);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02 ");
        let painted = paint(&mut dispatcher, &mut port, view.bindings(), Locale::EnUs);
        let (x, y) = painted.row_at(0);
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
