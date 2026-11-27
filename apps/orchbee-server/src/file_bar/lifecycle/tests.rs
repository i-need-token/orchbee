use super::{Demand, Lifecycle, Refusal, StartFailure, Step};
use std::time::{Duration, Instant};

fn seconds(value: u64) -> Duration {
    Duration::from_secs(value)
}

/// A lifecycle whose process is ready on link 1 since `now`.
fn ready(now: Instant) -> Lifecycle {
    let mut lifecycle = Lifecycle::new(false);
    assert_eq!(lifecycle.demand(now), Demand::Queue);
    assert_eq!(lifecycle.step(now, true), Step::Start);
    assert!(lifecycle.started(now, 1));
    lifecycle
}

#[test]
fn a_demand_starts_the_process_and_later_calls_route() {
    let start = Instant::now();
    let mut lifecycle = Lifecycle::new(false);
    assert_eq!(lifecycle.step(start, false), Step::Wait(None));
    assert_eq!(lifecycle.demand(start), Demand::Queue);
    assert_eq!(lifecycle.step(start, true), Step::Start);
    // A start in progress takes no second start, and calls keep waiting for it.
    assert_eq!(lifecycle.step(start, true), Step::Wait(None));
    assert_eq!(lifecycle.demand(start), Demand::Queue);
    assert!(lifecycle.started(start, 1));
    assert_eq!(lifecycle.demand(start), Demand::Route);
}

#[test]
fn ten_idle_minutes_drain_the_process_and_the_next_demand_starts_a_fresh_one() {
    let start = Instant::now();
    let mut lifecycle = ready(start);
    let idle_end = start + seconds(600);
    assert_eq!(lifecycle.step(start, false), Step::Wait(Some(idle_end)));
    assert_eq!(lifecycle.step(idle_end, false), Step::Drain(1));
    // A call during the drain waits for the successor.
    assert_eq!(lifecycle.demand(idle_end), Demand::Queue);
    assert_eq!(
        lifecycle.step(idle_end, true),
        Step::Wait(Some(idle_end + seconds(10)))
    );
    lifecycle.link_ended(idle_end + seconds(1), 1);
    // A drain is no crash, so nothing delays the next start.
    assert_eq!(lifecycle.step(idle_end + seconds(1), true), Step::Start);
    assert!(lifecycle.started(idle_end + seconds(1), 2));
    assert_eq!(lifecycle.demand(idle_end + seconds(1)), Demand::Route);
}

#[test]
fn open_calls_and_held_work_keep_the_process_and_their_end_restarts_the_idle_time() {
    let start = Instant::now();
    let mut lifecycle = ready(start);
    assert_eq!(lifecycle.demand(start), Demand::Route);
    assert_eq!(
        lifecycle.step(start + seconds(3600), false),
        Step::Wait(None)
    );
    lifecycle.holds(start + seconds(3600), 1, true);
    lifecycle.call_ended(start + seconds(3600), 1);
    assert_eq!(
        lifecycle.step(start + seconds(7200), false),
        Step::Wait(None)
    );
    lifecycle.holds(start + seconds(7200), 1, false);
    let idle_end = start + seconds(7200 + 600);
    assert_eq!(
        lifecycle.step(start + seconds(7200), false),
        Step::Wait(Some(idle_end))
    );
    assert_eq!(lifecycle.step(idle_end, false), Step::Drain(1));
}

#[test]
fn a_drain_that_does_not_end_the_link_in_time_closes_it() {
    let start = Instant::now();
    let mut lifecycle = ready(start);
    let idle_end = start + seconds(600);
    assert_eq!(lifecycle.step(idle_end, false), Step::Drain(1));
    assert_eq!(
        lifecycle.step(idle_end + seconds(10), false),
        Step::Close(1)
    );
}

#[test]
fn crashes_restart_after_0_1_5_and_30_seconds_and_the_fifth_in_ten_minutes_gives_up() {
    let mut now = Instant::now();
    let mut lifecycle = ready(now);
    for (link, delay) in (1..).zip([0, 1, 5, 30]) {
        assert!(!lifecycle.link_ended(now, link));
        assert_eq!(lifecycle.demand(now), Demand::Queue);
        if delay > 0 {
            assert_eq!(
                lifecycle.step(now, true),
                Step::Wait(Some(now + seconds(delay)))
            );
        }
        now += seconds(delay);
        assert_eq!(lifecycle.step(now, true), Step::Start);
        assert!(lifecycle.started(now, link + 1));
    }
    assert!(lifecycle.link_ended(now, 5));
    assert_eq!(
        lifecycle.demand(now + seconds(3600)),
        Demand::Refuse(Refusal::CrashedRepeatedly)
    );
    assert_eq!(lifecycle.step(now + seconds(3600), true), Step::Wait(None));
}

#[test]
fn crashes_more_than_ten_minutes_apart_never_give_up() {
    let mut now = Instant::now();
    let mut lifecycle = ready(now);
    for link in 1..=8 {
        lifecycle.link_ended(now, link);
        assert_eq!(lifecycle.step(now, true), Step::Start);
        assert!(lifecycle.started(now, link + 1));
        now += seconds(601);
    }
}

#[test]
fn a_failed_start_counts_as_a_crash_except_a_build_mismatch_or_a_missing_worker() {
    let start = Instant::now();
    let mut lifecycle = Lifecycle::new(false);
    assert_eq!(lifecycle.step(start, true), Step::Start);
    assert_eq!(
        lifecycle.start_failed(start, StartFailure::NoWorker),
        Refusal::Restarting
    );
    assert_eq!(lifecycle.step(start, true), Step::Start);
    assert_eq!(
        lifecycle.start_failed(start, StartFailure::Crashed),
        Refusal::Restarting
    );
    // The first crash restarts at once; the second waits a second.
    assert_eq!(lifecycle.step(start, true), Step::Start);
    assert_eq!(
        lifecycle.start_failed(start, StartFailure::Crashed),
        Refusal::Restarting
    );
    assert_eq!(
        lifecycle.step(start, true),
        Step::Wait(Some(start + seconds(1)))
    );
    assert_eq!(lifecycle.step(start + seconds(1), true), Step::Start);
    assert_eq!(
        lifecycle.start_failed(start + seconds(1), StartFailure::BuildMismatch),
        Refusal::BuildMismatch
    );
    assert_eq!(
        lifecycle.demand(start + seconds(3600)),
        Demand::Refuse(Refusal::BuildMismatch)
    );
}

#[test]
fn a_replaced_worker_starts_a_fresh_process_with_no_crash_counted() {
    let mut now = Instant::now();
    let mut lifecycle = ready(now);
    for link in 1..=5 {
        lifecycle.link_ended(now, link);
        if link < 5 {
            now += seconds(30);
            assert_eq!(lifecycle.step(now, true), Step::Start);
            assert!(lifecycle.started(now, link + 1));
        }
    }
    assert_eq!(
        lifecycle.demand(now),
        Demand::Refuse(Refusal::CrashedRepeatedly)
    );
    assert_eq!(lifecycle.retire(), None);
    assert_eq!(lifecycle.demand(now), Demand::Queue);
    assert_eq!(lifecycle.step(now, true), Step::Start);
    assert!(lifecycle.started(now, 6));
    // The old Worker's process held link 6 when its Worker was replaced: retiring it is no crash.
    assert_eq!(lifecycle.retire(), Some(6));
    lifecycle.link_ended(now, 6);
    assert_eq!(lifecycle.step(now, true), Step::Start);
}

#[test]
fn a_stopped_session_refuses_calls_and_drains_a_start_that_finishes_after_it() {
    let start = Instant::now();
    let mut lifecycle = ready(start);
    assert_eq!(lifecycle.stop(), Some(1));
    assert_eq!(lifecycle.demand(start), Demand::Refuse(Refusal::Stopping));
    let mut starting = Lifecycle::new(false);
    assert_eq!(starting.step(start, true), Step::Start);
    assert_eq!(starting.stop(), None);
    assert!(!starting.started(start, 1));
    assert_eq!(
        starting.start_failed(start, StartFailure::Crashed),
        Refusal::Stopping
    );
}

#[test]
fn a_release_build_never_starts_the_process() {
    let start = Instant::now();
    let mut lifecycle = Lifecycle::new(true);
    assert_eq!(lifecycle.demand(start), Demand::Refuse(Refusal::Withheld));
    assert_eq!(lifecycle.step(start, true), Step::Wait(None));
    assert_eq!(lifecycle.stop(), None);
    assert_eq!(lifecycle.demand(start), Demand::Refuse(Refusal::Withheld));
}

#[test]
fn the_end_of_a_link_already_replaced_changes_nothing() {
    let start = Instant::now();
    let mut lifecycle = ready(start);
    lifecycle.link_ended(start, 7);
    assert_eq!(lifecycle.demand(start), Demand::Route);
}
