use std::{
    cell::{Cell, RefCell},
    io::{self, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use super::{
    SavedConsoleModes, admit_windows_vt_and_enter_alternate, capture_and_enable_windows_raw_mode,
    leave_windows_alternate, restoration_state, restore_windows_console_modes,
};

fn debts(saved: &Mutex<Option<SavedConsoleModes>>) -> Option<(bool, bool)> {
    (*restoration_state(saved)).map(|saved| {
        (
            saved.output.restoration_required,
            saved.input.restoration_required,
        )
    })
}

fn capture_raw(saved: &Mutex<Option<SavedConsoleModes>>) -> io::Result<()> {
    capture_and_enable_windows_raw_mode(saved, || Ok(0x1234), || Ok(0x5678), |_| Ok(()))
}

#[test]
fn raw_admission_reads_before_debt_and_arms_input_before_native_mutation() {
    let saved = Mutex::new(None);
    let setter_called = Cell::new(false);
    let error = capture_and_enable_windows_raw_mode(
        &saved,
        || Ok(0x1234),
        || {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "output read",
            ))
        },
        |_| {
            setter_called.set(true);
            Ok(())
        },
    )
    .expect_err("a failed mode read must stop admission");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(!setter_called.get());
    assert_eq!(debts(&saved), None);

    let observed_mode = Cell::new(None);
    let error = capture_and_enable_windows_raw_mode(
        &saved,
        || Ok(0x1234),
        || Ok(0x5678),
        |mode| {
            assert_eq!(debts(&saved), Some((false, true)));
            observed_mode.set(Some(mode));
            Err(io::Error::other("input set"))
        },
    )
    .expect_err("the injected native mutation must fail");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(observed_mode.get(), Some((0x1234 | 0x0288) & !0x0057));
    assert_eq!(debts(&saved), Some((false, true)));
}

#[test]
fn native_setter_panic_keeps_published_debt_and_never_holds_the_state_lock() {
    let saved = Mutex::new(None);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = capture_and_enable_windows_raw_mode(
            &saved,
            || Ok(11),
            || Ok(22),
            |_| panic!("injected SetConsoleMode panic"),
        );
    }));
    assert!(outcome.is_err());
    assert_eq!(debts(&saved), Some((false, true)));

    let restored = Cell::new(None);
    restore_windows_console_modes(
        &saved,
        &AtomicBool::new(false),
        |_| panic!("no output debt exists"),
        |mode| {
            restored.set(Some(mode));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(restored.get(), Some(11));
    assert_eq!(debts(&saved), Some((false, false)));
}

#[test]
fn poisoned_saved_state_still_preserves_and_settles_complete_mode_words() {
    let saved = Mutex::new(None);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut state = saved.lock().unwrap();
        *state = None;
        panic!("poison the complete-value state");
    }));
    assert!(outcome.is_err());

    capture_raw(&saved).unwrap();
    assert_eq!(debts(&saved), Some((false, true)));
    restore_windows_console_modes(
        &saved,
        &AtomicBool::new(false),
        |_| unreachable!("output was never admitted"),
        |mode| {
            assert_eq!(mode, 0x1234);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(debts(&saved), Some((false, false)));
}

#[test]
fn failed_vt_mode_admission_writes_no_ansi_and_keeps_both_native_debts() {
    let saved = Mutex::new(None);
    capture_raw(&saved).unwrap();
    let vt = AtomicBool::new(false);
    let alternate = AtomicBool::new(false);
    let output_acquisitions = Cell::new(0);

    let error = admit_windows_vt_and_enter_alternate(
        &saved,
        &vt,
        &alternate,
        |mode| {
            assert_eq!(mode, 0x5678 | 0x0005);
            assert_eq!(debts(&saved), Some((true, true)));
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "VT set"))
        },
        || {
            output_acquisitions.set(output_acquisitions.get() + 1);
            Ok(io::sink())
        },
    )
    .expect_err("the injected VT mutation must fail");

    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(output_acquisitions.get(), 0);
    assert!(!vt.load(Ordering::Acquire));
    assert!(!alternate.load(Ordering::Acquire));
    assert_eq!(debts(&saved), Some((true, true)));
}

#[test]
fn writer_acquisition_failure_does_not_invent_an_alternate_screen_sequence() {
    let saved = Mutex::new(None);
    capture_raw(&saved).unwrap();
    let vt = AtomicBool::new(false);
    let alternate = AtomicBool::new(false);

    let error = admit_windows_vt_and_enter_alternate(
        &saved,
        &vt,
        &alternate,
        |_| Ok(()),
        || Err::<io::Sink, _>(io::Error::new(io::ErrorKind::BrokenPipe, "writer")),
    )
    .expect_err("the injected writer acquisition must fail");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert!(vt.load(Ordering::Acquire));
    assert!(!alternate.load(Ordering::Acquire));
    assert_eq!(debts(&saved), Some((true, true)));

    leave_windows_alternate(&vt, &alternate, || -> io::Result<io::Sink> {
        panic!("no leave is owed before the enter sequence starts")
    })
    .unwrap();
}

#[derive(Clone)]
struct PartialThenFails {
    accepted: Arc<Mutex<Vec<u8>>>,
    first: bool,
}

impl Write for PartialThenFails {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.first {
            self.first = false;
            let accepted = bytes.len().min(2);
            self.accepted
                .lock()
                .unwrap()
                .extend_from_slice(&bytes[..accepted]);
            Ok(accepted)
        } else {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fault after a positive prefix",
            ))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct AlwaysFails;

impl Write for AlwaysFails {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "leave fault"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_enter_owes_leave_and_a_failed_leave_cannot_suppress_native_restoration() {
    let saved = Mutex::new(None);
    capture_raw(&saved).unwrap();
    let vt = AtomicBool::new(false);
    let alternate = AtomicBool::new(false);
    let accepted = Arc::new(Mutex::new(Vec::new()));

    let error = admit_windows_vt_and_enter_alternate(
        &saved,
        &vt,
        &alternate,
        |_| Ok(()),
        || {
            Ok(PartialThenFails {
                accepted: Arc::clone(&accepted),
                first: true,
            })
        },
    )
    .expect_err("a partial alternate-screen command must fail");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(*accepted.lock().unwrap(), b"\x1b[");
    assert!(alternate.load(Ordering::Acquire));

    assert!(leave_windows_alternate(&vt, &alternate, || Ok(AlwaysFails)).is_err());
    assert!(alternate.load(Ordering::Acquire));

    let restorations = RefCell::new(Vec::new());
    restore_windows_console_modes(
        &saved,
        &vt,
        |mode| {
            restorations.borrow_mut().push(("output", mode));
            Ok(())
        },
        |mode| {
            restorations.borrow_mut().push(("input", mode));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        restorations.into_inner(),
        [("output", 0x5678), ("input", 0x1234)]
    );
    assert!(!vt.load(Ordering::Acquire));
    assert_eq!(debts(&saved), Some((false, false)));
}

#[test]
fn native_restorations_report_both_faults_and_retry_only_the_pending_side() {
    let saved = Mutex::new(None);
    capture_raw(&saved).unwrap();
    let vt = AtomicBool::new(false);
    let alternate = AtomicBool::new(false);
    admit_windows_vt_and_enter_alternate(
        &saved,
        &vt,
        &alternate,
        |_| Ok(()),
        || Ok(Vec::<u8>::new()),
    )
    .unwrap();
    leave_windows_alternate(&vt, &alternate, || Ok(Vec::<u8>::new())).unwrap();

    let attempts = RefCell::new(Vec::new());
    let error = restore_windows_console_modes(
        &saved,
        &vt,
        |mode| {
            attempts.borrow_mut().push(("output", mode));
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "output fault"))
        },
        |mode| {
            attempts.borrow_mut().push(("input", mode));
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "input fault",
            ))
        },
    )
    .expect_err("both native restorations must remain visible");
    assert!(error.to_string().contains("output fault"));
    assert!(error.to_string().contains("input fault"));
    assert_eq!(debts(&saved), Some((true, true)));

    let error = restore_windows_console_modes(
        &saved,
        &vt,
        |mode| {
            attempts.borrow_mut().push(("output", mode));
            Ok(())
        },
        |mode| {
            attempts.borrow_mut().push(("input", mode));
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "input again",
            ))
        },
    )
    .expect_err("the input side remains pending");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(!vt.load(Ordering::Acquire));
    assert_eq!(debts(&saved), Some((false, true)));

    restore_windows_console_modes(
        &saved,
        &vt,
        |_| panic!("the settled output side must not be retried"),
        |mode| {
            attempts.borrow_mut().push(("input", mode));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(debts(&saved), Some((false, false)));
    assert_eq!(
        attempts.into_inner(),
        [
            ("output", 0x5678),
            ("input", 0x1234),
            ("output", 0x5678),
            ("input", 0x1234),
            ("input", 0x1234),
        ]
    );
}
