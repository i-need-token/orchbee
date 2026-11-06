use std::{
    io,
    panic::{self, AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use super::test_support::{FakeTerminalModes, Operation, lock_panic_hook_tests};
use super::{PointerTrackingLevel, RestorationStep, TerminalModeGuard, TerminalModes};

#[test]
fn menu_motion_keeps_button_fallback_and_returns_every_owned_mode() {
    use PointerTrackingLevel::{AnyMotion, ButtonOnly, MenuMotion};
    use crossterm::Command;
    for (from, to, expected) in [
        (ButtonOnly, MenuMotion, "\x1b[?1002h\x1b[?1003h"),
        (AnyMotion, MenuMotion, "\x1b[?1002h\x1b[?1003h"),
        (MenuMotion, ButtonOnly, "\x1b[?1003l\x1b[?1002h"),
        (MenuMotion, AnyMotion, "\x1b[?1002l\x1b[?1003h"),
    ] {
        let mut wire = String::new();
        super::PointerTrackingTransition { from, to }
            .write_ansi(&mut wire)
            .unwrap();
        assert_eq!(wire, expected, "{from:?} -> {to:?}");
    }
    let mut wire = String::new();
    super::PointerCapture {
        enabled: true,
        level: PointerTrackingLevel::MenuMotion,
    }
    .write_ansi(&mut wire)
    .unwrap();
    super::PointerCapture {
        enabled: false,
        level: PointerTrackingLevel::MenuMotion,
    }
    .write_ansi(&mut wire)
    .unwrap();
    assert_eq!(
        wire,
        "\x1b[?1002h\x1b[?1003h\x1b[?1006h\x1b[?1003l\x1b[?1006l\x1b[?1002l"
    );
}

#[test]
fn tracking_switches_only_inside_capture_and_release_clears_all_owned_modes() {
    use crossterm::Command;
    let mut wire = String::new();
    super::PointerCapture {
        enabled: true,
        level: PointerTrackingLevel::ButtonOnly,
    }
    .write_ansi(&mut wire)
    .unwrap();
    super::PointerTrackingTransition {
        from: PointerTrackingLevel::ButtonOnly,
        to: PointerTrackingLevel::AnyMotion,
    }
    .write_ansi(&mut wire)
    .unwrap();
    super::PointerTrackingTransition {
        from: PointerTrackingLevel::AnyMotion,
        to: PointerTrackingLevel::ButtonOnly,
    }
    .write_ansi(&mut wire)
    .unwrap();
    super::PointerCapture {
        enabled: false,
        level: PointerTrackingLevel::ButtonOnly,
    }
    .write_ansi(&mut wire)
    .unwrap();
    assert_eq!(
        wire,
        "\x1b[?1003l\x1b[?1002h\x1b[?1006h\x1b[?1002l\x1b[?1003h\x1b[?1003l\x1b[?1002h\x1b[?1003l\x1b[?1006l\x1b[?1002l"
    );

    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    assert!(
        guard
            .set_pointer_tracking(PointerTrackingLevel::AnyMotion)
            .is_err()
    );
    guard.set_pointer_capture(true).unwrap();
    for level in [
        PointerTrackingLevel::AnyMotion,
        PointerTrackingLevel::AnyMotion,
        PointerTrackingLevel::ButtonOnly,
        PointerTrackingLevel::ButtonOnly,
        PointerTrackingLevel::AnyMotion,
    ] {
        guard.set_pointer_tracking(level).unwrap();
    }
    guard.set_pointer_capture(false).unwrap();
    assert!(
        guard
            .set_pointer_tracking(PointerTrackingLevel::ButtonOnly)
            .is_err()
    );
    guard.set_pointer_capture(true).unwrap();
    guard
        .set_pointer_tracking(PointerTrackingLevel::AnyMotion)
        .unwrap();
    guard.restore().unwrap();
    let pointer: Vec<_> = control
        .operations()
        .into_iter()
        .filter(|op| {
            matches!(
                op,
                Operation::EnablePointer
                    | Operation::DisablePointer
                    | Operation::PointerTracking(_)
            )
        })
        .collect();
    assert_eq!(
        pointer,
        [
            Operation::EnablePointer,
            Operation::PointerTracking(PointerTrackingLevel::AnyMotion),
            Operation::PointerTracking(PointerTrackingLevel::ButtonOnly),
            Operation::PointerTracking(PointerTrackingLevel::AnyMotion),
            Operation::DisablePointer,
            Operation::EnablePointer,
            Operation::PointerTracking(PointerTrackingLevel::AnyMotion),
            Operation::DisablePointer
        ]
    );
}

#[test]
fn failed_tracking_switch_is_not_retried_and_keeps_capture_cleanup_obligations() {
    use PointerTrackingLevel::{AnyMotion, ButtonOnly, MenuMotion};
    for (from, level) in [
        (ButtonOnly, AnyMotion),
        (AnyMotion, ButtonOnly),
        (ButtonOnly, MenuMotion),
        (MenuMotion, ButtonOnly),
        (MenuMotion, AnyMotion),
        (AnyMotion, MenuMotion),
    ] {
        let control = FakeTerminalModes::with_failures([(Operation::PointerTracking(level), 1)]);
        let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
        guard.set_pointer_capture(true).unwrap();
        guard.set_pointer_tracking(from).unwrap();
        assert!(guard.set_pointer_tracking(level).is_err());
        assert!(guard.set_pointer_tracking(level).is_err());
        guard.restore().unwrap();
        let operations = control.operations();
        assert_eq!(
            operations
                .iter()
                .filter(|op| **op == Operation::PointerTracking(level))
                .count(),
            1
        );
        assert!(operations.contains(&Operation::DisablePointer));
        assert!(operations.contains(&Operation::DisableRaw));
    }
}

#[test]
fn failed_typed_pointer_enable_is_not_retried_before_full_cleanup() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([(Operation::EnablePointer, 1)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;

    assert!(
        guard
            .set_pointer_capture_level(Some(PointerTrackingLevel::AnyMotion))
            .is_err()
    );
    assert!(
        guard
            .set_pointer_capture_level(Some(PointerTrackingLevel::AnyMotion))
            .is_err()
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::EnablePointer)
            .count(),
        1
    );

    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::DisablePointer)
            .count(),
        1
    );
    Ok(())
}

#[test]
fn failed_typed_tracking_switch_is_fenced_until_release_settles() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([(
        Operation::PointerTracking(PointerTrackingLevel::AnyMotion),
        1,
    )]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    guard.set_pointer_capture_level(Some(PointerTrackingLevel::ButtonOnly))?;

    assert!(
        guard
            .set_pointer_tracking(PointerTrackingLevel::AnyMotion)
            .is_err()
    );
    assert!(
        guard
            .set_pointer_tracking(PointerTrackingLevel::ButtonOnly)
            .is_err()
    );
    guard.set_pointer_capture_level(None)?;
    guard.set_pointer_capture_level(Some(PointerTrackingLevel::AnyMotion))?;
    guard.restore().map_err(io::Error::other)?;

    let pointer: Vec<_> = control
        .operations()
        .into_iter()
        .filter(|operation| {
            matches!(
                operation,
                Operation::EnablePointer
                    | Operation::DisablePointer
                    | Operation::PointerTracking(_)
            )
        })
        .collect();
    assert_eq!(
        pointer,
        [
            Operation::EnablePointer,
            Operation::PointerTracking(PointerTrackingLevel::AnyMotion),
            Operation::DisablePointer,
            Operation::EnablePointer,
            Operation::DisablePointer,
        ]
    );
    Ok(())
}

#[derive(Clone)]
struct SetupBarrierTerminalModes {
    continue_setup: Arc<Barrier>,
    control: FakeTerminalModes,
    setup_started: Arc<Barrier>,
}

impl TerminalModes for SetupBarrierTerminalModes {
    fn disable_focus_reporting(&self) -> io::Result<()> {
        self.control.disable_focus_reporting()
    }
    fn enable_focus_reporting(&self) -> io::Result<()> {
        self.control.enable_focus_reporting()
    }

    fn disable_bracketed_paste(&self) -> io::Result<()> {
        self.control.disable_bracketed_paste()
    }

    fn enable_bracketed_paste(&self) -> io::Result<()> {
        self.control.enable_bracketed_paste()
    }

    fn disable_pointer_capture(&self) -> io::Result<()> {
        self.control.disable_pointer_capture()
    }

    fn enable_pointer_capture(&self, level: PointerTrackingLevel) -> io::Result<()> {
        self.control.enable_pointer_capture(level)
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
    let observed_thread = thread::current().id();
    let singleton_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Concurrent catch_unwind witnesses also invoke this process-wide hook.
        if thread::current().id() == observed_thread {
            calls_for_hook.fetch_add(1, Ordering::Relaxed);
        }
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
fn focus_reporting_partial_enable_retains_exact_restoration_obligation() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([(Operation::EnableFocus, 1)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    assert!(guard.set_focus_reporting(true).is_err());
    assert!(guard.set_focus_reporting(true).is_err());
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::EnableFocus)
            .count(),
        1
    );
    guard.restore().map_err(io::Error::other)?;
    let operations = control.operations();
    let focus = operations
        .iter()
        .position(|op| *op == Operation::DisableFocus)
        .unwrap();
    let alternate = operations
        .iter()
        .position(|op| *op == Operation::LeaveAlternate)
        .unwrap();
    assert!(focus < alternate);
    guard.restore().map_err(io::Error::other)?;
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::DisableFocus)
            .count(),
        1
    );
    Ok(())
}

#[test]
fn focus_reporting_toggle_is_idempotent_and_drop_restores_it() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    {
        let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
        guard.set_focus_reporting(true)?;
        guard.set_focus_reporting(true)?;
        guard.set_focus_reporting(false)?;
        guard.set_focus_reporting(false)?;
        guard.set_focus_reporting(true)?;
    }
    let operations = control.operations();
    assert_eq!(
        operations
            .iter()
            .filter(|op| **op == Operation::EnableFocus)
            .count(),
        2
    );
    assert_eq!(
        operations
            .iter()
            .filter(|op| **op == Operation::DisableFocus)
            .count(),
        2
    );
    Ok(())
}

#[test]
fn failed_focus_restore_does_not_skip_other_terminal_modes() -> io::Result<()> {
    let control = FakeTerminalModes::with_failures([(Operation::DisableFocus, 2)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    guard.set_focus_reporting(true)?;
    let error = guard.restore().unwrap_err();
    assert!(error.to_string().contains("disable focus reporting"));
    let operations = control.operations();
    assert!(operations.contains(&Operation::LeaveAlternate));
    assert!(operations.contains(&Operation::DisableRaw));
    Ok(())
}

#[test]
fn window_title_failed_write_retains_restore_debt_and_no_write_after_restore() -> io::Result<()> {
    let control =
        FakeTerminalModes::with_failures([(Operation::SetTitle, 1), (Operation::RestoreTitle, 1)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    assert!(guard.set_window_title("new").is_err());
    guard.restore().map_err(io::Error::other)?;
    let ops = control.operations();
    assert_eq!(
        ops.iter()
            .filter(|op| **op == Operation::RestoreTitle)
            .count(),
        2
    );
    assert!(
        ops.iter().position(|op| *op == Operation::RestoreTitle)
            < ops.iter().position(|op| *op == Operation::LeaveAlternate)
    );
    assert!(guard.set_window_title("late").is_err());
    assert_eq!(control.operations(), ops);
    Ok(())
}
#[test]
fn window_title_capture_preserves_unicode_and_rejects_ambiguous_truncation() {
    use super::captured_window_title;
    assert_eq!(
        captured_window_title(Ok("你好😀\0\0".into())).unwrap(),
        "你好😀"
    );
    assert_eq!(
        captured_window_title(Err(io::Error::from_raw_os_error(0))).unwrap(),
        ""
    );
    assert!(captured_window_title(Err(io::Error::from_raw_os_error(5))).is_err());
    assert!(captured_window_title(Ok(format!("{}\0", "x".repeat(259)))).is_err());
    assert!(captured_window_title(Ok("unbounded".into())).is_err());
    assert!(captured_window_title(Ok("x\0not-padding".into())).is_err());
}

#[test]
fn window_title_owner_panic_restores_exactly_once() -> io::Result<()> {
    let _lock = lock_panic_hook_tests()?;
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter(control.clone())?;
    guard.set_window_title("new")?;
    assert!(catch_unwind(AssertUnwindSafe(|| panic!("title owner panic"))).is_err());
    let count = || {
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::RestoreTitle)
            .count()
    };
    assert_eq!(count(), 1);
    assert!(guard.set_window_title("late").is_err());
    drop(guard);
    assert_eq!(count(), 1);
    Ok(())
}

#[test]
fn notification_terminal_writer_is_fenced_after_restoration() -> io::Result<()> {
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    let message = orchbee_workbench_protocol::Notification::normalize(
        "Ready",
        None,
        None,
        orchbee_workbench_protocol::NotificationSound::None,
    )
    .unwrap();
    guard.notify_terminal(&message)?;
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::Notification)
            .count(),
        1
    );
    guard.restore().map_err(io::Error::other)?;
    let before = control.operations();
    assert!(guard.notify_terminal(&message).is_err());
    assert_eq!(control.operations(), before);
    Ok(())
}

#[test]
fn graphics_queries_use_the_lifecycle_writer_and_cannot_write_after_restoration_or_fencing() {
    use crossterm::Command;
    let query = crate::terminal_graphics::Query {
        cell: true,
        image_id: Some(31),
    };
    let mut bytes = String::new();
    super::GraphicsQuery(query).write_ansi(&mut bytes).unwrap();
    assert_eq!(
        bytes,
        "\x1b[16t\x1b_Ga=q,t=d,f=32,s=1,v=1,i=31;AAAAAA==\x1b\\"
    );
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    guard.query_graphics(query).unwrap();
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::QueryGraphics)
            .count(),
        1
    );
    guard.discard_frame_output();
    assert_eq!(
        guard.query_graphics(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    guard.restore().unwrap();
    assert_eq!(
        guard.query_graphics(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    let mut restored =
        TerminalModeGuard::enter_without_panic_hook(FakeTerminalModes::default()).unwrap();
    restored.restore().unwrap();
    assert_eq!(
        restored.query_graphics(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn pointer_admission_query_uses_the_lifecycle_writer_and_obeys_output_fences() {
    let query = crate::terminal_pointer_admission::Query;
    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();

    guard.query_pointer_admission(query).unwrap();
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::QueryPointerAdmission)
            .count(),
        1
    );

    guard.discard_frame_output();
    assert_eq!(
        guard.query_pointer_admission(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    guard.restore().unwrap();
    assert_eq!(
        guard.query_pointer_admission(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|operation| **operation == Operation::QueryPointerAdmission)
            .count(),
        1,
        "a fenced query never reaches the terminal control writer"
    );
}

#[test]
fn host_appearance_query_uses_the_lifecycle_writer_without_acquiring_a_mode() {
    use crossterm::Command;

    let query = crate::terminal_host_appearance::Query;
    let mut bytes = String::new();
    query.write_ansi(&mut bytes).unwrap();
    assert_eq!(bytes, query.sequence());

    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    guard.query_host_appearance(query).unwrap();
    assert_eq!(
        control.operations(),
        vec![
            Operation::EnableRaw,
            Operation::EnterAlternate,
            Operation::HideCursor,
            Operation::QueryHostAppearance,
        ]
    );

    guard.discard_frame_output();
    assert_eq!(
        guard.query_host_appearance(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    guard.restore().unwrap();
    let before = control.operations();
    assert_eq!(
        guard.query_host_appearance(query).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(control.operations(), before);
}

#[test]
fn bell_requires_a_closed_frame_and_stops_after_write_failure_or_restoration() {
    let control = FakeTerminalModes::with_failures([(Operation::Bell, 1)]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    let lease = guard.synchronized_update_lease();
    lease.open_frame();
    assert_eq!(
        guard.ring_bell().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(!control.operations().contains(&Operation::Bell));
    lease.confirm_frame_closed();
    assert!(guard.ring_bell().is_err());
    assert!(lease.frame_output_blocked());
    assert_eq!(
        guard.ring_bell().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    guard.restore().unwrap();
    assert_eq!(
        guard.ring_bell().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::Bell)
            .count(),
        1
    );

    let control = FakeTerminalModes::default();
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
    guard.ring_bell().unwrap();
    assert!(!guard.synchronized_update_lease().frame_output_blocked());
    guard.restore().unwrap();
    assert_eq!(
        guard.ring_bell().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        control
            .operations()
            .iter()
            .filter(|op| **op == Operation::Bell)
            .count(),
        1
    );
}

#[test]
fn bell_bytes_are_flushed_once_and_a_failed_flush_is_not_replayed() {
    #[derive(Default)]
    struct Writer {
        bytes: Vec<u8>,
        flushes: usize,
        fail_flush: bool,
    }
    impl io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(())
            }
        }
    }
    for fail_flush in [false, true] {
        let mut output = Writer {
            fail_flush,
            ..Writer::default()
        };
        assert_eq!(super::write_bell(&mut output).is_err(), fail_flush);
        assert_eq!(output.bytes, b"\x07");
        assert_eq!(output.flushes, 1);
    }
}

#[test]
fn graphics_cleanup_precedes_mode_restore_and_retains_debt_after_failed_flush() {
    for failures in [0, 1, usize::MAX] {
        let control = FakeTerminalModes::with_failures([(Operation::CleanupGraphics, failures)]);
        let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone()).unwrap();
        let lease = guard.synchronized_update_lease();
        assert!(lease.graphics().register(3));
        assert!(lease.graphics().register(9));
        lease.graphics().control_started();
        lease.open_frame();
        let result = guard.restore();
        assert_eq!(result.is_err(), failures == usize::MAX);
        let operations = control.operations();
        let position = |operation| {
            operations
                .iter()
                .position(|item| *item == operation)
                .unwrap()
        };
        assert!(position(Operation::CleanupGraphics) < position(Operation::EndSynchronizedUpdate));
        assert!(position(Operation::CleanupGraphics) < position(Operation::LeaveAlternate));
        assert!(operations.contains(&Operation::DisableRaw));
        let sequences = control.state.lock().unwrap().graphics_cleanup.clone();
        assert_eq!(sequences.len(), if failures == 0 { 1 } else { 2 });
        for sequence in sequences {
            assert!(sequence.starts_with("\x18\x1b\\"));
            assert!(sequence.contains(&super::graphics::delete(3)));
            assert!(sequence.contains(&super::graphics::delete(9)));
            assert!(!sequence.contains(&super::graphics::delete(7)));
        }
        assert_eq!(
            lease.graphics().capacity(),
            if failures == usize::MAX { 126 } else { 128 }
        );
        assert!(lease.frame_output_blocked());
        let _result = guard.restore();
        assert_eq!(
            control.operations(),
            operations,
            "restoration cannot renew exhausted cleanup attempts"
        );
    }
}

// Static checks cannot prove restoration after an unknown host-effect write.
#[test]
fn host_background_write_failure_is_fenced_and_cleanup_runs_before_leaving_screen() -> io::Result<()>
{
    use crossterm::Command;
    let mut wire = String::new();
    super::HostBackground(Some(0x123456))
        .write_ansi(&mut wire)
        .unwrap();
    super::HostBackground(None).write_ansi(&mut wire).unwrap();
    assert!(wire.contains("\x1b]11;#123456\x1b\\"));
    assert!(wire.contains("\x1b]111\x1b\\"));
    let control = FakeTerminalModes::with_failures([
        (Operation::HostBackground(Some(0x123456)), 1),
        (Operation::HostBackground(None), 1),
    ]);
    let mut guard = TerminalModeGuard::enter_without_panic_hook(control.clone())?;
    assert!(guard.set_host_background(Some(0x123456)).is_err());
    assert!(guard.set_host_background(Some(0xabcdef)).is_err());
    guard.restore().map_err(io::Error::other)?;
    let ops = control.operations();
    assert_eq!(
        ops.iter()
            .filter(|op| **op == Operation::HostBackground(None))
            .count(),
        2
    );
    assert!(
        ops.iter()
            .position(|op| *op == Operation::HostBackground(None))
            < ops.iter().position(|op| *op == Operation::LeaveAlternate)
    );
    guard.restore().map_err(io::Error::other)?;
    assert!(guard.set_host_background(Some(0x123456)).is_err());
    assert_eq!(control.operations(), ops);
    Ok(())
}

#[test]
fn host_background_panic_and_terminal_native_release_consume_cleanup_once() -> io::Result<()> {
    let _lock = lock_panic_hook_tests()?;
    for release_first in [false, true] {
        let control = FakeTerminalModes::default();
        let mut guard = TerminalModeGuard::enter(control.clone())?;
        guard.set_host_background(None)?;
        assert!(
            !control
                .operations()
                .contains(&Operation::HostBackground(None))
        );
        guard.set_host_background(Some(0x123456))?;
        if release_first {
            guard.set_host_background(None)?;
        }
        assert!(catch_unwind(AssertUnwindSafe(|| panic!("background owner panic"))).is_err());
        drop(guard);
        assert_eq!(
            control
                .operations()
                .iter()
                .filter(|op| **op == Operation::HostBackground(None))
                .count(),
            1
        );
    }
    Ok(())
}
