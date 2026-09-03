use std::{
    fmt,
    fs::File,
    io::{self, Read},
    os::fd::AsFd,
    time::Duration,
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use ratatui::{Terminal, TerminalOptions, Viewport, backend::Backend, layout::Rect};

use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_device::{InheritedTerminal, TerminalAdmissionError, WindowDimensions},
    terminal_input::{DispatchOutcome, UnavailableTerminalInputPort, dispatch_input},
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    termination_signal::{TerminationSignal, TerminationSource},
    tui_backend::{FixedViewportBackend, ResizableBackend},
    tui_render::render_shell,
};

const EVENT_POLL_INTERVAL_MILLISECONDS: u16 = 50;
const MAXIMUM_INPUT_BYTES_PER_POLL: usize = 4096;

#[derive(Debug)]
pub(crate) enum TuiFailure {
    Admission(TerminalAdmissionError),
    Runtime(io::Error),
}

impl fmt::Display for TuiFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(formatter, "terminal admission failed: {error}"),
            Self::Runtime(error) => write!(formatter, "TUI runtime failed: {error}"),
        }
    }
}

impl std::error::Error for TuiFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoopExit {
    Signal,
    User,
}

#[derive(Debug)]
enum LoopFailure<E> {
    Input(io::Error),
    Terminal(E),
    Viewport(io::Error),
}

impl LoopFailure<io::Error> {
    fn into_io_error(self) -> io::Error {
        match self {
            Self::Input(error) | Self::Terminal(error) | Self::Viewport(error) => error,
        }
    }
}

trait InputSource {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
}

trait ViewportSource {
    fn window_dimensions(&self) -> io::Result<WindowDimensions>;
}

struct InheritedInputSource {
    input: File,
}

impl InheritedInputSource {
    fn new(input: File) -> Self {
        Self { input }
    }
}

impl InputSource for InheritedInputSource {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let timeout = PollTimeout::try_from(timeout).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("input poll timeout is outside the supported range: {error}"),
            )
        })?;
        let mut descriptors = [PollFd::new(self.input.as_fd(), PollFlags::POLLIN)];
        match poll(&mut descriptors, timeout) {
            Ok(0) | Err(Errno::EINTR) => return Ok(None),
            Ok(_) => {}
            Err(error) => return Err(io::Error::from(error)),
        }
        let flags = match descriptors.first().and_then(PollFd::revents) {
            Some(flags) => flags,
            None => {
                return Err(io::Error::other(
                    "input poll returned unknown readiness flags",
                ));
            }
        };
        if !flags.contains(PollFlags::POLLIN) {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the inherited terminal input closed",
            ));
        }

        let mut buffer = [0_u8; MAXIMUM_INPUT_BYTES_PER_POLL];
        let count = self.input.read(&mut buffer)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the inherited terminal input reached end of file",
            ));
        }
        let input = buffer.get(..count).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "the inherited terminal returned an invalid input length",
            )
        })?;
        Ok(Some(input.to_vec()))
    }
}

impl ViewportSource for InheritedTerminal {
    fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        self.window_dimensions()
    }
}

pub(crate) fn run(locale: Locale) -> Result<(), TuiFailure> {
    let inherited = InheritedTerminal::admit().map_err(TuiFailure::Admission)?;
    let dimensions = inherited.window_dimensions().map_err(TuiFailure::Runtime)?;
    let termination = TerminationSignal::install().map_err(TuiFailure::Runtime)?;
    let mut modes = TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone()))
        .map_err(TuiFailure::Runtime)?;
    let output = File::from(inherited.duplicate_output().map_err(TuiFailure::Runtime)?);
    let backend = FixedViewportBackend::new(output, dimensions);
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .map_err(TuiFailure::Runtime)?;
    let input = File::from(inherited.duplicate_input().map_err(TuiFailure::Runtime)?);
    let mut inputs = InheritedInputSource::new(input);
    let mut terminal_input = UnavailableTerminalInputPort;

    let run_result = run_event_loop(
        &mut terminal,
        locale,
        &mut inputs,
        &mut terminal_input,
        &termination,
        &inherited,
        dimensions,
    )
    .map_err(LoopFailure::into_io_error);
    drop(terminal);
    let restore_result = modes.restore().map_err(io::Error::other);

    match (run_result, restore_result) {
        (Err(run_error), Err(restore_error)) => Err(TuiFailure::Runtime(io::Error::new(
            run_error.kind(),
            format!("{run_error}; terminal cleanup also failed: {restore_error}"),
        ))),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(TuiFailure::Runtime(error)),
        (Ok(LoopExit::Signal | LoopExit::User), Ok(())) => Ok(()),
    }
}

fn run_event_loop<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    locale: Locale,
    inputs: &mut impl InputSource,
    terminal_input: &mut impl crate::terminal_input::TerminalInputPort,
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    mut dimensions: WindowDimensions,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    let mut view = ClientView::new(dimensions.columns);
    terminal
        .draw(|frame| render_shell(frame, &view, locale))
        .map_err(LoopFailure::Terminal)?;

    loop {
        if termination.requested() {
            return Ok(LoopExit::Signal);
        }
        let input = inputs
            .next_input(Duration::from_millis(u64::from(
                EVENT_POLL_INTERVAL_MILLISECONDS,
            )))
            .map_err(LoopFailure::Input)?;
        let mut redraw = false;
        if let Some(input) = input {
            if dispatch_input(&mut view, &input, terminal_input) == DispatchOutcome::Exit {
                return Ok(LoopExit::User);
            }
            redraw = true;
        }
        if termination.requested() {
            return Ok(LoopExit::Signal);
        }

        let next_dimensions = viewport
            .window_dimensions()
            .map_err(LoopFailure::Viewport)?;
        if next_dimensions != dimensions {
            terminal
                .backend_mut()
                .set_window_dimensions(next_dimensions);
            terminal
                .resize(to_area(next_dimensions))
                .map_err(LoopFailure::Terminal)?;
            view.resize(next_dimensions.columns);
            dimensions = next_dimensions;
            redraw = true;
        }
        if redraw {
            terminal
                .draw(|frame| render_shell(frame, &view, locale))
                .map_err(LoopFailure::Terminal)?;
        }
    }
}

const fn to_area(dimensions: WindowDimensions) -> Rect {
    Rect::new(0, 0, dimensions.columns, dimensions.rows)
}

#[cfg(test)]
mod tests;
