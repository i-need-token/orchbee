use super::test_support::draw;
use super::*;
use crate::workbench_client::ui::tests::viewport::{controlling_ui, install_projection};
use crate::{
    client_view::ClientView,
    localization::Locale,
    tui_theme::Theme,
    workbench_client::{Geometry, pane_graphics::Layer},
};
use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
use ratatui::style::Color;

#[test]
fn actual_shell_paints_installed_canvas_but_acknowledges_only_a_successful_uncovered_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for (width, height) in [(80, 24), (120, 40), (60, 18), (32, 10)] {
                let (mut ui, _, _, _) = controlling_ui();
                let client = ClientView::new(width);
                let areas = crate::tui_layout::project_shell_areas(
                    ratatui::layout::Rect::new(0, 0, width, height),
                    client.sidebar_surface(),
                );
                let body = crate::tui_layout::active_pane_area(&areas, Some(ui.view())).unwrap();
                // The live crop reads this same per-pane geometry in the production shell.
                ui.observe_geometry(Geometry::new(body.width, body.height, 8, 16).unwrap());
                install_projection(&mut ui, Some((9, false, 24)), 0);
                ui.after_close_draw(draw(&ui, width, height, locale, &theme).0);
                let view = ui.view();
                let tab = view.tab(0).unwrap();
                let panel = tab.panel(0).unwrap();
                let panel_id = panel.panel_id().clone();
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
                let crop = ui.view().pane_live_view(&panel_id).unwrap();
                let image = crop.content(body);
                assert_eq!(buffer[(image.x, image.y)].symbol(), "▀");
                assert_eq!(buffer[(image.x, image.y)].fg, Color::Rgb(255, 0, 0));
                assert_eq!(buffer[(image.x, image.y)].bg, Color::Rgb(0, 0, 255));
                if crop.clipped {
                    assert_ne!(buffer[(body.x, body.y)].symbol(), "▀");
                }
                assert!(
                    buffer.content.iter().any(|cell| cell.symbol() == "▀"
                        && cell.fg == Color::Rgb(255, 0, 0)
                        && cell.bg == Color::Rgb(0, 0, 255)),
                    "{} {locale:?} {width}x{height}: draws={:?}, raster cells={:?}",
                    theme.name,
                    receipt.graphics_draws(),
                    buffer
                        .content
                        .iter()
                        .enumerate()
                        .filter(|(_, cell)| cell.symbol() == "▀")
                        .collect::<Vec<_>>(),
                );
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
