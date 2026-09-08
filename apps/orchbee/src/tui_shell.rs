use std::{
    fmt,
    fs::File,
    io::{self, Read},
    os::fd::AsFd,
    time::{Duration, Instant},
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
    terminal_input::{DispatchOutcome, InputDispatcher, UnavailableTerminalInputPort},
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    termination_signal::{TerminationSignal, TerminationSource},
    tui_backend::{FixedViewportBackend, ResizableBackend},
    tui_render::render_terminal_shell,
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

    fn sample_time(&self) -> Instant {
        Instant::now()
    }
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
    run_with_client(locale, None)
}

pub(crate) fn run_connected(
    locale: Locale,
    client: crate::local_client::LocalClient,
    intent: crate::terminal_session::TerminalIntent,
) -> Result<(), TuiFailure> {
    run_with_client(locale, Some((client, intent)))
}

fn run_with_client(
    locale: Locale,
    connection: Option<(
        crate::local_client::LocalClient,
        crate::terminal_session::TerminalIntent,
    )>,
) -> Result<(), TuiFailure> {
    let inherited = InheritedTerminal::admit().map_err(TuiFailure::Admission)?;
    let dimensions = inherited.window_dimensions().map_err(TuiFailure::Runtime)?;
    let terminal_input: Box<dyn TerminalSurface> = match connection {
        Some((client, intent)) => {
            let geometry = terminal_geometry(dimensions, &ClientView::new(dimensions.columns))
                .map_err(TuiFailure::Runtime)?;
            Box::new(
                crate::terminal_session::TerminalSession::start(client, intent, geometry)
                    .map_err(TuiFailure::Runtime)?,
            )
        }
        None => Box::new(UnavailableTerminalInputPort),
    };
    run_prepared_surface(locale, inherited, dimensions, terminal_input)
}

#[cfg(target_os = "linux")]
pub(crate) fn run_prepared(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    session: crate::terminal_session::TerminalSession,
) -> Result<(), TuiFailure> {
    run_prepared_surface(locale, inherited, dimensions, Box::new(session))
}

fn run_prepared_surface(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    mut terminal_input: Box<dyn TerminalSurface>,
) -> Result<(), TuiFailure> {
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

    let run_result = run_event_loop_with_modes(
        &mut terminal,
        locale,
        (&mut inputs, &mut |enabled| {
            modes.set_bracketed_paste(enabled)
        }),
        terminal_input.as_mut(),
        &termination,
        &inherited,
        dimensions,
    )
    .map_err(LoopFailure::into_io_error);
    drop(terminal);
    drop(terminal_input);
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

#[cfg(test)]
fn run_event_loop<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    locale: Locale,
    inputs: &mut impl InputSource,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    dimensions: WindowDimensions,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    run_event_loop_with_modes(
        terminal,
        locale,
        (inputs, &mut |_enabled| Ok(())),
        terminal_input,
        termination,
        viewport,
        dimensions,
    )
}

fn run_event_loop_with_modes<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    locale: Locale,
    input_runtime: (
        &mut impl InputSource,
        &mut impl FnMut(bool) -> io::Result<()>,
    ),
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    mut dimensions: WindowDimensions,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    let (inputs, mode) = input_runtime;
    let mut view = ClientView::new(dimensions.columns);
    let mut dispatcher = InputDispatcher::default();
    if termination.requested() {
        return Ok(LoopExit::Signal);
    }
    synchronize_viewport(
        terminal,
        viewport,
        &mut view,
        terminal_input,
        &mut dimensions,
    )?;
    let _changed = terminal_input.tick();
    draw_surface(terminal, &view, locale, terminal_input)?;
    loop {
        if termination.requested() {
            return Ok(LoopExit::Signal);
        }
        let input = inputs
            .next_input(Duration::from_millis(u64::from(
                EVENT_POLL_INTERVAL_MILLISECONDS,
            )))
            .map_err(LoopFailure::Input)?;
        let mut redraw = synchronize_viewport(
            terminal,
            viewport,
            &mut view,
            terminal_input,
            &mut dimensions,
        )?;
        redraw |= terminal_input.tick();
        let now = inputs.sample_time();
        redraw |= dispatcher.synchronize(&mut view, terminal_input, now, &mut |enabled| {
            mode(enabled).map_err(LoopFailure::Input)
        })?;
        if let Some(input) = input {
            let outcome = dispatcher.dispatch(
                &mut view,
                &input,
                terminal_input,
                now,
                |view, terminal_input| {
                    synchronize_viewport(terminal, viewport, view, terminal_input, &mut dimensions)
                        .map(|_changed| ())
                },
                &mut |enabled| mode(enabled).map_err(LoopFailure::Input),
            )?;
            if outcome == DispatchOutcome::Exit {
                return Ok(LoopExit::User);
            }
            redraw = true;
        }
        if termination.requested() {
            return Ok(LoopExit::Signal);
        }

        if redraw {
            draw_surface(terminal, &view, locale, terminal_input)?;
        }
    }
}

fn draw_surface<B: Backend>(
    terminal: &mut Terminal<B>,
    view: &ClientView,
    locale: Locale,
    surface: &mut (impl TerminalSurface + ?Sized),
) -> Result<(), LoopFailure<B::Error>> {
    let mut receipt = crate::terminal_session::NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = render_terminal_shell(frame, view, locale, surface.view());
        })
        .map_err(LoopFailure::Terminal)?;
    surface.after_close_draw(receipt);
    surface.after_draw();
    Ok(())
}

fn synchronize_viewport<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    viewport: &impl ViewportSource,
    view: &mut ClientView,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    dimensions: &mut WindowDimensions,
) -> Result<bool, LoopFailure<B::Error>> {
    let next = viewport
        .window_dimensions()
        .map_err(LoopFailure::Viewport)?;
    let changed = next != *dimensions;
    if changed {
        terminal_input.invalidate_close_display();
        terminal_input.invalidate_rename_display();
        terminal.backend_mut().set_window_dimensions(next);
        terminal
            .resize(to_area(next))
            .map_err(LoopFailure::Terminal)?;
        view.resize(next.columns);
        *dimensions = next;
    }
    let geometry = terminal_geometry(*dimensions, view).map_err(LoopFailure::Viewport)?;
    terminal_input.observe_geometry(geometry);
    Ok(changed)
}

pub(crate) fn terminal_geometry(
    dimensions: WindowDimensions,
    view: &ClientView,
) -> io::Result<orchbee_client_server_protocol::Geometry> {
    let panel =
        crate::tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface()).panel;
    let area = crate::tui_layout::terminal_area(panel);
    let width = dimensions
        .pixel_width
        .checked_div(dimensions.columns)
        .unwrap_or(0);
    let height = dimensions
        .pixel_height
        .checked_div(dimensions.rows)
        .unwrap_or(0);
    orchbee_client_server_protocol::Geometry::new(area.width, area.height, width, height)
        .map_err(io::Error::other)
}

trait TerminalSurface: crate::terminal_input::TerminalInputPort {
    fn tick(&mut self) -> bool {
        false
    }
    fn after_draw(&mut self) {}
    fn after_close_draw(&mut self, _receipt: crate::terminal_session::NativeDrawReceipt) {}
    fn view(&self) -> Option<crate::terminal_session::TerminalSessionView<'_>> {
        None
    }
}
impl TerminalSurface for UnavailableTerminalInputPort {}
impl TerminalSurface for crate::terminal_session::TerminalSession {
    fn tick(&mut self) -> bool {
        self.tick()
    }
    fn after_draw(&mut self) {
        self.after_draw()
    }
    fn after_close_draw(&mut self, receipt: crate::terminal_session::NativeDrawReceipt) {
        self.after_close_draw(receipt)
    }
    fn view(&self) -> Option<crate::terminal_session::TerminalSessionView<'_>> {
        Some(self.view())
    }
}

const fn to_area(dimensions: WindowDimensions) -> Rect {
    Rect::new(0, 0, dimensions.columns, dimensions.rows)
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod connected_tests;
