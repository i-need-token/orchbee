//! Owns the inherited-terminal TUI event loop, surface switching, bounded input polling,
//! drawing, and transfer of mode or draw receipts to feature owners. It does not own canonical
//! Workbench state or treat transport observations as painted output.

mod divider;
mod machines;
mod measurement;

use std::{
    fmt,
    io::{self, BufWriter, Write},
    time::{Duration, Instant},
};
#[cfg(unix)]
use std::{fs::File, io::Read};

use measurement::{MeasuredWriter, Measurements};
#[cfg(unix)]
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use ratatui::{Terminal, TerminalOptions, Viewport, backend::Backend, layout::Rect};
#[cfg(unix)]
use std::os::fd::AsFd;

use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_device::{InheritedTerminal, TerminalAdmissionError, WindowDimensions},
    terminal_input::{
        DispatchOutcome, InputDispatcher, ModeRequest, PointerCaptureRequest,
        UnavailableTerminalInputPort,
    },
    terminal_lifecycle::{InheritedTerminalModes, PointerTrackingLevel, TerminalModeGuard},
    termination_signal::{TerminationSignal, TerminationSource},
    tui_backend::{FixedViewportBackend, ResizableBackend, RestorationGatedWriter},
    tui_pointer::HitMapBuilder,
};

const EVENT_POLL_INTERVAL_MILLISECONDS: u16 = 50;
#[cfg(unix)]
const MAXIMUM_INPUT_BYTES_PER_POLL: usize = 4096;
const OUTPUT_BUFFER_CAPACITY: usize = 64 * 1024;

fn pointer_tracking_level(request: PointerCaptureRequest) -> Option<PointerTrackingLevel> {
    match request {
        PointerCaptureRequest::Release => None,
        PointerCaptureRequest::ButtonOnly => Some(PointerTrackingLevel::ButtonOnly),
        PointerCaptureRequest::AnyMotion => Some(PointerTrackingLevel::AnyMotion),
        PointerCaptureRequest::MenuMotion => Some(PointerTrackingLevel::MenuMotion),
    }
}

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
struct LoopCompletion {
    exit: LoopExit,
    locale: Locale,
    return_command: String,
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

    fn measurements(&self) -> Measurements {
        Measurements::default()
    }

    fn next_input_or_wakeup(
        &mut self,
        timeout: Duration,
        wakeup: &crate::workbench_client::UiWakeup,
    ) -> io::Result<Option<Vec<u8>>> {
        self.next_input(if wakeup.take() {
            Duration::ZERO
        } else {
            timeout
        })
    }

    fn sample_time(&self) -> Instant {
        Instant::now()
    }
}

trait ViewportSource {
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>>;
}

struct InheritedInputSource {
    measurements: Measurements,
    #[cfg(unix)]
    input: File,
    #[cfg(windows)]
    terminal: InheritedTerminal,
}

impl InheritedInputSource {
    fn open(terminal: &InheritedTerminal, measurements: Measurements) -> io::Result<Self> {
        Ok(Self {
            measurements,
            #[cfg(unix)]
            input: File::from(terminal.duplicate_input()?),
            #[cfg(windows)]
            terminal: terminal.clone(),
        })
    }
}

impl InputSource for InheritedInputSource {
    fn measurements(&self) -> Measurements {
        self.measurements.clone()
    }

    #[cfg(windows)]
    fn next_input_or_wakeup(
        &mut self,
        timeout: Duration,
        wakeup: &crate::workbench_client::UiWakeup,
    ) -> io::Result<Option<Vec<u8>>> {
        self.terminal.next_input_until(timeout, || wakeup.take())
    }

    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        #[cfg(windows)]
        {
            self.terminal.next_input(timeout)
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
}

impl ViewportSource for InheritedTerminal {
    fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        self.observe_window_dimensions()
    }
}

pub(crate) fn run(locale: Locale, mouse: bool) -> Result<(), TuiFailure> {
    run_with_client(locale, None, mouse)
}

#[cfg(all(test, unix))]
pub(crate) fn run_connected(
    locale: Locale,
    client: crate::local_client::LocalClient,
    mouse: bool,
) -> Result<(), TuiFailure> {
    run_with_client(locale, Some(client), mouse)
}

fn run_with_client(
    locale: Locale,
    connection: Option<crate::local_client::LocalClient>,
    mouse: bool,
) -> Result<(), TuiFailure> {
    let inherited = InheritedTerminal::admit().map_err(TuiFailure::Admission)?;
    let dimensions = inherited.window_dimensions().map_err(TuiFailure::Runtime)?;
    let server_configuration = connection.is_some();
    let terminal_input: Box<dyn TerminalSurface> = match connection {
        Some(client) => {
            let geometry =
                terminal_geometry(dimensions, &ClientView::new(dimensions.columns), None)
                    .and_then(|geometry| {
                        geometry.ok_or_else(|| {
                            io::Error::other("viewport too small to present a terminal pane")
                        })
                    })
                    .map_err(TuiFailure::Runtime)?;
            Box::new(
                crate::workbench_client::WorkbenchClient::start(client, geometry)
                    .map_err(TuiFailure::Runtime)?,
            )
        }
        None => Box::new(UnavailableTerminalInputPort),
    };
    run_prepared_surface(
        locale,
        inherited,
        dimensions,
        terminal_input,
        mouse,
        None,
        server_configuration,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) fn run_prepared(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    workbench_client: crate::workbench_client::WorkbenchClient,
    mouse: bool,
    notice: Option<crate::client_view::Notice>,
) -> Result<(), TuiFailure> {
    run_prepared_surface(
        locale,
        inherited,
        dimensions,
        Box::new(workbench_client),
        mouse,
        notice,
        true,
    )
}

fn run_prepared_surface(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    terminal_input: Box<dyn TerminalSurface>,
    mouse: bool,
    notice: Option<crate::client_view::Notice>,
    server_configuration: bool,
) -> Result<(), TuiFailure> {
    let measurement_recorder =
        measurement::Recorder::open(std::env::var_os("ORCHBEE_TUI_MEASUREMENTS"), locale)
            .map_err(TuiFailure::Runtime)?;
    let measurements = measurement_recorder.measurements();

    let mut terminal_input = terminal_input;
    let mut initial_view = ClientView::with_capabilities(
        dimensions.columns,
        crate::tui_capabilities::Capabilities::for_host(),
    );
    if let Some(notice) = notice {
        initial_view.startup_notice(notice);
    }
    {
        // A connected Client may paint one provisional local projection while the configuration
        // watcher obtains the Server's authoritative baseline. A later local fatal error cannot
        // invalidate an already admitted Session; Workbench startup still fails on that boundary.
        let host_colors = ["NO_COLOR", "FORCE_COLOR", "COLORTERM", "TERM", "WT_SESSION"]
            .map(|name| std::env::var(name).ok());
        initial_view.configure_no_color(
            host_colors[0]
                .as_ref()
                .is_some_and(|value| !value.is_empty()),
        );
        let multiplexed = ["TMUX", "ZELLIJ", "STY"]
            .iter()
            .any(|name| std::env::var_os(name).is_some());
        initial_view.configure_color_level(crate::tui_theme::resolve_color_level(
            host_colors[0].as_deref(),
            host_colors[1].as_deref(),
            host_colors[2].as_deref(),
            host_colors[3].as_deref(),
            host_colors[4].as_deref(),
            multiplexed,
        ));
    }
    {
        if crate::tui_capabilities::Capability::ConfigurationFile.available() {
            initial_view.onboarding = crate::onboarding::State::load();
            initial_view.configure_theme(orchbee_local_runtime::ThemeName::TerminalNative);
            let load = orchbee_local_runtime::read_user_settings(|name| std::env::var_os(name));
            if let Err(error) = &load
                && !server_configuration
            {
                return Err(TuiFailure::Runtime(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    error.to_string(),
                )));
            }
            if let Ok(load) = load {
                if !load.diagnostics.is_empty() {
                    initial_view.configuration_warning(true);
                }
                let settings = load.settings;
                initial_view.configure_theme(settings.theme);
                initial_view.configure_keys(&settings.keys);
                terminal_input.configure_panel_focus(settings.focus_new_panel);
                terminal_input.configure_agent_notifications(0, &settings.notifications);
                initial_view.notifications.configure(settings.notifications);
                initial_view
                    .machines
                    .configure_agent_sidebar(settings.agent_sidebar);
                initial_view
                    .machines
                    .configure_space_sidebar(settings.space_sidebar);
            }
        }
    }
    let termination = TerminationSignal::install().map_err(TuiFailure::Runtime)?;
    let mut modes = TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone()))
        .map_err(TuiFailure::Runtime)?;
    let synchronized_update = modes.synchronized_update_lease();
    let output = BufWriter::with_capacity(
        OUTPUT_BUFFER_CAPACITY,
        RestorationGatedWriter::new(
            MeasuredWriter::new(
                inherited.output().map_err(TuiFailure::Runtime)?,
                measurements.clone(),
            ),
            synchronized_update,
        ),
    );
    let backend = FixedViewportBackend::new(output, dimensions, modes.synchronized_update_lease());
    let backend = {
        let mut backend = backend;
        let mut nonce = [0; 4];
        if crate::tui_capabilities::Capability::Graphics.available()
            && getrandom::fill(&mut nonce).is_ok()
        {
            backend.enable_graphics_probe(u32::from_ne_bytes(nonce).max(1));
        }
        backend
    };
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .map_err(TuiFailure::Runtime)?;
    let mut inputs = InheritedInputSource::open(&inherited, measurements.clone())
        .map_err(TuiFailure::Runtime)?;
    let theme = crate::tui_theme::Theme::terminal_native();
    // Pointer reporting is armed (or left released) before the loop so the
    // first drawn frame already agrees with the dispatcher's settled flag.
    modes
        .set_pointer_capture_level(pointer_tracking_level(if mouse {
            PointerCaptureRequest::ButtonOnly
        } else {
            PointerCaptureRequest::Release
        }))
        .map_err(TuiFailure::Runtime)?;

    modes
        .set_focus_reporting(true)
        .map_err(TuiFailure::Runtime)?;

    let mut system_notifications = crate::notifications::system::SystemNotifications::default();
    let mut surface = SurfaceSlot::Owned(terminal_input);
    let run_result = run_surface_loop(
        &mut terminal,
        Chrome {
            locale,
            theme: &theme,
        },
        (&mut inputs, &mut |request| {
            let started = measurements.start();
            let result = match request {
                ModeRequest::Bell => modes.ring_bell(),
                ModeRequest::GraphicsQuery(query) => modes.query_graphics(query),
                ModeRequest::PointerAdmissionQuery(query) => modes.query_pointer_admission(query),
                ModeRequest::AppearanceQuery(query) => modes.query_host_appearance(query),
                ModeRequest::Notification(effect) => {
                    use orchbee_local_runtime::NotificationDelivery as Delivery;
                    let result = match effect.delivery {
                        Delivery::Terminal => modes.notify_terminal(&effect.message),
                        Delivery::System => system_notifications.submit(effect.message.clone()),
                        Delivery::Off | Delivery::Tui => Ok(()),
                    };
                    crate::notifications::sound(effect.message.sound);
                    result
                }
                ModeRequest::WindowTitle(title) => crate::tui_capabilities::Capability::WindowTitle
                    .require()
                    .map_err(io::Error::from)
                    .and_then(|()| {
                        modes.set_window_title(
                            title
                                .as_ref()
                                .map_or("Orchbee", orchbee_workbench_protocol::WindowTitle::as_str),
                        )
                    }),
                ModeRequest::BracketedPaste(enabled) => modes.set_bracketed_paste(enabled),
                ModeRequest::PointerCapture(request) => {
                    modes.set_pointer_capture_level(pointer_tracking_level(request))
                }
            };
            if let Some(started) = started {
                started.control_completed(result.is_ok());
            }
            result
        }),
        &mut surface,
        &termination,
        &inherited,
        dimensions,
        mouse,
        initial_view,
    )
    .map_err(LoopFailure::into_io_error);
    let detach_notice = run_result
        .as_ref()
        .ok()
        .filter(|completion| completion.exit == LoopExit::User)
        .map(|completion| completion.locale.tui_detached(&completion.return_command));
    if run_result.is_err() {
        // BufWriter's drop may retry buffered bytes. Fence incomplete frame
        // output first; mode restoration still sends its own control bytes.
        modes.discard_frame_output();
    }
    drop(system_notifications);
    drop(terminal);
    drop(surface);
    measurement_recorder
        .finish_after_restore(
            run_result.map(|_| ()),
            || {
                let restored = modes.restore().map_err(io::Error::other);
                drop(modes);
                restored
            },
            locale,
        )
        .map_err(TuiFailure::Runtime)?;
    if let Some(notice) = detach_notice {
        // Human guidance belongs after raw/alternate-screen restoration, on stderr.
        let mut stderr = io::stderr().lock();
        writeln!(stderr, "{notice}")
            .and_then(|_| stderr.flush())
            .map_err(TuiFailure::Runtime)?;
    }
    Ok(())
}

/// Presentation inputs that travel together through the event loop: the
/// localized copy and the resolved theme.
#[derive(Clone, Copy)]
struct Chrome<'a> {
    locale: Locale,
    theme: &'a crate::tui_theme::Theme,
}

fn frame_appearance(
    view: &ClientView,
    chrome: Chrome<'_>,
    workbench_client: Option<crate::workbench_client::WorkbenchClientView<'_>>,
) -> (crate::tui_theme::Theme, Locale) {
    let preview = match workbench_client.as_ref() {
        Some(native)
            if crate::tui_composition::project_capture(
                client_capture_claims(view),
                native.capture_claims(),
            ) == Some(crate::tui_composition::CaptureOwner::Native(
                crate::tui_composition::WorkbenchCaptureOwner::Settings,
            )) =>
        {
            native.settings_appearance_preview()
        }
        _ => None,
    };
    let (theme, locale) = match preview {
        Some(values) => (
            view.preview_theme(values),
            view.presentation_locale(chrome.locale, Some(values.locale)),
        ),
        None => (
            view.configured_theme().unwrap_or(*chrome.theme),
            view.effective_locale(chrome.locale),
        ),
    };
    (theme.with_no_color(view.no_color()), locale)
}

#[cfg(test)]
#[expect(clippy::too_many_arguments)]
fn run_event_loop<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    chrome: Chrome<'_>,
    inputs: &mut impl InputSource,
    terminal_input: &mut dyn TerminalSurface,
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
        ClientView::new(dimensions.columns),
    )
}

#[cfg(test)]
#[expect(clippy::too_many_arguments)]
fn run_event_loop_with_modes<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    chrome: Chrome<'_>,
    input_runtime: (
        &mut impl InputSource,
        &mut impl FnMut(ModeRequest) -> io::Result<()>,
    ),
    terminal_input: &mut dyn TerminalSurface,
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    dimensions: WindowDimensions,
    mouse: bool,
    view: ClientView,
) -> Result<LoopExit, LoopFailure<B::Error>> {
    let mut surface = SurfaceSlot::Borrowed(terminal_input);
    run_surface_loop(
        terminal,
        chrome,
        input_runtime,
        &mut surface,
        termination,
        viewport,
        dimensions,
        mouse,
        view,
    )
    .map(|completion| completion.exit)
}

enum SurfaceSlot<'a> {
    Owned(Box<dyn TerminalSurface>),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "borrowed surfaces belong to deterministic loop tests"
        )
    )]
    Borrowed(&'a mut dyn TerminalSurface),
}
impl SurfaceSlot<'_> {
    fn active(&mut self) -> &mut dyn TerminalSurface {
        match self {
            Self::Owned(surface) => surface.as_mut(),
            Self::Borrowed(surface) => *surface,
        }
    }
    fn replace(&mut self, next: Box<dyn TerminalSurface>) -> Box<dyn TerminalSurface> {
        match self {
            Self::Owned(surface) => {
                surface.set_ui_wakeup(None);
                std::mem::replace(surface, next)
            }
            Self::Borrowed(_) => unreachable!("machine manager requires an owned surface"),
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn run_surface_loop<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    chrome: Chrome<'_>,
    input_runtime: (
        &mut impl InputSource,
        &mut impl FnMut(ModeRequest) -> io::Result<()>,
    ),
    surface: &mut SurfaceSlot<'_>,
    termination: &impl TerminationSource,
    viewport: &impl ViewportSource,
    mut dimensions: WindowDimensions,
    mouse: bool,
    mut view: ClientView,
) -> Result<LoopCompletion, LoopFailure<B::Error>> {
    let mut dispatcher = InputDispatcher::default();
    let wakeup = crate::workbench_client::UiWakeup::current();
    surface.active().set_ui_wakeup(Some(wakeup.clone()));
    let result = (|| {
        let (inputs, mode) = input_runtime;
        let measurements = inputs.measurements();
        let mut machines = matches!(surface, SurfaceSlot::Owned(_))
            .then(|| machines::Manager::new(&mut view, wakeup.clone()));
        let terminal_input = surface.active();
        if let Some(id) = terminal.backend().graphics_probe_id() {
            dispatcher.activate_graphics_probe(id);
        }
        // The lifecycle write already armed or released host reporting; the
        // dispatcher and the status indicator settle to the same truth here.
        dispatcher.settle_pointer_capture(
            &mut view,
            terminal_input,
            if mouse {
                PointerCaptureRequest::ButtonOnly
            } else {
                PointerCaptureRequest::Release
            },
        );
        if termination.requested() {
            return Ok(LoopExit::Signal);
        }
        synchronize_viewport(
            terminal,
            viewport,
            &mut view,
            terminal_input,
            &mut dimensions,
            dispatcher.graphics_facts().measurement,
            (
                &dispatcher.resize_projection(),
                dispatcher.resize_presentation_allowed(),
            ),
        )?;
        dispatcher.query_graphics(
            (dimensions.columns, dimensions.rows),
            inputs.sample_time(),
            &mut |request| mode(request).map_err(LoopFailure::Input),
        )?;
        terminal_input.observe_host_graphics(dispatcher.graphics_facts());
        let _changed = terminal_input.tick_with_client_capture(client_capture_claims(&view));
        if terminal_input.client_control_tick(&mut view, to_area(dimensions), chrome.locale) {
            dispatcher.invalidate_hit_map(terminal_input);
            synchronize_viewport(
                terminal,
                viewport,
                &mut view,
                terminal_input,
                &mut dimensions,
                dispatcher.graphics_facts().measurement,
                (
                    &dispatcher.resize_projection(),
                    dispatcher.resize_presentation_allowed(),
                ),
            )?;
        }
        update_graphics_surface(terminal, &mut dispatcher, inputs.sample_time());
        let mut pointer_projection_redraw = draw_measured_surface(
            terminal,
            &mut view,
            chrome,
            terminal_input,
            &mut dispatcher,
            mode,
            &measurements,
        )?;
        if pointer_projection_redraw {
            wakeup.signal();
        }
        // Cold viewport synchronization and the first paint must not consume
        // the host's bounded reply window before the reader can run.
        dispatcher.begin_host_appearance(inputs.sample_time(), &mut |request| {
            mode(request).map_err(LoopFailure::Input)
        })?;
        dispatcher.begin_pointer_admission(inputs.sample_time(), &mut |request| {
            mode(request).map_err(LoopFailure::Input)
        })?;
        loop {
            if termination.requested() {
                return Ok(LoopExit::Signal);
            }
            let maximum_wait = Duration::from_millis(u64::from(EVENT_POLL_INTERVAL_MILLISECONDS));
            let timeout = surface
                .active()
                .next_poll_delay()
                .unwrap_or(maximum_wait)
                .min(maximum_wait);
            let started = measurements.start();
            let input = inputs.next_input_or_wakeup(timeout, &wakeup);
            let input_received_at = inputs.sample_time();
            let batch = started.and_then(|started| started.input_completed(&input));
            let input = input.map_err(LoopFailure::Input)?;
            let terminal_input = surface.active();
            let mut redraw = std::mem::take(&mut pointer_projection_redraw);
            let viewport_changed = synchronize_viewport(
                terminal,
                viewport,
                &mut view,
                terminal_input,
                &mut dimensions,
                dispatcher.graphics_facts().measurement,
                (
                    &dispatcher.resize_projection(),
                    dispatcher.resize_presentation_allowed(),
                ),
            )?;
            redraw |= viewport_changed;
            if viewport_changed {
                // Resize discards the recorded regions: until the next
                // successful draw publishes a fresh map, pointer events drop
                // instead of resolving against stale geometry.
                dispatcher.invalidate_hit_map(terminal_input);
            }
            redraw |= terminal_input.tick_with_client_capture(client_capture_claims(&view));
            let now = inputs.sample_time();
            redraw |= view.expire_notice(now);
            {
                let board_animates = match terminal_input.view() {
                    Some(native) => native.agent_board().is_some_and(|board| {
                        board.rows().iter().any(|row| matches!(row.status(), orchbee_agent_observation_protocol::AgentSemanticStatus::Working { .. } | orchbee_agent_observation_protocol::AgentSemanticStatus::NeedsAttention { .. }))
                    }),
                    _ => false,
                };
                redraw |= view.machines.advance_working_motion(now, board_animates);
            }
            redraw |= dispatcher.query_graphics(
                (dimensions.columns, dimensions.rows),
                now,
                &mut |request| mode(request).map_err(LoopFailure::Input),
            )?;
            terminal_input.observe_host_graphics(dispatcher.graphics_facts());
            // A ready continuation outranks the idle timeout, even when the
            // previous frame took longer than the decoder's bounded wait.
            // Dispatch still reconciles current owners before reading bytes.
            if input.is_none() {
                redraw |=
                    dispatcher.synchronize(&mut view, terminal_input, now, &mut |request| {
                        mode(request).map_err(LoopFailure::Input)
                    })?;
            }
            let menu_measurement = dispatcher.graphics_facts().measurement;
            let menu_resize = dispatcher.resize_projection();
            let menu_resize_allowed = dispatcher.resize_presentation_allowed();
            if let Some(outcome) = dispatcher.complete_action_menu(
                &mut view,
                terminal_input,
                now,
                &mut |view, terminal_input| {
                    synchronize_viewport(
                        terminal,
                        viewport,
                        view,
                        terminal_input,
                        &mut dimensions,
                        menu_measurement,
                        (&menu_resize, menu_resize_allowed),
                    )
                },
                &mut |request| mode(request).map_err(LoopFailure::Input),
            )? {
                if outcome == DispatchOutcome::Exit {
                    return Ok(LoopExit::User);
                }
                redraw = true;
            }
            let previous_measurement = dispatcher.graphics_facts().measurement;
            if let Some(input) = input {
                let measurement = dispatcher.graphics_facts().measurement;
                let resize = dispatcher.resize_projection();
                let resize_allowed = dispatcher.resize_presentation_allowed();
                let started = measurements.start();
                let outcome = dispatcher.dispatch(
                    &mut view,
                    &input,
                    terminal_input,
                    input_received_at,
                    |view, terminal_input| {
                        synchronize_viewport(
                            terminal,
                            viewport,
                            view,
                            terminal_input,
                            &mut dimensions,
                            measurement,
                            (&resize, resize_allowed),
                        )
                    },
                    &mut |request| mode(request).map_err(LoopFailure::Input),
                );
                if let Some(started) = started {
                    started.dispatch_completed(batch, outcome.is_ok());
                }
                let outcome = outcome?;
                if outcome == DispatchOutcome::Exit {
                    return Ok(LoopExit::User);
                }
                redraw |= dispatcher.dispatch_requested_redraw();
            }
            redraw |= dispatcher.query_graphics(
                (dimensions.columns, dimensions.rows),
                now,
                &mut |request| mode(request).map_err(LoopFailure::Input),
            )?;
            terminal_input.observe_host_graphics(dispatcher.graphics_facts());
            if previous_measurement != dispatcher.graphics_facts().measurement {
                redraw |= synchronize_viewport(
                    terminal,
                    viewport,
                    &mut view,
                    terminal_input,
                    &mut dimensions,
                    dispatcher.graphics_facts().measurement,
                    (
                        &dispatcher.resize_projection(),
                        dispatcher.resize_presentation_allowed(),
                    ),
                )?;
            }
            // Observe the newest owned draft without sampling or reporting an external
            // host change. This keeps both geometry consumers on one held position.
            observe_surface_geometry(
                &dimensions,
                &view,
                terminal_input,
                (
                    &dispatcher.resize_projection(),
                    dispatcher.resize_presentation_allowed(),
                ),
            )
            .map_err(LoopFailure::Viewport)?;
            if termination.requested() {
                return Ok(LoopExit::Signal);
            }

            if terminal_input.client_control_tick(&mut view, to_area(dimensions), chrome.locale) {
                synchronize_viewport(
                    terminal,
                    viewport,
                    &mut view,
                    terminal_input,
                    &mut dimensions,
                    dispatcher.graphics_facts().measurement,
                    (
                        &dispatcher.resize_projection(),
                        dispatcher.resize_presentation_allowed(),
                    ),
                )?;
                dispatcher.invalidate_hit_map(terminal_input);
                redraw = true;
            }
            if let Some(machines) = &mut machines {
                let change = machines.tick(surface, &mut view, dimensions, now, &mut |old| {
                    dispatcher.invalidate_hit_map(old);
                });
                redraw |= change.redraw;
                if change.switched {
                    surface.active().set_ui_wakeup(Some(wakeup.clone()));
                    dispatcher.machine_surface_changed();
                    if let Err(error) = mode(ModeRequest::WindowTitle(None)) {
                        view.window_title_unavailable(error.kind() == io::ErrorKind::Unsupported);
                    }

                    // This new surface has not participated in the old surface's controller tick.
                    surface.active().client_control_tick(
                        &mut view,
                        to_area(dimensions),
                        chrome.locale,
                    );
                    synchronize_viewport(
                        terminal,
                        viewport,
                        &mut view,
                        surface.active(),
                        &mut dimensions,
                        dispatcher.graphics_facts().measurement,
                        (
                            &dispatcher.resize_projection(),
                            dispatcher.resize_presentation_allowed(),
                        ),
                    )?;
                }
            }
            let terminal_input = surface.active();
            terminal_input.observe_host_graphics(dispatcher.graphics_facts());
            {
                redraw |= update_graphics_surface(terminal, &mut dispatcher, now);
            }
            if let Some(snapshot) = view.window_title.take()
                && let Err(error) = mode(ModeRequest::WindowTitle(snapshot.title))
            {
                view.window_title_unavailable(error.kind() == io::ErrorKind::Unsupported);
                redraw = true;
            }
            if redraw {
                pointer_projection_redraw = draw_measured_surface(
                    terminal,
                    &mut view,
                    chrome,
                    terminal_input,
                    &mut dispatcher,
                    mode,
                    &measurements,
                )?;
                if pointer_projection_redraw {
                    wakeup.signal();
                }
            }
            if let Some(effect) = view.notifications.take_effect()
                && let Err(error) = mode(ModeRequest::Notification(effect))
            {
                view.notification_unavailable(error.kind() == io::ErrorKind::Unsupported);
                pointer_projection_redraw = draw_measured_surface(
                    terminal,
                    &mut view,
                    chrome,
                    terminal_input,
                    &mut dispatcher,
                    mode,
                    &measurements,
                )?;
                if pointer_projection_redraw {
                    wakeup.signal();
                }
            }
        }
    })();
    dispatcher.cancel_pointer(surface.active());
    surface.active().set_ui_wakeup(None);
    result.map(|exit| LoopCompletion {
        exit,
        locale: view.effective_locale(chrome.locale),
        return_command: {
            if let crate::machine_picker::Choice::Remote(id) = &view.machines.current {
                format!("orchbee --machine {}", id.as_str())
            } else {
                "orchbee".to_owned()
            }
        },
    })
}

fn update_graphics_surface<B: Backend + ResizableBackend>(
    terminal: &Terminal<B>,
    dispatcher: &mut InputDispatcher,
    now: std::time::Instant,
) -> bool {
    let (replies, overflow) = dispatcher.take_graphics_replies();
    terminal
        .backend()
        .graphics_surface()
        .is_some_and(|graphics| {
            graphics.update(dispatcher.graphics_facts(), now, replies, overflow)
        })
}

#[cfg(test)]
fn draw_surface<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    surface: &mut (impl TerminalSurface + ?Sized),
    dispatcher: &mut InputDispatcher,
    mode: &mut impl FnMut(ModeRequest) -> io::Result<()>,
) -> Result<bool, LoopFailure<B::Error>> {
    draw_measured_surface(
        terminal,
        view,
        chrome,
        surface,
        dispatcher,
        mode,
        &Measurements::default(),
    )
}

fn draw_measured_surface<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    surface: &mut (impl TerminalSurface + ?Sized),
    dispatcher: &mut InputDispatcher,
    mode: &mut impl FnMut(ModeRequest) -> io::Result<()>,
    measurements: &Measurements,
) -> Result<bool, LoopFailure<B::Error>> {
    let mut frame_measurement = measurements.frame();
    // One reversible projection reaches every painter, including background panes and native
    // chrome. Runtime effects still consume only the separately confirmed preferences.
    let (theme, locale) = frame_appearance(view, chrome, surface.view());
    let chrome = Chrome {
        theme: &theme,
        locale,
    };
    surface.observe_client_viewport(view, terminal.get_frame().area());
    let capture_owner = {
        let native_claims = match surface.view() {
            Some(native) => native.capture_claims(),
            _ => crate::tui_composition::CaptureClaims::default(),
        };
        crate::tui_composition::project_capture(client_capture_claims(view), native_claims)
    };
    let allow_defer = capture_owner.is_none()
        && view.overlay() != Some(crate::client_view::Overlay::ShortcutHelp)
        && view.notifications.empty();
    if surface.prepare_draw(terminal.get_frame().area(), allow_defer) {
        // Pixels remain from the last successful frame, but its targets cannot authorize
        // actions against the newly selected terminal. No draw receipt is minted here.
        dispatcher.invalidate_hit_map(surface);
        return Ok(false);
    }
    let mut notification_painted = false;
    let mut receipt = crate::workbench_client::DrawReceipt::default();
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    let preview = if capture_owner.is_none() {
        dispatcher
            .divider_preview()
            .map(divider::Preview::Divider)
            .or_else(|| {
                dispatcher
                    .sidebar_width_preview()
                    .map(divider::Preview::Sidebar)
            })
    } else {
        None
    };
    let graphics = terminal.backend().graphics_surface();
    let resize = divider::project(
        view,
        surface,
        terminal.get_frame().area(),
        &dispatcher.resize_projection(),
        dispatcher.resize_presentation_allowed(),
    );
    let mut preview_painted = false;
    let tab_drag = capture_owner
        .is_none()
        .then(|| dispatcher.tab_drag_preview())
        .flatten();
    let sidebar_reorder = capture_owner
        .is_none()
        .then(|| dispatcher.sidebar_reorder_preview())
        .flatten();
    if let Some(measurement) = &mut frame_measurement {
        measurement.buffer_started();
    }
    let drawn = terminal.draw(|frame| {
        {
            (receipt, preview_painted) = divider::render(
                frame,
                view,
                chrome,
                surface,
                graphics.as_ref(),
                divider::Presentation {
                    projection: &resize,
                    preview,
                },
                &mut hits,
            );
        }

        let notifications_presentable =
            crate::tui_layout::project_shell_areas(frame.area(), view.sidebar_surface()).tier
                != crate::tui_layout::ViewportTier::TooSmall;
        if notifications_presentable && crate::notifications::available(view, surface.view()) {
            notification_painted =
                crate::notifications::render(frame, &view.notifications, chrome.theme, &mut hits)
                    .is_some();
        }
        render_machine_surface(
            frame,
            view,
            chrome,
            capture_owner,
            preview_painted,
            &mut hits,
        );
        if let Some(preview) = sidebar_reorder
            .as_ref()
            .and_then(|preview| hits.resolve_sidebar_reorder_preview(preview))
        {
            crate::tui_render::render_sidebar_reorder_preview(
                frame,
                &preview,
                chrome.locale,
                chrome.theme,
            );
        }
        if let Some(preview) = &tab_drag {
            crate::tui_render::render_tab_drag_preview(frame, preview, chrome.locale, chrome.theme);
        }
        if let Some(graphics) = &graphics {
            graphics.finish_frame(
                receipt.graphics_draws(),
                preview_painted
                    || capture_owner == Some(crate::tui_composition::CaptureOwner::Machine)
                    || notification_painted
                    || sidebar_reorder.is_some(),
            );
        }
        if let Some(measurement) = &mut frame_measurement {
            measurement.buffer_prepared();
        }
    });
    if let Some(measurement) = frame_measurement {
        measurement.finish(drawn.is_ok());
    }
    if let Err(error) = drawn {
        // A partial frame may already have reached the host. Its candidate
        // cannot authorize interaction; this loop exits before another input.
        dispatcher.invalidate_hit_map(surface);
        return Err(LoopFailure::Terminal(error));
    }
    if notification_painted {
        // Settle sound only after the card and its matching hit shield were
        // included in the successfully flushed frame.
        crate::notifications::sound(view.notifications.take_sound_after_draw());
    }
    let help_draw = dispatcher.publish_hit_map_with_help(hits.finish());
    view.settle_help_draw(help_draw);
    let pointer_projection_redraw = dispatcher.take_pointer_projection_redraw();
    if capture_owner == Some(crate::tui_composition::CaptureOwner::Machine) {
        // Covered native content cannot acknowledge successful visible presentation.
        surface.graphics_covered();
        surface.after_close_draw(crate::workbench_client::DrawReceipt::default());
        return Ok(pointer_projection_redraw);
    }
    if notification_painted {
        receipt = receipt.with_attention(None);
        {
            receipt = receipt.with_graphics(Vec::new()).with_position(None);
        }
    }
    surface.after_close_draw(receipt);
    surface.after_draw();
    surface
        .deliver_terminal_effect(&mut || mode(ModeRequest::Bell))
        .map_err(LoopFailure::Input)?;
    Ok(pointer_projection_redraw)
}

fn render_machine_surface(
    frame: &mut ratatui::Frame<'_>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    capture_owner: Option<crate::tui_composition::CaptureOwner>,
    preview_painted: bool,
    hits: &mut HitMapBuilder,
) {
    // The preview's persistent status owns this tail until the gesture ends.
    if preview_painted || capture_owner == Some(crate::tui_composition::CaptureOwner::Copy) {
    } else {
        view.machines
            .render(frame, chrome.locale, chrome.theme, hits);
    }
}

fn synchronize_sidebar_visibility(view: &mut ClientView, dimensions: WindowDimensions) -> bool {
    let owned = view.machines.sidebar_navigation_active();
    let areas = crate::tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface());
    let visible = areas.sidebar.is_some();
    view.machines.observe_agent_sidebar(visible);
    owned && !view.machines.sidebar_navigation_active()
}

fn popup_surface_area(
    dimensions: WindowDimensions,
    view: &ClientView,
    panel_count: usize,
    zoomed: bool,
) -> Rect {
    let areas = crate::tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface());
    if areas.tier == crate::tui_layout::ViewportTier::TooSmall {
        Rect::default()
    } else {
        crate::tui_layout::native_terminal_area(
            areas.panel,
            crate::tui_layout::PaneFrame::of(areas.tier.shows_panel_frame(), panel_count, zoomed),
        )
    }
}

fn synchronize_viewport<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    viewport: &impl ViewportSource,
    view: &mut ClientView,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    dimensions: &mut WindowDimensions,
    measurement: Option<crate::terminal_graphics::Measurement>,
    resize: (&crate::tui_layout::ResizeProjection, bool),
) -> Result<bool, LoopFailure<B::Error>> {
    let next = viewport
        .window_dimensions()
        .map_err(LoopFailure::Viewport)?
        .map(|next| measurement.map_or(next, |measurement| measurement.apply(next)))
        .unwrap_or(*dimensions);
    let changed = next != *dimensions;
    if changed {
        {
            view.machines.cancel_sidebar_scroll();
        }
        terminal_input.invalidate_close_display();
        terminal_input.invalidate_rename_display();
        terminal_input.invalidate_setup_display();
        terminal_input.invalidate_worktree_display();
        terminal.backend_mut().set_window_dimensions(next);
        terminal
            .resize(to_area(next))
            .map_err(LoopFailure::Terminal)?;
        view.resize(next.columns);
        *dimensions = next;
    }
    let changed = synchronize_sidebar_visibility(view, *dimensions) || changed;
    let resize = if changed {
        // Pixel-only host changes also retire a released presentation lease, even when
        // the cell rectangle still matches. An owned projection never enters this branch.
        (&crate::tui_layout::ResizeProjection::Canonical, false)
    } else {
        resize
    };
    observe_surface_geometry(dimensions, view, terminal_input, resize)
        .map_err(LoopFailure::Viewport)?;
    Ok(changed)
}

/// Apply the same admitted local projection used by paint, without changing host ownership.
fn observe_surface_geometry(
    dimensions: &WindowDimensions,
    view: &ClientView,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    resize: (&crate::tui_layout::ResizeProjection, bool),
) -> io::Result<()> {
    let resize = divider::project(
        view,
        terminal_input,
        to_area(*dimensions),
        resize.0,
        resize.1,
    );

    let mut client_view = terminal_input.view();
    if let Some(native) = client_view.as_mut() {
        native.set_resize_projection(&resize);
    }
    match client_view {
        Some(native) => {
            let panes = pane_geometries(*dimensions, view, &native)?;
            let presentable = crate::tui_layout::project_shell_areas(
                to_area(*dimensions),
                native.sidebar_surface(view.sidebar_surface()),
            )
            .tier
                != crate::tui_layout::ViewportTier::TooSmall;
            let (panel_count, zoomed) = (
                native
                    .active_position()
                    .and_then(|p| p.checked_sub(1))
                    .and_then(|i| native.tab(i))
                    .map_or(0, |tab| tab.panel_count()),
                native.zoom_active(),
            );
            // TooSmall withholds geometry; it is not evidence that the canonical panes left.
            if presentable {
                terminal_input.observe_panes(&panes);
            }
            {
                let area = popup_surface_area(*dimensions, view, panel_count, zoomed);
                let pixels = (
                    dimensions
                        .pixel_width
                        .checked_div(dimensions.columns)
                        .unwrap_or(0),
                    dimensions
                        .pixel_height
                        .checked_div(dimensions.rows)
                        .unwrap_or(0),
                );
                let pixels = if pixels.0 == 0 || pixels.1 == 0 {
                    (0, 0)
                } else {
                    pixels
                };
                terminal_input.popup_surface(area, pixels);
            }
        }
        _ => {
            let geometry = terminal_geometry(*dimensions, view, client_view)?;
            if let Some(geometry) = geometry {
                terminal_input.observe_geometry(geometry);
            }
        }
    }
    Ok(())
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
    workbench_client: Option<crate::workbench_client::WorkbenchClientView<'_>>,
) -> io::Result<Option<orchbee_client_server_protocol::Geometry>> {
    let sidebar = match workbench_client.as_ref() {
        Some(native) => native.sidebar_surface(view.sidebar_surface()),
        _ => view.sidebar_surface(),
    };
    let areas = crate::tui_layout::project_shell_areas(to_area(dimensions), sidebar);
    let Some(area) = active_pane_area(&areas, workbench_client) else {
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
/// [`crate::tui_render::render_terminal_shell`] paints: the whole panel content for a single
/// pane or a Minimal viewport (active pane only, no tree chrome), otherwise
/// the active leaf's content inside the projected split tree. `None` when the
/// viewport cannot present a pane at all — explicit, not inferred from a
/// returned rect.
pub(crate) fn active_pane_area(
    areas: &crate::tui_layout::ShellAreas,
    workbench_client: Option<crate::workbench_client::WorkbenchClientView<'_>>,
) -> Option<Rect> {
    if areas.tier == crate::tui_layout::ViewportTier::TooSmall {
        return None;
    }
    let framed = areas.tier.shows_panel_frame();
    let content = crate::tui_layout::terminal_area(areas.panel, framed);
    let native = match workbench_client {
        Some(native) => native,
        None => return (!content.is_empty()).then_some(content),
    };
    let tab = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))?;
    let content = crate::tui_layout::native_terminal_area(
        areas.panel,
        crate::tui_layout::PaneFrame::of(framed, tab.panel_count(), native.zoom_active()),
    );
    let area = if framed && tab.panel_count() > 1 && !native.zoom_active() {
        crate::tui_layout::project_panel_tree(
            content,
            tab.panel_tree(),
            tab.active_panel(),
            &|split, first, second| tab.split_ratio(split, first, second),
        )
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
    native: &crate::workbench_client::WorkbenchClientView<'_>,
) -> io::Result<
    Vec<(
        orchbee_workbench_protocol::WorkbenchPanelId,
        usize,
        orchbee_client_server_protocol::Geometry,
    )>,
> {
    use crate::tui_layout;
    let areas = tui_layout::project_shell_areas(
        to_area(dimensions),
        native.sidebar_surface(view.sidebar_surface()),
    );
    if areas.tier == tui_layout::ViewportTier::TooSmall {
        return Ok(Vec::new());
    }
    let framed = areas.tier.shows_panel_frame();
    let Some(tab) = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))
    else {
        return Ok(Vec::new());
    };
    let content = tui_layout::native_terminal_area(
        areas.panel,
        tui_layout::PaneFrame::of(framed, tab.panel_count(), false),
    );
    let zoomed_content = tui_layout::native_terminal_area(
        areas.panel,
        tui_layout::PaneFrame::of(framed, tab.panel_count(), true),
    );
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
        let projection = tui_layout::project_panel_tree(
            content,
            tab.panel_tree(),
            tab.active_panel(),
            &|split, first, second| tab.split_ratio(split, first, second),
        );
        let mut panes = Vec::with_capacity(projection.leaves.len());
        for leaf in &projection.leaves {
            let Some(panel) = tab.panel(leaf.panel_index) else {
                continue;
            };
            // Under zoom the observed set stays the full canonical leaf
            // list: the focused leaf reports the whole content area so its
            // terminal converges to the drawn size, while hidden leaves
            // report their unzoomed canonical rect — attached, alive, and
            // never resized.
            let rect = if native.zoom_active() && tab.active_panel() == Some(leaf.panel_index) {
                zoomed_content
            } else {
                leaf.content
            };
            panes.push((
                panel.panel_id().clone(),
                leaf.panel_index,
                leaf_geometry(rect)?,
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
    fn navigate_agent(
        &mut self,
        _target: &crate::machine_agents::Target,
    ) -> crate::workbench_client::ContextNavigationAdmission {
        crate::workbench_client::ContextNavigationAdmission::Unavailable
    }
    fn navigate_context(
        &mut self,
        _target: &crate::workbench_client::ClientContextTarget,
    ) -> crate::workbench_client::ContextNavigationAdmission {
        crate::workbench_client::ContextNavigationAdmission::Unavailable
    }
    fn set_ui_wakeup(&mut self, _wakeup: Option<crate::workbench_client::UiWakeup>) {}
    fn next_poll_delay(&self) -> Option<Duration> {
        None
    }
    /// The attached Session published an Agent change; Client-owned Agent reads start early.
    fn expedite_agent_reads(&mut self) {}
    fn observe_client_viewport(&mut self, _view: &ClientView, _viewport: Rect) {}
    fn popup_surface(&mut self, _area: Rect, _pixels: (u16, u16)) {}

    fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        None
    }
    fn suspend_machine(&mut self) -> Option<bool> {
        None
    }
    fn resume_machine(&mut self, _focus: Option<bool>) {}

    fn retained_frame_bank(&self) -> Option<crate::workbench_client::RetainedFrameBank> {
        None
    }

    fn reusable_machine(&self) -> bool {
        true
    }

    fn configure_panel_focus(&mut self, _focus_new_panel: bool) {}
    fn configure_agent_notifications(
        &mut self,
        _revision: u64,
        _settings: &orchbee_local_runtime::NotificationSettings,
    ) {
    }
    fn client_control_tick(
        &mut self,
        _view: &mut ClientView,
        _viewport: Rect,
        _locale: Locale,
    ) -> bool {
        false
    }
    fn tick(&mut self) -> bool {
        false
    }
    fn tick_with_client_capture(
        &mut self,
        _claims: crate::tui_composition::ClientCaptureClaims,
    ) -> bool {
        self.tick()
    }
    fn after_draw(&mut self) {}
    fn prepare_draw(&mut self, _viewport: Rect, _allow_defer: bool) -> bool {
        false
    }
    fn deliver_terminal_effect(
        &mut self,
        _emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        Ok(())
    }
    fn after_close_draw(&mut self, _receipt: crate::workbench_client::DrawReceipt) {}
    fn graphics_covered(&mut self) {}
    fn observe_panes(
        &mut self,
        _panes: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            orchbee_client_server_protocol::Geometry,
        )],
    ) {
    }
    fn view(&self) -> Option<crate::workbench_client::WorkbenchClientView<'_>> {
        None
    }
}
impl TerminalSurface for UnavailableTerminalInputPort {}
impl TerminalSurface for crate::workbench_client::WorkbenchClient {
    fn navigate_agent(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> crate::workbench_client::ContextNavigationAdmission {
        self.navigate_agent(target)
    }
    fn navigate_context(
        &mut self,
        target: &crate::workbench_client::ClientContextTarget,
    ) -> crate::workbench_client::ContextNavigationAdmission {
        self.navigate_context(target)
    }
    fn set_ui_wakeup(&mut self, wakeup: Option<crate::workbench_client::UiWakeup>) {
        self.set_ui_wakeup(wakeup);
    }
    fn next_poll_delay(&self) -> Option<Duration> {
        self.next_poll_delay()
    }
    fn expedite_agent_reads(&mut self) {
        self.expedite_agent_reads();
    }
    fn observe_client_viewport(&mut self, view: &ClientView, viewport: Rect) {
        self.observe_client_viewport(view, viewport);
    }
    fn configure_panel_focus(&mut self, focus_new_panel: bool) {
        self.configure_panel_focus(focus_new_panel);
    }
    fn configure_agent_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_runtime::NotificationSettings,
    ) {
        self.configure_agent_notifications(revision, settings);
    }

    fn popup_surface(&mut self, area: Rect, pixels: (u16, u16)) {
        self.observe_popup_surface(area, pixels);
    }

    fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        self.machine_source()
    }
    fn suspend_machine(&mut self) -> Option<bool> {
        self.suspend_machine()
    }
    fn resume_machine(&mut self, focus: Option<bool>) {
        self.resume_machine(focus);
    }
    fn retained_frame_bank(&self) -> Option<crate::workbench_client::RetainedFrameBank> {
        self.retained_frame_bank()
    }
    fn reusable_machine(&self) -> bool {
        // A canonical placeholder is a usable workspace, despite having no
        // attached terminal. Preserve it when revisiting the cached machine.
        let view = self.view();
        if view.retirement().is_none()
            && matches!(
                view.phase(),
                crate::workbench_client::DisplayPhase::SelectedWithoutAttachment
                    | crate::workbench_client::DisplayPhase::RetryableAttachUnavailable
            )
        {
            return true;
        }
        !matches!(
            self.status(),
            crate::workbench_client::TerminalStatus::Unavailable
                | crate::workbench_client::TerminalStatus::OutcomeUnknown
        )
    }
    fn client_control_tick(
        &mut self,
        view: &mut ClientView,
        viewport: Rect,
        locale: Locale,
    ) -> bool {
        let changed = self.shared_preferences_tick(view)
            | self.automatic_notifications_tick(view, view.effective_locale(locale));
        changed | self.client_control_tick(view, viewport, view.effective_locale(locale))
    }
    fn tick(&mut self) -> bool {
        self.tick()
    }
    fn tick_with_client_capture(
        &mut self,
        claims: crate::tui_composition::ClientCaptureClaims,
    ) -> bool {
        crate::workbench_client::WorkbenchClient::tick_with_client_capture(self, claims)
    }
    fn after_draw(&mut self) {
        self.after_draw()
    }
    fn prepare_draw(&mut self, viewport: Rect, allow_defer: bool) -> bool {
        self.prepare_draw(viewport, allow_defer)
    }
    fn deliver_terminal_effect(
        &mut self,
        emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        self.deliver_terminal_effect(emit_bell)
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
    fn after_close_draw(&mut self, receipt: crate::workbench_client::DrawReceipt) {
        self.after_close_draw(receipt)
    }
    fn graphics_covered(&mut self) {
        self.graphics_covered();
    }
    fn view(&self) -> Option<crate::workbench_client::WorkbenchClientView<'_>> {
        Some(self.view())
    }
}

const fn to_area(dimensions: WindowDimensions) -> Rect {
    Rect::new(0, 0, dimensions.columns, dimensions.rows)
}

fn client_capture_claims(_view: &ClientView) -> crate::tui_composition::ClientCaptureClaims {
    let claims = crate::tui_composition::ClientCaptureClaims::default();
    let claims = claims
        .with_machine(_view.machines.open)
        .with_resize(_view.resize_mode());
    claims
}

#[cfg(test)]
pub(crate) mod tests;
