use super::{Decision, Outcome, Pace, Unscheduled, Wake, rule, serve};
use orchbee_local_runtime::{
    CommandFailure, CommandOutput, EnvironmentKind, LogField, LogLevel, LogValue,
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    time::{Duration, Instant},
};

/// A clock that jumps to each moment the schedule waits for, or to an earlier minute a Client asks
/// for a run in, and reports the stop once the schedule waits past `end`.
struct Clock {
    origin: Instant,
    now: Cell<Instant>,
    end: Instant,
    /// The minutes, in order, at which a Client asks for a run.
    requests: RefCell<Vec<u64>>,
}

impl Clock {
    fn until(end: Duration) -> Self {
        let origin = Instant::now();
        Self {
            origin,
            now: Cell::new(origin),
            end: origin + end,
            requests: RefCell::new(vec![]),
        }
    }

    fn requesting(self, minutes: &[u64]) -> Self {
        *self.requests.borrow_mut() = minutes.to_vec();
        self
    }

    fn minute(&self) -> u64 {
        (self.now.get() - self.origin).as_secs() / 60
    }

    fn pass(&self, length: Duration) {
        self.now.set(self.now.get() + length);
    }
}

impl Pace for Clock {
    fn now(&self) -> Instant {
        self.now.get()
    }

    fn wait_until(&self, due: Instant) -> Option<Wake> {
        let mut requests = self.requests.borrow_mut();
        if let Some(&minute) = requests.first() {
            let asked = self.origin + minutes(minute);
            // A request made while a run was running waits for that run's end.
            if asked < due {
                requests.remove(0);
                self.now.set(self.now.get().max(asked));
                return Some(Wake::Requested);
            }
        }
        if due > self.end {
            return None;
        }
        self.now.set(self.now.get().max(due));
        Some(Wake::Due)
    }
}

fn minutes(count: u64) -> Duration {
    Duration::from_secs(count * 60)
}

/// What the helper's bounded exchange yields when it exits with `code` and prints `stdout`.
fn exited(code: i32, stdout: &str) -> CommandOutput {
    CommandOutput {
        exit_code: Some(code),
        failure: (code != 0).then_some(CommandFailure::ExitFailure),
        stdout: stdout.as_bytes().to_vec(),
        stderr: vec![],
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

fn update_document(phase: &str, version: Option<&str>, error: Option<&str>) -> String {
    serde_json::json!({
        "schema": "orchbee.update", "version": 1, "source": {"kind": "local-installation"},
        "result": {
            "operation": "update", "channel": "stable", "phase": phase, "channelSaved": false,
            "configPath": null, "releasePath": null, "reused": false, "activated": error.is_none(),
            "upToDate": false, "releaseVersion": version, "buildId": null, "cleanupPending": null,
            "error": error, "interrupted": false, "configurationFailure": null,
        }
    })
    .to_string()
}

fn installed() -> Outcome {
    Outcome::from_output(&exited(
        0,
        &update_document("complete", Some("0.1.0-beta.6"), None),
    ))
}

#[test]
fn the_first_run_comes_one_minute_after_ready_and_then_every_hour() {
    let clock = Clock::until(minutes(3 * 60 + 30));
    let starts = RefCell::new(vec![]);
    let records = RefCell::new(vec![]);
    serve(
        &clock,
        || true,
        || {
            starts.borrow_mut().push(clock.minute());
            installed()
        },
        |outcome| records.borrow_mut().push(*outcome),
    );
    assert_eq!(starts.into_inner(), [1, 61, 121, 181]);
    assert_eq!(records.into_inner(), [installed(); 4]);
}

#[test]
fn a_moment_with_automatic_updates_off_runs_nothing_and_records_nothing() {
    let clock = Clock::until(minutes(3 * 60 + 30));
    let mut starts = vec![];
    let mut records = 0;
    serve(
        &clock,
        || false,
        || {
            starts.push(clock.minute());
            installed()
        },
        |_| records += 1,
    );
    assert_eq!((starts, records), (vec![], 0));

    // The setting is read at each moment, so turning it off skips exactly the moments it covers.
    let clock = Clock::until(minutes(3 * 60 + 30));
    let mut starts = vec![];
    serve(
        &clock,
        || !(60..180).contains(&clock.minute()),
        || {
            starts.push(clock.minute());
            installed()
        },
        |_| {},
    );
    assert_eq!(starts, [1, 181]);
}

#[test]
fn a_run_that_outlasts_the_hour_never_overlaps_the_next_and_is_not_caught_up() {
    let clock = Clock::until(minutes(6 * 60));
    let runs = RefCell::new(vec![]);
    serve(
        &clock,
        || true,
        || {
            let start = clock.minute();
            // The second run takes two and a half hours, as a slow archive transfer may.
            clock.pass(minutes(if start == 61 { 150 } else { 2 }));
            runs.borrow_mut().push((start, clock.minute()));
            installed()
        },
        |_| {},
    );
    let runs = runs.into_inner();
    assert_eq!(
        runs.iter().map(|(start, _)| *start).collect::<Vec<_>>(),
        [1, 61, 241, 301]
    );
    for pair in runs.windows(2) {
        assert!(pair[1].0 >= pair[0].1, "{pair:?} overlap");
    }
}

#[test]
fn a_request_runs_at_once_whatever_the_setting_and_keeps_the_hourly_grid() {
    let clock = Clock::until(minutes(2 * 60 + 30)).requesting(&[20, 61, 62]);
    let runs = RefCell::new(vec![]);
    serve(
        &clock,
        || false,
        || {
            let start = clock.minute();
            // The run asked for at minute 61 outlasts the request made at minute 62.
            clock.pass(minutes(if start == 61 { 3 } else { 1 }));
            runs.borrow_mut().push((start, clock.minute()));
            installed()
        },
        |_| {},
    );
    // Automatic updates are off, so only requests run, each at once or right after the run
    // before it; the scheduled moments at 61 and 121 stay where they were and run nothing.
    assert_eq!(runs.into_inner(), [(20, 21), (61, 64), (64, 65)]);
}

fn record(outcome: Outcome) -> (LogLevel, Vec<LogField>) {
    outcome.fields()
}

#[test]
fn the_outcome_record_holds_the_documents_version_phase_and_error_code() {
    assert_eq!(
        record(installed()),
        (
            LogLevel::Info,
            vec![
                ("run", LogValue::Token("answered")),
                ("status", LogValue::Count(0)),
                ("version", LogValue::Token("0.1.0-beta.6")),
                ("phase", LogValue::Token("complete")),
                ("error", LogValue::Token("none")),
                ("upToDate", LogValue::Flag(false)),
            ]
        )
    );
    let busy = Outcome::from_output(&exited(
        1,
        &update_document("download", Some("0.1.0-beta.6"), Some("busy")),
    ));
    assert_eq!(
        record(busy),
        (
            LogLevel::Warning,
            vec![
                ("run", LogValue::Token("answered")),
                ("status", LogValue::Count(1)),
                ("version", LogValue::Token("0.1.0-beta.6")),
                ("phase", LogValue::Token("download")),
                ("error", LogValue::Token("busy")),
                ("upToDate", LogValue::Flag(false)),
            ]
        )
    );
    // A failure before the feed names no version.
    let unconfigured = Outcome::from_output(&exited(
        1,
        &update_document("source", None, Some("source-unconfigured")),
    ));
    assert!(
        !record(unconfigured)
            .1
            .iter()
            .any(|(name, _)| *name == "version")
    );
}

#[test]
fn helper_output_that_is_no_update_document_never_reaches_the_record() {
    // A value outside the log's token grammar is never copied.
    let hostile = Outcome::from_output(&exited(
        1,
        &update_document("download", Some("C:\\Users\\person\\x"), Some("busy\n")),
    ));
    let (_, fields) = record(hostile);
    assert!(fields.contains(&("version", LogValue::Token("invalid"))));
    assert!(fields.contains(&("error", LogValue::Token("invalid"))));

    let unanswered = |output: CommandOutput| {
        assert_eq!(
            record(Outcome::from_output(&output)),
            (
                LogLevel::Warning,
                vec![
                    ("run", LogValue::Token("unanswered")),
                    ("status", LogValue::Count(4))
                ]
            )
        );
    };
    // A release before the helper refuses the word with another document.
    unanswered(exited(4, r#"{"schema":"orchbee.failure","version":1}"#));
    unanswered(exited(4, "not json"));
    let mut cut = exited(4, &update_document("complete", None, None));
    cut.stdout_truncated = true;
    unanswered(cut);

    let failed = |failure| CommandOutput {
        exit_code: None,
        failure: Some(failure),
        ..exited(0, "")
    };
    for (failure, run) in [
        (CommandFailure::Cancelled, "cancelled"),
        (CommandFailure::Timeout, "timeout"),
        (CommandFailure::PhysicalUnknown, "unsettled"),
        (CommandFailure::Io, "failed"),
    ] {
        assert_eq!(
            record(Outcome::from_output(&failed(failure))),
            (LogLevel::Warning, vec![("run", LogValue::Token(run))])
        );
    }
    assert_eq!(
        Outcome::from_output(&failed(CommandFailure::SpawnUnavailable)),
        Outcome::NotStarted("spawn-failed")
    );
}

#[test]
fn only_a_store_server_of_a_build_that_runs_automatic_updates_schedules() {
    #[cfg(windows)]
    let (store, bundled) = (
        Some(Path::new(
            r"C:\Users\person\AppData\Local\Programs\Orchbee\versions\0.1.0",
        )),
        Some(Path::new(r"C:\Programs\Orchbee\desktop\resources\cohort")),
    );
    #[cfg(unix)]
    let (store, bundled) = (
        Some(Path::new(
            "/Users/person/.local/share/orchbee/versions/0.1.0",
        )),
        Some(Path::new(
            "/Applications/Orchbee.app/Contents/Resources/cohort",
        )),
    );
    use Decision::{Scheduled, Unscheduled as No};
    use EnvironmentKind::{Development, Release, Test};
    use Unscheduled::{NoFeed, OutsideStore, SwitchedOff};
    for (kind, live, feed, directory, expected) in [
        (Development, true, true, store, No(Unscheduled::Development)),
        (Test, true, false, store, No(NoFeed)),
        (Test, true, true, store, Scheduled),
        (Test, true, true, bundled, No(OutsideStore)),
        (Test, true, true, None, No(OutsideStore)),
        (Release, false, true, store, No(SwitchedOff)),
        (Release, true, false, store, Scheduled),
        (Release, true, false, bundled, No(OutsideStore)),
    ] {
        assert_eq!(
            rule(kind, live, feed, directory),
            expected,
            "{kind:?} live={live} feed={feed} {directory:?}"
        );
    }
}
