use super::fixture::{Effect, Fixture};
use std::{sync::atomic::Ordering, time::Instant};
#[test]
fn periodic_turns_use_one_process_only_viewer_and_release_before_next_attach() {
    let fixture = Fixture::new();
    let mut cursor = None;
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    assert_eq!(effects.len(), 24);
    for events in effects.chunks_exact(3) {
        let [
            Effect::Attach(a, _),
            Effect::ProcessInfo(b),
            Effect::Detach(c),
        ] = events
        else {
            panic!("unexpected effects: {events:?}")
        };
        assert_eq!(a, b);
        assert_eq!(b, c);
    }
    assert!(!fixture.service.process_observer_pending_for_test());
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    let targets: std::collections::BTreeSet<_> = effects
        .iter()
        .filter_map(|e| {
            if let Effect::Attach(_, target) = e {
                Some(target)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(targets.len(), 10);
}
#[test]
fn uncertain_cleanup_blocks_new_attachments_and_transfers_to_matching_worker_shutdown() {
    let fixture = Fixture::new();
    fixture.detach_refusals.store(2, Ordering::Release);
    let mut cursor = None;
    fixture.service.process_observer_turn_for_test(&mut cursor);
    assert!(fixture.service.process_observer_pending_for_test());
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::DetachRefused(..)))
            .count(),
        2
    );
    fixture.service.process_observer_turn_for_test(&mut cursor);
    assert!(!fixture.service.process_observer_pending_for_test());
    // A separate uncertainty survives sampler stop; exact retired Worker proof owns its release.
    fixture.detach_refusals.store(1, Ordering::Release);
    fixture.service.process_observer_turn_for_test(&mut cursor);
    let old = fixture
        .service
        .workers
        .lock()
        .unwrap()
        .request_handle()
        .unwrap();
    fixture
        .service
        .stop_process_observer_before(Instant::now())
        .unwrap();
    assert!(fixture.service.process_observer_pending_for_test());
    fixture.replace_worker();
    let current = fixture
        .service
        .workers
        .lock()
        .unwrap()
        .request_handle()
        .unwrap();
    assert!(
        fixture
            .service
            .finish_process_observer_after_worker_exit(&current)
            .is_err()
    );
    assert!(fixture.service.process_observer_pending_for_test());
    fixture
        .service
        .finish_process_observer_after_worker_exit(&old)
        .unwrap();
    assert!(!fixture.service.process_observer_pending_for_test());
}
#[test]
fn discovered_target_cannot_attach_a_new_worker_generation_or_run_after_cancel() {
    let fixture = Fixture::new();
    let target = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .process_observation_targets()
        .unwrap()
        .remove(0);
    let stop = std::sync::atomic::AtomicBool::new(true);
    assert!(
        fixture
            .service
            .sample_process_target(
                &target,
                &stop,
                &crate::agent_screen::ScreenDetector::bundled().unwrap()
            )
            .is_none()
    );
    assert!(fixture.barrier().unwrap().is_empty());
    stop.store(false, Ordering::Release);
    fixture.replace_worker();
    assert!(
        fixture
            .service
            .sample_process_target(
                &target,
                &stop,
                &crate::agent_screen::ScreenDetector::bundled().unwrap()
            )
            .is_none()
    );
    assert!(fixture.barrier().unwrap().is_empty());
}

#[test]
fn checkpoint_viewer_cleanup_is_owned_after_sampler_join_and_prevents_another_capture() {
    let fixture = Fixture::new();
    assert!(fixture.service.settle_checkpoint_viewer(None).is_err());
    fixture
        .service
        .stop_process_observer_before(Instant::now())
        .unwrap();
    let (_, targets) = fixture
        .service
        .coordinator
        .lock()
        .unwrap()
        .checkpoint_observation_targets()
        .unwrap();
    fixture.detach_refusals.store(2, Ordering::Release);
    let (_, pending) = fixture.service.sample_checkpoint_directory(&targets[0]);
    assert!(pending.is_some());
    assert!(fixture.service.settle_checkpoint_viewer(pending).is_err());
    assert!(fixture.service.settle_checkpoint_viewer(None).is_err());
    assert!(fixture.service.process_observer_pending_for_test());
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        1
    );
    fixture.service.settle_checkpoint_viewer(None).unwrap();
    assert!(!fixture.service.process_observer_pending_for_test());
    let (_, pending) = fixture.service.sample_checkpoint_directory(&targets[1]);
    assert!(pending.is_none());
    fixture.service.settle_checkpoint_viewer(pending).unwrap();
    let effects = fixture.barrier().unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Attach(..)))
            .count(),
        2
    );
}
