//! The relaunch decision over exit statuses, relaunch history and the start each outcome came from.
use super::{Next, Outcome, Start, Verdict, judge, next};
use std::time::{Duration, Instant};

/// Windows exit codes as `ExitStatus::code` reports them.
fn status(code: u32) -> Outcome {
    Outcome::Exited(code.cast_signed())
}

#[test]
fn a_failed_start_or_join_is_told_from_a_normal_exit_by_status() {
    for (outcome, verdict) in [
        (Outcome::NotStarted, Verdict::Failed),
        (status(76), Verdict::Failed),
        (status(101), Verdict::Failed),
        // Access violation, missing DLL, fail-fast abort, stack overflow.
        (status(0xC000_0005), Verdict::Failed),
        (status(0xC000_0135), Verdict::Failed),
        (status(0xC000_0409), Verdict::Failed),
        (status(0xC000_00FD), Verdict::Failed),
        (status(75), Verdict::Relaunch),
        // Every CLI exit code, a TUI startup failure included, and a console exit.
        (status(0), Verdict::Normal),
        (status(1), Verdict::Normal),
        (status(6), Verdict::Normal),
        (status(8), Verdict::Normal),
        (status(130), Verdict::Normal),
        (status(0xC000_013A), Verdict::Normal),
        (status(0x8000_0003), Verdict::Normal),
    ] {
        assert_eq!(judge(outcome), verdict, "{outcome:?}");
    }
}

#[test]
fn each_start_moves_on_by_its_verdict() {
    let now = Instant::now();
    for (start, outcome, step) in [
        (Start::First, status(0), Next::End(0)),
        (Start::First, status(3), Next::End(3)),
        (
            Start::First,
            status(0xC000_013A),
            Next::End(0xC000_013A_u32.cast_signed()),
        ),
        (Start::First, status(75), Next::Relaunch),
        // A failed first start ends with the Client's status; a failed join reads as the
        // startup failure the Client reports without the protocol.
        (Start::First, Outcome::NotStarted, Next::Unavailable),
        (Start::First, status(76), Next::End(1)),
        (Start::First, status(101), Next::End(101)),
        // The new Client of a relaunch that fails to start or to join returns to the one it
        // replaced; its normal ends and its own relaunch request stand.
        (Start::Relaunch, Outcome::NotStarted, Next::Return),
        (Start::Relaunch, status(76), Next::Return),
        (Start::Relaunch, status(0xC000_0005), Next::Return),
        (Start::Relaunch, status(1), Next::End(1)),
        (Start::Relaunch, status(0), Next::End(0)),
        (Start::Relaunch, status(75), Next::Relaunch),
        // A return falls back no further.
        (Start::Return, Outcome::NotStarted, Next::Unavailable),
        (Start::Return, status(76), Next::End(1)),
        (Start::Return, status(0), Next::End(0)),
        (Start::Return, status(75), Next::Relaunch),
    ] {
        assert_eq!(
            next(start, outcome, &[], now),
            step,
            "{start:?} {outcome:?}"
        );
    }
}

#[test]
fn a_launcher_relaunches_at_most_three_times_in_ten_seconds() {
    let now = Instant::now() + Duration::from_secs(60);
    let ago = |seconds: f64| now - Duration::from_secs_f64(seconds);
    for (history, step) in [
        (vec![ago(9.9), ago(5.0)], Next::Relaunch),
        (vec![ago(9.9), ago(5.0), ago(0.1)], Next::End(75)),
        (vec![ago(10.0), ago(5.0), ago(0.1)], Next::Relaunch),
        (vec![ago(30.0), ago(20.0), ago(12.0)], Next::Relaunch),
    ] {
        for start in [Start::First, Start::Relaunch, Start::Return] {
            assert_eq!(next(start, status(75), &history, now), step, "{history:?}");
        }
    }
}
