use super::*;
use crate::{
    local_client::tests::server::FakeServer,
    machine_agents::{
        Checkpoint,
        tests::{accept, checkpoint},
    },
};
use std::io;
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
        cancel: Arc::new(AtomicU8::new(0)),
        task: Some(task),
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
        let cancel = Arc::new(AtomicU8::new(0));
        flags.push(cancel.clone());
        let worker = cancel.clone();
        let task = thread::spawn(move || {
            while worker.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            Completed {
                at: Instant::now(),
                rows: Ok(checkpoint(Vec::new())),
            }
        });
        Ok(Job {
            cancel,
            task: Some(task),
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
    assert!(flags.iter().all(|f| f.load(Ordering::Acquire) == 1));
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
    collector.tick_with_start(sources(&connection, 1), now, |_, _| Ok(completed(now)));
    let stale = collector.tick_with_start(
        sources(&connection, 1),
        now + FRESH_FOR + Duration::from_secs(1),
        |_, _| panic!("cooldown starts at consumption"),
    );
    assert_eq!(stale[0].1, Observation::Unavailable);
    collector.clear();
    let next = collector.tick_with_start(sources(&connection, 1), now, |_, _| Ok(completed(now)));
    assert_eq!(
        next[0].1,
        Observation::Loading,
        "cleared checkpoint must not resurface on profile replacement"
    );
    Ok(())
}

fn pending<T: Send + Sync + 'static>(deadline: Instant, flags: &mut Vec<Arc<AtomicU8>>) -> Job<T> {
    let cancel = Arc::new(AtomicU8::new(0));
    flags.push(cancel.clone());
    let flag = cancel.clone();
    let task = thread::spawn(move || {
        while flag.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        Completed {
            at: Instant::now(),
            rows: Err(()),
        }
    });
    Job {
        cancel,
        task: Some(task),
    }
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
    assert!(flags.iter().all(|f| f.load(Ordering::Acquire) == 1));
    drop(agents);
    drop(spaces);
    Ok(())
}
