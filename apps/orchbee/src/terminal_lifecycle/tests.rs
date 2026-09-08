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
    DisableRaw,
    EnableRaw,
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

    fn disable_raw_mode(&self) -> io::Result<()> {
        self.perform(Operation::DisableRaw)
    }

    fn enable_raw_mode(&self) -> io::Result<()> {
        self.perform(Operation::EnableRaw)
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

    fn disable_raw_mode(&self) -> io::Result<()> {
        self.control.disable_raw_mode()
    }

    fn enable_raw_mode(&self) -> io::Result<()> {
        self.setup_started.wait();
        self.continue_setup.wait();
        self.control.enable_raw_mode()
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
