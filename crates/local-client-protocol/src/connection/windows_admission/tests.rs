use super::*;
use std::cell::Cell;

#[test]
fn authenticated_connection_may_outlast_poll_slice_within_operation_deadline() {
    let started = Instant::now();
    let now = Cell::new(started);
    let attempts = Cell::new(0);
    let connected = connect_stream_with(
        started + Duration::from_secs(5),
        || now.get(),
        || Ok(()),
        |completion_deadline, wait_limit| {
            attempts.set(attempts.get() + 1);
            assert_eq!(wait_limit, Duration::from_millis(10));
            // The kernel dial succeeds; authentication/scheduling crosses only the poll slice.
            now.set(started + Duration::from_millis(11));
            if now.get() >= completion_deadline {
                Err(ConnectClientError::ConnectFailed(
                    io::ErrorKind::TimedOut.into(),
                ))
            } else {
                Ok(Some("authenticated candidate"))
            }
        },
    );
    assert!(connected.is_ok(), "candidate was discarded: {connected:?}");
    assert_eq!(attempts.get(), 1);
}

#[test]
fn preconnection_pending_rechecks_wake_without_renewing_deadline() {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(5);
    let attempts = Cell::new(0);
    let wakes = Cell::new(0);
    let result = connect_stream_with(
        deadline,
        || started,
        || {
            wakes.set(wakes.get() + 1);
            if wakes.get() == 2 {
                Err(ConnectClientError::CancellationReady)
            } else {
                Ok(())
            }
        },
        |actual_deadline, _| {
            assert_eq!(actual_deadline, deadline);
            attempts.set(attempts.get() + 1);
            Ok(None::<()>)
        },
    );
    assert!(matches!(result, Err(ConnectClientError::CancellationReady)));
    assert_eq!(attempts.get(), 1);
}

#[test]
fn connected_authentication_error_is_not_retried_even_with_a_wait_timeout_code() {
    for code in [Some(121), None] {
        let attempts = Cell::new(0);
        let result = connect_stream_with(
            Instant::now() + Duration::from_secs(5),
            Instant::now,
            || Ok(()),
            |_, _| {
                attempts.set(attempts.get() + 1);
                let error = code.map_or_else(
                    || io::ErrorKind::TimedOut.into(),
                    io::Error::from_raw_os_error,
                );
                Err::<Option<()>, _>(ConnectClientError::Io(error))
            },
        );
        let Err(ConnectClientError::Io(error)) = result else {
            panic!("authentication failure changed phase")
        };
        assert_eq!(error.raw_os_error(), code);
        if code.is_none() {
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        }
        assert_eq!(attempts.get(), 1);
    }
}

#[test]
fn pending_waits_then_success_keep_whole_deadline_and_check_each_wake() {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(5);
    let now = Cell::new(started);
    let attempts = Cell::new(0);
    let wakes = Cell::new(0);
    let result = connect_stream_with(
        deadline,
        || now.get(),
        || {
            wakes.set(wakes.get() + 1);
            Ok(())
        },
        |actual_deadline, wait_limit| {
            assert_eq!(actual_deadline, deadline);
            assert_eq!(wait_limit, Duration::from_millis(10));
            attempts.set(attempts.get() + 1);
            now.set(now.get() + wait_limit);
            Ok(if attempts.get() < 3 {
                None
            } else {
                Some("one authenticated connection")
            })
        },
    )
    .unwrap();
    assert_eq!(result, "one authenticated connection");
    assert_eq!(attempts.get(), 3);
    assert_eq!(wakes.get(), 4);
}

#[test]
fn late_or_cancelled_connected_candidate_drops_before_hello_without_redial() {
    struct Candidate<'a>(&'a Cell<usize>);
    impl Drop for Candidate<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for cancel in [false, true] {
        let started = Instant::now();
        let deadline = started + Duration::from_secs(5);
        let now = Cell::new(started);
        let drops = Cell::new(0);
        let attempts = Cell::new(0);
        let wakes = Cell::new(0);
        let result = connect_stream_with(
            deadline,
            || now.get(),
            || {
                wakes.set(wakes.get() + 1);
                if cancel && wakes.get() == 2 {
                    Err(ConnectClientError::CancellationReady)
                } else {
                    Ok(())
                }
            },
            |_, _| {
                attempts.set(attempts.get() + 1);
                if !cancel {
                    now.set(deadline);
                }
                Ok(Some(Candidate(&drops)))
            },
        );
        if cancel {
            assert!(matches!(result, Err(ConnectClientError::CancellationReady)));
        } else {
            assert!(
                matches!(result, Err(ConnectClientError::Io(error)) if error.kind() == io::ErrorKind::TimedOut)
            );
        }
        assert_eq!(attempts.get(), 1);
        assert_eq!(drops.get(), 1);
    }
}
