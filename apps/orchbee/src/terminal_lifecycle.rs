use std::{
    fmt,
    fs::File,
    io,
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
    ExecutableCommand,
    cursor::{Hide, Show},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcsetattr};

use crate::terminal_device::InheritedTerminal;

const RESTORATION_ATTEMPTS_PER_MODE: u8 = 2;

pub(crate) const SYNCHRONIZED_UPDATE_BEGIN: &[u8] = b"\x1b[?2026h";
pub(crate) const SYNCHRONIZED_UPDATE_END: &[u8] = b"\x1b[?2026l";

struct BracketedPaste(bool);

impl crossterm::Command for BracketedPaste {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(if self.0 { "\x1b[?2004h" } else { "\x1b[?2004l" })
    }
}

/// Pointer reporting: button-event tracking plus SGR coordinates
/// (`?1002` + `?1006`). Never `?1003` (any-event) — hover traffic is not
/// consumed — and never crossterm's bundled `EnableMouseCapture`, which
/// would also claim `?1003`/`?1015`. Disable restores in reverse order.
struct PointerCapture(bool);

impl crossterm::Command for PointerCapture {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str(if self.0 {
            "\x1b[?1002h\x1b[?1006h"
        } else {
            "\x1b[?1006l\x1b[?1002l"
        })
    }
}

struct SynchronizedUpdateEnd;

impl crossterm::Command for SynchronizedUpdateEnd {
    fn write_ansi(&self, output: &mut impl fmt::Write) -> fmt::Result {
        output.write_str("\x1b[?2026l")
    }
}

pub(crate) trait TerminalModes: Send + Sync + 'static {
    fn disable_bracketed_paste(&self) -> io::Result<()>;
    fn enable_bracketed_paste(&self) -> io::Result<()>;
    fn disable_pointer_capture(&self) -> io::Result<()>;
    fn enable_pointer_capture(&self) -> io::Result<()>;
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
    DisableBracketedPaste,
    DisablePointerCapture,
    DisableRaw,
    EndSynchronizedUpdate,
    LeaveAlternate,
    ShowCursor,
}

impl fmt::Display for RestorationStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DisableBracketedPaste => formatter.write_str("disable bracketed paste"),
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
    frame_output_blocked: AtomicBool,
    obligation: ModeLease,
}

impl SynchronizedUpdateLease {
    pub(crate) const fn new() -> Self {
        Self {
            frame_output_blocked: AtomicBool::new(false),
            obligation: ModeLease::new(),
        }
    }

    pub(crate) fn open_frame(&self) {
        self.obligation.mark_operation_started();
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
    bracketed_paste: ModeLease,
    pointer_capture: ModeLease,
    alternate_screen: ModeLease,
    cursor: ModeLease,
    raw_mode: ModeLease,
    synchronized_update: Arc<SynchronizedUpdateLease>,
}

impl TerminalModeState {
    fn new() -> Self {
        Self {
            bracketed_paste: ModeLease::new(),
            pointer_capture: ModeLease::new(),
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
    control: Arc<C>,
    _owner_thread_only: PhantomData<Rc<()>>,
    _panic_registration: Option<PanicRestorationRegistration>,
    state: Arc<TerminalModeState>,
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
        restore_modes(self.control.as_ref(), &self.state)
    }

    pub(crate) fn synchronized_update_lease(&self) -> Arc<SynchronizedUpdateLease> {
        Arc::clone(&self.state.synchronized_update)
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
        if enabled {
            if self.pointer_capture_enabled {
                return Ok(());
            }
            if self.state.pointer_capture.restoration_required() {
                return Err(io::Error::other(
                    "pointer capture still requires restoration",
                ));
            }
            self.state.pointer_capture.mark_operation_started();
            self.control.enable_pointer_capture()?;
            self.pointer_capture_enabled = true;
            return Ok(());
        }
        self.pointer_capture_enabled = false;
        let mut failures = Vec::new();
        restore_mode(
            &self.state.pointer_capture,
            RestorationStep::DisablePointerCapture,
            || self.control.disable_pointer_capture(),
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
    restore_mode(
        &state.synchronized_update.obligation,
        RestorationStep::EndSynchronizedUpdate,
        || control.end_synchronized_update(),
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
    original_termios: Arc<Mutex<Option<Termios>>>,
    terminal: InheritedTerminal,
}

impl InheritedTerminalModes {
    pub(crate) fn new(terminal: InheritedTerminal) -> Self {
        Self {
            original_termios: Arc::new(Mutex::new(None)),
            terminal,
        }
    }

    fn write_command(&self, command: impl crossterm::Command) -> io::Result<()> {
        let mut output = File::from(self.terminal.duplicate_output()?);
        output.execute(command).map(drop)
    }
}

impl TerminalModes for InheritedTerminalModes {
    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.write_command(BracketedPaste(false))
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.write_command(BracketedPaste(true))
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.write_command(PointerCapture(false))
    }

    fn enable_pointer_capture(&self) -> io::Result<()> {
        self.write_command(PointerCapture(true))
    }

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
        self.write_command(EnterAlternateScreen)
    }

    fn hide_cursor(&self) -> io::Result<()> {
        self.write_command(Hide)
    }

    fn leave_alternate_screen(&self) -> io::Result<()> {
        self.write_command(LeaveAlternateScreen)
    }

    fn show_cursor(&self) -> io::Result<()> {
        self.write_command(Show)
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
