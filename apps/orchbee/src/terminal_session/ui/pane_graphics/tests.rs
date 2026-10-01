use super::*;
use crate::terminal_session::ui::{
    move_membership_tests::ready_ui, viewport_tests::install_projection,
};
use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_session::{NativeDrawReceipt, TerminalSessionView, pane_graphics::Layer},
    tui_pointer::HitMapBuilder,
    tui_theme::Theme,
};
use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

fn draw(
    ui: &NativeUi,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (NativeDrawReceipt, Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(width),
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    (receipt, terminal.backend().buffer().clone())
}

#[test]
fn actual_shell_paints_installed_canvas_but_acknowledges_only_a_successful_uncovered_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for (width, height) in [(80, 24), (120, 40), (60, 18), (32, 10)] {
                let (mut ui, _, _, _) = ready_ui();
                install_projection(&mut ui, Some((9, false, 24)), 0);
                ui.after_close_draw(draw(&ui, width, height, locale, &theme).0);
                let view = ui.view();
                let tab = view.tab(0).unwrap();
                let panel = tab.panel(0).unwrap();
                let source = Source::capture(panel, view.selected_presentation(panel));
                let layer = Layer::new(
                    PaneGraphicsLayer {
                        layer_id: "primary".into(),
                        z_index: 0,
                        width: 1,
                        height: 2,
                        placement: PaneGraphicsPlacement {
                            grid_columns: 2,
                            grid_rows: 1,
                            ..Default::default()
                        },
                        resource_revision: WorkbenchRevision::new(1),
                        stream_id: None,
                    },
                    vec![255, 0, 0, 255, 0, 0, 255, 255],
                )
                .unwrap();
                ui.graphics.install_for_test(
                    PaneGraphicsCanvas::new(source, WorkbenchRevision::new(1), vec![layer])
                        .unwrap(),
                );
                let observation = ui
                    .graphics_observation(&ui.control_observation(false).unwrap().1)
                    .unwrap();
                assert!(
                    observation
                        .panes
                        .iter()
                        .all(|pane| pane.drawn_revision != Some(WorkbenchRevision::new(1))),
                    "resource acquisition is not a draw receipt"
                );
                let (receipt, buffer) = draw(&ui, width, height, locale, &theme);
                assert!(buffer.content.iter().any(|cell| cell.symbol() == "▀"
                    && cell.fg == Color::Rgb(255, 0, 0)
                    && cell.bg == Color::Rgb(0, 0, 255)));
                ui.after_close_draw(receipt);
                let observation = ui
                    .graphics_observation(&ui.control_observation(false).unwrap().1)
                    .unwrap();
                assert!(
                    observation
                        .panes
                        .iter()
                        .any(|pane| pane.drawn_revision == Some(WorkbenchRevision::new(1)))
                );
                ui.board.open();
                ui.after_close_draw(draw(&ui, width, height, locale, &theme).0);
                let observation = ui
                    .graphics_observation(&ui.control_observation(false).unwrap().1)
                    .unwrap();
                assert!(observation.panes.is_empty());
                assert!(observation.complete);
                let view = ui.view();
                let tab = view.tab(0).unwrap();
                assert!(view.panel_graphics(tab.panel(0).unwrap()).is_none());
            }
        }
    }
}
