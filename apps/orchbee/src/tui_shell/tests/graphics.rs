//! Actual shell loop witnesses: capability replies cannot become pane input.
use super::*;
use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_client::test_support::terminal_graphics::Facts;
use orchbee_workbench_client::test_support::terminal_graphics::FactsTestAccess;
use orchbee_workbench_client::test_support::terminal_graphics::Query;
use orchbee_workbench_client::test_support::terminal_graphics::QueryFixture;
use ratatui::backend::Backend;

struct Inputs {
    now: Instant,
    packets: VecDeque<(Duration, Vec<u8>)>,
}
impl InputSource for Inputs {
    fn next_input(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let (advance, packet) = self
            .packets
            .pop_front()
            .ok_or_else(|| io::Error::other("input fixture exhausted"))?;
        self.now += advance;
        Ok(Some(packet))
    }
    fn sample_time(&self) -> Instant {
        self.now
    }
}
struct HostViewport(WindowDimensions);
impl ViewportSource for HostViewport {
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        Ok(Some(self.0))
    }
}
#[derive(Default)]
struct Surface {
    bytes: Vec<u8>,
    facts: Vec<Facts>,
    geometry: Vec<Geometry>,
    intent: usize,
}
impl TerminalInputPort for Surface {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn observe_terminal(&mut self, observation: TerminalObservation) {
        match observation {
            TerminalObservation::HostGraphics(facts) => self.facts.push(facts),
            TerminalObservation::Geometry(geometry) => self.geometry.push(geometry),
            TerminalObservation::InputIntent => self.intent += 1,
            _ => {}
        }
    }
}
impl super::super::TerminalSurface for Surface {}

#[test]
fn graphics_measurement_flows_through_the_actual_shell_without_reply_or_prefix_leaks()
-> io::Result<()> {
    let initial = WindowDimensions {
        columns: 100,
        rows: 30,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut backend = FixedViewportBackend::new(
        Vec::<u8>::new(),
        initial,
        Arc::new(
            orchbee_workbench_client::test_support::terminal_lifecycle::synchronized_update_lease(),
        ),
    );
    backend.enable_graphics_probe(31);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(initial)),
        },
    )?;
    let mut inputs = Inputs {
        now: Instant::now(),
        packets: VecDeque::from([
            (Duration::from_millis(10), b"a\x1b[6;16;".to_vec()),
            (Duration::from_millis(10), b"8t\x1b_Gi=31;".to_vec()),
            (
                Duration::from_millis(10),
                b"OK\x1b\\\x02\x1b_Gi=31;OK\x1b\\q".to_vec(),
            ),
        ]),
    };
    let mut surface = Surface::default();
    let mut queries = Vec::<Query>::new();
    let outcome = super::super::run_event_loop_with_modes(
        &mut terminal,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        (&mut inputs, &mut |request| {
            if let ModeRequest::GraphicsQuery(query) = request {
                queries.push(query);
            }
            Ok(())
        }),
        &mut surface,
        &FixedTermination(false),
        &HostViewport(initial),
        initial,
        true,
        ClientView::new(initial.columns),
    );
    assert!(matches!(outcome, Ok(LoopExit::User)));
    assert_eq!(surface.bytes, b"a");
    assert_eq!(
        surface.intent, 3,
        "only a, the prefix, and q are human input"
    );
    assert_eq!(
        queries,
        [Query::from(QueryFixture {
            cell: true,
            image_id: Some(31)
        })]
    );
    assert!(
        surface
            .geometry
            .iter()
            .any(|g| (g.cell_pixel_width(), g.cell_pixel_height()) == (8, 16))
    );
    assert!(
        surface
            .facts
            .iter()
            .any(|facts| facts.measurement.is_some())
    );
    assert_eq!(
        terminal.backend_mut().window_size()?.pixels,
        ratatui::layout::Size::new(800, 480)
    );
    for facts in surface.facts {
        assert_eq!(
            facts.cell_capabilities().transport,
            orchbee_workbench_protocol::PaneGraphicsTransport::CellRaster
        );
        assert!(!facts.cell_capabilities().pixel_mouse);
    }
    Ok(())
}

#[test]
fn absent_and_late_graphics_replies_do_not_delay_the_first_draw_or_restore_unknown_pixels()
-> io::Result<()> {
    let initial = WindowDimensions {
        columns: 100,
        rows: 30,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut backend = FixedViewportBackend::new(
        Vec::<u8>::new(),
        initial,
        Arc::new(
            orchbee_workbench_client::test_support::terminal_lifecycle::synchronized_update_lease(),
        ),
    );
    backend.enable_graphics_probe(31);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(initial)),
        },
    )?;
    let mut inputs = Inputs {
        now: Instant::now(),
        packets: VecDeque::from([
            (
                Duration::from_millis(350),
                b"x\x1b[6;16;8t\x1b_Gi=31;OK\x1b\\".to_vec(),
            ),
            (Duration::from_millis(10), b"\x02q".to_vec()),
        ]),
    };
    let mut surface = Surface::default();
    let outcome = super::super::run_event_loop_with_modes(
        &mut terminal,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        (&mut inputs, &mut |_| Ok(())),
        &mut surface,
        &FixedTermination(false),
        &HostViewport(initial),
        initial,
        true,
        ClientView::new(initial.columns),
    );
    assert!(matches!(outcome, Ok(LoopExit::User)));
    assert_eq!(surface.bytes, b"x");
    assert_eq!(surface.intent, 3);
    assert!(
        surface
            .facts
            .iter()
            .all(|facts| facts.measurement.is_none() && !facts.kitty())
    );
    assert!(
        surface
            .geometry
            .iter()
            .all(|g| (g.cell_pixel_width(), g.cell_pixel_height()) == (0, 0))
    );
    assert_eq!(
        terminal.backend_mut().window_size()?.pixels,
        ratatui::layout::Size::ZERO
    );
    Ok(())
}
