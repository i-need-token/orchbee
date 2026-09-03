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
    dimensions: WindowDimensions,
    writer_failure: Arc<AtomicBool>,
}

impl ViewportSource for FailingResizeViewportSource {
    fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        self.writer_failure.store(true, Ordering::Relaxed);
        Ok(self.dimensions)
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
    terminal_input: &mut impl TerminalInputPort,
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
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(initial)])),
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
        dimensions: std::cell::RefCell::new(VecDeque::new()),
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
        dimensions: std::cell::RefCell::new(VecDeque::new()),
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

#[test]
fn forwards_read_input_before_a_subsequent_viewport_failure() -> io::Result<()> {
    let initial = dimensions(80, 24);
    let mut terminal = test_terminal(initial);
    let mut inputs = QueueInputSource {
        inputs: VecDeque::from([Ok(Some(b"already read".to_vec()))]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([Err(io::Error::other(
            "injected viewport failure",
        ))])),
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
    assert_eq!(input.forwarded, [b"already read".to_vec()]);
    Ok(())
}

#[test]
fn forwards_read_input_before_a_subsequent_terminal_resize_failure() -> io::Result<()> {
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
        dimensions: narrow,
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
    assert_eq!(input.forwarded, [b"before resize".to_vec()]);
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
        dimensions: std::cell::RefCell::new(VecDeque::from([Ok(narrow), Ok(narrow), Ok(narrow)])),
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
