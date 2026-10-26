use std::{
    collections::VecDeque,
    convert::Infallible,
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ratatui::{Terminal, TerminalOptions, Viewport, backend::TestBackend};

use crate::{
    client_view::{ClientAction, ClientActionDisposition, ClientView},
    localization::Locale,
    terminal_device::WindowDimensions,
    terminal_input::{ModeRequest, TerminalInputPort, TerminalInputUnavailable},
    terminal_lifecycle::SynchronizedUpdateLease,
    termination_signal::TerminationSource,
    tui_backend::{FixedViewportBackend, ResizableBackend},
};

use super::{InputSource, LoopExit, LoopFailure, ViewportSource, run_event_loop, to_area};
mod action_menu;
mod divider;
mod effects;
mod graphics;
mod machine_picker_capture;
mod notification_too_small;
mod rename;
mod settings;
pub(crate) use rename::run_native_rename_resize;
pub(crate) use settings::{SettingsFrameSource, settings_frame};

struct QueueInputSource {
    inputs: VecDeque<io::Result<Option<Vec<u8>>>>,
}

pub(crate) fn run_native_close_resize_confirmation(
    session: &mut crate::workbench_client::WorkbenchClient,
) -> io::Result<Vec<bool>> {
    struct Surface<'a> {
        session: &'a mut crate::workbench_client::WorkbenchClient,
        prompts_after_draw: Vec<bool>,
    }
    impl TerminalInputPort for Surface<'_> {
        fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
            self.session.native_capture_claims()
        }
        fn native_capture_context(
            &self,
            owner: crate::tui_composition::NativeCaptureOwner,
        ) -> Option<crate::workbench_client::NativeCaptureContext> {
            self.session.native_capture_context(owner)
        }
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.session.forward(bytes)
        }
        fn apply_client_action(&mut self, action: ClientAction) -> Option<ClientActionDisposition> {
            self.session.apply_client_action(action)
        }
        fn capture_tab_confirmation_key_for(
            &mut self,
            token: crate::workbench_client::NativeCloseConfirmationToken,
            key: crate::client_view::ClientKey,
        ) -> bool {
            self.session.capture_tab_confirmation_key_for(token, key)
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
        fn after_close_draw(&mut self, receipt: crate::workbench_client::NativeDrawReceipt) {
            self.session.after_close_draw(receipt);
            self.prompts_after_draw.push({
                let view = self.session.view();
                view.close_confirmation().is_some()
            });
        }
        fn view(&self) -> Option<crate::workbench_client::WorkbenchClientView<'_>> {
            Some(self.session.view())
        }
    }
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let mut terminal = test_terminal(initial);
    // The lone `Esc` packet resolves at the next loop iteration once the
    // decoder's bounded wait elapses; the idle poll in between advances the
    // clocked sample time past the deadline.
    let mut inputs = ClockedInputSource {
        inputs: QueueInputSource {
            inputs: VecDeque::from([
                Ok(Some(b"\r".to_vec())),
                Ok(Some(b"\x1b".to_vec())),
                Ok(None),
                Ok(Some(b"\x02q".to_vec())),
            ]),
        },
        now: std::cell::Cell::new(Instant::now()),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(Some(resized)),
            Ok(Some(resized)),
            Ok(Some(resized)),
            Ok(Some(resized)),
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

/// A queued source whose `sample_time` advances past the decoder's bounded
/// lone-Esc wait on every call: an idle poll resolves a queued `Esc` without
/// real sleeps, while a ready continuation survives a busy frame.
struct ClockedInputSource {
    inputs: QueueInputSource,
    now: std::cell::Cell<Instant>,
}

impl InputSource for ClockedInputSource {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.inputs.next_input(timeout)
    }

    fn sample_time(&self) -> Instant {
        let now = self.now.get();
        self.now.set(now + Duration::from_millis(60));
        now
    }
}

struct FixedTermination(bool);

impl TerminationSource for FixedTermination {
    fn requested(&self) -> bool {
        self.0
    }
}

/// Admits the startup frame, then requests termination at the first loop turn.
#[derive(Default)]
struct TerminationAfterStartup(std::cell::Cell<bool>);

impl TerminationSource for TerminationAfterStartup {
    fn requested(&self) -> bool {
        self.0.replace(true)
    }
}

struct QueueViewportSource {
    dimensions: std::cell::RefCell<VecDeque<io::Result<Option<WindowDimensions>>>>,
}

struct FailingResizeViewportSource {
    initial: WindowDimensions,
    changed: WindowDimensions,
    calls: std::cell::Cell<usize>,
    writer_failure: Arc<AtomicBool>,
}

impl ViewportSource for FailingResizeViewportSource {
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        let calls = self.calls.get();
        self.calls.set(calls + 1);
        if calls == 0 {
            Ok(Some(self.initial))
        } else {
            self.writer_failure.store(true, Ordering::Relaxed);
            Ok(Some(self.changed))
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
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
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
fn connected_geometry_requires_real_cell_dimensions() -> io::Result<()> {
    let original = dimensions(120, 32);
    let view = ClientView::new(original.columns);
    let geometry = super::terminal_geometry(original, &view, None)?
        .expect("a Full viewport must present a pane");
    // 32 rows less the tab strip, reserved status row, frame and leaf title.
    assert_eq!((geometry.columns(), geometry.rows()), (88, 28));
    assert_eq!(
        (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
        (8, 20)
    );
    // Zero cell counts classify as TooSmall: the resize is withheld (Ok(None))
    // rather than erroring or fabricating a 1x1 pane.
    for empty in [
        WindowDimensions {
            columns: 0,
            ..original
        },
        WindowDimensions {
            rows: 0,
            ..original
        },
    ] {
        assert_eq!(super::terminal_geometry(empty, &view, None)?, None);
    }
    Ok(())
}

#[test]
fn too_small_geometry_withholds_resize_instead_of_fabricating() -> io::Result<()> {
    for dimensions in [
        dimensions(15, 4),
        dimensions(120, 2),
        dimensions(2, 30),
        dimensions(1, 1),
    ] {
        let view = ClientView::new(dimensions.columns);
        let geometry = super::terminal_geometry(dimensions, &view, None)?;
        assert!(
            geometry.is_none(),
            "too-small {dimensions:?} must withhold the resize (Ok(None)), not fabricate one"
        );
    }
    Ok(())
}

#[test]
fn too_small_native_popup_surface_is_empty_until_the_shell_can_paint_it() {
    for tiny in [dimensions(100, 4), dimensions(19, 24)] {
        let view = ClientView::new(tiny.columns);
        assert_eq!(
            super::popup_surface_area(tiny, &view, 1, false),
            ratatui::layout::Rect::default(),
            "{tiny:?} must suspend a popup that the shell cannot paint"
        );
    }

    let restored = dimensions(100, 24);
    let view = ClientView::new(restored.columns);
    let area = super::popup_surface_area(restored, &view, 1, false);
    assert!(area.width > 0 && area.height > 0);
}

#[test]
fn connected_geometry_reports_unknown_when_any_cell_pixel_axis_is_missing() -> io::Result<()> {
    let original = dimensions(120, 32);
    let view = ClientView::new(original.columns);
    for measured in [
        WindowDimensions {
            pixel_width: 0,
            pixel_height: 0,
            ..original
        },
        WindowDimensions {
            pixel_width: 0,
            ..original
        },
        WindowDimensions {
            pixel_height: 0,
            ..original
        },
    ] {
        let geometry = super::terminal_geometry(measured, &view, None)?
            .expect("a Full viewport still presents a pane");
        assert_eq!(
            (geometry.cell_pixel_width(), geometry.cell_pixel_height()),
            (0, 0)
        );
        assert_eq!((geometry.columns(), geometry.rows()), (88, 28));
    }
    Ok(())
}

#[test]
fn minimal_geometry_deducts_no_invisible_frame() -> io::Result<()> {
    let view = ClientView::new(50);
    // 50x10 is Minimal: the panel spans the host area above the reserved status row,
    // with no border cells deducted because none are drawn.
    let geometry = super::terminal_geometry(dimensions(50, 10), &view, None)?
        .expect("a Minimal viewport still presents a pane");
    assert_eq!((geometry.columns(), geometry.rows()), (50, 9));
    Ok(())
}

#[test]
fn too_small_viewport_pauses_resize_and_recovers_with_real_geometry() -> io::Result<()> {
    let initial = dimensions(100, 24);
    let tiny = dimensions(15, 4);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"x".to_vec())), Ok(Some(b"\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(Some(tiny)),
            Ok(Some(initial)),
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
            initial,
        )?,
        LoopExit::User
    );

    let geometry = super::terminal_geometry(initial, &ClientView::new(100), None)?
        .expect("a presentable viewport must produce pane geometry");
    // While too small no resize is issued — the session keeps its last valid
    // geometry, typing still reaches it, and observation resumes unchanged
    // once a real pane fits again.
    assert_eq!(
        surface.events,
        [
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::AfterDraw,
            SurfaceEvent::InvalidateCloseDisplay,
            SurfaceEvent::Tick,
            SurfaceEvent::Forward(b"x".to_vec()),
            SurfaceEvent::AfterDraw,
            SurfaceEvent::InvalidateCloseDisplay,
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::Action(ClientAction::Exit),
        ]
    );
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
        super::Chrome {
            locale,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        inputs,
        terminal_input,
        termination,
        viewport,
        dimensions,
        true,
    ) {
        Ok(exit) => Ok(exit),
        Err(LoopFailure::Input(error) | LoopFailure::Viewport(error)) => Err(error),
        Err(LoopFailure::Terminal(never)) => match never {},
    }
}

#[test]
fn settled_button_only_initialization_queries_once_after_the_first_draw() -> io::Result<()> {
    let initial = dimensions(80, 24);
    for (mouse, expected_queries) in [(false, 0), (true, 1)] {
        let mut terminal = test_terminal(initial);
        let mut inputs = QueueInputSource {
            inputs: VecDeque::new(),
        };
        let viewport = QueueViewportSource {
            dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial))])),
        };
        let mut input = RecordingTerminalInput::default();
        let mut modes = Vec::new();
        let result = super::run_event_loop_with_modes(
            &mut terminal,
            super::Chrome {
                locale: Locale::EnUs,
                theme: &crate::tui_theme::Theme::terminal_native(),
            },
            (&mut inputs, &mut |request| {
                modes.push(request);
                Ok(())
            }),
            &mut input,
            &TerminationAfterStartup::default(),
            &viewport,
            initial,
            mouse,
            ClientView::new(initial.columns),
        );
        assert_eq!(result.unwrap(), LoopExit::Signal);
        assert!(
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .any(|cell| !cell.symbol().trim().is_empty()),
            "the startup frame paints before host queries consume their reply window"
        );
        let mut expected_modes = vec![ModeRequest::AppearanceQuery(
            crate::terminal_host_appearance::Query,
        )];
        if expected_queries == 1 {
            expected_modes.push(ModeRequest::PointerAdmissionQuery(
                crate::terminal_pointer_admission::Query,
            ));
        }
        assert_eq!(modes, expected_modes);
    }
    Ok(())
}

#[test]
fn exits_through_the_prefixed_user_binding_without_forwarding_it() -> io::Result<()> {
    let initial = dimensions(120, 30);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial)), Ok(Some(initial))])),
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
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial))])),
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
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial))])),
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

    fn apply_client_action(&mut self, action: ClientAction) -> Option<ClientActionDisposition> {
        self.events.push(SurfaceEvent::Action(action));
        Some(ClientActionDisposition::KeepNavigator)
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
            Ok(Some(measured)),
            Ok(Some(measured)),
            Ok(Some(measured)),
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

    let docked = super::terminal_geometry(measured, &ClientView::new(measured.columns), None)?
        .expect("a presentable viewport must produce pane geometry");
    let mut expanded_view = ClientView::new(measured.columns);
    use crate::client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput};
    let key = |code, mods| PrefixInput::Key(KeyEvent::press(code, mods));
    let prefix = expanded_view.decide(key(KeyCode::Char('b'), Modifiers::CTRL));
    expanded_view.commit(prefix, ClientActionDisposition::KeepNavigator);
    let toggle = expanded_view.decide(key(KeyCode::Char('b'), Modifiers::NONE));
    expanded_view.commit(toggle, ClientActionDisposition::KeepNavigator);
    let expanded = super::terminal_geometry(measured, &expanded_view, None)?
        .expect("a presentable viewport must produce pane geometry");
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
            Ok(Some(initial)),
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
        Arc::new(SynchronizedUpdateLease::new()),
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
        super::Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        &mut inputs,
        &mut input,
        &FixedTermination(false),
        &viewport,
        initial,
        true,
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
            Ok(Some(initial)),
            Ok(Some(narrow)),
            Ok(Some(narrow)),
            Ok(Some(narrow)),
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
        fn after_close_draw(&mut self, _receipt: crate::workbench_client::NativeDrawReceipt) {
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
        Arc::new(SynchronizedUpdateLease::new()),
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
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(dimensions)),
            Ok(Some(dimensions)),
        ])),
    };
    assert!(matches!(
        run_event_loop(
            &mut terminal,
            super::Chrome {
                locale: Locale::EnUs,
                theme: &crate::tui_theme::Theme::terminal_native(),
            },
            &mut inputs,
            &mut surface,
            &FixedTermination(false),
            &viewport,
            dimensions,
            true
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
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(Some(initial)), Ok(Some(resized))])),
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
    let geometry = super::terminal_geometry(initial, &ClientView::new(100), None)?
        .expect("a presentable viewport must produce pane geometry");
    assert_eq!(
        geometry,
        super::terminal_geometry(resized, &ClientView::new(70), None)?
            .expect("a presentable viewport must produce pane geometry")
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

#[test]
fn transient_zero_cell_observations_preserve_the_last_valid_viewport() -> io::Result<()> {
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([
            Ok(None),
            Ok(None),
            Ok(Some(b"kept".to_vec())),
            Ok(Some(b"\x02q".to_vec())),
        ]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(None),
            Ok(None),
            Ok(Some(resized)),
            Ok(Some(resized)),
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
    assert_eq!(input.forwarded, [b"kept".to_vec()]);
    assert_eq!(terminal.size().map_err(infallible_to_io)?.width, 70);
    assert_eq!(terminal.size().map_err(infallible_to_io)?.height, 24);
    Ok(())
}

#[test]
fn transient_zero_cells_do_not_swallow_a_real_viewport_failure() -> io::Result<()> {
    let initial = dimensions(80, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(None), Ok(None)]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(None),
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected viewport failure",
            )),
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

    assert!(matches!(
        result,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied
    ));
    Ok(())
}

#[test]
fn transient_zero_cells_repeat_only_the_last_valid_geometry() -> io::Result<()> {
    let initial = dimensions(100, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(None), Ok(Some(b"\x02q".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(None),
            Ok(None),
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
            initial,
        )?,
        LoopExit::User
    );
    let geometry = super::terminal_geometry(initial, &ClientView::new(100), None)?
        .expect("an unavailable workbench still projects a pane");
    assert_eq!(
        surface.events,
        [
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::AfterDraw,
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Observe(geometry),
            SurfaceEvent::Tick,
            SurfaceEvent::Action(ClientAction::Exit),
        ]
    );
    Ok(())
}

mod window_title;

#[test]
fn cached_theme_replaces_the_complete_frame_without_changing_geometry_or_forwarding_input() {
    use crate::terminal_text::display_width;
    use crate::tui_theme::{ColorLevel, Role, Theme};
    use orchbee_local_site_runtime::WindowsThemeName;
    for (width, height) in [(80, 24), (120, 40), (70, 18), (40, 10), (18, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut terminal = test_terminal(dimensions(width, height));
            let mut view = ClientView::new(width);
            view.configure_color_level(ColorLevel::Truecolor);
            let areas = crate::tui_layout::project_shell_areas(
                to_area(dimensions(width, height)),
                view.sidebar_surface(),
            );
            let context = format!("{width}x{height} {locale:?} {:?}", areas.tier);
            let geometry =
                super::terminal_geometry(dimensions(width, height), &view, None).unwrap();
            let mut surface = OrderedSurface::default();
            let mut dispatcher = crate::terminal_input::InputDispatcher::default();
            for (name, expected) in [
                (WindowsThemeName::OneLight, Theme::one_light()),
                (WindowsThemeName::OneDark, Theme::one_dark()),
                (WindowsThemeName::TerminalNative, Theme::terminal_native()),
            ] {
                view.configure_theme(name);
                assert!(
                    super::draw_surface(
                        &mut terminal,
                        &mut view,
                        super::Chrome {
                            locale,
                            theme: &Theme::terminal_native()
                        },
                        &mut surface,
                        &mut dispatcher,
                        &mut |_| Ok(())
                    )
                    .is_ok(),
                    "{context} {name:?}: draw"
                );
                assert_eq!(
                    super::terminal_geometry(dimensions(width, height), &view, None).unwrap(),
                    geometry,
                    "{context} {name:?}: geometry"
                );
                // This sessionless fixture has canvas beneath the unavailable notice and panel
                // frame. Chrome owns the projected bars, the reserved status row and sidebar.
                for y in 0..height {
                    let mut x = 0;
                    while x < width {
                        let cell = &terminal.backend().buffer()[(x, y)];
                        let position = (x, y).into();
                        let role = if areas.status.contains(position)
                            || areas.tabs.contains(position)
                            || areas
                                .sidebar
                                .is_some_and(|sidebar| sidebar.area.contains(position))
                        {
                            Role::SurfaceChrome
                        } else {
                            Role::SurfaceCanvas
                        };
                        assert_eq!(
                            cell.bg,
                            expected.color(role),
                            "{context} {name:?}: {role:?} at ({x}, {y})"
                        );
                        // Wide graphemes paint their continuation cells; TestBackend
                        // retains no independent style for those hidden buffer slots.
                        x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
                    }
                }
                assert!(
                    surface
                        .events
                        .iter()
                        .all(|event| !matches!(event, SurfaceEvent::Forward(_))),
                    "{context} {name:?}: forwarded input"
                );
            }
            view.configure_color_level(ColorLevel::Monochrome);
            view.configure_theme(WindowsThemeName::OneLight);
            assert!(
                super::draw_surface(
                    &mut terminal,
                    &mut view,
                    super::Chrome {
                        locale,
                        theme: &Theme::terminal_native()
                    },
                    &mut surface,
                    &mut dispatcher,
                    &mut |_| Ok(())
                )
                .is_ok(),
                "{context} Monochrome OneLight: draw"
            );
            assert_eq!(
                super::terminal_geometry(dimensions(width, height), &view, None).unwrap(),
                geometry,
                "{context} Monochrome OneLight: geometry"
            );
            assert!(
                surface
                    .events
                    .iter()
                    .all(|event| !matches!(event, SurfaceEvent::Forward(_))),
                "{context} Monochrome OneLight: forwarded input"
            );
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .all(|cell| cell.fg == ratatui::style::Color::Reset
                        && cell.bg == ratatui::style::Color::Reset),
                "{context} Monochrome OneLight: foreground and background must reset"
            );
        }
    }
}
