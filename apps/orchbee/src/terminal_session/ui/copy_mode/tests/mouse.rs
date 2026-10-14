//! Real painted controls and decoder reports reach the existing native effect owners.
use super::*;
use crate::{
    client_view::{ClientAction, ClientView, SessionActionDisposition},
    localization::Locale,
    terminal_copy_mode::{
        CopyControl, CopyModeCursor, CopyModeStatus, CopyModeToken, CopySurfaceToken,
    },
    terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable},
    terminal_session::TerminalSessionView,
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend};
use std::io;

struct Port {
    ui: NativeUi,
    forwarded: Vec<u8>,
}
impl TerminalInputPort for Port {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn copy_mode_active(&self) -> bool {
        self.ui.copy_mode_active()
    }
    fn capture_copy_mode_key(&mut self, key: &KeyEvent) -> bool {
        self.ui.capture_copy_mode_key(key)
    }
    fn capture_copy_mode_paste(&mut self, text: Option<&str>) -> bool {
        self.ui.capture_copy_mode_paste(text)
    }
    fn click_copy_control(&mut self, token: CopyModeToken, control: CopyControl) {
        self.ui.click_copy_control(token, control);
    }
    fn copy_pointer_cell(
        &mut self,
        token: CopySurfaceToken,
        cell: CopyModeCursor,
        start: bool,
    ) -> bool {
        self.ui.copy_pointer_cell(token, cell, start)
    }
    fn scroll_copy_mode(
        &mut self,
        token: CopyModeToken,
        up: bool,
        lines: usize,
    ) -> Option<CopyModeToken> {
        self.ui.scroll_copy_mode(token, up, lines)
    }
}

fn fixture(width: u16) -> (Port, Arc<CommandMailbox>, InputDispatcher, ClientView) {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    install_wide_tail(&mut ui, 2, 2, None);
    ui.enter_copy_mode();
    let mut port = Port {
        ui,
        forwarded: Vec::new(),
    };
    let mut view = ClientView::new(width);
    view.settle_copy_mode(true);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<_, io::Error>(())
        })
        .unwrap();
    (port, mailbox, dispatcher, view)
}
fn painted(
    port: &Port,
    view: &ClientView,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (Terminal<TestBackend>, FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                view,
                locale,
                Some(TerminalSessionView::Native(port.ui.view())),
                theme,
                &mut hits,
            );
        })
        .unwrap();
    (terminal, hits.finish())
}
fn paint(
    port: &Port,
    view: &ClientView,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> FrameHitMap {
    painted(port, view, width, height, locale, theme).1
}
fn cell(
    map: &FrameHitMap,
    width: u16,
    height: u16,
    accepts: impl Fn(&HitTarget) -> bool,
) -> (u16, u16) {
    (0..height)
        .flat_map(|row| (0..width).map(move |column| (column, row)))
        .find(|&(column, row)| map.hit_target_at(column, row).is_some_and(&accepts))
        .expect("painted target")
}
fn button(map: &FrameHitMap, width: u16, height: u16, control: CopyControl) -> (u16, u16) {
    cell(
        map,
        width,
        height,
        |target| matches!(target, HitTarget::CopyControl { control: current, .. } if *current == control),
    )
}
fn report(position: (u16, u16), code: u8, release: bool) -> String {
    format!(
        "\x1b[<{code};{};{}{}",
        position.0 + 1,
        position.1 + 1,
        if release { 'm' } else { 'M' }
    )
}
fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut Port,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            port,
            Instant::now(),
            |_, _| Ok::<_, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}
fn click(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut Port,
    position: (u16, u16),
) {
    dispatch(
        dispatcher,
        view,
        port,
        format!(
            "{}{}",
            report(position, 0, false),
            report(position, 0, true)
        )
        .as_bytes(),
    );
}

#[test]
fn copy_is_the_only_painted_owner_while_resize_remains_ready_underneath() {
    const WIDTH: u16 = 80;
    const HEIGHT: u16 = 28;

    let (mut port, _, _, mut view) = fixture(WIDTH);
    let transition = view.decide_action(ClientAction::EnterResizeMode);
    assert_eq!(transition.action(), Some(ClientAction::EnterResizeMode));
    view.commit(transition, SessionActionDisposition::KeepNavigator);
    assert!(view.resize_mode());
    assert!(view.copy_mode_active());

    let copy = paint(
        &port,
        &view,
        WIDTH,
        HEIGHT,
        Locale::EnUs,
        &Theme::one_dark(),
    );
    let has = |predicate: &dyn Fn(&HitTarget) -> bool| {
        (0..HEIGHT).any(|row| {
            (0..WIDTH).any(|column| copy.hit_target_at(column, row).is_some_and(predicate))
        })
    };
    assert!(has(&|target| matches!(
        target,
        HitTarget::CopySurface { .. }
    )));
    assert!(has(&|target| matches!(
        target,
        HitTarget::CopyControl { .. }
    )));
    assert!(!has(&|target| matches!(target, HitTarget::Resize(_))));

    assert!(port.ui.cancel_copy_mode());
    view.settle_copy_mode(false);
    assert!(view.resize_mode(), "the covered resize owner is retained");
    let resize = paint(
        &port,
        &view,
        WIDTH,
        HEIGHT,
        Locale::EnUs,
        &Theme::one_dark(),
    );
    let mut resize_hit = false;
    for row in 0..HEIGHT {
        for column in 0..WIDTH {
            match resize.hit_target_at(column, row) {
                Some(HitTarget::Resize(_)) => resize_hit = true,
                Some(HitTarget::CopySurface { .. } | HitTarget::CopyControl { .. }) => {
                    panic!("a settled Copy owner left a painted target")
                }
                _ => {}
            }
        }
    }
    assert!(
        resize_hit,
        "the retained Resize owner paints on the next frame"
    );
}

#[test]
fn too_small_root_compositor_keeps_copy_exit_status_without_copy_targets() {
    for (width, heights) in [(100, &[1, 3, 4][..]), (19, &[24][..])] {
        let (port, _, _, view) = fixture(width);
        for &height in heights {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let theme = Theme::one_dark();
                let (terminal, map) = painted(&port, &view, width, height, locale, &theme);
                assert_eq!(map.help_draw_source(), None);
                for row in 0..height {
                    for column in 0..width {
                        if let Some(target) = map.hit_target_at(column, row) {
                            assert_eq!(
                                target,
                                &HitTarget::OverlaySurface,
                                "{width}x{height} published hidden Copy target {target:?}"
                            );
                        }
                    }
                }

                let rows = (0..height)
                    .map(|row| {
                        (0..width)
                            .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>();
                let visible_body = rows[..usize::from(height.saturating_sub(1))]
                    .iter()
                    .map(|row| row.replace(' ', ""))
                    .filter(|row| !row.is_empty())
                    .collect::<Vec<_>>();
                let marker = format!("{} ", theme.glyphs.status_bullet);
                let marker_width = crate::terminal_text::display_width(&marker);
                let expected_notice = if marker_width < usize::from(width) {
                    format!(
                        "{marker}{}",
                        crate::terminal_text::truncate_display(
                            locale.shell().too_small_notice,
                            usize::from(width) - marker_width,
                            theme.glyphs.ellipsis,
                        )
                    )
                } else {
                    crate::terminal_text::truncate_display(
                        locale.shell().too_small_notice,
                        usize::from(width),
                        theme.glyphs.ellipsis,
                    )
                };
                let expected_body = (height > 1)
                    .then(|| expected_notice.replace(' ', ""))
                    .into_iter()
                    .collect::<Vec<_>>();
                assert_eq!(visible_body, expected_body, "{width}x{height}");

                let status = &rows[usize::from(height - 1)];
                assert!(status.contains("Esc"), "{width}x{height}: {status:?}");
                assert!(
                    status.replace(' ', "").contains(
                        &locale
                            .copy_mode_label(CopyModeStatus::Ready, false)
                            .replace(' ', "")
                    ),
                    "{width}x{height}: {status:?}"
                );
            }
        }
    }
}

#[test]
fn copy_mouse_drag_normalizes_cells_coalesces_updates_and_exports_only_after_copy_release() {
    let (mut port, mailbox, mut dispatcher, mut view) = fixture(80);
    let map = paint(&port, &view, 80, 28, Locale::EnUs, &Theme::one_dark());
    let found = cell(&map, 80, 28, |target| {
        matches!(target, HitTarget::CopySurface { .. })
    });
    let HitTarget::CopySurface { area, token } = map.hit_target_at(found.0, found.1).unwrap()
    else {
        unreachable!()
    };
    let area = *area;
    let token = *token;
    dispatcher.publish_hit_map(map);
    let start = (area.x + 3, area.y + 2); // trailing half of 中 normalizes to column 2
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut port,
        report(start, 0, false).as_bytes(),
    );
    let (key, command, action) = take_selection(&mut port.ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Start {
                point: orchbee_client_server_protocol::ViewerSelectionViewportPoint {
                    column: 2,
                    row: 2
                },
                ..
            },
            ..
        }
    ));
    for column in 4..=7 {
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut port,
            report((area.x + column, area.y + 2), 32, false).as_bytes(),
        );
    }
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut port,
        format!("{}owned", report((area.x + 7, area.y + 2), 0, true)).as_bytes(),
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "one pending Host command"
    );
    assert_eq!(port.ui.copy_mode.mode().unwrap().desired().col, 7);
    reply_selection(
        &mut port.ui,
        &mailbox,
        key,
        command,
        SelectionChangedState::None,
    );
    port.ui.tick_copy_mode();
    let (key, command, action) = take_selection(&mut port.ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Update {
                point: orchbee_client_server_protocol::ViewerSelectionViewportPoint {
                    column: 7,
                    row: 2
                },
                ..
            },
            ..
        }
    ));
    reply_selection(&mut port.ui, &mailbox, key, command, active_selection());
    port.ui.tick_copy_mode();
    assert_eq!(
        port.ui.copy_mode_view().unwrap().status,
        CopyModeStatus::Ready
    );
    assert!(port.forwarded.is_empty());
    let map = paint(&port, &view, 80, 28, Locale::EnUs, &Theme::one_dark());
    let copy = button(&map, 80, 28, CopyControl::Copy);
    dispatcher.publish_hit_map(map);
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut port,
        report(copy, 0, false).as_bytes(),
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut port,
        report(copy, 0, true).as_bytes(),
    );
    assert_eq!(
        port.ui.copy_mode_view().unwrap().status,
        CopyModeStatus::Exporting
    );
    // A mouse coordinate from this episode cannot mutate a later episode.
    port.ui.cancel_copy_mode();
    port.ui.enter_copy_mode();
    assert!(
        !port
            .ui
            .copy_pointer_cell(token, CopyModeCursor { column: 1, row: 1 }, true)
    );
}

#[test]
fn copy_toolbar_search_back_direction_close_and_wheel_work_in_both_locales_and_small_geometry() {
    for (width, height) in [(40, 10), (80, 24), (80, 28), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::one_dark(),
                Theme::one_light(),
                Theme::terminal_native(),
            ] {
                let (mut port, _, mut dispatcher, mut view) = fixture(width);
                let (mut terminal, map) = painted(&port, &view, width, height, locale, &theme);
                let body = cell(&map, width, height, |target| {
                    matches!(target, HitTarget::CopySurface { .. })
                });
                let HitTarget::CopySurface { area, .. } =
                    map.hit_target_at(body.0, body.1).unwrap()
                else {
                    unreachable!()
                };
                let capacity = area.height.min(23);
                dispatcher.publish_hit_map(map);
                if (width, height, locale, theme.name) == (120, 40, Locale::EnUs, "terminal-native")
                {
                    crate::tui_render::test_capture::capture_screen(
                        &mut terminal,
                        "alignment-copy-ready-en-us-120x40-terminal-native",
                        locale,
                        &theme,
                    );
                }
                dispatch(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    report(body, 65, false).repeat(100).as_bytes(),
                );
                assert_eq!(
                    port.ui.copy_mode.mode().unwrap().desired().row,
                    u32::from(capacity)
                );
                dispatcher.publish_hit_map(paint(&port, &view, width, height, locale, &theme));
                dispatch(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    report(body, 64, false).as_bytes(),
                );
                assert_eq!(
                    port.ui.copy_mode.mode().unwrap().desired().row,
                    u32::from(capacity.saturating_sub(3)),
                    "first reversal wins"
                );
                let map = paint(&port, &view, width, height, locale, &theme);
                let search = button(&map, width, height, CopyControl::Search);
                dispatcher.publish_hit_map(map);
                click(&mut dispatcher, &mut view, &mut port, search);
                assert!(port.ui.copy_mode_view().unwrap().search.is_some());
                dispatch(&mut dispatcher, &mut view, &mut port, b"needle");
                let (mut terminal, map) = painted(&port, &view, width, height, locale, &theme);
                let reverse = button(&map, width, height, CopyControl::ReverseSearch);
                dispatcher.publish_hit_map(map);
                if (width, height, locale, theme.name) == (80, 24, Locale::ZhCn, "terminal-native")
                {
                    crate::tui_render::test_capture::capture_screen(
                        &mut terminal,
                        "alignment-copy-search-zh-cn-80x24-terminal-native",
                        locale,
                        &theme,
                    );
                }
                click(&mut dispatcher, &mut view, &mut port, reverse);
                assert_eq!(
                    port.ui.copy_mode_view().unwrap().search.unwrap().direction,
                    crate::terminal_copy::CopySearchDirection::Backward
                );
                let map = paint(&port, &view, width, height, locale, &theme);
                let back = button(&map, width, height, CopyControl::CancelSearch);
                dispatcher.publish_hit_map(map);
                click(&mut dispatcher, &mut view, &mut port, back);
                assert!(port.ui.copy_mode_view().unwrap().search.is_none());
                assert!(port.ui.copy_mode_active());
                let map = paint(&port, &view, width, height, locale, &theme);
                let close = button(&map, width, height, CopyControl::Exit);
                dispatcher.publish_hit_map(map);
                dispatch(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    report(close, 0, false).as_bytes(),
                );
                dispatch(&mut dispatcher, &mut view, &mut port, b"j");
                dispatch(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    report(close, 0, true).as_bytes(),
                );
                assert!(
                    port.ui.copy_mode_active(),
                    "keyboard cancels the old close press"
                );
                dispatcher.publish_hit_map(paint(&port, &view, width, height, locale, &theme));
                dispatch(
                    &mut dispatcher,
                    &mut view,
                    &mut port,
                    format!("{}{}owned", report(close, 0, false), report(close, 0, true))
                        .as_bytes(),
                );
                assert!(!port.ui.copy_mode_active());
                assert!(port.forwarded.is_empty());
                dispatcher.publish_hit_map(paint(&port, &view, width, height, locale, &theme));
                dispatch(&mut dispatcher, &mut view, &mut port, b"fresh");
                assert_eq!(port.forwarded, b"fresh");
            }
        }
    }
}
