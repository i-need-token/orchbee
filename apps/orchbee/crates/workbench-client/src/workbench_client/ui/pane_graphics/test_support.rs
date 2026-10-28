//! Whole-frame graphics evidence: the Client state and assertions stay here while the owning
//! application supplies its real native backend.

use super::super::test_support::{controlling_ui, install_projection};
use super::*;
use crate::{
    client_view::ClientView,
    localization::Locale,
    tui_graphics::Surface,
    tui_pointer::HitMapBuilder,
    tui_theme::Theme,
    workbench_client::{DrawReceipt, pane_graphics::Layer},
};
use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

pub(super) fn draw(
    ui: &WorkbenchClientUi,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (DrawReceipt, Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &ClientView::new(width),
                locale,
                Some(ui.view()),
                theme,
                &mut HitMapBuilder::new(),
            );
        })
        .unwrap();
    (receipt, terminal.backend().buffer().clone())
}

#[derive(Clone, Default)]
pub struct HostWire(std::rc::Rc<std::cell::RefCell<(Vec<u8>, usize)>>);

struct RepliesOnly;

impl crate::terminal_input::TerminalInputPort for RepliesOnly {
    fn forward(&mut self, _: &[u8]) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        panic!("host replies must never reach a retained pane");
    }
}

impl std::io::Write for HostWire {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.borrow_mut().1 += 1;
        Ok(())
    }
}

/// Scenario body for the owning application's test of the same name: `native` builds the real
/// native backend over `HostWire` with the given graphics probe id and returns its graphics surface.
pub fn actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board<B>(
    native: impl FnOnce(HostWire, crate::terminal_device::WindowDimensions, u32) -> (B, Surface),
) where
    B: ratatui::backend::Backend,
    B::Error: std::fmt::Debug,
{
    use crate::{
        terminal_device::WindowDimensions,
        terminal_graphics::{Probe, Reply},
    };
    use ratatui::{TerminalOptions, Viewport};
    let (mut ui, _, _, _) = controlling_ui();
    install_projection(&mut ui, Some((9, false, 24)), 0);
    let theme = Theme::rgb_test_dark();
    ui.after_close_draw(draw(&ui, 80, 24, Locale::EnUs, &theme).0);
    let view = ui.view();
    let panel = view.tab(0).unwrap().panel(0).unwrap();
    let source = Source::capture(panel, view.selected_presentation(panel));
    let metadata = PaneGraphicsLayer {
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
    };
    ui.graphics.install_for_test(
        PaneGraphicsCanvas::new(
            source,
            WorkbenchRevision::new(1),
            vec![Layer::new(metadata, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap()],
        )
        .unwrap(),
    );
    let now = std::time::Instant::now();
    let mut probe = Probe::default();
    probe.activate(99);
    probe.query((80, 24), now).unwrap();
    probe.reply(Reply::Cell(Some((1, 2))), now);
    probe.reply(
        Reply::Kitty {
            image_id: 99,
            placement_id: None,
            success: true,
        },
        now,
    );
    let facts = probe.facts();
    ui.observe_host_graphics(facts);
    let wire = HostWire::default();
    let (backend, graphics) = native(
        wire.clone(),
        WindowDimensions {
            columns: 80,
            rows: 24,
            pixel_width: 80,
            pixel_height: 48,
        },
        99,
    );
    graphics.update(facts, now, Vec::new(), false);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 80, 24)),
        },
    )
    .unwrap();
    *wire.0.borrow_mut() = (Vec::new(), 0);
    let mut dispatcher = crate::terminal_input::InputDispatcher::default();
    let mut input_view = ClientView::new(80);
    let mut receive = |packets: &[&[u8]]| {
        for packet in packets {
            dispatcher
                .dispatch(
                    &mut input_view,
                    packet,
                    &mut RepliesOnly,
                    now,
                    |_, _| Ok::<bool, std::io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        let (replies, overflow) = dispatcher.take_graphics_replies();
        graphics.update(facts, now, replies, overflow);
    };
    let paint = |ui: &mut WorkbenchClientUi, terminal: &mut Terminal<B>| {
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
                    &mut HitMapBuilder::new(),
                );
                graphics.finish_frame(receipt.graphics_draws(), false);
            })
            .unwrap()
            .buffer
            .clone();
        ui.after_close_draw(receipt);
        (
            ui.graphics_observation(&ui.control_observation(false).unwrap().1)
                .unwrap(),
            buffer,
        )
    };
    let (fallback, cells) = paint(&mut ui, &mut terminal);
    assert_eq!(
        fallback.capabilities.transport,
        orchbee_workbench_protocol::PaneGraphicsTransport::CellRaster
    );
    assert!(
        cells
            .content
            .iter()
            .any(|cell| cell.symbol() == "▀" && cell.fg == Color::Rgb(255, 0, 0))
    );
    assert!(
        String::from_utf8(wire.0.borrow().0.clone())
            .unwrap()
            .contains("a=t,t=d,f=32,s=2,v=2,i=1")
    );
    receive(&[b"\x1b_Gi=1;O", b"K\x1b\\"]);
    let (pending, cells) = paint(&mut ui, &mut terminal);
    assert_eq!(
        pending.capabilities.transport,
        orchbee_workbench_protocol::PaneGraphicsTransport::Kitty
    );
    assert!(
        pending
            .panes
            .iter()
            .all(|pane| pane.drawn_revision.is_none())
    );
    assert!(
        !cells
            .content
            .iter()
            .any(|cell| cell.symbol() == "▀" && cell.fg == Color::Rgb(255, 0, 0))
    );
    receive(&[b"\x1b_Gi=1,p=1;OK\x1b\\"]);
    let (confirmed, _) = paint(&mut ui, &mut terminal);
    assert!(
        confirmed
            .panes
            .iter()
            .any(|pane| pane.drawn_revision == Some(WorkbenchRevision::new(1)))
    );
    ui.board.open();
    let (covered, _) = paint(&mut ui, &mut terminal);
    assert!(covered.panes.is_empty());
    let wire = wire.0.borrow();
    let output = String::from_utf8(wire.0.clone()).unwrap();
    assert!(output.contains("a=p,i=1,p=1,c=2,r=1"));
    let last = output.rsplit("\x1b[?2026h").next().unwrap();
    assert!(last.contains("a=d,d=I,i=1,q=2"));
    assert!(!last.contains("a=p"));
    assert_eq!(output.matches("\x1b[?2026h").count(), 4);
    assert_eq!(output.matches("\x1b[?2026l").count(), 4);
    assert_eq!(
        wire.1, 4,
        "physical output shares each native draw's only flush"
    );
}
