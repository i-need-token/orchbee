use std::{
    fmt, io,
    io::Write as _,
    marker::PhantomData,
    panic,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    thread::{self, ThreadId},
};

use crossterm::{
    cursor::{Hide, Show},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(unix)]
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcsetattr};

use crate::terminal_device::InheritedTerminal;

mod command;
pub(crate) use command::write_ansi_command;

const RESTORATION_ATTEMPTS_PER_MODE: u8 = 2;

pub(crate) const SYNCHRONIZED_UPDATE_BEGIN: &[u8] = b"\x1b[?2026h";
pub(crate) const SYNCHRONIZED_UPDATE_END: &[u8] = b"\x1b[?2026l";
fn write_bell(output: &mut impl io::Write) -> io::Result<()> {
    output.write_all(b"\x07")?;
    output.flush()
}
#[cfg(windows)]
pub(crate) mod graphics;

struct FocusReporting(bool);

struct GraphicsQuery(crate::terminal_graphics::Query);
impl crossterm::Command for GraphicsQuery {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(&self.0.sequence())
    }
}

impl crossterm::Command for FocusReporting {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(if self.0 { "\x1b[?1004h" } else { "\x1b[?1004l" })
    }
}

struct BracketedPaste(bool);

impl crossterm::Command for BracketedPaste {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(if self.0 { "\x1b[?2004h" } else { "\x1b[?2004l" })
    }
}

/// The one outer-terminal tracking level selected inside the pointer lease.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PointerTrackingLevel {
    ButtonOnly,
    AnyMotion,
    #[cfg(windows)]
    MenuMotion,
}

/// Capture selects one tracking policy and SGR cells. Global levels disable
/// the other level first. The scoped menu policy retains button reporting
/// when a host ignores its any-motion request. Release clears every owned level.
struct PointerCapture {
    enabled: bool,
    level: PointerTrackingLevel,
}

impl crossterm::Command for PointerCapture {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        let sequence = match (self.enabled, self.level) {
            (true, PointerTrackingLevel::ButtonOnly) => "\x1b[?1003l\x1b[?1002h\x1b[?1006h",
            (true, PointerTrackingLevel::AnyMotion) => "\x1b[?1002l\x1b[?1003h\x1b[?1006h",
            #[cfg(windows)]
            (true, PointerTrackingLevel::MenuMotion) => "\x1b[?1002h\x1b[?1003h\x1b[?1006h",
            (false, _) => "\x1b[?1003l\x1b[?1006l\x1b[?1002l",
        };
        output.write_str(sequence)
    }
}

struct PointerTrackingTransition {
    from: PointerTrackingLevel,
    to: PointerTrackingLevel,
}

impl crossterm::Command for PointerTrackingTransition {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        let sequence = match (self.from, self.to) {
            (PointerTrackingLevel::ButtonOnly, PointerTrackingLevel::AnyMotion) => {
                "\x1b[?1002l\x1b[?1003h"
            }
            (PointerTrackingLevel::AnyMotion, PointerTrackingLevel::ButtonOnly) => {
                "\x1b[?1003l\x1b[?1002h"
            }
            #[cfg(windows)]
            (PointerTrackingLevel::MenuMotion, PointerTrackingLevel::ButtonOnly) => {
                "\x1b[?1003l\x1b[?1002h"
            }
            #[cfg(windows)]
            (
                PointerTrackingLevel::ButtonOnly | PointerTrackingLevel::AnyMotion,
                PointerTrackingLevel::MenuMotion,
            ) => "\x1b[?1002h\x1b[?1003h",
            #[cfg(windows)]
            (PointerTrackingLevel::MenuMotion, PointerTrackingLevel::AnyMotion) => {
                "\x1b[?1002l\x1b[?1003h"
            }
            _ => "",
        };
        output.write_str(sequence)
    }
}

struct SynchronizedUpdateEnd;

impl crossterm::Command for SynchronizedUpdateEnd {
    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }

    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str("\x1b[?2026l")
    }
}

pub(crate) trait TerminalModes: Send + Sync + 'static {
    fn ring_bell(&self) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn cleanup_graphics(&self, _sequence: &str) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn query_graphics(&self, _query: crate::terminal_graphics::Query) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn query_pointer_admission(
        &self,
        _query: crate::terminal_pointer_admission::Query,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn notify_terminal(
        &self,
        _message: &orchbee_workbench_protocol::Notification,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn set_window_title(&self, _title: &str) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    #[cfg(windows)]
    fn restore_window_title(&self) -> io::Result<()> {
        Ok(())
    }

    fn disable_bracketed_paste(&self) -> io::Result<()>;
    fn enable_bracketed_paste(&self) -> io::Result<()>;
    fn disable_focus_reporting(&self) -> io::Result<()>;
    fn enable_focus_reporting(&self) -> io::Result<()>;
    fn disable_pointer_capture(&self) -> io::Result<()>;
    fn enable_pointer_capture(&self, level: PointerTrackingLevel) -> io::Result<()>;
    fn set_pointer_tracking(
        &self,
        _from: PointerTrackingLevel,
        _to: PointerTrackingLevel,
    ) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn disable_raw_mode(&self) -> io::Result<()>;
    fn enable_raw_mode(&self) -> io::Result<()>;
    fn end_synchronized_update(&self) -> io::Result<()>;
    fn enter_alternate_screen(&self) -> io::Result<()>;
    fn hide_cursor(&self) -> io::Result<()>;
    fn leave_alternate_screen(&self) -> io::Result<()>;
    fn show_cursor(&self) -> io::Result<()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RestorationStep {
    #[cfg(windows)]
    CleanupGraphics,
    #[cfg(windows)]
    RestoreWindowTitle,
    DisableBracketedPaste,
    DisablePointerCapture,
    DisableFocusReporting,
    DisableRaw,
    EndSynchronizedUpdate,
    LeaveAlternate,
    ShowCursor,
}

impl fmt::Display for RestorationStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(windows)]
            Self::CleanupGraphics => formatter.write_str("clean up owned graphics"),
            #[cfg(windows)]
            Self::RestoreWindowTitle => formatter.write_str("restore window title"),
            Self::DisableBracketedPaste => formatter.write_str("disable bracketed paste"),
            Self::DisableFocusReporting => formatter.write_str("disable focus reporting"),
            Self::DisablePointerCapture => formatter.write_str("disable pointer capture"),
            Self::DisableRaw => formatter.write_str("disable raw mode"),
            Self::EndSynchronizedUpdate => formatter.write_str("end synchronized update"),
            Self::LeaveAlternate => formatter.write_str("leave alternate screen"),
            Self::ShowCursor => formatter.write_str("show cursor"),
        }
    }
}

#[derive(Debug)]
struct RestorationFailure {
    error: io::Error,
    step: RestorationStep,
}

#[derive(Debug)]
pub(crate) struct TerminalRestoreError {
    failures: Vec<RestorationFailure>,
}

impl TerminalRestoreError {
    #[cfg(test)]
    fn failed_steps(&self) -> Vec<RestorationStep> {
        self.failures.iter().map(|failure| failure.step).collect()
    }
}

impl fmt::Display for TerminalRestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "terminal restoration exhausted its bounded retry policy for {} mode(s)",
            self.failures.len()
        )?;
        for failure in &self.failures {
            write!(formatter, "; {}: {}", failure.step, failure.error)?;
        }
        Ok(())
    }
}

impl std::error::Error for TerminalRestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.failures
            .first()
            .map(|failure| &failure.error as &(dyn std::error::Error + 'static))
    }
}

#[derive(Debug)]
struct ModeLease {
    restoration_attempts: AtomicU8,
    restoration_required: AtomicBool,
}

impl ModeLease {
    const fn new() -> Self {
        Self {
            restoration_attempts: AtomicU8::new(0),
            restoration_required: AtomicBool::new(false),
        }
    }

    fn mark_operation_started(&self) {
        self.restoration_attempts
            .store(RESTORATION_ATTEMPTS_PER_MODE, Ordering::Release);
        self.restoration_required.store(true, Ordering::Release);
    }

    fn claim_restoration_attempt(&self) -> bool {
        self.restoration_attempts
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
    }

    fn clear_after_success(&self) {
        self.restoration_required.store(false, Ordering::Release);
    }

    fn restoration_required(&self) -> bool {
        self.restoration_required.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub(crate) struct SynchronizedUpdateLease {
    #[cfg(windows)]
    graphics: graphics::ImageLease,
    frame_output_blocked: AtomicBool,
    obligation: ModeLease,
}

impl SynchronizedUpdateLease {
    pub(crate) const fn new() -> Self {
        Self {
            #[cfg(windows)]
            graphics: graphics::ImageLease::new(),
            frame_output_blocked: AtomicBool::new(false),
            obligation: ModeLease::new(),
        }
    }

    pub(crate) fn open_frame(&self) {
        self.obligation.mark_operation_started();
    }
    #[cfg(windows)]
    pub(crate) fn graphics(&self) -> &graphics::ImageLease {
        &self.graphics
    }

    pub(crate) fn frame_open(&self) -> bool {
        self.obligation.restoration_required()
    }

    pub(crate) fn confirm_frame_closed(&self) {
        self.obligation.clear_after_success();
    }

    pub(crate) fn frame_output_blocked(&self) -> bool {
        self.frame_output_blocked.load(Ordering::Acquire)
    }

    pub(crate) fn block_frame_output(&self) {
        self.frame_output_blocked.store(true, Ordering::Release);
    }
}

#[derive(Debug)]
struct TerminalModeState {
    #[cfg(windows)]
    window_title: ModeLease,
    bracketed_paste: ModeLease,
    pointer_capture: ModeLease,
    focus_reporting: ModeLease,
    alternate_screen: ModeLease,
    cursor: ModeLease,
    raw_mode: ModeLease,
    synchronized_update: Arc<SynchronizedUpdateLease>,
}

impl TerminalModeState {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            window_title: ModeLease::new(),
            bracketed_paste: ModeLease::new(),
            pointer_capture: ModeLease::new(),
            focus_reporting: ModeLease::new(),
            alternate_screen: ModeLease::new(),
            cursor: ModeLease::new(),
            raw_mode: ModeLease::new(),
            synchronized_update: Arc::new(SynchronizedUpdateLease::new()),
        }
    }
}

pub(crate) struct TerminalModeGuard<C: TerminalModes> {
    bracketed_paste_enabled: bool,
    pointer_capture_enabled: bool,
    pointer_tracking: PointerTrackingState,
    focus_reporting_enabled: bool,
    control: Arc<C>,
    _owner_thread_only: PhantomData<Rc<()>>,
    _panic_registration: Option<PanicRestorationRegistration>,
    state: Arc<TerminalModeState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PointerTrackingState {
    Released,
    Active(PointerTrackingLevel),
    /// A capture or level write may have emitted a prefix. Only the complete
    /// release sequence may settle the lease after this point.
    TransitionUnknown,
}

impl<C: TerminalModes> TerminalModeGuard<C> {
    pub(crate) fn enter(control: C) -> io::Result<Self> {
        Self::enter_inner(control, true)
    }

    #[cfg(test)]
    fn enter_without_panic_hook(control: C) -> io::Result<Self> {
        Self::enter_inner(control, false)
    }

    fn enter_inner(control: C, protect_from_panics: bool) -> io::Result<Self> {
        let control = Arc::new(control);
        let state = Arc::new(TerminalModeState::new());
        let panic_registration = if protect_from_panics {
            let control_for_panic = Arc::clone(&control);
            let state_for_panic = Arc::clone(&state);
            Some(register_panic_restoration(Arc::new(move || {
                let _ = restore_modes(control_for_panic.as_ref(), &state_for_panic);
            }))?)
        } else {
            None
        };
        let mut guard = Self {
            bracketed_paste_enabled: false,
            pointer_capture_enabled: false,
            pointer_tracking: PointerTrackingState::Released,
            focus_reporting_enabled: false,
            _owner_thread_only: PhantomData,
            _panic_registration: panic_registration,
            control,
            state,
        };

        guard.state.raw_mode.mark_operation_started();
        if let Err(error) = guard.control.enable_raw_mode() {
            return Err(guard.initialization_failure("enable raw mode", error));
        }

        guard.state.alternate_screen.mark_operation_started();
        if let Err(error) = guard.control.enter_alternate_screen() {
            return Err(guard.initialization_failure("enter alternate screen", error));
        }

        guard.state.cursor.mark_operation_started();
        if let Err(error) = guard.control.hide_cursor() {
            return Err(guard.initialization_failure("hide cursor", error));
        }

        Ok(guard)
    }

    /// Stops buffered frame output before its writer is dropped after a draw
    /// failure. The lifecycle still owns any pending synchronized-update end
    /// and restores it through the separate control channel.
    pub(crate) fn discard_frame_output(&self) {
        self.state.synchronized_update.block_frame_output();
    }

    pub(crate) fn restore(&mut self) -> Result<(), TerminalRestoreError> {
        self.bracketed_paste_enabled = false;
        self.pointer_capture_enabled = false;
        self.pointer_tracking = PointerTrackingState::TransitionUnknown;
        self.focus_reporting_enabled = false;
        let result = restore_modes(self.control.as_ref(), &self.state);
        if result.is_ok() {
            self.pointer_tracking = PointerTrackingState::Released;
        }
        result
    }

    pub(crate) fn synchronized_update_lease(&self) -> Arc<SynchronizedUpdateLease> {
        Arc::clone(&self.state.synchronized_update)
    }

    /// Emits one irreversible bell only outside a successfully closed frame.
    /// A write failure fences subsequent frame output; this call never retries.
    pub(crate) fn ring_bell(&self) -> io::Result<()> {
        let lease = &self.state.synchronized_update;
        if !self.state.raw_mode.restoration_required() || lease.frame_output_blocked() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if lease.frame_open() {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let result = self.control.ring_bell();
        if result.is_err() {
            lease.block_frame_output();
        }
        result
    }

    pub(crate) fn query_graphics(&self, query: crate::terminal_graphics::Query) -> io::Result<()> {
        if !self.state.raw_mode.restoration_required()
            || self.state.synchronized_update.frame_output_blocked()
        {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.control.query_graphics(query)
    }

    pub(crate) fn query_pointer_admission(
        &self,
        query: crate::terminal_pointer_admission::Query,
    ) -> io::Result<()> {
        if !self.state.raw_mode.restoration_required()
            || self.state.synchronized_update.frame_output_blocked()
        {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.control.query_pointer_admission(query)
    }

    #[cfg(windows)]
    pub(crate) fn notify_terminal(
        &mut self,
        message: &orchbee_workbench_protocol::Notification,
    ) -> io::Result<()> {
        if self.state.synchronized_update.frame_output_blocked() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.control.notify_terminal(message)
    }
    #[cfg(windows)]
    pub(crate) fn set_window_title(&mut self, title: &str) -> io::Result<()> {
        if self.state.synchronized_update.frame_output_blocked() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.state.window_title.mark_operation_started();
        self.control.set_window_title(title)
    }
    pub(crate) fn set_bracketed_paste(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            if self.bracketed_paste_enabled {
                return Ok(());
            }
            if self.state.bracketed_paste.restoration_required() {
                return Err(io::Error::other(
                    "bracketed paste still requires restoration",
                ));
            }
            self.state.bracketed_paste.mark_operation_started();
            self.control.enable_bracketed_paste()?;
            self.bracketed_paste_enabled = true;
            return Ok(());
        }
        self.bracketed_paste_enabled = false;
        let mut failures = Vec::new();
        restore_mode(
            &self.state.bracketed_paste,
            RestorationStep::DisableBracketedPaste,
            || self.control.disable_bracketed_paste(),
            &mut failures,
        );
        if failures.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(TerminalRestoreError { failures }))
        }
    }

    /// Arms or releases host pointer reporting (`?1002` + `?1006`). Same
    /// lease discipline as bracketed paste: enabling marks the
    /// restoration obligation before the write; releasing runs the
    /// bounded restore and only clears the flag when it succeeds.
    pub(crate) fn set_pointer_capture(&mut self, enabled: bool) -> io::Result<()> {
        self.set_pointer_capture_level(enabled.then_some(PointerTrackingLevel::ButtonOnly))
    }

    /// Arms the requested tracking policy, or releases every owned pointer
    /// mode. Callers retain the endpoint capability decision; this owner only
    /// publishes a level after the complete command succeeded.
    pub(crate) fn set_pointer_capture_level(
        &mut self,
        level: Option<PointerTrackingLevel>,
    ) -> io::Result<()> {
        if let Some(level) = level {
            if self.pointer_capture_enabled {
                return self.set_pointer_tracking(level);
            }
            if self.state.pointer_capture.restoration_required() {
                return Err(io::Error::other(
                    "pointer capture still requires restoration",
                ));
            }
            self.state.pointer_capture.mark_operation_started();
            self.pointer_tracking = PointerTrackingState::TransitionUnknown;
            self.control.enable_pointer_capture(level)?;
            self.pointer_capture_enabled = true;
            self.pointer_tracking = PointerTrackingState::Active(level);
            return Ok(());
        }
        self.pointer_capture_enabled = false;
        self.pointer_tracking = PointerTrackingState::TransitionUnknown;
        let mut failures = Vec::new();
        restore_mode(
            &self.state.pointer_capture,
            RestorationStep::DisablePointerCapture,
            || self.control.disable_pointer_capture(),
            &mut failures,
        );
        if failures.is_empty() {
            self.pointer_tracking = PointerTrackingState::Released;
            Ok(())
        } else {
            Err(io::Error::other(TerminalRestoreError { failures }))
        }
    }

    /// Switches inside an existing lease. Global levels disable the old level first;
    /// menu motion retains button fallback. An unknown partial write fences later
    /// runtime switches until the full release sequence settles cleanup.
    pub(crate) fn set_pointer_tracking(&mut self, level: PointerTrackingLevel) -> io::Result<()> {
        if !self.pointer_capture_enabled {
            return Err(io::Error::other("pointer tracking requires active capture"));
        }
        let current = match self.pointer_tracking {
            PointerTrackingState::Active(current) => current,
            PointerTrackingState::Released => {
                return Err(io::Error::other("pointer capture is released"));
            }
            PointerTrackingState::TransitionUnknown => {
                return Err(io::Error::other(
                    "pointer tracking still requires restoration",
                ));
            }
        };
        if current == level {
            return Ok(());
        }
        self.pointer_tracking = PointerTrackingState::TransitionUnknown;
        self.control.set_pointer_tracking(current, level)?;
        self.pointer_tracking = PointerTrackingState::Active(level);
        Ok(())
    }

    pub(crate) fn set_focus_reporting(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            if self.focus_reporting_enabled {
                return Ok(());
            }
            if self.state.focus_reporting.restoration_required() {
                return Err(io::Error::other(
                    "focus reporting still requires restoration",
                ));
            }
            self.state.focus_reporting.mark_operation_started();
            self.control.enable_focus_reporting()?;
            self.focus_reporting_enabled = true;
            return Ok(());
        }
        self.focus_reporting_enabled = false;
        let mut failures = Vec::new();
        restore_mode(
            &self.state.focus_reporting,
            RestorationStep::DisableFocusReporting,
            || self.control.disable_focus_reporting(),
            &mut failures,
        );
        if failures.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(TerminalRestoreError { failures }))
        }
    }

    fn initialization_failure(&mut self, step: &str, error: io::Error) -> io::Error {
        let kind = error.kind();
        match self.restore() {
            Ok(()) => io::Error::new(kind, format!("could not {step}: {error}")),
            Err(restoration) => io::Error::new(
                kind,
                format!("could not {step}: {error}; cleanup also failed: {restoration}"),
            ),
        }
    }
}

impl<C: TerminalModes> Drop for TerminalModeGuard<C> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let _ = self.restore();
        }
    }
}

fn restore_modes<C: TerminalModes>(
    control: &C,
    state: &TerminalModeState,
) -> Result<(), TerminalRestoreError> {
    let mut failures = Vec::with_capacity(5);
    state.synchronized_update.block_frame_output();
    #[cfg(windows)]
    restore_mode(
        &state.synchronized_update.graphics.obligation,
        RestorationStep::CleanupGraphics,
        || {
            let sequence = state.synchronized_update.graphics.cleanup_sequence();
            control.cleanup_graphics(&sequence)?;
            state.synchronized_update.graphics.restored();
            Ok(())
        },
        &mut failures,
    );
    restore_mode(
        &state.synchronized_update.obligation,
        RestorationStep::EndSynchronizedUpdate,
        || control.end_synchronized_update(),
        &mut failures,
    );
    #[cfg(windows)]
    restore_mode(
        &state.window_title,
        RestorationStep::RestoreWindowTitle,
        || control.restore_window_title(),
        &mut failures,
    );
    restore_mode(
        &state.bracketed_paste,
        RestorationStep::DisableBracketedPaste,
        || control.disable_bracketed_paste(),
        &mut failures,
    );
    restore_mode(
        &state.pointer_capture,
        RestorationStep::DisablePointerCapture,
        || control.disable_pointer_capture(),
        &mut failures,
    );
    restore_mode(
        &state.focus_reporting,
        RestorationStep::DisableFocusReporting,
        || control.disable_focus_reporting(),
        &mut failures,
    );
    restore_mode(
        &state.cursor,
        RestorationStep::ShowCursor,
        || control.show_cursor(),
        &mut failures,
    );
    restore_mode(
        &state.alternate_screen,
        RestorationStep::LeaveAlternate,
        || control.leave_alternate_screen(),
        &mut failures,
    );
    restore_mode(
        &state.raw_mode,
        RestorationStep::DisableRaw,
        || control.disable_raw_mode(),
        &mut failures,
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(TerminalRestoreError { failures })
    }
}

fn restore_mode(
    lease: &ModeLease,
    step: RestorationStep,
    mut operation: impl FnMut() -> io::Result<()>,
    failures: &mut Vec<RestorationFailure>,
) {
    if !lease.restoration_required() {
        return;
    }

    let mut last_error = None;
    while lease.restoration_required() && lease.claim_restoration_attempt() {
        match operation() {
            Ok(()) => lease.clear_after_success(),
            Err(error) => last_error = Some(error),
        }
    }
    if lease.restoration_required() {
        let error = match last_error {
            Some(error) => error,
            None => io::Error::other("terminal restoration retry budget was already exhausted"),
        };
        failures.push(RestorationFailure { error, step });
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InheritedTerminalModes {
    #[cfg(unix)]
    original_termios: Arc<Mutex<Option<Termios>>>,
    #[cfg(windows)]
    original_console: Arc<Mutex<Option<SavedConsoleModes>>>,
    #[cfg(windows)]
    original_title: Arc<Mutex<Option<Arc<str>>>>,
    #[cfg(windows)]
    alternate_sequence_started: Arc<AtomicBool>,
    #[cfg(windows)]
    output_vt_enabled: Arc<AtomicBool>,
    terminal: InheritedTerminal,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
struct SavedConsoleMode {
    original: u32,
    restoration_required: bool,
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug)]
struct SavedConsoleModes {
    input: SavedConsoleMode,
    output: SavedConsoleMode,
}

#[cfg(windows)]
fn restoration_state<T>(state: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A panic must not make the saved restoration values inaccessible to the
    // panic hook. These state updates replace or copy complete values only.
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(windows)]
fn capture_and_enable_windows_raw_mode(
    saved_modes: &Mutex<Option<SavedConsoleModes>>,
    read_input_mode: impl FnOnce() -> io::Result<u32>,
    read_output_mode: impl FnOnce() -> io::Result<u32>,
    set_input_mode: impl FnOnce(u32) -> io::Result<()>,
) -> io::Result<()> {
    // Reads cannot mutate the host. Complete both before publishing a debt so
    // a failed inspection never invents a mode that needs restoration.
    let input = read_input_mode()?;
    let output = read_output_mode()?;
    {
        let mut saved = restoration_state(saved_modes);
        if saved.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        // SetConsoleMode may change state before returning an error or panic.
        // Publish the exact original word before crossing that boundary.
        *saved = Some(SavedConsoleModes {
            input: SavedConsoleMode {
                original: input,
                restoration_required: true,
            },
            output: SavedConsoleMode {
                original: output,
                restoration_required: false,
            },
        });
    }

    // Keep VT input so ConPTY retains bracketed-paste delimiters. Our one
    // record reader preserves VK=0 control characters instead of feeding
    // them through a key-layout translator. Pointer capture owns the mouse
    // bit separately; raw admission must not capture with --mouse off.
    set_input_mode((input | 0x0288) & !0x0057)
}

#[cfg(windows)]
fn write_admitted_windows_command<W: io::Write>(
    output_vt_enabled: &AtomicBool,
    acquire_output: impl FnOnce() -> io::Result<W>,
    command: &impl crossterm::Command,
) -> io::Result<()> {
    if !output_vt_enabled.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "VT output processing is not active for this terminal session",
        ));
    }
    let mut output = acquire_output()?;
    write_ansi_command(&mut output, command)?;
    output.flush()
}

#[cfg(windows)]
fn admit_windows_vt_and_enter_alternate<W: io::Write>(
    saved_modes: &Mutex<Option<SavedConsoleModes>>,
    output_vt_enabled: &AtomicBool,
    alternate_sequence_started: &AtomicBool,
    set_output_mode: impl FnOnce(u32) -> io::Result<()>,
    acquire_output: impl FnOnce() -> io::Result<W>,
) -> io::Result<()> {
    let admitted_mode = {
        let mut saved = restoration_state(saved_modes);
        let saved = saved.as_mut().ok_or_else(|| {
            io::Error::other("console modes were not captured before VT admission")
        })?;
        // SetConsoleMode may change state before reporting failure. Arm the
        // exact output-word debt while the state lock is short-held, then
        // release the lock before calling into the host.
        saved.output.restoration_required = true;
        saved.output.original | 0x0005
    };
    set_output_mode(admitted_mode)?;
    // ANSI is admitted only after the native output-mode call has completed.
    output_vt_enabled.store(true, Ordering::Release);
    let mut output = acquire_output()?;
    // From this point a write or flush fault can follow an emitted prefix, so
    // cleanup conservatively owes a leave while VT remains admitted.
    alternate_sequence_started.store(true, Ordering::Release);
    write_ansi_command(&mut output, &EnterAlternateScreen)?;
    output.flush()
}

#[cfg(windows)]
fn leave_windows_alternate<W: io::Write>(
    output_vt_enabled: &AtomicBool,
    alternate_sequence_started: &AtomicBool,
    acquire_output: impl FnOnce() -> io::Result<W>,
) -> io::Result<()> {
    if !output_vt_enabled.load(Ordering::Acquire)
        || !alternate_sequence_started.load(Ordering::Acquire)
    {
        return Ok(());
    }
    write_admitted_windows_command(output_vt_enabled, acquire_output, &LeaveAlternateScreen)?;
    alternate_sequence_started.store(false, Ordering::Release);
    Ok(())
}

#[cfg(windows)]
fn restore_windows_console_modes(
    saved_modes: &Mutex<Option<SavedConsoleModes>>,
    output_vt_enabled: &AtomicBool,
    mut set_output_mode: impl FnMut(u32) -> io::Result<()>,
    mut set_input_mode: impl FnMut(u32) -> io::Result<()>,
) -> io::Result<()> {
    let pending = {
        let saved = *restoration_state(saved_modes);
        saved.map(|saved| {
            (
                saved
                    .output
                    .restoration_required
                    .then_some(saved.output.original),
                saved
                    .input
                    .restoration_required
                    .then_some(saved.input.original),
            )
        })
    };
    let Some((output, input)) = pending else {
        return Ok(());
    };

    // Settle each side independently. A failure on one side cannot suppress
    // the other attempt, and a later outer retry sees only still-pending debt.
    let mut output_error = None;
    if let Some(mode) = output {
        match set_output_mode(mode) {
            Ok(()) => {
                if let Some(saved) = restoration_state(saved_modes).as_mut() {
                    saved.output.restoration_required = false;
                }
                output_vt_enabled.store(false, Ordering::Release);
            }
            Err(error) => output_error = Some(error),
        }
    }
    let mut input_error = None;
    if let Some(mode) = input {
        match set_input_mode(mode) {
            Ok(()) => {
                if let Some(saved) = restoration_state(saved_modes).as_mut() {
                    saved.input.restoration_required = false;
                }
            }
            Err(error) => input_error = Some(error),
        }
    }

    match (output_error, input_error) {
        (None, None) => Ok(()),
        (Some(error), None) | (None, Some(error)) => Err(error),
        (Some(output), Some(input)) => {
            let kind = output.kind();
            Err(io::Error::new(
                kind,
                format!(
                    "could not restore output console mode: {output}; could not restore input console mode: {input}"
                ),
            ))
        }
    }
}

impl InheritedTerminalModes {
    pub(crate) fn new(terminal: InheritedTerminal) -> Self {
        Self {
            #[cfg(unix)]
            original_termios: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            original_console: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            original_title: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            alternate_sequence_started: Arc::new(AtomicBool::new(false)),
            #[cfg(windows)]
            output_vt_enabled: Arc::new(AtomicBool::new(false)),
            terminal,
        }
    }

    fn write_command(&self, command: impl crossterm::Command) -> io::Result<()> {
        #[cfg(windows)]
        {
            write_admitted_windows_command(
                &self.output_vt_enabled,
                || self.terminal.output(),
                &command,
            )
        }
        #[cfg(unix)]
        {
            let mut output = self.terminal.output()?;
            write_ansi_command(&mut output, &command)?;
            output.flush()
        }
    }
}

impl TerminalModes for InheritedTerminalModes {
    fn ring_bell(&self) -> io::Result<()> {
        write_bell(&mut self.terminal.output()?)
    }
    #[cfg(windows)]
    fn cleanup_graphics(&self, sequence: &str) -> io::Result<()> {
        let mut output = self.terminal.output()?;
        output.write_all(sequence.as_bytes())?;
        output.flush()
    }
    fn query_graphics(&self, query: crate::terminal_graphics::Query) -> io::Result<()> {
        self.write_command(GraphicsQuery(query))
    }
    fn query_pointer_admission(
        &self,
        query: crate::terminal_pointer_admission::Query,
    ) -> io::Result<()> {
        self.write_command(query)
    }
    #[cfg(windows)]
    fn notify_terminal(
        &self,
        message: &orchbee_workbench_protocol::Notification,
    ) -> io::Result<()> {
        use std::io::Write;
        let bytes = crate::notifications::terminal::sequence(message)?;
        let mut output = self.terminal.output()?;
        output.write_all(&bytes)?;
        output.flush()
    }
    #[cfg(windows)]
    fn set_window_title(&self, title: &str) -> io::Result<()> {
        let capture_required = restoration_state(&self.original_title).is_none();
        if capture_required {
            winsafe::SetLastError(winsafe::co::ERROR::SUCCESS);
            let captured = Arc::<str>::from(captured_window_title(
                win32console::console::WinConsole::get_title(),
            )?);
            let mut original = restoration_state(&self.original_title);
            if original.is_none() {
                *original = Some(captured);
            }
        }
        win32console::console::WinConsole::set_title(title)
    }
    #[cfg(windows)]
    fn restore_window_title(&self) -> io::Result<()> {
        let original = restoration_state(&self.original_title).clone();
        match original {
            Some(title) => win32console::console::WinConsole::set_title(&title),
            None => Ok(()),
        }
    }

    fn disable_focus_reporting(&self) -> io::Result<()> {
        self.write_command(FocusReporting(false))
    }

    fn enable_focus_reporting(&self) -> io::Result<()> {
        self.write_command(FocusReporting(true))
    }

    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.write_command(BracketedPaste(false))
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.write_command(BracketedPaste(true))
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        #[cfg(windows)]
        self.terminal
            .set_input_mode(self.terminal.input_mode()? & !0x0010)?;
        self.write_command(PointerCapture {
            enabled: false,
            level: PointerTrackingLevel::ButtonOnly,
        })
    }

    fn enable_pointer_capture(&self, level: PointerTrackingLevel) -> io::Result<()> {
        #[cfg(windows)]
        self.terminal
            .set_input_mode(self.terminal.input_mode()? | 0x0010)?;
        self.write_command(PointerCapture {
            enabled: true,
            level,
        })
    }

    fn set_pointer_tracking(
        &self,
        from: PointerTrackingLevel,
        to: PointerTrackingLevel,
    ) -> io::Result<()> {
        self.write_command(PointerTrackingTransition { from, to })
    }

    #[cfg(unix)]
    fn disable_raw_mode(&self) -> io::Result<()> {
        let original = self
            .original_termios
            .lock()
            .map_err(|_| io::Error::other("raw-mode state lock was poisoned"))?
            .clone();
        let Some(original) = original else {
            return Ok(());
        };
        tcsetattr(
            self.terminal.input().as_ref(),
            OptionalActions::Now,
            &original,
        )
        .map_err(io::Error::from)?;
        self.original_termios
            .lock()
            .map_err(|_| io::Error::other("raw-mode state lock was poisoned"))?
            .take();
        Ok(())
    }

    #[cfg(unix)]
    fn enable_raw_mode(&self) -> io::Result<()> {
        let original = tcgetattr(self.terminal.input().as_ref()).map_err(io::Error::from)?;
        let mut raw = original.clone();
        raw.make_raw();
        {
            let mut slot = self
                .original_termios
                .lock()
                .map_err(|_| io::Error::other("raw-mode state lock was poisoned"))?;
            if slot.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "raw mode is already owned by this terminal session",
                ));
            }
            *slot = Some(original);
        }
        tcsetattr(self.terminal.input().as_ref(), OptionalActions::Now, &raw)
            .map_err(io::Error::from)
    }

    fn end_synchronized_update(&self) -> io::Result<()> {
        self.write_command(SynchronizedUpdateEnd)
    }

    fn enter_alternate_screen(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            admit_windows_vt_and_enter_alternate(
                &self.original_console,
                &self.output_vt_enabled,
                &self.alternate_sequence_started,
                |mode| self.terminal.set_output_mode(mode),
                || self.terminal.output(),
            )
        }
        #[cfg(unix)]
        self.write_command(EnterAlternateScreen)
    }

    fn hide_cursor(&self) -> io::Result<()> {
        self.write_command(Hide)
    }

    fn leave_alternate_screen(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            leave_windows_alternate(
                &self.output_vt_enabled,
                &self.alternate_sequence_started,
                || self.terminal.output(),
            )
        }
        #[cfg(unix)]
        self.write_command(LeaveAlternateScreen)
    }

    fn show_cursor(&self) -> io::Result<()> {
        self.write_command(Show)
    }
    #[cfg(windows)]
    fn enable_raw_mode(&self) -> io::Result<()> {
        capture_and_enable_windows_raw_mode(
            &self.original_console,
            || self.terminal.input_mode(),
            || self.terminal.output_mode(),
            |mode| self.terminal.set_input_mode(mode),
        )
    }
    #[cfg(windows)]
    fn disable_raw_mode(&self) -> io::Result<()> {
        restore_windows_console_modes(
            &self.original_console,
            &self.output_vt_enabled,
            |mode| self.terminal.set_output_mode(mode),
            |mode| self.terminal.set_input_mode(mode),
        )
    }
}

type PanicRestorer = Arc<dyn Fn() + Send + Sync + 'static>;

struct ActivePanicRestoration {
    id: u64,
    owner_thread: ThreadId,
    restore: PanicRestorer,
}

struct PanicRestorationRegistry {
    active: Mutex<Option<ActivePanicRestoration>>,
    next_id: AtomicU64,
}

impl PanicRestorationRegistry {
    fn register(
        &'static self,
        owner_thread: ThreadId,
        restore: PanicRestorer,
    ) -> io::Result<PanicRestorationRegistration> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut active = self
            .active
            .lock()
            .map_err(|_| io::Error::other("panic-restoration registry was poisoned"))?;
        if active.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "another terminal session already owns panic restoration",
            ));
        }
        *active = Some(ActivePanicRestoration {
            id,
            owner_thread,
            restore,
        });
        Ok(PanicRestorationRegistration { id, registry: self })
    }

    fn restore_active_for(&self, panicking_thread: ThreadId) {
        let restore = self.active.lock().ok().and_then(|active| {
            active
                .as_ref()
                .filter(|active| active.owner_thread == panicking_thread)
                .map(|active| Arc::clone(&active.restore))
        });
        if let Some(restore) = restore {
            restore();
        }
    }

    fn unregister(&self, id: u64) {
        if let Ok(mut active) = self.active.lock()
            && active.as_ref().is_some_and(|active| active.id == id)
        {
            *active = None;
        }
    }
}

struct PanicRestorationRegistration {
    id: u64,
    registry: &'static PanicRestorationRegistry,
}

impl Drop for PanicRestorationRegistration {
    fn drop(&mut self) {
        self.registry.unregister(self.id);
    }
}

fn register_panic_restoration(restore: PanicRestorer) -> io::Result<PanicRestorationRegistration> {
    process_panic_registry().register(thread::current().id(), restore)
}

fn process_panic_registry() -> &'static PanicRestorationRegistry {
    static REGISTRY: OnceLock<PanicRestorationRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let registry = PanicRestorationRegistry {
            active: Mutex::new(None),
            next_id: AtomicU64::new(1),
        };
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            if let Some(registry) = REGISTRY.get() {
                registry.restore_active_for(thread::current().id());
            }
            previous(panic_info);
        }));
        registry
    })
}

#[cfg(test)]
mod tests;
#[cfg(all(test, windows))]
mod windows_tests;

#[cfg(windows)]
fn captured_window_title(result: io::Result<String>) -> io::Result<String> {
    let raw = match result {
        Ok(raw) => raw,
        Err(error) if error.raw_os_error() == Some(0) => return Ok(String::new()),
        Err(error) => return Err(error),
    };
    let (title, padding) = raw
        .split_once('\0')
        .ok_or_else(|| io::Error::other("console title capture may be truncated"))?;
    if title.encode_utf16().count() >= 259 || padding.chars().any(|ch| ch != '\0') {
        return Err(io::Error::other("console title capture may be truncated"));
    }
    Ok(title.to_owned())
}
