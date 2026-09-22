#[cfg(unix)]
use std::io::Read;
use std::{
    fs::File,
    io::{self, Write},
    time::{Duration, Instant},
};

use crossterm::{
    ExecutableCommand,
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};
#[cfg(unix)]
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use ratatui::layout::Rect;
#[cfg(unix)]
use std::os::fd::AsFd;

use crate::{
    client_view::ClientView,
    terminal_device::{InheritedTerminal, WindowDimensions},
    terminal_input::{DispatchOutcome, InputDispatcher, UnavailableTerminalInputPort},
    terminal_lifecycle::{TerminalModeGuard, TerminalModes},
    termination_signal::TerminationSource,
    tui_layout,
};

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(unix)]
const WAIT_INPUT_BUFFER_BYTES: usize = 4096;

/// The bounded AwaitingGeometry outcome. `Available` carries the dimensions
/// that satisfied the presentability predicate so the original startup path
/// opens the session against the same report it judged drawable.
pub(crate) enum StartupWait {
    Available(WindowDimensions),
    Cancelled,
    TimedOut,
    Fault(io::Error),
}

/// Localized copy drawn once when the wait owns the alternate screen.
pub(crate) struct WaitPrompt<'a> {
    pub(crate) notice: &'a str,
    pub(crate) exit_hint: &'a str,
}

/// The injected geometry sampler; the production adapter reads the inherited
/// terminal's reported cell dimensions.
pub(crate) trait WaitViewport {
    fn window_dimensions(&mut self) -> io::Result<WindowDimensions>;
}

/// The injected input source. `Ok(None)` is a poll timeout, `Ok(Some)` is one
/// consumed input packet, and `Err` is a real read fault or end of input —
/// the same classification the running event loop applies.
pub(crate) trait WaitInput {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
}

/// The presentability decision for one dimension report: `Some` is a real
/// drawable content rect, `None` means the screen cannot present it now.
/// Never synthesize a nonzero rect — a `None` verdict only pauses startup.
///
/// The standalone intent owns no panel tree yet, so the bound verdict is
/// the shared layout owner's [`tui_shell::active_pane_area`] with no
/// session — the same projection `terminal_geometry` consults for the
/// first Open, so wait-gating and wire geometry cannot diverge.
pub(crate) fn standalone_presentable_area(dimensions: WindowDimensions) -> Option<Rect> {
    let view = ClientView::new(dimensions.columns);
    let areas = tui_layout::project_shell_areas(
        Rect::new(0, 0, dimensions.columns, dimensions.rows),
        view.sidebar_surface(),
    );
    crate::tui_shell::active_pane_area(&areas, None)
}

/// The mode-lease surface the wait needs. Production enters the shared
/// [`TerminalModeGuard`], keeping mode ownership and panic restoration with
/// the lifecycle owner; tests inject a scripted guard through `enter`.
pub(crate) trait WaitModeGuard {
    fn set_bracketed_paste(&mut self, enabled: bool) -> io::Result<()>;
    fn set_pointer_capture(&mut self, enabled: bool) -> io::Result<()>;
    fn restore(&mut self) -> io::Result<()>;
}

impl<M: TerminalModes> WaitModeGuard for TerminalModeGuard<M> {
    fn set_bracketed_paste(&mut self, enabled: bool) -> io::Result<()> {
        TerminalModeGuard::set_bracketed_paste(self, enabled)
    }
    fn set_pointer_capture(&mut self, enabled: bool) -> io::Result<()> {
        TerminalModeGuard::set_pointer_capture(self, enabled)
    }
    fn restore(&mut self) -> io::Result<()> {
        TerminalModeGuard::restore(self).map_err(io::Error::other)
    }
}

/// Every dependency the wait borrows, injected so tests can script geometry,
/// time, input, output, termination, and presentability deterministically.
pub(crate) struct StartupWaitDeps<'a, V, I, W, T, P, C> {
    pub(crate) viewport: &'a mut V,
    pub(crate) input: &'a mut I,
    pub(crate) output: &'a mut W,
    pub(crate) termination: &'a T,
    pub(crate) clock: &'a C,
    pub(crate) deadline: Instant,
    pub(crate) prompt: WaitPrompt<'a>,
    pub(crate) presentable: &'a mut P,
}

/// Wait for a drawable geometry inside `deps.deadline`. The first sample that
/// is already drawable returns without entering terminal modes, so the
/// existing startup path gains no flicker or extra queries. Only a
/// non-drawable report enters the alternate screen, draws the localized
/// prompt, and polls input and geometry until one of the four outcomes. The
/// deadline is never reset: a resize landing at or after the deadline still
/// loses to `TimedOut`.
///
/// Waiting consumes input through the shared dispatcher into an unavailable
/// port, so prefix/exit bindings stay authoritative while bytes are dropped
/// instead of being replayed into the session created later.
pub(crate) fn await_presentable_geometry<G, E, V, I, W, T, P, C>(
    enter: E,
    deps: StartupWaitDeps<'_, V, I, W, T, P, C>,
) -> StartupWait
where
    G: WaitModeGuard,
    E: FnOnce() -> io::Result<G>,
    V: WaitViewport,
    I: WaitInput,
    W: Write,
    T: TerminationSource,
    P: FnMut(WindowDimensions) -> Option<Rect>,
    C: Fn() -> Instant,
{
    match sample(deps.viewport, deps.presentable) {
        Sample::Available(dimensions) => return StartupWait::Available(dimensions),
        Sample::Waiting => {}
        Sample::Fault(error) => return StartupWait::Fault(error),
    }
    if (deps.clock)() >= deps.deadline {
        return StartupWait::TimedOut;
    }
    let mut guard = match enter() {
        Ok(guard) => guard,
        Err(error) => return StartupWait::Fault(error),
    };
    if let Err(error) = draw_prompt(deps.output, deps.prompt) {
        return finish(&mut guard, StartupWait::Fault(error));
    }
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(0);
    let mut port = UnavailableTerminalInputPort;
    loop {
        if (deps.clock)() >= deps.deadline {
            return finish(&mut guard, StartupWait::TimedOut);
        }
        if deps.termination.requested() {
            return finish(&mut guard, StartupWait::Cancelled);
        }
        let remaining = deps.deadline.saturating_duration_since((deps.clock)());
        match deps.input.next_input(remaining.min(WAIT_POLL_INTERVAL)) {
            Err(error) => return finish(&mut guard, StartupWait::Fault(error)),
            Ok(Some(bytes)) => {
                if (deps.clock)() >= deps.deadline {
                    return finish(&mut guard, StartupWait::TimedOut);
                }
                match dispatcher.dispatch(
                    &mut view,
                    &bytes,
                    &mut port,
                    (deps.clock)(),
                    |_, _| Ok(()),
                    &mut |request| match request {
                        crate::terminal_input::ModeRequest::BracketedPaste(enabled) => {
                            guard.set_bracketed_paste(enabled)
                        }
                        crate::terminal_input::ModeRequest::PointerCapture(enabled) => {
                            guard.set_pointer_capture(enabled)
                        }
                    },
                ) {
                    Err(error) => return finish(&mut guard, StartupWait::Fault(error)),
                    Ok(DispatchOutcome::Exit) => {
                        return finish(&mut guard, StartupWait::Cancelled);
                    }
                    Ok(DispatchOutcome::Continue) => {}
                }
            }
            Ok(None) => {}
        }
        if (deps.clock)() >= deps.deadline {
            return finish(&mut guard, StartupWait::TimedOut);
        }
        match sample(deps.viewport, deps.presentable) {
            Sample::Available(dimensions) => {
                return finish(&mut guard, StartupWait::Available(dimensions));
            }
            Sample::Waiting => {}
            Sample::Fault(error) => return finish(&mut guard, StartupWait::Fault(error)),
        }
    }
}

enum Sample {
    Available(WindowDimensions),
    Waiting,
    Fault(io::Error),
}

fn sample<V: WaitViewport>(
    viewport: &mut V,
    presentable: &mut impl FnMut(WindowDimensions) -> Option<Rect>,
) -> Sample {
    match viewport.window_dimensions() {
        Ok(dimensions) if presentable(dimensions).is_some() => Sample::Available(dimensions),
        Ok(_) => Sample::Waiting,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => Sample::Waiting,
        Err(error) => Sample::Fault(error),
    }
}

fn draw_prompt(output: &mut impl Write, prompt: WaitPrompt<'_>) -> io::Result<()> {
    output
        .execute(Clear(ClearType::All))?
        .execute(MoveTo(0, 0))?;
    output.write_all(prompt.notice.as_bytes())?;
    output.write_all(prompt.exit_hint.as_bytes())?;
    // The session renderer terminates every complete frame with a double
    // reset; the waiting prompt is a whole frame too, so it closes the same
    // way instead of leaving an open attribute state on the screen.
    output.write_all(b"\x1b[0m\x1b[0m")?;
    output.flush()
}

fn finish<G: WaitModeGuard>(guard: &mut G, outcome: StartupWait) -> StartupWait {
    match guard.restore() {
        Ok(()) => outcome,
        Err(error) => StartupWait::Fault(error),
    }
}

/// The production sampler: reads the inherited terminal's reported cell
/// dimensions on the same descriptor admission already validated.
pub(crate) struct InheritedWaitViewport<'a> {
    terminal: &'a InheritedTerminal,
}

impl<'a> InheritedWaitViewport<'a> {
    pub(crate) const fn new(terminal: &'a InheritedTerminal) -> Self {
        Self { terminal }
    }
}

impl WaitViewport for InheritedWaitViewport<'_> {
    fn window_dimensions(&mut self) -> io::Result<WindowDimensions> {
        self.terminal.window_dimensions()
    }
}

/// The production input source: polls and reads a duplicate of the inherited
/// terminal input descriptor — never a fresh `/dev/tty`.
pub(crate) struct InheritedWaitInput {
    input: File,
}

impl InheritedWaitInput {
    pub(crate) fn open(terminal: &InheritedTerminal) -> io::Result<Self> {
        Ok(Self {
            input: File::from(terminal.duplicate_input()?),
        })
    }
}

impl WaitInput for InheritedWaitInput {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        #[cfg(windows)]
        {
            let _ = &self.input;
            crate::windows_console_input::next_input(timeout)
        }
        #[cfg(unix)]
        {
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
            let mut buffer = [0_u8; WAIT_INPUT_BUFFER_BYTES];
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
}

#[cfg(test)]
mod tests;
