use super::*;

/// Closing checks with fixed answers that count what was asked.
struct Checks {
    superseded: bool,
    worker_idle: bool,
    begin: bool,
    asked_worker: u32,
    asked_begin: u32,
}

impl Checks {
    fn new(superseded: bool, worker_idle: bool, begin: bool) -> Self {
        Self {
            superseded,
            worker_idle,
            begin,
            asked_worker: 0,
            asked_begin: 0,
        }
    }
}

impl Closing for Checks {
    fn superseded(&mut self) -> bool {
        self.superseded
    }
    fn worker_idle(&mut self) -> bool {
        self.asked_worker += 1;
        self.worker_idle
    }
    fn begin(&mut self) -> bool {
        self.asked_begin += 1;
        self.begin
    }
}

/// Looks at each of `seconds` after `start`, finding `activity` there, and answers the seconds at
/// which the stop began.
fn run(
    countdown: &mut Countdown,
    checks: &mut Checks,
    start: Instant,
    seconds: std::ops::RangeInclusive<u64>,
    activity: impl Fn(u64) -> Activity,
) -> Vec<u64> {
    seconds
        .filter(|second| {
            countdown.observe(
                start + Duration::from_secs(*second),
                activity(*second),
                checks,
            )
        })
        .collect()
}

fn idle(_: u64) -> Activity {
    Activity::default()
}

#[test]
fn the_stop_begins_after_one_unbroken_idle_minute_and_each_condition_restarts_it() {
    let start = Instant::now();
    let mut countdown = Countdown::default();
    let mut checks = Checks::new(true, true, true);
    assert_eq!(run(&mut countdown, &mut checks, start, 0..=60, idle), [60]);

    let found: [fn(&mut Activity); 3] = [
        |activity| activity.terminal = true,
        |activity| activity.helper = true,
        |activity| activity.operation = true,
    ];
    for find in found {
        // Activity at second 30 cancels the countdown begun at 0; a new one starts at 31.
        let mut countdown = Countdown::default();
        let mut checks = Checks::new(true, true, true);
        let at_30 = |second| {
            let mut activity = Activity::default();
            if second == 30 {
                find(&mut activity);
            }
            activity
        };
        assert_eq!(run(&mut countdown, &mut checks, start, 0..=91, at_30), [91]);
        assert_eq!(
            checks.asked_worker, 1,
            "the Worker is asked only at the end"
        );
    }
}

#[test]
fn a_late_arrival_or_a_busy_worker_waits_a_new_minute_and_a_server_not_older_never_moves() {
    let start = Instant::now();
    // A record or launch arriving after the last look refuses the begin; the next try is a full
    // minute later.
    let mut countdown = Countdown::default();
    let mut checks = Checks::new(true, true, false);
    assert!(run(&mut countdown, &mut checks, start, 0..=119, idle).is_empty());
    assert_eq!(checks.asked_begin, 1);
    checks.begin = true;
    assert_eq!(
        run(&mut countdown, &mut checks, start, 120..=120, idle),
        [120]
    );

    // A Worker that runs a command, or cannot say, keeps the Session and is asked once a minute.
    let mut countdown = Countdown::default();
    let mut checks = Checks::new(true, false, true);
    assert!(run(&mut countdown, &mut checks, start, 0..=180, idle).is_empty());
    assert_eq!((checks.asked_worker, checks.asked_begin), (3, 0));

    // A Server not older than the store's active version never asks the Worker and never stops.
    let mut countdown = Countdown::default();
    let mut checks = Checks::new(false, true, true);
    assert!(run(&mut countdown, &mut checks, start, 0..=3600, idle).is_empty());
    assert_eq!((checks.asked_worker, checks.asked_begin), (0, 0));
    assert!(older("0.1.0-beta.8", "0.1.0-beta.9"));
    assert!(older("0.1.0-beta.9", "0.1.0"));
    assert!(!older("0.1.0-beta.9", "0.1.0-beta.9"));
    assert!(!older("0.1.0", "0.1.0-beta.9"));
}
