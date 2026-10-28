//! Recording terminal-mode fakes and wire writers shared by lifecycle tests and the owning
//! application's backend tests.

use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use super::{PointerTrackingLevel, TerminalModes};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Operation {
    Bell,
    CleanupGraphics,
    QueryGraphics,
    QueryPointerAdmission,
    QueryHostAppearance,
    DisablePaste,
    EnablePaste,
    DisablePointer,
    EnablePointer,
    PointerTracking(PointerTrackingLevel),
    DisableFocus,
    EnableFocus,
    DisableRaw,
    EnableRaw,
    EndSynchronizedUpdate,
    EnterAlternate,
    HideCursor,
    LeaveAlternate,
    ShowCursor,
    SetTitle,
    Notification,
    RestoreTitle,
}

#[derive(Default)]
pub struct FakeState {
    pub graphics_cleanup: Vec<String>,
    failures_remaining: HashMap<Operation, usize>,
    operations: Vec<Operation>,
}

#[derive(Clone, Default)]
pub struct FakeTerminalModes {
    pub state: Arc<Mutex<FakeState>>,
}

impl FakeTerminalModes {
    pub fn with_failures(failures: impl IntoIterator<Item = (Operation, usize)>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                failures_remaining: failures.into_iter().collect(),
                operations: Vec::new(),
                graphics_cleanup: Vec::new(),
            })),
        }
    }

    pub fn operations(&self) -> Vec<Operation> {
        match self.state.lock() {
            Ok(state) => state.operations.clone(),
            Err(poisoned) => poisoned.into_inner().operations.clone(),
        }
    }

    fn perform(&self, operation: Operation) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("fake terminal state was poisoned"))?;
        state.operations.push(operation);
        let failures_remaining = state.failures_remaining.entry(operation).or_default();
        if *failures_remaining == 0 {
            Ok(())
        } else {
            *failures_remaining -= 1;
            Err(io::Error::other("injected terminal-mode failure"))
        }
    }
}

impl TerminalModes for FakeTerminalModes {
    fn ring_bell(&self) -> io::Result<()> {
        self.perform(Operation::Bell)
    }
    fn cleanup_graphics(&self, sequence: &str) -> io::Result<()> {
        self.state
            .lock()
            .map_err(|_| io::Error::other("fake state poisoned"))?
            .graphics_cleanup
            .push(sequence.to_owned());
        self.perform(Operation::CleanupGraphics)
    }
    fn query_graphics(&self, _: crate::terminal_graphics::Query) -> io::Result<()> {
        self.perform(Operation::QueryGraphics)
    }
    fn query_pointer_admission(
        &self,
        _: crate::terminal_pointer_admission::Query,
    ) -> io::Result<()> {
        self.perform(Operation::QueryPointerAdmission)
    }
    fn query_host_appearance(&self, _: crate::terminal_host_appearance::Query) -> io::Result<()> {
        self.perform(Operation::QueryHostAppearance)
    }
    fn notify_terminal(&self, _: &orchbee_workbench_protocol::Notification) -> io::Result<()> {
        self.perform(Operation::Notification)
    }
    fn set_window_title(&self, _: &str) -> io::Result<()> {
        self.perform(Operation::SetTitle)
    }
    fn restore_window_title(&self) -> io::Result<()> {
        self.perform(Operation::RestoreTitle)
    }
    fn disable_focus_reporting(&self) -> io::Result<()> {
        self.perform(Operation::DisableFocus)
    }
    fn enable_focus_reporting(&self) -> io::Result<()> {
        self.perform(Operation::EnableFocus)
    }

    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.perform(Operation::DisablePaste)
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.perform(Operation::EnablePaste)
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.perform(Operation::DisablePointer)
    }

    fn enable_pointer_capture(&self, _level: PointerTrackingLevel) -> io::Result<()> {
        self.perform(Operation::EnablePointer)
    }

    fn set_pointer_tracking(
        &self,
        _from: PointerTrackingLevel,
        to: PointerTrackingLevel,
    ) -> io::Result<()> {
        self.perform(Operation::PointerTracking(to))
    }

    fn disable_raw_mode(&self) -> io::Result<()> {
        self.perform(Operation::DisableRaw)
    }

    fn enable_raw_mode(&self) -> io::Result<()> {
        self.perform(Operation::EnableRaw)
    }

    fn end_synchronized_update(&self) -> io::Result<()> {
        self.perform(Operation::EndSynchronizedUpdate)
    }

    fn enter_alternate_screen(&self) -> io::Result<()> {
        self.perform(Operation::EnterAlternate)
    }

    fn hide_cursor(&self) -> io::Result<()> {
        self.perform(Operation::HideCursor)
    }

    fn leave_alternate_screen(&self) -> io::Result<()> {
        self.perform(Operation::LeaveAlternate)
    }

    fn show_cursor(&self) -> io::Result<()> {
        self.perform(Operation::ShowCursor)
    }
}

pub fn lock_panic_hook_tests() -> io::Result<MutexGuard<'static, ()>> {
    static PANIC_HOOK_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    PANIC_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| io::Error::other("panic-hook test lock was poisoned"))
}

#[derive(Clone, Default)]
pub struct OrderedWire {
    events: Arc<Mutex<Vec<String>>>,
}

impl OrderedWire {
    fn record(&self, event: impl Into<String>) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.into());
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        match self.events.lock() {
            Ok(events) => events.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[derive(Clone)]
pub struct OrderedWireModes {
    pub wire: OrderedWire,
}

impl TerminalModes for OrderedWireModes {
    fn cleanup_graphics(&self, sequence: &str) -> io::Result<()> {
        self.wire.record(format!("graphics-cleanup:{sequence}"));
        Ok(())
    }
    fn disable_focus_reporting(&self) -> io::Result<()> {
        self.wire.record("disable-focus-reporting");
        Ok(())
    }
    fn enable_focus_reporting(&self) -> io::Result<()> {
        self.wire.record("enable-focus-reporting");
        Ok(())
    }

    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.wire.record("disable-bracketed-paste");
        Ok(())
    }
    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.wire.record("enable-bracketed-paste");
        Ok(())
    }
    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.wire.record("disable-pointer-capture");
        Ok(())
    }
    fn enable_pointer_capture(&self, _level: PointerTrackingLevel) -> io::Result<()> {
        self.wire.record("enable-pointer-capture");
        Ok(())
    }
    fn disable_raw_mode(&self) -> io::Result<()> {
        self.wire.record("disable-raw-mode");
        Ok(())
    }
    fn enable_raw_mode(&self) -> io::Result<()> {
        self.wire.record("enable-raw-mode");
        Ok(())
    }
    fn end_synchronized_update(&self) -> io::Result<()> {
        self.wire.record("end-synchronized-update");
        Ok(())
    }
    fn enter_alternate_screen(&self) -> io::Result<()> {
        self.wire.record("enter-alternate-screen");
        Ok(())
    }
    fn hide_cursor(&self) -> io::Result<()> {
        self.wire.record("hide-cursor");
        Ok(())
    }
    fn leave_alternate_screen(&self) -> io::Result<()> {
        self.wire.record("leave-alternate-screen");
        Ok(())
    }
    fn show_cursor(&self) -> io::Result<()> {
        self.wire.record("show-cursor");
        Ok(())
    }
}

pub struct OrderedWireWriter {
    pub wire: OrderedWire,
}

impl io::Write for OrderedWireWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.wire.record(format!("frame-bytes:{}", bytes.len()));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct FailingOrderedWireWriter {
    wire: OrderedWire,
    failure: Arc<std::sync::atomic::AtomicBool>,
    partial_first_write: bool,
}

impl FailingOrderedWireWriter {
    pub fn partial_then_fail(wire: OrderedWire) -> (Self, Arc<std::sync::atomic::AtomicBool>) {
        let failure = Arc::new(std::sync::atomic::AtomicBool::new(false));
        (
            Self {
                wire,
                failure: Arc::clone(&failure),
                partial_first_write: true,
            },
            failure,
        )
    }

    pub fn armed(wire: OrderedWire) -> (Self, Arc<std::sync::atomic::AtomicBool>) {
        let failure = Arc::new(std::sync::atomic::AtomicBool::new(false));
        (
            Self {
                wire,
                failure: Arc::clone(&failure),
                partial_first_write: false,
            },
            failure,
        )
    }
}

impl io::Write for FailingOrderedWireWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.partial_first_write {
            self.partial_first_write = false;
            let accepted = bytes
                .len()
                .checked_sub(1)
                .filter(|accepted| *accepted > 0)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "partial write needs two bytes")
                })?;
            self.wire.record(format!("frame-bytes:{accepted}"));
            self.failure
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(accepted);
        }
        if self.failure.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected wire failure",
            ));
        }
        self.wire.record(format!("frame-bytes:{}", bytes.len()));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
