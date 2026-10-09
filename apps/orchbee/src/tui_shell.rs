#[cfg(windows)]
mod divider;
#[cfg(windows)]
mod machines;

#[cfg(unix)]
use std::io::Read;
use std::{
    fmt,
    fs::File,
    io::{self, BufWriter},
    time::{Duration, Instant},
};

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
    terminal_input::{DispatchOutcome, InputDispatcher, ModeRequest, UnavailableTerminalInputPort},
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
    termination_signal::{TerminationSignal, TerminationSource},
    tui_backend::{FixedViewportBackend, ResizableBackend, RestorationGatedWriter},
    tui_pointer::HitMapBuilder,
    tui_render::render_terminal_shell,
};

const EVENT_POLL_INTERVAL_MILLISECONDS: u16 = 50;
#[cfg(unix)]
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
    #[cfg(windows)]
    terminal: InheritedTerminal,
}

impl InheritedInputSource {
    fn new(input: File, _terminal: &InheritedTerminal) -> Self {
        Self {
            input,
            #[cfg(windows)]
            terminal: _terminal.clone(),
        }
    }
}

impl InputSource for InheritedInputSource {
    fn next_input(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        #[cfg(windows)]
        {
            let _ = &self.input;
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
    let server_configuration = connection.is_some();
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

#[cfg(any(target_os = "linux", windows))]
pub(crate) fn run_prepared(
    locale: Locale,
    inherited: InheritedTerminal,
    dimensions: WindowDimensions,
    session: crate::terminal_session::TerminalSession,
    mouse: bool,
    notice: Option<crate::client_view::Notice>,
) -> Result<(), TuiFailure> {
    run_prepared_surface(
        locale,
        inherited,
        dimensions,
        Box::new(session),
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
    #[cfg(not(windows))]
    let _ = server_configuration;
    #[cfg(windows)]
    let mut terminal_input = terminal_input;
    let mut initial_view = ClientView::new(dimensions.columns);
    if let Some(notice) = notice {
        initial_view.startup_notice(notice);
    }
    #[cfg(windows)]
    {
        // A connected Client may paint one provisional local projection while the configuration
        // watcher obtains the Server's authoritative baseline. A later local fatal error cannot
        // invalidate an already admitted Site; standalone startup still fails on that boundary.
        let host_colors = ["NO_COLOR", "FORCE_COLOR", "COLORTERM", "TERM", "WT_SESSION"]
            .map(|name| std::env::var(name).ok());
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
        initial_view.configure_theme(orchbee_local_site_runtime::WindowsThemeName::TerminalNative);
        let load =
            orchbee_local_site_runtime::read_windows_user_settings(|name| std::env::var_os(name));
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
    let termination = TerminationSignal::install().map_err(TuiFailure::Runtime)?;
    let mut modes = TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone()))
        .map_err(TuiFailure::Runtime)?;
    let synchronized_update = modes.synchronized_update_lease();
    let output = BufWriter::with_capacity(
        OUTPUT_BUFFER_CAPACITY,
        RestorationGatedWriter::new(
            inherited.output().map_err(TuiFailure::Runtime)?,
            synchronized_update,
        ),
    );
    let mut backend =
        FixedViewportBackend::new(output, dimensions, modes.synchronized_update_lease());
    #[cfg(windows)]
    {
        let mut nonce = [0; 4];
        if getrandom::fill(&mut nonce).is_ok() {
            backend.enable_graphics_probe(u32::from_ne_bytes(nonce).max(1));
        }
    }
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .map_err(TuiFailure::Runtime)?;
    let input = File::from(inherited.duplicate_input().map_err(TuiFailure::Runtime)?);
    let mut inputs = InheritedInputSource::new(input, &inherited);
    let theme = crate::tui_theme::Theme::terminal_native();
    // Pointer reporting is armed (or left released) before the loop so the
    // first drawn frame already agrees with the dispatcher's settled flag.
    modes
        .set_pointer_capture(mouse)
        .map_err(TuiFailure::Runtime)?;

    #[cfg(windows)]
    modes
        .set_focus_reporting(true)
        .map_err(TuiFailure::Runtime)?;

    #[cfg(windows)]
    let mut system_notifications = crate::notifications::system::SystemNotifications::default();
    let mut surface = SurfaceSlot::Owned(terminal_input);
    let run_result = run_surface_loop(
        &mut terminal,
        Chrome {
            locale,
            theme: &theme,
        },
        (&mut inputs, &mut |request| match request {
            ModeRequest::Bell => modes.ring_bell(),
            ModeRequest::GraphicsQuery(query) => modes.query_graphics(query),
            #[cfg(windows)]
            ModeRequest::Notification(effect) => {
                use orchbee_local_site_runtime::WindowsNotificationDelivery as Delivery;
                let result = match effect.delivery {
                    Delivery::Terminal => modes.notify_terminal(&effect.message),
                    Delivery::System => system_notifications.submit(effect.message.clone()),
                    Delivery::Off | Delivery::Tui => Ok(()),
                };
                crate::notifications::sound(effect.message.sound);
                result
            }
            #[cfg(windows)]
            ModeRequest::WindowTitle(title) => modes.set_window_title(
                title
                    .as_ref()
                    .map_or("Orchbee", orchbee_workbench_protocol::WindowTitle::as_str),
            ),
            ModeRequest::BracketedPaste(enabled) => modes.set_bracketed_paste(enabled),
            ModeRequest::PointerCapture(enabled) => modes.set_pointer_capture(enabled),
        }),
        &mut surface,
        &termination,
        &inherited,
        dimensions,
        mouse,
        initial_view,
    )
    .map_err(LoopFailure::into_io_error);
    if run_result.is_err() {
        // BufWriter's drop may retry buffered bytes. Fence incomplete frame
        // output first; mode restoration still sends its own control bytes.
        modes.discard_frame_output();
    }
    #[cfg(windows)]
    drop(system_notifications);
    drop(terminal);
    drop(surface);
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
    #[cfg(windows)]
    fn replace(&mut self, next: Box<dyn TerminalSurface>) -> Box<dyn TerminalSurface> {
        match self {
            Self::Owned(surface) => std::mem::replace(surface, next),
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
) -> Result<LoopExit, LoopFailure<B::Error>> {
    let mut dispatcher = InputDispatcher::default();
    let result = (|| {
        let (inputs, mode) = input_runtime;
        #[cfg(windows)]
        let mut machines =
            matches!(surface, SurfaceSlot::Owned(_)).then(|| machines::Manager::new(&mut view));
        let terminal_input = surface.active();
        if let Some(id) = terminal.backend().graphics_probe_id() {
            dispatcher.activate_graphics_probe(id);
        }
        // The lifecycle write already armed or released host reporting; the
        // dispatcher and the status indicator settle to the same truth here.
        dispatcher.set_pointer_capture(&mut view, terminal_input, mouse);
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
        )?;
        dispatcher.query_graphics(
            (dimensions.columns, dimensions.rows),
            inputs.sample_time(),
            &mut |request| mode(request).map_err(LoopFailure::Input),
        )?;
        terminal_input.observe_host_graphics(dispatcher.graphics_facts());
        let _changed = terminal_input.tick();
        #[cfg(windows)]
        if terminal_input.client_control_tick(&mut view, to_area(dimensions), chrome.locale) {
            dispatcher.invalidate_hit_map(terminal_input);
            synchronize_viewport(
                terminal,
                viewport,
                &mut view,
                terminal_input,
                &mut dimensions,
                dispatcher.graphics_facts().measurement,
            )?;
        }
        #[cfg(windows)]
        update_graphics_surface(terminal, &mut dispatcher, inputs.sample_time());
        draw_surface(
            terminal,
            &mut view,
            chrome,
            terminal_input,
            &mut dispatcher,
            mode,
        )?;
        loop {
            if termination.requested() {
                return Ok(LoopExit::Signal);
            }
            let input = inputs
                .next_input(Duration::from_millis(u64::from(
                    EVENT_POLL_INTERVAL_MILLISECONDS,
                )))
                .map_err(LoopFailure::Input)?;
            let terminal_input = surface.active();
            let mut redraw = synchronize_viewport(
                terminal,
                viewport,
                &mut view,
                terminal_input,
                &mut dimensions,
                dispatcher.graphics_facts().measurement,
            )?;
            if redraw {
                // Resize discards the recorded regions: until the next
                // successful draw publishes a fresh map, pointer events drop
                // instead of resolving against stale geometry.
                dispatcher.invalidate_hit_map(terminal_input);
            }
            redraw |= terminal_input.tick();
            let now = inputs.sample_time();
            redraw |= dispatcher.query_graphics(
                (dimensions.columns, dimensions.rows),
                now,
                &mut |request| mode(request).map_err(LoopFailure::Input),
            )?;
            terminal_input.observe_host_graphics(dispatcher.graphics_facts());
            redraw |= dispatcher.synchronize(&mut view, terminal_input, now, &mut |request| {
                mode(request).map_err(LoopFailure::Input)
            })?;
            #[cfg(windows)]
            let menu_measurement = dispatcher.graphics_facts().measurement;
            #[cfg(windows)]
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
                let outcome = dispatcher.dispatch(
                    &mut view,
                    &input,
                    terminal_input,
                    now,
                    |view, terminal_input| {
                        synchronize_viewport(
                            terminal,
                            viewport,
                            view,
                            terminal_input,
                            &mut dimensions,
                            measurement,
                        )
                    },
                    &mut |request| mode(request).map_err(LoopFailure::Input),
                )?;
                if outcome == DispatchOutcome::Exit {
                    return Ok(LoopExit::User);
                }
                redraw = true;
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
                )?;
            }
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
                )?;
                dispatcher.invalidate_hit_map(terminal_input);
                redraw = true;
            }
            #[cfg(windows)]
            if let Some(machines) = &mut machines {
                let change = machines.tick(surface, &mut view, dimensions, now, &mut |old| {
                    dispatcher.invalidate_hit_map(old);
                });
                redraw |= change.redraw;
                if change.switched {
                    dispatcher.machine_surface_changed();
                    if mode(ModeRequest::WindowTitle(None)).is_err() {
                        view.window_title_unavailable();
                    }
                    view.machines.invalidate();
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
                    )?;
                }
            }
            let terminal_input = surface.active();
            terminal_input.observe_host_graphics(dispatcher.graphics_facts());
            #[cfg(windows)]
            {
                redraw |= update_graphics_surface(terminal, &mut dispatcher, now);
            }
            #[cfg(windows)]
            if let Some(snapshot) = view.window_title.take()
                && mode(ModeRequest::WindowTitle(snapshot.title)).is_err()
            {
                view.window_title_unavailable();
                redraw = true;
            }
            if redraw {
                draw_surface(
                    terminal,
                    &mut view,
                    chrome,
                    terminal_input,
                    &mut dispatcher,
                    mode,
                )?;
            }
            #[cfg(windows)]
            if let Some(effect) = view.notifications.take_effect()
                && mode(ModeRequest::Notification(effect)).is_err()
            {
                view.notification_unavailable();
                draw_surface(
                    terminal,
                    &mut view,
                    chrome,
                    terminal_input,
                    &mut dispatcher,
                    mode,
                )?;
            }
        }
    })();
    dispatcher.cancel_pointer(surface.active());
    result
}

#[cfg(windows)]
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

fn draw_surface<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    surface: &mut (impl TerminalSurface + ?Sized),
    dispatcher: &mut InputDispatcher,
    mode: &mut impl FnMut(ModeRequest) -> io::Result<()>,
) -> Result<(), LoopFailure<B::Error>> {
    // Copy one complete cached palette for this frame, including every overlay and notice.
    #[cfg(windows)]
    let configured_theme = view.configured_theme();
    #[cfg(windows)]
    let chrome = Chrome {
        theme: configured_theme.as_ref().unwrap_or(chrome.theme),
        ..chrome
    };
    #[cfg(windows)]
    surface.observe_client_viewport(view, terminal.get_frame().area());
    #[cfg(windows)]
    let mut notification_painted = false;
    let mut receipt = crate::terminal_session::NativeDrawReceipt::default();
    let mut hits = HitMapBuilder::new();
    #[cfg(windows)]
    let preview = dispatcher
        .divider_preview()
        .map(divider::Preview::Divider)
        .or_else(|| {
            dispatcher
                .sidebar_width_preview()
                .map(divider::Preview::Sidebar)
        });
    #[cfg(windows)]
    let graphics = terminal.backend().graphics_surface();
    #[cfg(windows)]
    let mut preview_painted = false;
    let drawn = terminal.draw(|frame| {
        #[cfg(windows)]
        {
            (receipt, preview_painted) = divider::render(
                frame,
                view,
                chrome,
                surface,
                graphics.as_ref(),
                preview,
                &mut hits,
            );
        }
        #[cfg(not(windows))]
        {
            receipt = render_terminal_shell(
                frame,
                view,
                chrome.locale,
                surface.view(),
                chrome.theme,
                &mut hits,
            );
        }
        #[cfg(windows)]
        if !view.machines.open && crate::notifications::available(view, surface.view()) {
            notification_painted =
                crate::notifications::render(frame, &view.notifications, chrome.theme);
        }
        #[cfg(windows)]
        render_machine_surface(frame, view, chrome, preview_painted, &hits);
        #[cfg(windows)]
        if let Some(graphics) = &graphics {
            graphics.finish_frame(
                receipt.graphics_draws(),
                preview_painted || view.machines.open || notification_painted,
            );
        }
    });
    if let Err(error) = drawn {
        // A failed draw never publishes: the previous map stays the
        // authority for the last frame that reached the screen whole.
        // The failure itself is fatal to the loop.
        return Err(LoopFailure::Terminal(error));
    }
    #[cfg(windows)]
    if notification_painted {
        // The card is intentionally noncapturing, so ordinary pane clicks
        // keep their existing policy. Its covered cells must not expose a
        // terminal link source or preserve an earlier link effect handoff.
        hits.suppress_link_sources();
        crate::notifications::sound(view.notifications.take_sound_after_draw());
    }
    dispatcher.publish_hit_map(hits.finish());
    #[cfg(windows)]
    if view.machines.open {
        // Covered native content cannot acknowledge successful visible presentation.
        surface.graphics_covered();
        return Ok(());
    }
    #[cfg(windows)]
    if notification_painted {
        receipt = receipt.with_graphics(Vec::new()).with_position(None);
    }
    let help_page = receipt.help_page();
    surface.after_close_draw(receipt);
    if let Some(page) = help_page {
        view.settle_help_page(page);
    }
    surface.after_draw();
    surface
        .deliver_terminal_effect(&mut || mode(ModeRequest::Bell))
        .map_err(LoopFailure::Input)?;
    Ok(())
}

#[cfg(windows)]
fn render_machine_surface(
    frame: &mut ratatui::Frame<'_>,
    view: &mut ClientView,
    chrome: Chrome<'_>,
    preview_painted: bool,
    hits: &crate::tui_pointer::HitMapBuilder,
) {
    // The preview's persistent status owns this tail until the gesture ends.
    if preview_painted || (view.copy_mode_active() && !view.machines.open) {
        view.machines.invalidate();
    } else {
        view.machines
            .render(frame, chrome.locale, chrome.theme, hits.machine_indicator());
    }
}

#[cfg(windows)]
fn synchronize_sidebar_visibility(view: &mut ClientView, dimensions: WindowDimensions) -> bool {
    let owned = view.machines.sidebar_navigation_active();
    let visible =
        crate::tui_layout::project_shell_areas(to_area(dimensions), view.sidebar_surface())
            .sidebar
            .is_some();
    view.machines.observe_agent_sidebar(visible);
    owned && !view.machines.sidebar_navigation_active()
}

fn synchronize_viewport<B: Backend + ResizableBackend>(
    terminal: &mut Terminal<B>,
    viewport: &impl ViewportSource,
    view: &mut ClientView,
    terminal_input: &mut (impl TerminalSurface + ?Sized),
    dimensions: &mut WindowDimensions,
    measurement: Option<crate::terminal_graphics::Measurement>,
) -> Result<bool, LoopFailure<B::Error>> {
    let Some(next) = viewport
        .window_dimensions()
        .map_err(LoopFailure::Viewport)?
    else {
        #[cfg(windows)]
        return Ok(synchronize_sidebar_visibility(view, *dimensions));
        #[cfg(not(windows))]
        return Ok(false);
    };
    let next = measurement.map_or(next, |measurement| measurement.apply(next));
    let changed = next != *dimensions;
    if changed {
        #[cfg(windows)]
        {
            view.machines.invalidate();
            view.machines.cancel_sidebar_scroll();
        }
        terminal_input.invalidate_close_display();
        terminal_input.invalidate_rename_display();
        terminal_input.invalidate_setup_display();
        #[cfg(windows)]
        terminal_input.invalidate_worktree_display();
        terminal.backend_mut().set_window_dimensions(next);
        terminal
            .resize(to_area(next))
            .map_err(LoopFailure::Terminal)?;
        view.resize(next.columns);
        *dimensions = next;
    }
    #[cfg(windows)]
    let changed = synchronize_sidebar_visibility(view, *dimensions) || changed;
    let session_view = terminal_input.view();
    match session_view {
        Some(crate::terminal_session::TerminalSessionView::Native(native)) => {
            let panes =
                pane_geometries(*dimensions, view, &native).map_err(LoopFailure::Viewport)?;
            #[cfg(windows)]
            let split = native
                .active_position()
                .and_then(|p| p.checked_sub(1))
                .and_then(|i| native.tab(i))
                .is_some_and(|tab| tab.panel_count() > 1 && !native.zoom_active());
            terminal_input.observe_panes(&panes);
            #[cfg(windows)]
            {
                let areas = crate::tui_layout::project_shell_areas(
                    to_area(*dimensions),
                    view.sidebar_surface(),
                );
                let area = crate::tui_layout::native_terminal_area(
                    areas.panel,
                    areas.tier.shows_panel_frame(),
                    split,
                );
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
    let content = crate::tui_layout::native_terminal_area(
        areas.panel,
        framed,
        tab.panel_count() > 1 && !native.zoom_active(),
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
    let Some(tab) = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))
    else {
        return Ok(Vec::new());
    };
    let content = tui_layout::native_terminal_area(areas.panel, framed, tab.panel_count() > 1);
    let zoomed_content = tui_layout::native_terminal_area(areas.panel, framed, false);
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
    #[cfg(windows)]
    fn observe_client_viewport(&mut self, _view: &ClientView, _viewport: Rect) {}
    #[cfg(windows)]
    fn popup_surface(&mut self, _area: Rect, _pixels: (u16, u16)) {}

    #[cfg(windows)]
    fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        None
    }
    #[cfg(windows)]
    fn suspend_machine(&mut self) -> Option<bool> {
        None
    }
    #[cfg(windows)]
    fn resume_machine(&mut self, _focus: Option<bool>) {}

    #[cfg(windows)]
    fn reusable_machine(&self) -> bool {
        true
    }

    #[cfg(windows)]
    fn configure_agent_notifications(
        &mut self,
        _revision: u64,
        _settings: &orchbee_local_site_runtime::WindowsNotificationSettings,
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
    fn after_draw(&mut self) {}
    fn deliver_terminal_effect(
        &mut self,
        _emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        Ok(())
    }
    fn after_close_draw(&mut self, _receipt: crate::terminal_session::NativeDrawReceipt) {}
    #[cfg(windows)]
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
    fn view(&self) -> Option<crate::terminal_session::TerminalSessionView<'_>> {
        None
    }
}
impl TerminalSurface for UnavailableTerminalInputPort {}
impl TerminalSurface for crate::terminal_session::TerminalSession {
    #[cfg(windows)]
    fn observe_client_viewport(&mut self, view: &ClientView, viewport: Rect) {
        self.observe_client_viewport(view, viewport);
    }
    #[cfg(windows)]
    fn configure_agent_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_site_runtime::WindowsNotificationSettings,
    ) {
        self.configure_agent_notifications(revision, settings);
    }

    #[cfg(windows)]
    fn popup_surface(&mut self, area: Rect, pixels: (u16, u16)) {
        self.observe_popup_surface(area, pixels);
    }

    #[cfg(windows)]
    fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        self.machine_source()
    }
    #[cfg(windows)]
    fn suspend_machine(&mut self) -> Option<bool> {
        self.suspend_machine()
    }
    #[cfg(windows)]
    fn resume_machine(&mut self, focus: Option<bool>) {
        self.resume_machine(focus);
    }
    #[cfg(windows)]
    fn reusable_machine(&self) -> bool {
        // A canonical placeholder is a usable workspace, despite having no
        // attached terminal. Preserve it when revisiting the cached machine.
        if let crate::terminal_session::TerminalSessionView::Native(view) = self.view()
            && view.retirement().is_none()
            && view.phase()
                == crate::terminal_session::NativeDisplayPhase::SelectedWithoutAttachment
        {
            return true;
        }
        !matches!(
            self.status(),
            crate::terminal_session::TerminalStatus::Unavailable
                | crate::terminal_session::TerminalStatus::OutcomeUnknown
        )
    }
    #[cfg(windows)]
    fn client_control_tick(
        &mut self,
        view: &mut ClientView,
        viewport: Rect,
        locale: Locale,
    ) -> bool {
        self.client_control_tick(view, viewport, locale)
    }
    fn tick(&mut self) -> bool {
        self.tick()
    }
    fn after_draw(&mut self) {
        self.after_draw()
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
    fn after_close_draw(&mut self, receipt: crate::terminal_session::NativeDrawReceipt) {
        self.after_close_draw(receipt)
    }
    #[cfg(windows)]
    fn graphics_covered(&mut self) {
        self.graphics_covered();
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

#[cfg(all(test, unix))]
mod connected_tests;
