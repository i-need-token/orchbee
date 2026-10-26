//! `FakeServer` admission guards: the accept loop must fail bounded — a
//! missing client can never leave `finish` joining a live thread forever —
//! while a timely client, handler results, and handler failures keep their
//! established semantics. The real `start` path is proven once against its
//! true guard; every other case uses either an immediate client or the
//! internal short-deadline entry so the suite does not pay the guard per
//! test.
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, read_workbench_request,
    write_workbench_response,
};

use super::*;
use crate::local_client::LocalClient;
use std::sync::mpsc::sync_channel;
use std::{
    io,
    sync::{
        Arc, Condvar, Mutex,
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

/// Independent native readers must not inherit head-of-line blocking from
/// another admitted reader that has received its Snapshot but has not yet
/// issued the next request. Cleanup precedes the assertion so the expected
/// failure against the serial helper cannot retain a pipe or worker thread.
#[test]
fn optional_connections_admit_a_pending_destination_while_an_observation_is_paused()
-> io::Result<()> {
    const COHORT: &str = "independent-lane-fixture";
    let request = SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("11111111111111111111111111111111")
            .map_err(io::Error::other)?,
    };
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let handler_release = Arc::clone(&release);
    let (destination_attempted, attempted) = sync_channel(1);
    let (destination_admitted, admitted) = sync_channel(1);
    let server = FakeServer::start_optional_connections(2, move |index, mut stream| {
        if !matches!(
            read_message(&mut stream).map_err(io::Error::other)?,
            LocalClientMessage::ClientHello(_)
        ) {
            return Err(io::Error::other("lane must begin with ClientHello"));
        }
        write_message(
            &mut stream,
            &LocalClientMessage::ClientAccepted {
                generation: std::num::NonZeroU64::MIN,
            },
        )
        .map_err(io::Error::other)?;
        if index == 0 {
            let WorkbenchRequest::SnapshotRead(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("observation must begin with SnapshotRead"));
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchSnapshotOutcome::Denied {},
                }),
            )
            .map_err(io::Error::other)?;
            let (released, wake) = &*handler_release;
            let mut released = released
                .lock()
                .map_err(|_| io::Error::other("observation release gate poisoned"))?;
            while !*released {
                released = wake
                    .wait(released)
                    .map_err(|_| io::Error::other("observation release gate poisoned"))?;
            }
            return Ok(());
        }
        let _ = destination_admitted.send(());
        Ok(())
    })?;

    let deadline = Instant::now() + ACCEPT_GUARD_TIMEOUT;
    let mut first_error = None;
    let mut observation = match LocalClient::connect_before(server.endpoint(), COHORT, deadline) {
        Ok(client) => Some(client),
        Err(error) => {
            first_error = Some(io::Error::other(error));
            None
        }
    };
    if let Some(client) = &mut observation {
        match client.submit_workbench_snapshot_read_before(request, deadline) {
            Ok(result) if matches!(result.outcome, WorkbenchSnapshotOutcome::Denied {}) => {}
            Ok(result) => {
                first_error = Some(io::Error::other(format!(
                    "unexpected observation Snapshot result: {result:?}"
                )));
            }
            Err(error) => first_error = Some(io::Error::other(error)),
        }
    }

    let destination = if first_error.is_none() {
        let endpoint = server.endpoint().to_owned();
        Some(thread::spawn(move || -> io::Result<LocalClient> {
            let _ = destination_attempted.send(());
            LocalClient::connect_before(&endpoint, COHORT, deadline).map_err(io::Error::other)
        }))
    } else {
        None
    };
    let attempt_pending =
        destination.is_some() && attempted.recv_timeout(SERVER_GUARD_TIMEOUT).is_ok();
    if destination.is_some() && !attempt_pending && first_error.is_none() {
        first_error = Some(io::Error::new(
            io::ErrorKind::TimedOut,
            "destination connect attempt did not start",
        ));
    }
    let admitted_before_release =
        attempt_pending && admitted.recv_timeout(SERVER_GUARD_TIMEOUT).is_ok();

    {
        let (released, wake) = &*release;
        let mut released = released
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *released = true;
        wake.notify_all();
    }
    drop(observation.take());
    if let Some(destination) = destination {
        match destination.join() {
            Ok(Ok(destination)) => drop(destination),
            Ok(Err(error)) if first_error.is_none() => first_error = Some(error),
            Err(_) if first_error.is_none() => {
                first_error = Some(io::Error::other("destination connection worker panicked"));
            }
            _ => {}
        }
    }
    if let Err(error) = finish_bounded(server)
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    assert!(
        admitted_before_release,
        "an admitted observation serialized the pending destination connect attempt"
    );
    Ok(())
}

#[test]
fn optional_connections_join_all_handlers_and_preserve_a_later_error() {
    const LANES: usize = 3;
    let entered = Arc::new(std::sync::Barrier::new(LANES));
    let settled = Arc::new(std::sync::Barrier::new(LANES));
    let settled_bits = Arc::new(AtomicU64::new(0));
    let (reported, reports) = sync_channel(LANES);
    let handler_entered = Arc::clone(&entered);
    let handler_settled = Arc::clone(&settled);
    let handler_bits = Arc::clone(&settled_bits);
    let server = FakeServer::start_optional_connections(LANES, move |index, _stream| {
        reported
            .send(index)
            .map_err(|_| io::Error::other("admission report receiver dropped"))?;
        handler_entered.wait();
        handler_bits.fetch_or(1 << index, Ordering::SeqCst);
        handler_settled.wait();
        if index == 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "later handler marker",
            ));
        }
        Ok(())
    })
    .expect("server starts");
    let clients = (0..LANES)
        .map(|_| UnixStream::connect(server.endpoint()).expect("client connects"))
        .collect::<Vec<_>>();
    let mut admitted = (0..LANES)
        .map(|_| {
            reports
                .recv_timeout(SERVER_GUARD_TIMEOUT)
                .expect("each handler starts")
        })
        .collect::<Vec<_>>();
    admitted.sort_unstable();
    assert_eq!(admitted, vec![0, 1, 2]);
    let error = finish_bounded(server).expect_err("later handler error must surface");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("later handler marker"));
    assert_eq!(settled_bits.load(Ordering::SeqCst), 0b111);
    drop(clients);
}
