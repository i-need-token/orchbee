use super::test_support::draw;
use super::*;
use crate::workbench_client::ui::tests::viewport::{controlling_ui, install_projection};
use crate::{
    client_view::ClientView,
    localization::Locale,
    tui_graphics::Surface,
    tui_theme::Theme,
    workbench_client::{DrawReceipt, Geometry, pane_graphics::Layer},
};
use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
use ratatui::style::Color;

#[test]
fn actual_shell_paints_installed_canvas_but_acknowledges_only_a_successful_uncovered_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
        ] {
            for (width, height) in [(80, 24), (120, 40), (60, 18), (32, 10)] {
                let (mut ui, _, _, _) = controlling_ui();
                let client = ClientView::new(width);
                let areas = crate::tui_layout::project_shell_areas(
                    ratatui::layout::Rect::new(0, 0, width, height),
                    client.sidebar_surface(),
                );
                let body = crate::tui_layout::arranged_pane_area(&areas, &ui.view()).unwrap();
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

const KITTY_PLACED: &[u8] = include_bytes!(
    "../../../../../../../../schemas/terminal/v1/fixtures/ghostty-kitty-graphics-placed.bin"
);
const KITTY_SCROLLED: &[u8] = include_bytes!(
    "../../../../../../../../schemas/terminal/v1/fixtures/ghostty-kitty-graphics-scrolled.bin"
);
const KITTY_DELETED: &[u8] = include_bytes!(
    "../../../../../../../../schemas/terminal/v1/fixtures/ghostty-kitty-graphics-deleted.bin"
);

/// The Host's own frames for a program that places a red-over-blue image on the second row,
/// scrolls it to the first, then deletes it, through the actual decode, retention and shell
/// renderers: Kitty commands for a capable host, cell raster and no bytes for an incapable one.
#[test]
fn host_kitty_frames_reach_the_shell_as_physical_images_or_cell_raster() {
    use crate::terminal_graphics::{Probe, Reply};
    use crate::terminal_lifecycle::SynchronizedUpdateLease;
    use crate::terminal_projection::TerminalProjection;
    use orchbee_client_server_protocol::TerminalViewerAction;
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding,
        TerminalGraphicsImageFormat, encode_transport_envelope, inspect_terminal_frame_route,
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Position};
    use std::{sync::Arc, time::Instant};

    let receive = |ui: &mut WorkbenchClientUi, bytes: &[u8]| {
        let record = encode_transport_envelope(&Envelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Event,
            kind: if bytes[6] == 1 { 41 } else { 42 },
            payload_encoding: PayloadEncoding::TerminalFrame,
            host_epoch: Some(inspect_terminal_frame_route(bytes).unwrap().host_epoch),
            correlation_id: None,
            optional_message: false,
            payload: bytes,
        })
        .unwrap();
        let outcome = ui.projections.last_mut().unwrap().value.receive(&record);
        assert!(matches!(
            outcome,
            Ok(Some(TerminalViewerAction::FrameApplied { .. }))
        ));
    };
    let surface = |kitty: bool| {
        let now = Instant::now();
        let mut probe = Probe::default();
        probe.activate(99);
        probe.query((80, 24), now).unwrap();
        probe.reply(Reply::Cell(Some((8, 16))), now);
        if kitty {
            probe.reply(
                Reply::Kitty {
                    image_id: 99,
                    placement_id: None,
                    success: true,
                },
                now,
            );
        }
        let facts = probe.facts();
        let surface = Surface::new(99, Arc::new(SynchronizedUpdateLease::new()));
        surface.update(facts, now, Vec::new(), false);
        (surface, facts, now)
    };
    let theme = Theme::rgb_test_dark();
    let paint = |ui: &mut WorkbenchClientUi, graphics: &Surface| {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut receipt = DrawReceipt::default();
        graphics.begin_frame(false);
        let buffer = terminal
            .draw(|frame| {
                let mut native = ui.view();
                native.set_graphics_surface(Some(graphics.clone()));
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    &ClientView::new(80),
                    Locale::EnUs,
                    Some(native),
                    &theme,
                    &mut crate::tui_hit_map::HitMapBuilder::new(),
                );
                graphics.finish_frame(receipt.graphics_draws(), false);
            })
            .unwrap()
            .buffer
            .clone();
        let mut output = Vec::new();
        let deleted = graphics
            .write_frame(&mut output, Position { x: 0, y: 0 })
            .unwrap();
        graphics.flushed(&deleted);
        ui.after_close_draw(receipt);
        (buffer, String::from_utf8(output).unwrap())
    };
    let raster_at = |buffer: &ratatui::buffer::Buffer, x: u16, y: u16| {
        let cell = &buffer[(x, y)];
        cell.symbol() == "▀" && cell.fg == Color::Rgb(255, 0, 0) && cell.bg == Color::Rgb(0, 0, 255)
    };

    for kitty in [false, true] {
        let (mut ui, _, _, _) = controlling_ui();
        let areas = crate::tui_layout::project_shell_areas(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            ClientView::new(80).sidebar_surface(),
        );
        let body = crate::tui_layout::arranged_pane_area(&areas, &ui.view()).unwrap();
        ui.observe_geometry(Geometry::new(body.width, body.height, 8, 16).unwrap());
        install_projection(&mut ui, None, 0);
        let route = inspect_terminal_frame_route(KITTY_PLACED).unwrap();
        ui.projections.last_mut().unwrap().value =
            TerminalProjection::new(route.attachment_id, route.host_epoch);
        let (graphics, facts, now) = surface(kitty);

        receive(&mut ui, KITTY_PLACED);
        let retained = ui.projections.last().unwrap().value.graphics().unwrap();
        assert_eq!(retained.images.len(), 1);
        assert_eq!(retained.images[0].format, TerminalGraphicsImageFormat::Rgb);
        assert_eq!(
            retained.images[0].pixels.as_slice(),
            &[255, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 255]
        );
        let placement = retained.placements[0];
        assert_eq!((placement.column, placement.row), (0, 1));
        assert_eq!(
            (placement.destination_width, placement.destination_height),
            (16, 16)
        );
        // Retained text frames never hold the pixels.
        assert!(
            ui.projections
                .last()
                .unwrap()
                .value
                .frame()
                .unwrap()
                .graphics
                .is_none()
        );

        let (cells, output) = paint(&mut ui, &graphics);
        assert!(raster_at(&cells, body.x, body.y + 1) && raster_at(&cells, body.x + 1, body.y + 1));
        assert!(!raster_at(&cells, body.x + 2, body.y + 1));
        let observation = ui
            .graphics_observation(&ui.control_observation(false).unwrap().1)
            .unwrap();
        assert!(
            observation
                .panes
                .iter()
                .all(|pane| pane.drawn_revision.is_none()),
            "program images are not Session layer receipts"
        );
        if !kitty {
            assert_eq!(output, "", "an incapable host receives no graphics bytes");
            receive(&mut ui, KITTY_SCROLLED);
            let (cells, output) = paint(&mut ui, &graphics);
            assert!(raster_at(&cells, body.x, body.y) && !raster_at(&cells, body.x, body.y + 1));
            assert_eq!(output, "");
            receive(&mut ui, KITTY_DELETED);
            let (cells, output) = paint(&mut ui, &graphics);
            assert!(!cells.content.iter().any(|cell| cell.symbol() == "▀"));
            assert_eq!(output, "");
            continue;
        }
        // The whole 2x1-cell image is uploaded at the measured 8x16 cell size.
        assert!(output.contains("a=t,t=d,f=32,s=16,v=16,i=1,"), "{output:?}");
        let upload = Reply::Kitty {
            image_id: 1,
            placement_id: None,
            success: true,
        };
        graphics.update(facts, now, vec![upload], false);
        let (cells, output) = paint(&mut ui, &graphics);
        assert!(
            !raster_at(&cells, body.x, body.y + 1),
            "physical output owns the cells"
        );
        assert!(
            output.contains(&format!(
                "\x1b[{};{}H\x1b_Ga=p,i=1,p=1,c=2,r=1,z=0,",
                body.y + 2,
                body.x + 1
            )),
            "{output:?}"
        );

        // The scrolled image retires its old copy and shows as cells until the new one lands.
        receive(&mut ui, KITTY_SCROLLED);
        let (cells, output) = paint(&mut ui, &graphics);
        assert!(raster_at(&cells, body.x, body.y) && !raster_at(&cells, body.x, body.y + 1));
        assert!(output.contains("a=d,d=I,i=1,q=2"), "{output:?}");
        assert!(output.contains("a=t,t=d,f=32,s=16,v=16,i=2,"), "{output:?}");

        // Deletion by the program releases every physical image.
        receive(&mut ui, KITTY_DELETED);
        assert!(
            ui.projections
                .last()
                .unwrap()
                .value
                .graphics()
                .unwrap()
                .is_empty()
        );
        let (cells, output) = paint(&mut ui, &graphics);
        assert!(!cells.content.iter().any(|cell| cell.symbol() == "▀"));
        assert!(output.contains("a=d,d=I,i=2,q=2"), "{output:?}");
    }
}
