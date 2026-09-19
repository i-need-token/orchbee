use std::{
    fmt,
    fs::File,
    io::{self, BufWriter, Read},
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
    terminal_input::{DispatchOutcome, InputDispatcher, ModeRequest, UnavailableTerminalInputPort},
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    termination_signal::{TerminationSignal, TerminationSource},
    tui_backend::{FixedViewportBackend, ResizableBackend, RestorationGatedWriter},
    tui_pointer::HitMapBuilder,
    tui_render::render_terminal_shell,
};

const EVENT_POLL_INTERVAL_MILLISECONDS: u16 = 50;
const MAXIMUM_INPUT_BYTES_PER_POLL: usize = 4096;
const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;

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
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>>;
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
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        self.observe_window_dimensions()
    }
}

pub(crate) fn run(locale: Locale, mouse: bool) -> Result<(), TuiFailure> {
    run_with_client(locale, None, mouse)
}

#[cfg(test)]
pub(crate) fn run_connected(
    locale: Locale,
    client: crate::local_client::LocalClient,
    intent: crate::terminal_session::TerminalIntent,
    mouse: bool,
) -> Result<(), TuiFailure> {
    run_with_client(locale, Some((client, intent)), mouse)
}

fn run_with_client(
    locale: Locale,
    connection: Option<(
        crate::local_client::LocalClient,
        crate::terminal_session::TerminalIntent,
    )>,
    mouse: bool,
) -> Result<(), TuiFailure> {
    let inherited = InheritedTerminal::admit().map_err(TuiFailure::Admission)?;
    let dimensions = inherited.window_dimensions().map_err(TuiFailure::Runtime)?;
    let terminal_input: Box<dyn TerminalSurface> = match connection {
        Some((client, intent)) => {
            let geometry =
                terminal_geometry(dimensions, &ClientView::new(dimensions.columns), None)
                    .and_then(|geometry| {
                        geometry.ok_or_else(|| {
                            io::Error::other("viewport too small to present a terminal pane")
                        })
                    })
                    .map_err(TuiFailure::Runtime)?;
            Box::new(
                crate::terminal_session::TerminalSession::start(client, intent, geometry)
                    .map_err(TuiFailure::Runtime)?,
            )
        }
        None => Box::new(UnavailableTerminalInputPort),
    };
    run_prepared_surface(locale, inherited, dimensions, terminal_input, mouse)
}

#[cfg(target_os = "linux")]
pub(crate) fn run_prepared(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    session: crate::terminal_session::TerminalSession,
    mouse: bool,
) -> Result<(), TuiFailure> {
    run_prepared_surface(locale, inherited, dimensions, Box::new(session), mouse)
}

fn run_prepared_surface(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    mut terminal_input: Box<dyn TerminalSurface>,
    mouse: bool,
) -> Result<(), TuiFailure> {
    let termination = TerminationSignal::install().map_err(TuiFailure::Runtime)?;
    let mut modes = TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone()))
        .map_err(TuiFailure::Runtime)?;
    let synchronized_update = modes.synchronized_update_lease();
    let output = BufWriter::with_capacity(
        OUTPUT_BUFFER_CAPACITY,
        RestorationGatedWriter::new(
            File::from(inherited.duplicate_output().map_err(TuiFailure::Runtime)?),
            synchronized_update,
        ),
    );
    let backend = FixedViewportBackend::new(output, dimensions, modes.synchronized_update_lease());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .map_err(TuiFailure::Runtime)?;
    let input = File::from(inherited.duplicate_input().map_err(TuiFailure::Runtime)?);
    let mut inputs = InheritedInputSource::new(input);
    let theme = crate::tui_theme::Theme::terminal_native();
    // Pointer reporting is armed (or left released) before the loop so the
    // first drawn frame already agrees with the dispatcher's settled flag.
    modes
        .set_pointer_capture(mouse)
        .map_err(TuiFailure::Runtime)?;

    let run_result = run_event_loop_with_modes(
        &mut terminal,
        Chrome {
            locale,
            theme: &theme,
        },
        (&mut inputs, &mut |request| match request {
            ModeRequest::BracketedPaste(enabled) => modes.set_bracketed_paste(enabled),
            ModeRequest::PointerCapture(enabled) => modes.set_pointer_capture(enabled),
        }),
        terminal_input.as_mut(),
        &termination,
        &inherited,
        dimensions,
        mouse,
    )
    .map_err(LoopFailure::into_io_error);
    if run_result.is_err() {
        // BufWriter's drop may retry buffered bytes. Fence incomplete frame
        // output first; mode restoration still sends its own control bytes.
        modes.discard_frame_output();
    }
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

/// Presentation inputs that travel together through the event loop: the
/// localized copy and the resolved theme.
#[derive(Clone, Copy)]
struct Chrome<'a> {
    locale: Locale,
    theme: &'a crate::tui_theme::Theme,
}

#[cfg(test)]
#[expect(clippy::too_many_arguments)]
fn run_event_loop<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    chrome: Chrome<'_>,
    inputs: &mut impl InputSource,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    dimensions: WindowDimensions,
    mouse: bool,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    run_event_loop_with_modes(
        terminal,
        chrome,
        (inputs, &mut |_request| Ok(())),
        terminal_input,
        termination,
        viewport,
        dimensions,
        mouse,
    )
}

#[expect(clippy::too_many_arguments)]
fn run_event_loop_with_modes<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    chrome: Chrome<'_>,
    input_runtime: (
        &mut impl InputSource,
        &mut impl FnMut(ModeRequest) -> io::Result<()>,
    ),
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    mut dimensions: WindowDimensions,
    mouse: bool,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    let (inputs, mode) = input_runtime;
    let mut view = ClientView::new(dimensions.columns);
    let mut dispatcher = InputDispatcher::default();
    // The lifecycle write already armed or released host reporting; the
    // dispatcher and the status indicator settle to the same truth here.
    dispatcher.set_pointer_capture(&mut view, mouse);
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
    draw_surface(terminal, &mut view, chrome, terminal_input, &mut dispatcher)?;
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
        if redraw {
            // Resize discards the recorded regions: until the next
            // successful draw publishes a fresh map, pointer events drop
            // instead of resolving against stale geometry.
            dispatcher.invalidate_hit_map();
        }
        redraw |= terminal_input.tick();
        let now = inputs.sample_time();
        redraw |= dispatcher.synchronize(&mut view, terminal_input, now, &mut |request| {
            mode(request).map_err(LoopFailure::Input)
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
                &mut |request| mode(request).map_err(LoopFailure::Input),
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
            draw_surface(terminal, &mut view, chrome, terminal_input, &mut dispatcher)?;
        }
    }
}

fn draw_surface<B: Backend>(
    terminal: &mut Terminal<B>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    surface: &mut (impl TerminalSurface + ?Sized),
    dispatcher: &mut InputDispatcher,
) -> Result<(), LoopFailure<B::Error>> {
    let mut receipt = crate::terminal_session::NativeDrawReceipt::default();
    let mut hits = HitMapBuilder::new();
    let drawn = terminal.draw(|frame| {
        receipt = render_terminal_shell(
            frame,
            view,
            chrome.locale,
            surface.view(),
            chrome.theme,
            &mut hits,
        );
    });
    if let Err(error) = drawn {
        // A failed draw never publishes: the previous map stays the
        // authority for the last frame that reached the screen whole.
        // The failure itself is fatal to the loop.
        return Err(LoopFailure::Terminal(error));
    }
    dispatcher.publish_hit_map(hits.finish());
    let help_page = receipt.help_page();
    surface.after_close_draw(receipt);
    if let Some(page) = help_page {
        view.settle_help_page(page);
    }
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
    let Some(next) = viewport
        .window_dimensions()
        .map_err(LoopFailure::Viewport)?
    else {
        return Ok(false);
    };
    let changed = next != *dimensions;
    if changed {
        terminal_input.invalidate_close_display();
        terminal_input.invalidate_rename_display();
        terminal_input.invalidate_setup_display();
        terminal.backend_mut().set_window_dimensions(next);
        terminal
            .resize(to_area(next))
            .map_err(LoopFailure::Terminal)?;
        view.resize(next.columns);
        *dimensions = next;
    }
    let session_view = terminal_input.view();
    match session_view {
        Some(crate::terminal_session::TerminalSessionView::Native(native)) => {
            let panes =
                pane_geometries(*dimensions, view, &native).map_err(LoopFailure::Viewport)?;
            terminal_input.observe_panes(&panes);
        }
        _ => {
            let geometry = terminal_geometry(*dimensions, view, session_view)
                .map_err(LoopFailure::Viewport)?;
            if let Some(geometry) = geometry {
                terminal_input.observe_geometry(geometry);
            }
        }
    }
    Ok(changed)
}

/// The session's real pane geometry, computed from the same pure layout the
/// renderer paints. `Ok(None)` means the viewport cannot present a pane right
/// now (too small, or the active leaf is empty): the caller keeps the last
/// valid geometry instead of resizing — the session is never told a
/// fabricated size. `Err` means `Geometry` validation rejected the built
/// value (for example an out-of-range or oversized geometry); a zero cell
/// pixel pair is legal "pixels unknown" and stays `(0, 0)` — never repaired
/// with fake cells.
pub(crate) fn terminal_geometry(
    dimensions: WindowDimensions,
    view: &ClientView,
    session: Option<crate::terminal_session::TerminalSessionView<'_>>,
) -> io::Result<Option<orchbee_client_server_protocol::Geometry>> {
    let areas = crate::tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface());
    let Some(area) = active_pane_area(&areas, session) else {
        return Ok(None);
    };
    let (width, height) = match (
        dimensions
            .pixel_width
            .checked_div(dimensions.columns)
            .unwrap_or(0),
        dimensions
            .pixel_height
            .checked_div(dimensions.rows)
            .unwrap_or(0),
    ) {
        (0, _) | (_, 0) => (0, 0),
        cell_pixels => cell_pixels,
    };
    orchbee_client_server_protocol::Geometry::new(area.width, area.height, width, height)
        .map(Some)
        .map_err(io::Error::other)
}

/// The cell rect the live terminal attaches to, identical to what
/// [`render_terminal_shell`] paints: the whole panel content for a single
/// pane or a Minimal viewport (active pane only, no tree chrome), otherwise
/// the active leaf's content inside the projected split tree. `None` when the
/// viewport cannot present a pane at all — explicit, not inferred from a
/// returned rect.
pub(crate) fn active_pane_area(
    areas: &crate::tui_layout::ShellAreas,
    session: Option<crate::terminal_session::TerminalSessionView<'_>>,
) -> Option<Rect> {
    use crate::terminal_session::TerminalSessionView;
    if areas.tier == crate::tui_layout::ViewportTier::TooSmall {
        return None;
    }
    let framed = areas.tier.shows_panel_frame();
    let content = crate::tui_layout::terminal_area(areas.panel, framed);
    let native = match session {
        Some(TerminalSessionView::Native(native)) => native,
        #[cfg(test)]
        Some(TerminalSessionView::Legacy { .. }) => {
            return (!content.is_empty()).then_some(content);
        }
        None => return (!content.is_empty()).then_some(content),
    };
    let tab = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))?;
    let area = if framed && tab.panel_count() > 1 {
        crate::tui_layout::project_panel_tree(content, tab.panel_tree(), tab.active_panel())
            .active_content(tab.active_panel())
    } else {
        Some(content)
    }?;
    (!area.is_empty()).then_some(area)
}

/// Every visible leaf's `(panel_id, panel_index, geometry)` for the native
/// session — the membership observation that gives each pane its own viewer.
/// The paned path consumes the same `project_panel_tree` leaves the renderer
/// paints; the single-pane path observes the focus pane on the whole content
/// rect, matching what the renderer draws there.
pub(crate) fn pane_geometries(
    dimensions: WindowDimensions,
    view: &ClientView,
    native: &crate::terminal_session::NativeSessionView<'_>,
) -> io::Result<
    Vec<(
        orchbee_workbench_protocol::WorkbenchPanelId,
        usize,
        orchbee_client_server_protocol::Geometry,
    )>,
> {
    use crate::tui_layout;
    let areas = tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface());
    if areas.tier == tui_layout::ViewportTier::TooSmall {
        return Ok(Vec::new());
    }
    let framed = areas.tier.shows_panel_frame();
    let content = tui_layout::terminal_area(areas.panel, framed);
    let Some(tab) = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))
    else {
        return Ok(Vec::new());
    };
    let (cell_width, cell_height) = match (
        dimensions
            .pixel_width
            .checked_div(dimensions.columns)
            .unwrap_or(0),
        dimensions
            .pixel_height
            .checked_div(dimensions.rows)
            .unwrap_or(0),
    ) {
        (0, _) | (_, 0) => (0, 0),
        cell_pixels => cell_pixels,
    };
    let leaf_geometry = |rect: Rect| {
        orchbee_client_server_protocol::Geometry::new(
            rect.width,
            rect.height,
            cell_width,
            cell_height,
        )
        .map_err(io::Error::other)
    };
    if framed && tab.panel_count() > 1 {
        let projection =
            tui_layout::project_panel_tree(content, tab.panel_tree(), tab.active_panel());
        let mut panes = Vec::with_capacity(projection.leaves.len());
        for leaf in &projection.leaves {
            let Some(panel) = tab.panel(leaf.panel_index) else {
                continue;
            };
            panes.push((
                panel.panel_id().clone(),
                leaf.panel_index,
                leaf_geometry(leaf.content)?,
            ));
        }
        return Ok(panes);
    }
    let Some(panel_id) = native.focus_panel_id() else {
        return Ok(Vec::new());
    };
    let index = tab.active_panel().unwrap_or(0);
    Ok(vec![(panel_id.clone(), index, leaf_geometry(content)?)])
}

trait TerminalSurface: crate::terminal_input::TerminalInputPort {
    fn tick(&mut self) -> bool {
        false
    }
    fn after_draw(&mut self) {}
    fn after_close_draw(&mut self, _receipt: crate::terminal_session::NativeDrawReceipt) {}
    fn observe_panes(
        &mut self,
        _panes: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            orchbee_client_server_protocol::Geometry,
        )],
    ) {
    }
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
    fn observe_panes(
        &mut self,
        panes: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            orchbee_client_server_protocol::Geometry,
        )],
    ) {
        self.observe_panes(panes)
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
