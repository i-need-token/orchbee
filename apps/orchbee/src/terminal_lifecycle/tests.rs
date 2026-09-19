use std::{
    collections::HashMap,
    io,
    panic::{self, AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use super::{RestorationStep, TerminalModeGuard, TerminalModes};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Operation {
    DisablePaste,
    EnablePaste,
    DisablePointer,
    EnablePointer,
    DisableRaw,
    EnableRaw,
    EndSynchronizedUpdate,
    EnterAlternate,
    HideCursor,
    LeaveAlternate,
    ShowCursor,
}

#[derive(Default)]
struct FakeState {
    failures_remaining: HashMap<Operation, usize>,
    operations: Vec<Operation>,
}

#[derive(Clone, Default)]
struct FakeTerminalModes {
    state: Arc<Mutex<FakeState>>,
}

impl FakeTerminalModes {
    fn with_failures(failures: impl IntoIterator<Item = (Operation, usize)>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeState {
                failures_remaining: failures.into_iter().collect(),
                operations: Vec::new(),
            })),
        }
    }

    fn operations(&self) -> Vec<Operation> {
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
    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.perform(Operation::DisablePaste)
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.perform(Operation::EnablePaste)
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.perform(Operation::DisablePointer)
    }

    fn enable_pointer_capture(&self) -> io::Result<()> {
        self.perform(Operation::EnablePointer)
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

#[derive(Clone)]
struct SetupBarrierTerminalModes {
    continue_setup: Arc<Barrier>,
    control: FakeTerminalModes,
    setup_started: Arc<Barrier>,
}

impl TerminalModes for SetupBarrierTerminalModes {
    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.control.disable_bracketed_paste()
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.control.enable_bracketed_paste()
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.control.disable_pointer_capture()
    }

    fn enable_pointer_capture(&self) -> io::Result<()> {
        self.control.enable_pointer_capture()
    }

    fn disable_raw_mode(&self) -> io::Result<()> {
        self.control.disable_raw_mode()
    }

    fn enable_raw_mode(&self) -> io::Result<()> {
        self.setup_started.wait();
        self.continue_setup.wait();
        self.control.enable_raw_mode()
    }

    fn end_synchronized_update(&self) -> io::Result<()> {
        self.control.end_synchronized_update()
    }

    fn enter_alternate_screen(&self) -> io::Result<()> {
        self.control.enter_alternate_screen()
    }

    fn hide_cursor(&self) -> io::Result<()> {
        self.control.hide_cursor()
    }

    fn leave_alternate_screen(&self) -> io::Result<()> {
        self.control.leave_alternate_screen()
    }

    fn show_cursor(&self) -> io::Result<()> {
        self.control.show_cursor()
    }
}

fn lock_panic_hook_tests() -> io::Result<MutexGuard<'static, ()>> {
    static PANIC_HOOK_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    PANIC_HOOK_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| io::Error::other("panic-hook test lock was poisoned"))
}

#[test]
fn paste_mode_repeated_requests_do_not_reacquire_or_restore_a_live_lease() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    guard.set_bracketed_paste(true)?;
    guard.set_bracketed_paste(true)?;
    guard.set_bracketed_paste(false)?;
    guard.set_bracketed_paste(false)?;
    guard.set_bracketed_paste(true)?;
    drop(guard);
    let paste: Vec<_> = control
        .operations()
        .into_iter()
        .filter(|operation| matches!(operation, Operation::EnablePaste | Operation::DisablePaste))
        .collect();
    assert_eq!(
        paste,
        [
            Operation::EnablePaste,
            Operation::DisablePaste,
            Operation::EnablePaste,
            Operation::DisablePaste
        ]
    );
    Ok(())
}

#[test]
fn partial_paste_enable_and_failed_restore_cannot_renew_the_cleanup_budget() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([
        (Operation::EnablePaste, 1),
        (Operation::DisablePaste, usize::MAX),
    ]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    assert!(guard.set_bracketed_paste(true).is_err());
    assert!(guard.set_bracketed_paste(true).is_err());
    assert!(guard.set_bracketed_paste(false).is_err());
    assert!(guard.set_bracketed_paste(true).is_err());
    assert!(guard.restore().is_err());
    drop(guard);
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::EnablePaste)
            .count(),
        1
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::DisablePaste)
            .count(),
        2
    );
    Ok(())
}

#[test]
fn panic_restores_owned_paste_before_other_terminal_modes() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter(control.clone())?;
    guard.set_bracketed_paste(true)?;
    assert!(catch_unwind(|| panic!("injected panic while Rename owns paste")).is_err());
    assert_eq!(
        &control.operations()[4..],
        [
            Operation::DisablePaste,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw
        ]
    );
    drop(guard);
    assert_eq!(control.operations().len(), 8);
    Ok(())
}

#[test]
fn restores_every_acquired_mode_in_reverse_order_on_drop() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    {
        let _guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    }

    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );
    Ok(())
}

#[test]
fn conservatively_inverts_an_operation_that_may_partially_succeed_before_failure() {
    let cases = [
        (
            Operation::EnableRaw,
            vec![Operation::EnableRaw, Operation::DisableRaw],
        ),
        (
            Operation::EnterAlternate,
            vec![
                Operation::EnableRaw,
                Operation::EnterAlternate,
                Operation::LeaveAlternate,
                Operation::DisableRaw,
            ],
        ),
        (
            Operation::HideCursor,
            vec![
                Operation::EnableRaw,
                Operation::EnterAlternate,
                Operation::HideCursor,
                Operation::ShowCursor,
                Operation::LeaveAlternate,
                Operation::DisableRaw,
            ],
        ),
    ];

    for (failure, expected) in cases {
        let control = FakeTerminalModes::with_failures([(failure, 1)]);
        assert!(TerminalModeGuard::enter_without_panic_hook(control.clone()).is_err());
        assert_eq!(control.operations(), expected);
    }
}

#[test]
fn retains_cleanup_ownership_until_a_retry_succeeds() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    {
        let mut state = control
            .state
            .lock()
            .map_err(|_| io::Error::other("fake terminal state was poisoned"))?;
        state
            .failures_remaining
            .extend([(Operation::ShowCursor, 1), (Operation::DisableRaw, 1)]);
    }

    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::ShowCursor,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
            Operation::DisableRaw,
        ]
    );
    drop(guard);
    assert_eq!(control.operations().len(), 8);
    Ok(())
}

#[test]
fn aggregates_every_mode_that_exhausts_its_bounded_retry_budget() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    {
        let mut state = control
            .state
            .lock()
            .map_err(|_| io::Error::other("fake terminal state was poisoned"))?;
        state.failures_remaining.extend([
            (Operation::ShowCursor, 2),
            (Operation::LeaveAlternate, 2),
            (Operation::DisableRaw, 2),
        ]);
    }

    let error = match guard.restore() {
        Ok(()) => return Err(io::Error::other("injected restoration failures succeeded")),
        Err(error) => error,
    };
    assert_eq!(
        error.failed_steps(),
        [
            RestorationStep::ShowCursor,
            RestorationStep::LeaveAlternate,
            RestorationStep::DisableRaw,
        ]
    );
    assert_eq!(control.operations().len(), 9);
    drop(guard);
    assert_eq!(control.operations().len(), 9);
    Ok(())
}

#[test]
fn process_singleton_restores_an_actual_panic_without_clobbering_a_later_hook() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let _ = super::process_panic_registry();
    let later_hook_calls = Arc::new(AtomicUsize::new(0));
    let calls_for_hook = Arc::clone(&later_hook_calls);
    let singleton_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        calls_for_hook.fetch_add(1, Ordering::Relaxed);
        singleton_hook(panic_info);
    }));

    let control = FakeTerminalModes::default();
    let guard = TerminalModeGuard::enter(control.clone())?;

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        panic!("injected panic after terminal modes were acquired");
    }));
    assert!(panic_result.is_err());
    assert_eq!(later_hook_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );

    drop(guard);
    assert_eq!(control.operations().len(), 6);

    let panic_after_drop = catch_unwind(|| {
        panic!("injected panic after the terminal guard was dropped");
    });
    assert!(panic_after_drop.is_err());
    assert_eq!(later_hook_calls.load(Ordering::Relaxed), 2);
    Ok(())
}

#[test]
fn another_thread_panic_cannot_clear_a_mode_lease_while_owner_setup_is_blocked() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let control = FakeTerminalModes::default();
    let setup_started = Arc::new(Barrier::new(2));
    let continue_setup = Arc::new(Barrier::new(2));
    let barrier_control = SetupBarrierTerminalModes {
        continue_setup: Arc::clone(&continue_setup),
        control: control.clone(),
        setup_started: Arc::clone(&setup_started),
    };
    let owner = thread::spawn(move || -> io::Result<()> {
        let guard = TerminalModeGuard::enter(barrier_control)?;
        drop(guard);
        Ok(())
    });

    setup_started.wait();
    let unrelated_panic = catch_unwind(|| {
        panic!("injected panic on a thread that does not own terminal modes");
    });
    let operations_during_setup = control.operations();
    continue_setup.wait();
    let owner_result = owner
        .join()
        .map_err(|_| io::Error::other("terminal owner thread panicked"))?;
    owner_result?;

    assert!(unrelated_panic.is_err());
    assert!(operations_during_setup.is_empty());
    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );
    Ok(())
}

#[test]
fn synchronized_update_recovery_runs_before_other_modes_and_only_when_owed() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    let synchronized_update = guard.synchronized_update_lease();
    synchronized_update.open_frame();

    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::EndSynchronizedUpdate,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );
    assert!(synchronized_update.frame_output_blocked());
    assert!(!synchronized_update.frame_open());

    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::EndSynchronizedUpdate)
            .count(),
        0,
        "a clean close emits no extra end marker"
    );
    Ok(())
}

#[test]
fn synchronized_update_recovery_retries_within_its_budget() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([(Operation::EndSynchronizedUpdate, 1)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    let synchronized_update = guard.synchronized_update_lease();
    synchronized_update.open_frame();

    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::EndSynchronizedUpdate)
            .count(),
        2,
        "the first recovery attempt fails and the second succeeds"
    );
    assert!(!synchronized_update.frame_open());
    Ok(())
}

#[test]
fn synchronized_update_recovery_exhaustion_is_reported_and_stays_pending() -> io::Result<()> {
    let control =
        FakeTerminalModes::with_failures([(Operation::EndSynchronizedUpdate, usize::MAX)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    let synchronized_update = guard.synchronized_update_lease();
    synchronized_update.open_frame();

    let error = match guard.restore() {
        Ok(()) => return Err(io::Error::other("exhausted recovery succeeded")),
        Err(error) => error,
    };
    assert_eq!(
        error.failed_steps(),
        [RestorationStep::EndSynchronizedUpdate]
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::EndSynchronizedUpdate)
            .count(),
        2,
        "recovery is bounded by the same budget as other modes"
    );
    assert!(
        synchronized_update.frame_open(),
        "an unrecovered pair stays pending"
    );
    Ok(())
}

#[test]
fn owner_panic_recovers_an_open_pair_before_other_terminal_modes() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let control = FakeTerminalModes::default();
    let guard = TerminalModeGuard::enter(control.clone())?;
    let synchronized_update = guard.synchronized_update_lease();
    synchronized_update.open_frame();

    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            panic!("injected panic while a synchronized frame is open")
        }))
        .is_err()
    );
    assert!(synchronized_update.frame_output_blocked());
    assert!(!synchronized_update.frame_open());
    assert_eq!(
        &control.operations()[3..],
        [
            Operation::EndSynchronizedUpdate,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );
    drop(guard);
    assert_eq!(control.operations().len(), 7);
    Ok(())
}

#[test]
fn a_non_owner_panic_neither_recovers_nor_blocks_an_open_pair() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let control = FakeTerminalModes::default();
    let guard = TerminalModeGuard::enter(control.clone())?;
    let synchronized_update = guard.synchronized_update_lease();
    synchronized_update.open_frame();

    let worker = thread::spawn(|| {
        let caught =
            catch_unwind(|| panic!("injected panic on a thread that does not own the terminal"));
        assert!(caught.is_err());
    });
    worker
        .join()
        .map_err(|_| io::Error::other("non-owner panic thread failed"))?;

    assert!(!synchronized_update.frame_output_blocked());
    assert!(synchronized_update.frame_open());
    assert_eq!(
        control.operations(),
        [
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
        ]
    );

    drop(guard);
    assert_eq!(
        &control.operations()[3..],
        [
            Operation::EndSynchronizedUpdate,
            Operation::ShowCursor,
            Operation::LeaveAlternate,
            Operation::DisableRaw,
        ]
    );
    Ok(())
}

#[derive(Clone, Default)]
struct OrderedWire {
    events: Arc<Mutex<Vec<String>>>,
}

impl OrderedWire {
    fn record(&self, event: impl Into<String>) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.into());
        }
    }

    fn snapshot(&self) -> Vec<String> {
        match self.events.lock() {
            Ok(events) => events.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[derive(Clone)]
struct OrderedWireModes {
    wire: OrderedWire,
}

impl TerminalModes for OrderedWireModes {
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
    fn enable_pointer_capture(&self) -> io::Result<()> {
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

struct OrderedWireWriter {
    wire: OrderedWire,
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

#[test]
fn owner_panic_blocks_buffered_frame_bytes_from_reaching_the_wire() -> io::Result<()> {
    let _panic_hook_test = lock_panic_hook_tests()?;
    let wire = OrderedWire::default();
    let guard = TerminalModeGuard::enter(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        use ratatui::backend::Backend;
        let mut backend = crate::tui_backend::FixedViewportBackend::new(
            io::BufWriter::with_capacity(
                64 * 1024,
                crate::tui_backend::RestorationGatedWriter::new(
                    OrderedWireWriter { wire: wire.clone() },
                    Arc::clone(&synchronized_update),
                ),
            ),
            crate::terminal_device::WindowDimensions {
                columns: 4,
                pixel_height: 32,
                pixel_width: 16,
                rows: 2,
            },
            Arc::clone(&synchronized_update),
        );
        let mut cell = ratatui::buffer::Cell::default();
        cell.set_symbol("a");
        backend
            .draw(std::iter::once((0, 0, &cell)))
            .expect("draw buffers begin + cells");
        panic!("injected panic while a synchronized frame is buffered");
    }));
    assert!(outcome.is_err(), "injected panic was caught");

    let events = wire.snapshot();
    assert_eq!(
        events,
        [
            "enable-raw-mode",
            "enter-alternate-screen",
            "hide-cursor",
            "end-synchronized-update",
            "show-cursor",
            "leave-alternate-screen",
            "disable-raw-mode",
        ],
        "recovery ends the open pair before the other modes"
    );
    assert!(
        !events.iter().any(|event| event.starts_with("frame-bytes:")),
        "no buffered frame bytes may reach the wire after restoration: {events:?}"
    );
    Ok(())
}

struct FailingOrderedWireWriter {
    wire: OrderedWire,
    fail_writes_from: Option<usize>,
    failure: Arc<std::sync::atomic::AtomicBool>,
    write_calls: usize,
}

impl FailingOrderedWireWriter {
    fn fail_from(index: usize, wire: OrderedWire) -> Self {
        Self {
            wire,
            fail_writes_from: Some(index),
            failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            write_calls: 0,
        }
    }

    fn armed(wire: OrderedWire) -> (Self, Arc<std::sync::atomic::AtomicBool>) {
        let failure = Arc::new(std::sync::atomic::AtomicBool::new(false));
        (
            Self {
                wire,
                fail_writes_from: None,
                failure: Arc::clone(&failure),
                write_calls: 0,
            },
            failure,
        )
    }
}

impl io::Write for FailingOrderedWireWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let index = self.write_calls;
        self.write_calls += 1;
        if self.failure.load(std::sync::atomic::Ordering::Relaxed)
            || self.fail_writes_from.is_some_and(|from| index >= from)
        {
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

fn failed_frame_backend(
    writer: FailingOrderedWireWriter,
    synchronized_update: &Arc<super::SynchronizedUpdateLease>,
) -> crate::tui_backend::FixedViewportBackend<
    io::BufWriter<crate::tui_backend::RestorationGatedWriter<FailingOrderedWireWriter>>,
> {
    crate::tui_backend::FixedViewportBackend::new(
        io::BufWriter::with_capacity(
            64,
            crate::tui_backend::RestorationGatedWriter::new(
                writer,
                Arc::clone(synchronized_update),
            ),
        ),
        crate::terminal_device::WindowDimensions {
            columns: 4,
            pixel_height: 32,
            pixel_width: 16,
            rows: 2,
        },
        Arc::clone(synchronized_update),
    )
}

#[test]
fn discard_frame_output_emits_no_control_io_and_keeps_the_pending_end() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    guard.synchronized_update_lease().open_frame();

    let before = control.operations();
    guard.discard_frame_output();
    assert!(guard.synchronized_update_lease().frame_output_blocked());
    assert_eq!(
        control.operations(),
        before,
        "discarding performs no terminal I/O of its own"
    );
    guard.discard_frame_output();

    guard.restore().map_err(io::Error::other)?;
    let repaid = control
        .operations()
        .iter()
        .filter(|operation| **operation == Operation::EndSynchronizedUpdate)
        .count();
    assert_eq!(
        repaid, 1,
        "the pending end survives discard and is repaid exactly once"
    );
    Ok(())
}

#[test]
fn a_discarded_failed_frame_drops_without_new_wire_bytes() -> io::Result<()> {
    let wire = OrderedWire::default();
    let mut guard =
        TerminalModeGuard::enter_without_panic_hook(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();
    let mut backend = failed_frame_backend(
        FailingOrderedWireWriter::fail_from(1, wire.clone()),
        &synchronized_update,
    );
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        assert!(
            backend.draw((0..8u16).map(|x| (x, 0, &cell))).is_err(),
            "the injected mid-frame failure propagates"
        );
    }
    assert!(synchronized_update.frame_open());

    // run_prepared_surface's error order: discard before the writer drops.
    let settled = wire.snapshot().len();
    guard.discard_frame_output();
    drop(backend);
    assert_eq!(
        wire.snapshot().len(),
        settled,
        "the discarded writer's drop-flush emits nothing new"
    );

    guard.restore().map_err(io::Error::other)?;
    let events = wire.snapshot();
    assert_eq!(
        &events[settled..],
        [
            "end-synchronized-update",
            "show-cursor",
            "leave-alternate-screen",
            "disable-raw-mode"
        ],
        "the control channel repays the pending end, then restores modes"
    );
    Ok(())
}

#[test]
fn an_unfenced_writer_drop_retries_the_failed_frames_residue() {
    // Witness for why discard_frame_output must run before the writer drops:
    // BufWriter's drop always retries buffered bytes, so a recovered fd
    // accepts the failed frame's residue.
    let wire = OrderedWire::default();
    let synchronized_update = Arc::new(super::SynchronizedUpdateLease::new());
    let (writer, failure) = FailingOrderedWireWriter::armed(wire.clone());
    let mut backend = failed_frame_backend(writer, &synchronized_update);
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        failure.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = backend.draw(std::iter::once((0, 0, &cell)));
        failure.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    let settled = wire.snapshot().len();
    drop(backend);
    assert!(
        wire.snapshot().len() > settled,
        "without the output fence the drop-flush retries the failed frame's residue"
    );
}

#[test]
fn a_failed_end_is_repaid_through_the_control_channel() -> io::Result<()> {
    let wire = OrderedWire::default();
    let mut guard =
        TerminalModeGuard::enter_without_panic_hook(OrderedWireModes { wire: wire.clone() })?;
    let synchronized_update = guard.synchronized_update_lease();
    let (writer, failure) = FailingOrderedWireWriter::armed(wire.clone());
    let mut backend = failed_frame_backend(writer, &synchronized_update);
    let mut cell = ratatui::buffer::Cell::default();
    cell.set_symbol("a");
    {
        use ratatui::backend::Backend;
        backend
            .draw(std::iter::once((0, 0, &cell)))
            .expect("draw buffers the frame");
        failure.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            backend.flush().is_err(),
            "the injected end-marker failure propagates"
        );
    }
    let settled = wire.snapshot().len();
    guard.discard_frame_output();
    drop(backend);
    guard.restore().map_err(io::Error::other)?;
    let events = wire.snapshot();
    assert!(
        events[settled..]
            .iter()
            .filter(|event| *event == "end-synchronized-update")
            .count()
            == 1,
        "the failed end is repaid once through the control channel"
    );
    assert!(
        !events[settled..]
            .iter()
            .any(|event| event.starts_with("frame-bytes:")),
        "no frame bytes follow the discard"
    );
    Ok(())
}
