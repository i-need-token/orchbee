use super::*;
use crate::plugin_jobs::PluginJobs;
use std::time::Instant;

fn intent() -> ConfiguredCommandIntent {
    ConfiguredCommandIntent {
        command_id: "slow-start".into(),
        directory: std::env::current_dir().unwrap().to_str().unwrap().into(),
        command: "exit 0".into(),
        environment: vec![],
    }
}

#[test]
fn delayed_spawn_is_confirmed_once_and_shutdown_joins_the_retained_owner() {
    let mut jobs = PluginJobs::new();
    let id = OperationId::from_bytes([8; 16]).unwrap();
    let command = intent();
    let outcome = start_with(
        jobs.inner.get_mut().unwrap(),
        id,
        command.clone(),
        SPAWN_CONFIRMATION_TIMEOUT,
        |_, cancel, spawned| {
            // Reproduce an acknowledgement later than the former 500 ms cutoff.
            thread::sleep(Duration::from_millis(750));
            spawned.send(Ok(())).unwrap();
            while !cancel.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
            }
            true
        },
    );
    assert_eq!(
        outcome,
        Outcome::ConfiguredStarted {
            job_id: id,
            existing: false
        }
    );
    assert_eq!(
        start(jobs.inner.get_mut().unwrap(), id, command),
        Outcome::ConfiguredStarted {
            job_id: id,
            existing: true
        }
    );
    jobs.stop_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert!(!jobs.inner.get_mut().unwrap().retired);
}

#[test]
fn expired_spawn_confirmation_stays_unknown_after_late_completion_without_replay() {
    let mut jobs = PluginJobs::new();
    let id = OperationId::from_bytes([9; 16]).unwrap();
    let command = intent();
    let (release, released) = mpsc::channel();
    let outcome = start_with(
        jobs.inner.get_mut().unwrap(),
        id,
        command.clone(),
        Duration::ZERO,
        move |_, cancel, spawned| {
            released.recv().unwrap();
            assert!(spawned.send(Ok(())).is_err());
            while !cancel.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(1));
            }
            true
        },
    );
    assert_eq!(outcome, Outcome::Refused(Refusal::PhysicalUnknown));
    release.send(()).unwrap();
    assert_eq!(
        start(jobs.inner.get_mut().unwrap(), id, command.clone()),
        Outcome::Refused(Refusal::PhysicalUnknown)
    );
    assert!(
        jobs.stop_before(Instant::now() + Duration::from_secs(3))
            .is_err()
    );
    let inner = jobs.inner.get_mut().unwrap();
    assert!(inner.commands.iter().all(|record| !record.running()));
    assert_eq!(
        start(inner, id, command.clone()),
        Outcome::Refused(Refusal::PhysicalUnknown)
    );
    assert_eq!(
        start(inner, OperationId::from_bytes([10; 16]).unwrap(), command),
        Outcome::Refused(Refusal::PhysicalUnknown)
    );
}
