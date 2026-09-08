use std::{
    collections::VecDeque,
    convert::Infallible,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ratatui::{Terminal, TerminalOptions, Viewport, backend::TestBackend};

use crate::{
    client_view::{ClientAction, ClientView, SessionActionDisposition},
    localization::Locale,
    terminal_device::WindowDimensions,
    terminal_input::{TerminalInputPort, TerminalInputUnavailable},
    termination_signal::TerminationSource,
    tui_backend::{FixedViewportBackend, ResizableBackend},
};

use super::{InputSource, LoopExit, LoopFailure, ViewportSource, run_event_loop, to_area};

struct QueueInputSource {
    inputs: VecDeque<io::Result<Option<Vec<u8>>>>,
}

pub(crate) fn run_native_close_resize_confirmation(
    session: &mut crate::terminal_session::TerminalSession,
) -> io::Result<Vec<bool>> {
    struct Surface<'a> {
        session: &'a mut crate::terminal_session::TerminalSession,
        prompts_after_draw: Vec<bool>,
    }
    impl TerminalInputPort for Surface<'_> {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.session.forward(bytes)
        }
        fn apply_client_action(
            &mut self,
            action: ClientAction,
        ) -> Option<SessionActionDisposition> {
            self.session.apply_client_action(action)
        }
        fn capture_confirmation_key(&mut self, key: crate::client_view::ClientKey) -> bool {
            self.session.capture_confirmation_key(key)
        }
        fn invalidate_close_display(&mut self) {
            self.session.invalidate_close_display();
        }
        fn observe_geometry(&mut self, geometry: orchbee_client_server_protocol::Geometry) {
            self.session.observe_geometry(geometry);
        }
    }
    impl super::TerminalSurface for Surface<'_> {
        fn tick(&mut self) -> bool {
            self.session.tick()
        }
        fn after_draw(&mut self) {
            self.session.after_draw();
        }
        fn after_close_draw(&mut self, receipt: crate::terminal_session::NativeDrawReceipt) {
            self.session.after_close_draw(receipt);
            self.prompts_after_draw.push(matches!(self.session.view(),
                crate::terminal_session::TerminalSessionView::Native(view) if view.close_confirmation().is_some()));
        }
        fn view(&self) -> Option<crate::terminal_session::TerminalSessionView<'_>> {
            Some(self.session.view())
        }
    }
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([
            Ok(Some(b"y".to_vec())),
            Ok(Some(b"n".to_vec())),
            Ok(Some(b"\x02q".to_vec())),
        ]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(initial),
            Ok(resized),
            Ok(resized),
            Ok(resized),
        ])),
    };
    let mut surface = Surface {
        session,
        prompts_after_draw: Vec::new(),
    };
    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &viewport,
            initial
        )?,
        LoopExit::User
    );
    Ok(surface.prompts_after_draw)
}

impl InputSource for QueueInputSource {
    fn next_input(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        match self.inputs.pop_front() {
            Some(input) => input,
            None => Err(io::Error::other("test input queue exhausted")),
        }
    }
}

struct FixedTermination(bool);

impl TerminationSource for FixedTermination {
    fn requested(&self) -> bool {
        self.0
    }
}

struct QueueViewportSource {
    dimensions: std::cell::RefCell<VecDeque<io::Result<WindowDimensions>>>,
}

struct FailingResizeViewportSource {
    initial: WindowDimensions,
    changed: WindowDimensions,
    calls: std::cell::Cell<usize>,
    writer_failure: Arc<AtomicBool>,
}

impl ViewportSource for FailingResizeViewportSource {
    fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        let calls = self.calls.get();
        self.calls.set(calls + 1);
        if calls == 0 {
            Ok(self.initial)
        } else {
            self.writer_failure.store(true, Ordering::Relaxed);
            Ok(self.changed)
        }
    }
}

struct SwitchableFailureWriter {
    failure: Arc<AtomicBool>,
}

impl Write for SwitchableFailureWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.failure.load(Ordering::Relaxed) {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected terminal writer failure",
            ))
        } else {
            Ok(buffer.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.failure.load(Ordering::Relaxed) {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected terminal writer failure",
            ))
        } else {
            Ok(())
        }
    }
}

impl ViewportSource for QueueViewportSource {
    fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        match self.dimensions.borrow_mut().pop_front() {
            Some(dimensions) => dimensions,
            None => Err(io::Error::other("test viewport queue exhausted")),
        }
    }
}

#[derive(Default)]
struct RecordingTerminalInput {
    forwarded: Vec<Vec<u8>>,
}

impl super::TerminalSurface for RecordingTerminalInput {}

impl TerminalInputPort for RecordingTerminalInput {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.push(input.to_vec());
        Err(TerminalInputUnavailable)
    }
}

impl ResizableBackend for TestBackend {
    fn set_window_dimensions(&mut self, dimensions: WindowDimensions) {
        self.resize(dimensions.columns, dimensions.rows);
    }
}

const fn dimensions(columns: u16, rows: u16) -> WindowDimensions {
    WindowDimensions {
        columns,
        pixel_height: rows * 20,
        pixel_width: columns * 8,
        rows,
    }
}

#[test]
fn connected_geometry_requires_real_cell_and_pixel_dimensions() -> io::Result<()> {
    let original = dimensions(120, 32);
    let view = ClientView::new(original.columns);
    let geometry = super::terminal_geometry(original, &view)?;
    assert_eq!((geometry.columns(), geometry.rows()), (88, 25));
    assert_eq!(
        (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
        (8, 20)
    );
    for incomplete in [
        WindowDimensions {
            pixel_width: 0,
            ..original
        },
        WindowDimensions {
            pixel_height: 0,
            ..original
        },
        WindowDimensions {
            columns: 0,
            ..original
        },
        WindowDimensions {
            rows: 0,
            ..original
        },
    ] {
        assert!(super::terminal_geometry(incomplete, &view).is_err());
    }
    Ok(())
}

fn test_terminal(dimensions: WindowDimensions) -> Terminal<TestBackend> {
    match Terminal::with_options(
        TestBackend::new(dimensions.columns, dimensions.rows),
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    ) {
        Ok(terminal) => terminal,
        Err(never) => match never {},
    }
}

fn test_loop(
    terminal: &mut Terminal<TestBackend>,
    locale: Locale,
    inputs: &mut impl InputSource,
    terminal_input: &mut impl super::TerminalSurface,
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    dimensions: WindowDimensions,
) -> io::Result<LoopExit> {
    match run_event_loop(
        terminal,
        locale,
        inputs,
        terminal_input,
        termination,
        viewport,
        dimensions,
    ) {
        Ok(exit) => Ok(exit),
        Err(LoopFailure::Input(error) | LoopFailure::Viewport(error)) => Err(error),
        Err(LoopFailure::Terminal(never)) => match never {},
    }
}

#[test]
fn exits_through_the_prefixed_user_binding_without_forwarding_it() -> io::Result<()> {
    let initial = dimensions(120, 30);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(initial), Ok(initial)])),
    };
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut input,
            &FixedTermination(false),
            &viewport,
            initial,
        )?,
        LoopExit::User
    );
    assert!(input.forwarded.is_empty());
    Ok(())
}

#[test]
fn returns_from_a_requested_signal_without_reading_input_or_viewport() -> io::Result<()> {
    let initial = dimensions(80, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::new(),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(initial)])),
    };
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut input,
            &FixedTermination(true),
            &viewport,
            initial,
        )?,
        LoopExit::Signal
    );
    Ok(())
}

#[test]
fn propagates_input_errors_to_the_restoring_session_boundary() -> io::Result<()> {
    let initial = dimensions(80, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Err(io::Error::other("injected poll failure"))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(initial)])),
    };
    let mut input = RecordingTerminalInput::default();

    let result = test_loop(
        &mut terminal,
        Locale::EnUs,
        &mut inputs,
        &mut input,
        &FixedTermination(false),
        &viewport,
        initial,
    );
    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::Other));
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
enum SurfaceEvent {
    InvalidateCloseDisplay,
    Observe(orchbee_client_server_protocol::Geometry),
    Tick,
    AfterDraw,
    Action(ClientAction),
    Forward(Vec<u8>),
}

#[derive(Default)]
struct OrderedSurface {
    events: Vec<SurfaceEvent>,
}

impl TerminalInputPort for OrderedSurface {
    fn invalidate_close_display(&mut self) {
        self.events.push(SurfaceEvent::InvalidateCloseDisplay);
    }
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.events.push(SurfaceEvent::Forward(input.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.events.push(SurfaceEvent::Action(action));
        Some(SessionActionDisposition::KeepNavigator)
    }

    fn observe_geometry(&mut self, geometry: orchbee_client_server_protocol::Geometry) {
        self.events.push(SurfaceEvent::Observe(geometry));
    }
}

impl super::TerminalSurface for OrderedSurface {
    fn tick(&mut self) -> bool {
        self.events.push(SurfaceEvent::Tick);
        false
    }

    fn after_draw(&mut self) {
        self.events.push(SurfaceEvent::AfterDraw);
    }
}

#[test]
fn observes_fresh_geometry_before_initial_draw_tick_and_same_batch_effects() -> io::Result<()> {
    let inherited = dimensions(120, 30);
    let measured = dimensions(100, 28);
    let mut terminal = test_terminal(inherited);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"\x02b\x02cbytes\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(measured),
            Ok(measured),
            Ok(measured),
        ])),
    };
    let mut surface = OrderedSurface::default();

    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &viewport,
            inherited,
        )?,
        LoopExit::User
    );

    let docked = super::terminal_geometry(measured, &ClientView::new(measured.columns))?;
    let mut expanded_view = ClientView::new(measured.columns);
    let prefix = expanded_view.decide(crate::client_view::ClientKey::ControlB);
    expanded_view.commit(prefix, SessionActionDisposition::KeepNavigator);
    let toggle = expanded_view.decide(crate::client_view::ClientKey::Plain('b'));
    expanded_view.commit(toggle, SessionActionDisposition::KeepNavigator);
    let expanded = super::terminal_geometry(measured, &expanded_view)?;
    assert_ne!(docked, expanded);
    assert_eq!(
        surface.events,
        [
            SurfaceEvent::InvalidateCloseDisplay,
            SurfaceEvent::Observe(docked),
            SurfaceEvent::Tick,
            SurfaceEvent::AfterDraw,
            SurfaceEvent::Observe(docked),
            SurfaceEvent::Tick,
            SurfaceEvent::Action(ClientAction::ToggleBusinessSidebar),
            SurfaceEvent::Observe(expanded),
            SurfaceEvent::Action(ClientAction::CreateTab),
            SurfaceEvent::Forward(b"bytes".to_vec()),
            SurfaceEvent::Action(ClientAction::Exit),
        ]
    );
    Ok(())
}

#[test]
fn required_viewport_failure_prevents_dispatch_of_an_already_read_batch() -> io::Result<()> {
    let initial = dimensions(80, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"already read".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(initial),
            Err(io::Error::other("injected viewport failure")),
        ])),
    };
    let mut input = RecordingTerminalInput::default();

    let result = test_loop(
        &mut terminal,
        Locale::EnUs,
        &mut inputs,
        &mut input,
        &FixedTermination(false),
        &viewport,
        initial,
    );

    assert!(matches!(result, Err(error) if error.kind() == io::ErrorKind::Other));
    assert!(input.forwarded.is_empty());
    Ok(())
}

#[test]
fn required_terminal_resize_failure_prevents_dispatch_of_an_already_read_batch() -> io::Result<()> {
    let initial = dimensions(120, 30);
    let narrow = dimensions(70, 24);
    let writer_failure = Arc::new(AtomicBool::new(false));
    let backend = FixedViewportBackend::new(
        SwitchableFailureWriter {
            failure: Arc::clone(&writer_failure),
        },
        initial,
    );
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(initial)),
        },
    )?;
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"before resize".to_vec()))]),
    };
    let viewport = FailingResizeViewportSource {
        initial,
        changed: narrow,
        calls: std::cell::Cell::new(0),
        writer_failure: Arc::clone(&writer_failure),
    };
    let mut input = RecordingTerminalInput::default();

    let result = run_event_loop(
        &mut terminal,
        Locale::EnUs,
        &mut inputs,
        &mut input,
        &FixedTermination(false),
        &viewport,
        initial,
    );
    writer_failure.store(false, Ordering::Relaxed);

    assert!(matches!(
        result,
        Err(LoopFailure::Terminal(error)) if error.kind() == io::ErrorKind::BrokenPipe
    ));
    assert!(input.forwarded.is_empty());
    Ok(())
}

#[test]
fn applies_explicit_viewport_changes_without_backend_global_queries() -> io::Result<()> {
    let initial = dimensions(120, 30);
    let narrow = dimensions(70, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([
            Ok(None),
            Ok(Some(b"ordinary".to_vec())),
            Ok(Some(b"\x02q".to_vec())),
        ]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(initial),
            Ok(narrow),
            Ok(narrow),
            Ok(narrow),
        ])),
    };
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut input,
            &FixedTermination(false),
            &viewport,
            initial,
        )?,
        LoopExit::User
    );
    assert_eq!(input.forwarded, [b"ordinary".to_vec()]);
    assert_eq!(terminal.size().map_err(infallible_to_io)?.width, 70);
    assert_eq!(terminal.size().map_err(infallible_to_io)?.height, 24);
    Ok(())
}

fn infallible_to_io(never: Infallible) -> io::Error {
    match never {}
}

#[test]
fn failed_draw_never_acknowledges_the_pending_terminal_frame() {
    struct Surface {
        failure: Arc<AtomicBool>,
        acknowledged: usize,
        close_acknowledged: usize,
        ticks: usize,
    }
    impl TerminalInputPort for Surface {
        fn forward(&mut self, _input: &[u8]) -> Result<(), TerminalInputUnavailable> {
            Ok(())
        }
    }
    impl super::TerminalSurface for Surface {
        fn after_close_draw(&mut self, _receipt: crate::terminal_session::NativeDrawReceipt) {
            self.close_acknowledged += 1;
        }
        fn tick(&mut self) -> bool {
            self.ticks += 1;
            if self.ticks == 2 {
                self.failure.store(true, Ordering::Relaxed);
            }
            true
        }
        fn after_draw(&mut self) {
            self.acknowledged += 1;
        }
    }
    let dimensions = dimensions(120, 32);
    let failure = Arc::new(AtomicBool::new(false));
    let backend = FixedViewportBackend::new(
        SwitchableFailureWriter {
            failure: failure.clone(),
        },
        dimensions,
    );
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .expect("test backend");
    let mut surface = Surface {
        failure,
        acknowledged: 0,
        close_acknowledged: 0,
        ticks: 0,
    };
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(None)]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(dimensions), Ok(dimensions)])),
    };
    assert!(matches!(
        run_event_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &viewport,
            dimensions
        ),
        Err(LoopFailure::Terminal(_))
    ));
    assert_eq!(
        surface.acknowledged, 1,
        "only the successful initial draw may publish feedback"
    );
    assert_eq!(
        surface.close_acknowledged, 1,
        "failed draw cannot publish a Close visibility receipt"
    );
}

#[test]
fn outer_resize_invalidates_close_before_input_even_when_terminal_geometry_is_equal()
-> io::Result<()> {
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"y\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(initial), Ok(resized)])),
    };
    let mut surface = OrderedSurface::default();
    assert_eq!(
        test_loop(
            &mut terminal,
            Locale::EnUs,
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &viewport,
            initial
        )?,
        LoopExit::User
    );
    let geometry = super::terminal_geometry(initial, &ClientView::new(100))?;
    assert_eq!(
        geometry,
        super::terminal_geometry(resized, &ClientView::new(70))?
    );
    assert_eq!(
        surface.events,
        [
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::AfterDraw,
            SurfaceEvent::InvalidateCloseDisplay,
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::Forward(b"y".to_vec()),
            SurfaceEvent::Action(ClientAction::Exit),
        ]
    );
    Ok(())
}
