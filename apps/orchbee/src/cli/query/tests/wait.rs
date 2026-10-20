//! Deterministic wait-path evidence: an injectable clock and flag make the
//! deadline, cancel, backoff, and per-attachment sequence behavior exact.

use super::super::*;
use std::cell::Cell;

/// A `WaitClock` whose `now` only moves when the test says so; `sleep`
/// records and advances the requested duration instead of blocking.
struct FakeWaitClock {
    now: Cell<Instant>,
    slept: Cell<Duration>,
}

impl FakeWaitClock {
    fn new() -> Self {
        Self {
            now: Cell::new(Instant::now()),
            slept: Cell::new(Duration::ZERO),
        }
    }
    fn advance(&self, duration: Duration) {
        self.now.set(self.now.get() + duration);
    }
}

impl WaitClock for FakeWaitClock {
    fn now(&self) -> Instant {
        self.now.get()
    }
    fn sleep(&self, duration: Duration) {
        self.slept.set(self.slept.get() + duration);
        self.now.set(self.now.get() + duration);
    }
}

struct FlaggedTermination(Cell<bool>);

impl TerminationSource for FlaggedTermination {
    fn requested(&self) -> bool {
        self.0.get()
    }
}

fn fake_run<'a>(
    clock: &'a FakeWaitClock,
    flag: &'a FlaggedTermination,
    deadline_total: Instant,
) -> WaitRun<'a> {
    WaitRun {
        gate: WaitGate {
            clock,
            signal: flag,
            deadline_total: super::super::super::wait_deadline::WaitDeadline::new(
                clock.now(),
                Some(
                    u32::try_from(deadline_total.duration_since(clock.now()).as_millis()).unwrap(),
                ),
            )
            .unwrap(),
        },
        timeout_ms: Some(30_000),
        observations: 0,
        next_row_sequence: 1,
        poll_interval: EMPTY_POLL_INTERVAL,
        after: None,
        terminal_id: String::new(),
    }
}

#[test]
fn settle_reports_cancel_before_expiry() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::from_secs(1);
    let gate = WaitGate {
        clock: &clock,
        signal: &flag,
        deadline_total: super::super::super::wait_deadline::WaitDeadline::new(
            clock.now(),
            Some(u32::try_from(deadline.duration_since(clock.now()).as_millis()).unwrap()),
        )
        .unwrap(),
    };
    assert!(gate.settle().is_ok());
    flag.0.set(true);
    // A pending cancel wins even while budget remains — and also when
    // the deadline already passed at the same instant.
    assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Cancelled);
    clock.advance(Duration::from_secs(2));
    assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Cancelled);
}

#[test]
fn settle_reports_timeout_only_after_deadline() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::from_millis(50);
    let gate = WaitGate {
        clock: &clock,
        signal: &flag,
        deadline_total: super::super::super::wait_deadline::WaitDeadline::new(
            clock.now(),
            Some(u32::try_from(deadline.duration_since(clock.now()).as_millis()).unwrap()),
        )
        .unwrap(),
    };
    assert!(gate.settle().is_ok());
    clock.advance(Duration::from_millis(50));
    assert_eq!(gate.settle().unwrap_err().kind, FailureKind::Timeout);
}

#[test]
fn budget_is_the_earlier_of_allowance_and_remaining() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::from_millis(100);
    let gate = WaitGate {
        clock: &clock,
        signal: &flag,
        deadline_total: super::super::super::wait_deadline::WaitDeadline::new(
            clock.now(),
            Some(u32::try_from(deadline.duration_since(clock.now()).as_millis()).unwrap()),
        )
        .unwrap(),
    };
    let step = gate.budget(NETWORK_BUDGET).expect("budget");
    assert_eq!(step, clock.now() + Duration::from_millis(100));
    clock.advance(Duration::from_millis(100));
    assert_eq!(
        gate.budget(NETWORK_BUDGET).unwrap_err().kind,
        FailureKind::Timeout
    );
}

#[test]
fn forever_retains_step_budget_backoff_and_cancellation_after_long_elapsed_time() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let mut run = WaitRun {
        gate: WaitGate {
            clock: &clock,
            signal: &flag,
            deadline_total: super::super::super::wait_deadline::WaitDeadline::new(
                clock.now(),
                None,
            )
            .unwrap(),
        },
        timeout_ms: None,
        observations: 0,
        next_row_sequence: 1,
        poll_interval: EMPTY_POLL_INTERVAL,
        after: None,
        terminal_id: String::new(),
    };
    clock.advance(Duration::from_secs(86_400));
    assert_eq!(run.step_deadline().unwrap(), clock.now() + NETWORK_BUDGET);
    for _ in 0..10 {
        run.sleep().unwrap();
    }
    assert_eq!(run.poll_interval, WAIT_POLL_BACKOFF);
    assert_eq!(clock.slept.get(), Duration::from_millis(750));
    flag.0.set(true);
    assert_eq!(
        run.step_deadline().unwrap_err().kind,
        FailureKind::Cancelled
    );
    assert_eq!(run.sleep().unwrap_err().kind, FailureKind::Cancelled);
    assert_eq!(clock.slept.get(), Duration::from_millis(750));
}

#[test]
fn row_sequences_increase_strictly_and_overflow_is_real() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::from_secs(60);
    let mut run = fake_run(&clock, &flag, deadline);
    assert_eq!(run.next_sequence().unwrap().get(), 1);
    assert_eq!(run.next_sequence().unwrap().get(), 2);
    assert_eq!(run.next_sequence().unwrap().get(), 3);
    run.next_row_sequence = u64::MAX;
    assert_eq!(
        run.next_sequence().unwrap_err().kind,
        FailureKind::InternalFailure
    );
    // Overflow never resets: the counter stays where it failed.
    assert_eq!(run.next_row_sequence, u64::MAX);
}

#[test]
fn sleep_backs_off_to_ceiling_and_clamps_to_deadline() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::from_millis(290);
    let mut run = fake_run(&clock, &flag, deadline);
    // 10 → 20 → 40 → 80 → 100 (cap) → clamped 60 to the deadline.
    run.sleep().expect("sleep 1");
    run.sleep().expect("sleep 2");
    run.sleep().expect("sleep 3");
    run.sleep().expect("sleep 4");
    run.sleep().expect("sleep 5");
    run.sleep().expect("sleep 6");
    assert_eq!(
        clock.slept.get(),
        Duration::from_millis(10 + 20 + 40 + 80 + 100 + 40)
    );
    // 40ms of headroom remained after the 100ms-cap sleep; the last
    // sleep consumed it, so the next sleep sees an expired deadline.
    assert_eq!(run.sleep().unwrap_err().kind, FailureKind::Timeout);
}

#[test]
fn sleep_honors_cancel_before_any_sleep() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(true));
    let deadline = clock.now() + Duration::from_secs(60);
    let mut run = fake_run(&clock, &flag, deadline);
    assert_eq!(run.sleep().unwrap_err().kind, FailureKind::Cancelled);
    assert_eq!(clock.slept.get(), Duration::ZERO);
}

#[test]
fn check_maps_expiry_to_detailed_timeout() {
    let clock = FakeWaitClock::new();
    let flag = FlaggedTermination(Cell::new(false));
    let deadline = clock.now() + Duration::ZERO;
    let mut run = fake_run(&clock, &flag, deadline);
    run.observations = 7;
    let failure = run.check().unwrap_err();
    assert_eq!(failure.kind, FailureKind::Timeout);
    let details = failure.details.expect("timeout details");
    assert_eq!(details["stage"], serde_json::json!("wait"));
    assert_eq!(details["timeoutMs"], serde_json::json!(30_000));
    assert_eq!(
        details["observed"]["completedObservations"],
        serde_json::json!(7)
    );
}
