//! The refresh rules on a fake clock: what starts a status, what waits, what is superseded.
use super::*;

/// Time zero plus `ms` milliseconds.
fn at(ms: u64) -> Instant {
    base() + Duration::from_millis(ms)
}

fn base() -> Instant {
    use std::sync::OnceLock;
    static BASE: OnceLock<Instant> = OnceLock::new();
    *BASE.get_or_init(Instant::now)
}

/// A subscribed, focused repository whose first status (run 1) has ended at time zero.
fn settled() -> Refresh {
    let mut refresh = Refresh::new();
    refresh.set_focused(at(0), true);
    refresh.set_subscribed(at(0), true);
    assert_eq!(refresh.refresh(at(0)), [Effect::Start(1)]);
    assert_eq!(refresh.status_ended(at(0), 1, Some(false)), []);
    refresh
}

/// Wakes at every time the scheduler asks for up to `until`, collecting what starts.
fn run_until(refresh: &mut Refresh, until: u64) -> Vec<(u64, Effect)> {
    let mut started = Vec::new();
    while let Some(wake) = refresh.next_wake().filter(|wake| *wake <= at(until)) {
        let ms = u64::try_from(wake.duration_since(base()).as_millis()).unwrap_or(u64::MAX);
        started.extend(refresh.wake(wake).into_iter().map(|effect| (ms, effect)));
    }
    started
}

#[test]
fn changes_coalesce_into_one_status_after_the_debounce_and_the_cooldown() {
    let mut refresh = settled();
    for ms in [100, 600, 1200] {
        assert_eq!(refresh.changed(at(ms), Change::Content), []);
    }
    // The debounce restarts with each change; the cooldown after run 1 ends at 5000.
    assert_eq!(run_until(&mut refresh, 10_000), [(5000, Effect::Start(2))]);
    assert_eq!(refresh.status_ended(at(5200), 2, Some(false)), []);
    assert_eq!(refresh.changed(at(5300), Change::Content), []);
    assert_eq!(
        run_until(&mut refresh, 20_000),
        [(10_200, Effect::Start(3))]
    );
}

#[test]
fn a_change_during_a_status_runs_once_more_after_it() {
    let mut refresh = settled();
    assert_eq!(refresh.changed(at(6000), Change::Content), []);
    assert_eq!(run_until(&mut refresh, 7000), [(7000, Effect::Start(2))]);
    assert_eq!(refresh.changed(at(7100), Change::Content), []);
    assert_eq!(run_until(&mut refresh, 9000), []);
    assert_eq!(refresh.status_ended(at(9000), 2, Some(false)), []);
    assert_eq!(
        run_until(&mut refresh, 20_000),
        [(14_000, Effect::Start(3))]
    );
}

#[test]
fn a_due_refresh_waits_for_focus_and_nothing_runs_without_a_subscriber() {
    let mut refresh = settled();
    refresh.set_focused(at(10), false);
    refresh.changed(at(6000), Change::Content);
    assert_eq!(run_until(&mut refresh, 60_000), []);
    assert_eq!(refresh.set_focused(at(60_000), true), [Effect::Start(2)]);
    refresh.status_ended(at(60_100), 2, Some(false));

    refresh.set_subscribed(at(70_000), false);
    refresh.changed(at(70_000), Change::Content);
    assert_eq!(run_until(&mut refresh, 100_000), []);
    // An explicit refresh still runs, and subscribing again starts nothing by itself.
    assert_eq!(refresh.refresh(at(100_000)), [Effect::Start(3)]);
    refresh.status_ended(at(100_100), 3, Some(false));
    assert_eq!(refresh.set_subscribed(at(110_000), true), []);
}

#[test]
fn a_finished_write_refreshes_at_once_and_supersedes_a_running_status() {
    let mut refresh = settled();
    refresh.changed(at(6000), Change::Content);
    assert_eq!(run_until(&mut refresh, 7000), [(7000, Effect::Start(2))]);
    refresh.write_started();
    // Changes during the write are left to the refresh that follows it.
    assert_eq!(refresh.changed(at(7100), Change::Content), []);
    assert_eq!(
        refresh.write_ended(at(7200)),
        [Effect::Cancel(2), Effect::Start(3)]
    );
    // The superseded run's end is no answer.
    assert_eq!(refresh.status_ended(at(7300), 2, Some(true)), []);
    assert_eq!(refresh.status_ended(at(7400), 3, Some(false)), []);
    assert_eq!(run_until(&mut refresh, 60_000), []);
}

#[test]
fn agent_completion_skips_debounce_focus_and_cooldown_but_follows_running_work() {
    let mut refresh = settled();
    refresh.set_focused(at(10), false);
    assert_eq!(refresh.refresh(at(100)), [Effect::Start(2)]);
    // Agent completion while that status runs: once more after it, cooldown or not.
    assert_eq!(refresh.refresh(at(200)), []);
    assert_eq!(
        refresh.status_ended(at(300), 2, Some(false)),
        [Effect::Start(3)]
    );
    refresh.status_ended(at(400), 3, Some(false));
    // During a write it waits for the write, whose end refreshes once.
    refresh.write_started();
    assert_eq!(refresh.refresh(at(500)), []);
    assert_eq!(refresh.write_ended(at(600)), [Effect::Start(4)]);
}

#[test]
fn past_the_entry_limit_only_repository_changes_and_explicit_refreshes_run() {
    let mut refresh = settled();
    assert_eq!(refresh.refresh(at(6000)), [Effect::Start(2)]);
    refresh.status_ended(at(6100), 2, Some(true));
    refresh.changed(at(20_000), Change::Content);
    assert_eq!(run_until(&mut refresh, 60_000), []);
    refresh.changed(at(60_000), Change::Repository);
    assert_eq!(
        run_until(&mut refresh, 70_000),
        [(61_000, Effect::Start(3))]
    );
    // A failed run keeps the last answer about the limit.
    refresh.status_ended(at(61_100), 3, None);
    refresh.changed(at(80_000), Change::Content);
    assert_eq!(run_until(&mut refresh, 90_000), []);
    assert_eq!(refresh.refresh(at(90_000)), [Effect::Start(4)]);
}
