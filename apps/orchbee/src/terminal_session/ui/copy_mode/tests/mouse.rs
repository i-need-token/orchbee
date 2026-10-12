//! Real painted controls and decoder reports reach the existing native effect owners.
use super::*;
use crate::{
    client_view::ClientView,
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
fn paint(
    port: &Port,
    view: &ClientView,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> FrameHitMap {
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
    hits.finish()
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
    for (width, height) in [(40, 10), (80, 28), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::one_dark(),
                Theme::one_light(),
                Theme::terminal_native(),
            ] {
                let (mut port, _, mut dispatcher, mut view) = fixture(width);
                let map = paint(&port, &view, width, height, locale, &theme);
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
                let map = paint(&port, &view, width, height, locale, &theme);
                let reverse = button(&map, width, height, CopyControl::ReverseSearch);
                dispatcher.publish_hit_map(map);
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
