//! `FakeServer` admission guards: the accept loop must fail bounded — a
//! missing client can never leave `finish` joining a live thread forever —
//! while a timely client, handler results, and handler failures keep their
//! established semantics. The real `start` path is proven once against its
//! true guard; every other case uses either an immediate client or the
//! internal short-deadline entry so the suite does not pay the guard per
//! test.

use super::*;
use std::sync::mpsc::sync_channel;
use std::{
    io,
    os::unix::net::UnixStream,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// How long `finish` may take before the test itself is declared hung: far
/// above the accept guard so only a genuinely unbounded join can trip it.
const FINISH_BACKSTOP: Duration = Duration::from_secs(10);

/// Run `finish` on a side thread with a hard backstop: if a regression ever
/// makes `finish` join forever, the test fails after `FINISH_BACKSTOP`
/// instead of hanging the suite.
fn finish_bounded(server: FakeServer) -> io::Result<()> {
    let (result, returned) = sync_channel(1);
    thread::spawn(move || {
        let _ = result.send(server.finish());
    });
    returned
        .recv_timeout(FINISH_BACKSTOP)
        .expect("finish must complete inside the backstop")
}

/// The real `start` entry with no client: `finish` must error `TimedOut`
/// inside the accept guard and the handler must never run. The elapsed wall
/// clock is the wiring proof — roughly `ACCEPT_GUARD_TIMEOUT`, not instant
/// and not unbounded.
#[test]
fn start_without_client_times_out_bounded() {
    let called = Arc::new(AtomicBool::new(false));
    let flag = called.clone();
    // The accept deadline starts inside `start`, so the clock starts before
    // the call — otherwise legal thread scheduling could measure an elapsed
    // time just below the guard.
    let began = Instant::now();
    let server = FakeServer::start(move |_stream| {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .expect("server starts");
    let error = finish_bounded(server).expect_err("no client must not pass");
    let elapsed = began.elapsed();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(
        elapsed >= ACCEPT_GUARD_TIMEOUT && elapsed < FINISH_BACKSTOP,
        "accept guard should fire near {ACCEPT_GUARD_TIMEOUT:?}, took {elapsed:?}"
    );
    assert!(!called.load(Ordering::SeqCst), "handler never runs");
}

/// A client arriving inside the guard is admitted exactly once and the
/// handler's `Ok` result survives `finish`.
#[test]
fn start_with_timely_client_runs_handler_once() {
    let calls = Arc::new(AtomicU64::new(0));
    let count = calls.clone();
    let server = FakeServer::start(move |_stream| {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
    .expect("server starts");
    let _client = UnixStream::connect(server.endpoint()).expect("client connects");
    finish_bounded(server).expect("handler result preserved");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// A handler `Err` reaches the caller unchanged — the guard must not turn
/// handler failures into passes.
#[test]
fn start_propagates_handler_error() {
    let server = FakeServer::start(|_stream| {
        Err(io::Error::new(io::ErrorKind::InvalidData, "handler marker"))
    })
    .expect("server starts");
    let _client = UnixStream::connect(server.endpoint()).expect("client connects");
    let error = finish_bounded(server).expect_err("handler error surfaces");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("handler marker"));
}

/// A handler panic is not swallowed: `join` maps it to the established
/// "fake Server panicked" error.
#[test]
fn start_propagates_handler_panic() {
    let server = FakeServer::start(|_stream| -> io::Result<()> { panic!("handler panic marker") })
        .expect("server starts");
    let _client = UnixStream::connect(server.endpoint()).expect("client connects");
    let error = finish_bounded(server).expect_err("panic surfaces");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(error.to_string().contains("panicked"));
}

/// The internal short-deadline entry proves the shared accept guard returns
/// a clear `TimedOut` quickly — the suite does not spend the full 3 s guard
/// on timeout coverage, while `start` itself is proven wired by the real
/// guard test above.
#[test]
fn start_with_short_accept_deadline_times_out_fast() {
    let began = Instant::now();
    let server =
        FakeServer::start_with_accept_deadline(|_stream| Ok(()), Duration::from_millis(50))
            .expect("server starts");
    let error = finish_bounded(server).expect_err("no client must not pass");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(began.elapsed() < Duration::from_secs(1));
}

/// `start_connections` still admits every connection through the shared
/// bounded accept — multi-admission is unchanged by the helper extraction.
#[test]
fn start_connections_admits_each_connection() {
    let seen = Arc::new(AtomicU64::new(0));
    let flags = seen.clone();
    let server = FakeServer::start_connections(2, move |index, _stream| {
        flags.fetch_or(1 << index, Ordering::SeqCst);
        Ok(())
    })
    .expect("server starts");
    let _first = UnixStream::connect(server.endpoint()).expect("first client");
    let _second = UnixStream::connect(server.endpoint()).expect("second client");
    finish_bounded(server).expect("both admissions finish");
    assert_eq!(seen.load(Ordering::SeqCst), 0b11);
}
