use super::*;
use crate::{
    local_client::test_support::server::FakeServer,
    machine_agents::{Checkpoint, test_support::checkpoint, tests::accept},
};
use std::{io, thread};
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn source_connection() -> io::Result<Connection> {
    // The scheduler seam never reconnects; this admission supplies a real owned origin.
    let peer = FakeServer::start(|mut stream| accept(&mut stream))?;
    let client = LocalClient::connect_before(
        peer.endpoint(),
        "test-cohort",
        Instant::now() + Duration::from_secs(3),
    )
    .map_err(io::Error::other)?;
    let source = Connection::from_origin(client.origin(), client.operation_clock());
    drop(client);
    peer.finish()?;
    Ok(source)
}
fn sources(connection: &Connection, count: usize) -> Vec<Source> {
    (0..count)
        .map(|n| Source {
            choice: Choice::Remote(ProfileId::parse(&id(n as u64 + 1)).unwrap()),
            connection: connection.clone(),
        })
        .collect()
}
fn completed(at: Instant) -> Job<Checkpoint> {
    let task = thread::spawn(move || Completed {
        at,
        rows: Ok(checkpoint(Vec::new())),
    });
    let cutoff = Instant::now() + Duration::from_secs(3);
    while !task.is_finished() {
        assert!(Instant::now() < cutoff);
        thread::yield_now();
    }
    Job {
        cancel: Arc::new(OwnedCancellation::new().unwrap()),
        task: BackgroundThread::adopt(task),
    }
}
#[test]
fn observation_collector_bounds_workers_and_cancels_removed_sources() -> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::<Checkpoint>::default();
    let mut flags = Vec::new();
    collector.tick_with_start(sources(&connection, 10), now, |_, deadline| {
        assert_eq!(deadline, now + Duration::from_secs(5));
        let cancel = Arc::new(OwnedCancellation::new().unwrap());
        flags.push(cancel.clone());
        let worker = cancel.clone();
        let task = thread::spawn(move || {
            while !worker.is_cancelled() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            Completed {
                at: Instant::now(),
                rows: Ok(checkpoint(Vec::new())),
            }
        });
        Ok(Job {
            cancel,
            task: BackgroundThread::adopt(task),
        })
    });
    assert_eq!(flags.len(), 4);
    collector.tick_with_start(sources(&connection, 10), now, |_, _| {
        panic!("no fifth worker")
    });
    assert!(
        collector
            .tick_with_start(Vec::new(), now, |_, _| panic!("removed source"))
            .is_empty()
    );
    assert!(flags.iter().all(|f| f.is_cancelled()));
    drop(collector); // Joins every cancelled writer before fixture disposal.
    Ok(())
}
#[test]
fn observation_collector_rotates_past_four_sources_without_waiting_for_first_machines_to_stale()
-> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::<Checkpoint>::default();
    let mut latest = Vec::new();
    for turn in 0..5 {
        let at = now + Duration::from_millis(turn);
        latest = collector.tick_with_start(sources(&connection, 10), at, |_, _| Ok(completed(at)));
    }
    assert_eq!(latest.len(), 10);
    assert!(
        latest
            .iter()
            .all(|(_, o)| matches!(o, Observation::Ready(rows) if rows.is_empty()))
    );
    Ok(())
}
#[test]
fn observation_collector_ages_from_worker_completion_and_discards_previous_generation()
-> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut collector = Collector::<Checkpoint>::default();
    collector.sample_external_sources(true);
    collector.tick_with_start(sources(&connection, 1), now, |_, _| Ok(completed(now)));
    let stale = collector.tick_with_start(
        sources(&connection, 1),
        now + FRESH_FOR + Duration::from_secs(1),
        |_, _| panic!("cooldown starts at consumption"),
    );
    assert_eq!(stale[0].1, Observation::Unavailable);
    let replacement = source_connection()?;
    let next = collector.tick_with_start(sources(&replacement, 1), now, |_, _| Ok(completed(now)));
    assert_eq!(
        next[0].1,
        Observation::Loading,
        "the same logical choice cannot retain a checkpoint from a replaced carrier"
    );
    Ok(())
}

#[test]
fn carrier_snapshot_gate_and_reset_retire_completed_reads_before_accepting_fresh_state()
-> io::Result<()> {
    use crate::local_client::test_support::multiplex_peer::{Open, admit, read_open, respond};
    use orchbee_workbench_protocol::{
        MultiplexMessage, StreamFrameKind, StreamMode, WorkbenchRequest, WorkbenchResponse,
        WorkbenchRevision, WorkbenchSnapshotResult, read_multiplex_message, read_workbench_request,
        write_multiplex_message,
    };
    use std::{num::NonZeroU64, sync::mpsc};
    fn reply(open: &Open, revision: u64) -> WorkbenchResponse {
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap()
        else {
            panic!("only snapshot reads");
        };
        let mut snapshot = crate::machine_agents::test_support::fixture();
        snapshot.revision = WorkbenchRevision::new(revision);
        WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        })
    }
    fn read_revision(
        client: &mut LocalClient,
        deadline: Instant,
        cancel: Cancellation<'_>,
    ) -> Result<Arc<u64>, ()> {
        snapshot(client, deadline, cancel).map(|snapshot| Arc::new(snapshot.revision.value()))
    }
    let (registered, registration) = mpsc::sync_channel(1);
    let (initial, initial_allowed) = mpsc::sync_channel(1);
    let (second, second_started) = mpsc::sync_channel(1);
    let (race, run_race) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let watch = read_open(&mut peer)?;
        assert_eq!(watch.mode, StreamMode::Invalidations);
        registered.send(()).unwrap();
        initial_allowed
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        respond(&mut peer, &watch, &reply(&watch, 1), 1)?;
        let first = read_open(&mut peer)?;
        assert_eq!(first.mode, StreamMode::Request);
        // Publish while the first complete read is still pending. The second read is necessary.
        write_multiplex_message(
            &mut peer,
            &MultiplexMessage::StreamFrame {
                stream_id: watch.id,
                sequence: NonZeroU64::new(2).unwrap(),
                kind: StreamFrameKind::Update,
                record: 2_u64.to_be_bytes().to_vec(),
            },
        )?;
        respond(&mut peer, &first, &reply(&first, 1), 1)?;
        let prior = read_open(&mut peer)?;
        second.send(()).unwrap();
        run_race.recv_timeout(Duration::from_secs(2)).unwrap();
        respond(&mut peer, &prior, &reply(&prior, 2), 1)?;
        write_multiplex_message(
            &mut peer,
            &MultiplexMessage::Gap {
                stream_id: watch.id,
                through: NonZeroU64::new(2).unwrap(),
            },
        )?;
        let mut record = Vec::new();
        orchbee_workbench_protocol::write_workbench_response(&mut record, &reply(&watch, 3))
            .map_err(io::Error::other)?;
        write_multiplex_message(
            &mut peer,
            &MultiplexMessage::StreamFrame {
                stream_id: watch.id,
                sequence: NonZeroU64::new(3).unwrap(),
                kind: StreamFrameKind::Snapshot,
                record,
            },
        )?;
        // A separate correlated reply fences reception of both reset and fresh snapshot before
        // the UI drains this watch. The preceding job is deliberately already complete.
        let fence = read_open(&mut peer)?;
        respond(&mut peer, &fence, &reply(&fence, 30), 1)?;
        let fresh = read_open(&mut peer)?;
        respond(&mut peer, &fresh, &reply(&fresh, 3), 1)?;
        loop {
            match read_multiplex_message(&mut peer) {
                Ok(MultiplexMessage::StreamClose { .. } | MultiplexMessage::LaneClose { .. }) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                other => panic!("no periodic canonical read: {other:?}"),
            }
        }
    })?;
    let mut client = LocalClient::connect(server.endpoint(), "test-cohort")
        .map_err(io::Error::other)?
        .into_multiplexed()?;
    let source = Connection::from_origin(client.origin(), client.operation_clock());
    let mut collector = Collector::<u64>::default();
    let start = Instant::now();
    collector.tick(sources(&source, 1), start, read_revision);
    registration.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        collector.tick(sources(&source, 1), start, read_revision)[0].1,
        Observation::Loading
    );
    assert!(
        collector.entries[0].job.is_none(),
        "registration alone cannot admit a read"
    );
    initial.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        collector.tick(sources(&source, 1), Instant::now(), read_revision);
        if second_started.try_recv().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    race.send(()).unwrap();
    assert_eq!(
        *read_revision(&mut client, deadline, Cancellation::new(None)).unwrap(),
        30
    );
    while !collector.entries[0].job.as_ref().unwrap().finished() {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let after_reset = collector.tick(sources(&source, 1), Instant::now(), read_revision);
    assert_eq!(
        after_reset[0].1,
        Observation::Unavailable,
        "old completed read cannot cross the reset even when its fresh snapshot is coalesced"
    );
    loop {
        let current = collector.tick(sources(&source, 1), Instant::now(), read_revision);
        if matches!(&current[0].1, Observation::Ready(revision) if **revision == 3) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let later = collector.tick(
        sources(&source, 1),
        Instant::now() + Duration::from_secs(3),
        read_revision,
    );
    assert!(matches!(&later[0].1, Observation::Ready(revision) if **revision == 3));
    assert!(
        collector.entries[0].job.is_none(),
        "canonical state does not start a two-second fallback read"
    );
    drop((collector, source, client));
    server.finish()
}

fn pending<T: Send + Sync + 'static>(
    deadline: Instant,
    flags: &mut Vec<Arc<OwnedCancellation>>,
) -> Job<T> {
    let cancel = Arc::new(OwnedCancellation::new().unwrap());
    flags.push(cancel.clone());
    let flag = cancel.clone();
    let task = thread::spawn(move || {
        while !flag.is_cancelled() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        Completed {
            at: Instant::now(),
            rows: Err(()),
        }
    });
    Job {
        cancel,
        task: BackgroundThread::adopt(task),
    }
}

#[test]
fn failed_subscription_admission_and_first_read_recover_without_successful_state_polling()
-> io::Result<()> {
    use crate::local_client::test_support::multiplex_peer::{admit, read_open, respond};
    use orchbee_workbench_protocol::{
        MultiplexMessage, StreamMode, WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotResult,
        WorkbenchSnapshotUnavailableReason, read_multiplex_message, read_workbench_request,
    };
    use std::sync::mpsc;
    fn read_revision(
        client: &mut LocalClient,
        deadline: Instant,
        cancel: Cancellation<'_>,
    ) -> Result<Arc<u64>, ()> {
        snapshot(client, deadline, cancel).map(|snapshot| Arc::new(snapshot.revision.value()))
    }
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&id(1)).unwrap(),
    });
    let (full, filled) = mpsc::sync_channel(1);
    let (closed, released) = mpsc::sync_channel(1);
    let server = FakeServer::start(move |mut peer| {
        admit(&mut peer, 1)?;
        let mut last = None;
        for _ in 0..63 {
            last = Some(read_open(&mut peer)?.id);
        }
        full.send(()).unwrap();
        assert!(
            matches!(read_multiplex_message(&mut peer)?, MultiplexMessage::StreamClose { stream_id, .. } if Some(stream_id) == last)
        );
        closed.send(()).unwrap();
        for (mode, available) in [
            (StreamMode::Invalidations, true),
            (StreamMode::Request, false),
            (StreamMode::Request, true),
        ] {
            let open = read_open(&mut peer)?;
            assert_eq!(open.mode, mode);
            let WorkbenchRequest::SnapshotRead(request) =
                read_workbench_request(&mut io::Cursor::new(&open.request)).unwrap()
            else {
                panic!("snapshot read");
            };
            respond(
                &mut peer,
                &open,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: if available {
                        WorkbenchSnapshotOutcome::Available {
                            snapshot: crate::machine_agents::test_support::fixture(),
                        }
                    } else {
                        WorkbenchSnapshotOutcome::Unavailable {
                            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                        }
                    },
                }),
                1,
            )?;
        }
        loop {
            match read_multiplex_message(&mut peer) {
                Ok(MultiplexMessage::StreamClose { .. } | MultiplexMessage::LaneClose { .. }) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                value => panic!("successful canonical state must stay idle: {value:?}"),
            }
        }
    })?;
    let client = LocalClient::connect(server.endpoint(), "test-cohort")
        .map_err(io::Error::other)?
        .into_multiplexed()?;
    let mut held = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while held.len() < 63 {
        match client.origin().subscribe(&request, StreamMode::State) {
            Ok(stream) => held.push(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
            Err(error) => return Err(error),
        }
    }
    filled.recv_timeout(Duration::from_secs(2)).unwrap();
    let source = Connection::from_origin(client.origin(), client.operation_clock());
    let mut collector = Collector::<u64>::default();
    let now = Instant::now();
    assert_eq!(
        collector.tick(sources(&source, 1), now, read_revision)[0].1,
        Observation::Unavailable
    );
    let retry = collector.entries[0]
        .watch_retry
        .expect("capacity failure schedules bounded admission recovery");
    assert!(retry > now);
    drop(held.pop());
    released.recv_timeout(Duration::from_secs(2)).unwrap();
    // Leave room for the subscription and its one-shot domain read after admission succeeds.
    drop(held.pop());
    loop {
        collector.tick(sources(&source, 1), retry, read_revision);
        if collector.entries[0].read_failures > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let retry_read = collector.entries[0]
        .next
        .expect("failed first read schedules recovery");
    assert!(retry_read > retry);
    assert_eq!(collector.entries[0].observation, Observation::Unavailable);
    loop {
        let values = collector.tick(sources(&source, 1), retry_read, read_revision);
        if matches!(values[0].1, Observation::Ready(_)) {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(collector.entries[0].watch_failures, 0);
    assert_eq!(collector.entries[0].read_failures, 0);
    let values = collector.tick(
        sources(&source, 1),
        retry_read + Duration::from_secs(3),
        read_revision,
    );
    assert!(matches!(values[0].1, Observation::Ready(_)));
    assert!(collector.entries[0].job.is_none());
    assert_eq!(collector.entries[0].next, None);
    drop((held, collector, source, client));
    server.finish()
}
#[test]
fn independent_agent_and_workspace_pools_bound_eight_jobs_and_join_both_on_retirement()
-> io::Result<()> {
    let connection = source_connection()?;
    let now = Instant::now();
    let mut agents = Collector::<Checkpoint>::default();
    let mut spaces = Collector::<crate::machine_spaces::Checkpoint>::default();
    let mut flags = Vec::new();
    agents.tick_with_start(sources(&connection, 6), now, |_, deadline| {
        Ok(pending(deadline, &mut flags))
    });
    spaces.tick_with_start(sources(&connection, 6), now, |_, deadline| {
        Ok(pending(deadline, &mut flags))
    });
    assert_eq!(flags.len(), 8);
    agents.tick_with_start(sources(&connection, 6), now, |_, _| {
        panic!("fifth Agent job")
    });
    spaces.tick_with_start(sources(&connection, 6), now, |_, _| {
        panic!("fifth Workspace job")
    });
    agents.clear();
    spaces.clear();
    assert!(flags.iter().all(|f| f.is_cancelled()));
    drop(agents);
    drop(spaces);
    Ok(())
}
