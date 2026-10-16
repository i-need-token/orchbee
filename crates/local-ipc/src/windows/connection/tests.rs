use super::*;
use std::cell::Cell;

#[test]
fn synthetic_timeout_is_pending_only_before_a_pipe_opens() {
    let deadline = Instant::now() + Duration::from_secs(5);
    let pending = attempt(
        deadline,
        Duration::from_millis(10),
        Instant::now,
        |_| Err::<(), _>(io::ErrorKind::TimedOut.into()),
        |_| -> io::Result<()> { panic!("no candidate to authenticate") },
    )
    .expect("dependency wait exhaustion is pending");
    assert_eq!(pending, None);
    let error = attempt(
        deadline,
        Duration::from_millis(10),
        Instant::now,
        |_| Ok(()),
        |_| Err::<(), _>(io::ErrorKind::TimedOut.into()),
    )
    .unwrap_err();
    let ConnectAttemptError::Admission(error) = error else {
        panic!("opened pipe lost phase")
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(error.raw_os_error(), None);
}

#[test]
fn authentication_crosses_wait_slice_without_losing_candidate() {
    let started = Instant::now();
    let now = Cell::new(started);
    let result = attempt(
        started + Duration::from_secs(5),
        Duration::from_millis(10),
        || now.get(),
        |wait| {
            assert_eq!(wait, Duration::from_millis(10));
            Ok(17)
        },
        |candidate| {
            now.set(started + Duration::from_millis(11));
            Ok(candidate)
        },
    )
    .unwrap();
    assert_eq!(result, Some(17));
}

#[test]
fn only_preconnection_busy_or_wait_timeout_is_pending() {
    for code in [121, 231] {
        let result = attempt(
            Instant::now() + Duration::from_secs(5),
            Duration::from_millis(10),
            Instant::now,
            |_| Err::<(), _>(io::Error::from_raw_os_error(code)),
            |_| -> io::Result<()> { panic!("no connected candidate to authenticate") },
        )
        .unwrap();
        assert_eq!(result, None);
    }
    for code in [2, 5] {
        let error = attempt(
            Instant::now() + Duration::from_secs(5),
            Duration::from_millis(10),
            Instant::now,
            |_| Err::<(), _>(io::Error::from_raw_os_error(code)),
            |_| -> io::Result<()> { panic!("dial failed") },
        )
        .unwrap_err();
        let ConnectAttemptError::Dial(error) = error else {
            panic!("preconnection error")
        };
        assert_eq!(error.raw_os_error(), Some(code));
    }
}

#[test]
fn authentication_errors_keep_postconnection_provenance_including_timeout_codes() {
    for code in [2, 5, 121, 231] {
        let error = attempt(
            Instant::now() + Duration::from_secs(5),
            Duration::from_millis(10),
            Instant::now,
            |_| Ok(()),
            |_| Err::<(), _>(io::Error::from_raw_os_error(code)),
        )
        .unwrap_err();
        let ConnectAttemptError::Admission(error) = error else {
            panic!("candidate already existed")
        };
        assert_eq!(error.raw_os_error(), Some(code));
    }
}

#[test]
fn whole_deadline_still_discards_and_drops_a_late_authenticated_candidate() {
    struct Candidate<'a>(&'a Cell<usize>);
    impl Drop for Candidate<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let started = Instant::now();
    let deadline = started + Duration::from_millis(5);
    let now = Cell::new(started);
    let drops = Cell::new(0);
    let result = attempt(
        deadline,
        Duration::from_millis(10),
        || now.get(),
        |wait| {
            assert_eq!(wait, Duration::from_millis(5));
            Ok(Candidate(&drops))
        },
        |candidate| {
            now.set(deadline);
            Ok(candidate)
        },
    );
    let Err(ConnectAttemptError::Admission(error)) = result else {
        panic!("late candidate accepted")
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(drops.get(), 1);
}

#[test]
fn expired_deadline_or_zero_wait_limit_never_dials() {
    let now = Instant::now();
    for (deadline, limit, expected) in [
        (now, Duration::from_millis(10), io::ErrorKind::TimedOut),
        (
            now + Duration::from_secs(5),
            Duration::ZERO,
            io::ErrorKind::InvalidInput,
        ),
    ] {
        let result = attempt(
            deadline,
            limit,
            || now,
            |_| -> io::Result<()> { panic!("invalid attempt dialed") },
            |_| -> io::Result<()> { panic!("no candidate") },
        );
        let Err(ConnectAttemptError::Dial(error)) = result else {
            panic!("invalid attempt admitted")
        };
        assert_eq!(error.kind(), expected);
    }
}
