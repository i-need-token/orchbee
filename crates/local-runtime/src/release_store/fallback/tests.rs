//! The bound of a fallback after a failed move, one refusal per case, on the pure transitions; and
//! the recorder's write under `install.lock` on a private store root.
use super::*;
use crate::private_filesystem as permissions;
use std::fs;

const START: u64 = 1_794_000_000_000;

fn entry(version: &str) -> Entry {
    let digits: String = version.bytes().map(|byte| format!("{byte:02x}")).collect();
    Entry {
        product: version.to_owned(),
        manifest: format!("{digits:0<64}"),
    }
}

fn activation(number: u32) -> String {
    format!("{number:032x}")
}

/// A store moved from 0.1.0, written by a build that records, to 0.2.0, which may fall back.
fn moved() -> Pointer {
    let first = complete_move(None, entry("0.1.0"), |_| true).unwrap();
    let pointer = complete_move(Some(first), entry("0.2.0"), |_| true).unwrap();
    assert!(pointer.counting_move().unwrap().back);
    pointer
}

fn written(recorded: Recorded) -> Pointer {
    match recorded {
        Recorded::Written(pointer) => pointer,
        other => panic!("expected a write, got {other:?}"),
    }
}

/// Records a Client failure of `version` at `now`, with the previous version's folder present.
fn client_failure(pointer: &Pointer, version: &str, now: u64) -> Recorded {
    record_event(
        pointer,
        "0.1.0",
        FallbackEvent::ClientFailure { version },
        now,
        |_| true,
    )
}

/// Three Client failures of the active version a second apart, the last one's outcome.
fn three_client_failures(pointer: Pointer) -> Recorded {
    let version = pointer.active.product.clone();
    let once = written(client_failure(&pointer, &version, START));
    let twice = written(client_failure(&once, &version, START + 1_000));
    client_failure(&twice, &version, START + 2_000)
}

#[test]
fn the_third_failure_in_its_window_falls_back_only_to_previous_and_holds_the_failed_version() {
    let Recorded::FellBack { pointer, from, to } = three_client_failures(moved()) else {
        panic!("three Client failures fall back");
    };
    assert_eq!((from, to), (entry("0.2.0"), entry("0.1.0")));
    assert_eq!(pointer.active, entry("0.1.0"));
    assert_eq!(pointer.previous, None);
    assert_eq!(held_versions(&pointer), ["0.2.0"]);
    let last_move = pointer.counting_move().unwrap();
    assert!(!last_move.back && last_move.restore);

    // The Session window counts its own three failures, each of a generation the moved version
    // started.
    let mut pointer = moved();
    for number in 1..=3 {
        pointer = written(record_event(
            &pointer,
            "0.2.0",
            FallbackEvent::SessionStart {
                activation: &activation(number),
            },
            START,
            |_| true,
        ));
    }
    for number in 1..=2 {
        pointer = written(record_event(
            &pointer,
            "0.1.0",
            FallbackEvent::SessionFailure {
                activation: &activation(number),
            },
            START + 1,
            |_| true,
        ));
    }
    let third = record_event(
        &pointer,
        "0.1.0",
        FallbackEvent::SessionFailure {
            activation: &activation(3),
        },
        START + 2,
        |_| true,
    );
    assert!(matches!(third, Recorded::FellBack { .. }), "{third:?}");
}

#[test]
fn a_move_from_a_version_that_does_not_record_never_falls_back() {
    // A release before slice 1, or a build whose switch is off, writes no record...
    let unrecorded = Pointer::activate(None, entry("0.1.0"));
    let pointer = complete_move(Some(unrecorded), entry("0.2.0"), |_| true).unwrap();
    assert!(!pointer.counting_move().unwrap().back);
    assert_eq!(client_failure(&pointer, "0.2.0", START), Recorded::Nothing);

    // ...and a move it makes keeps the record verbatim, which then names another entry.
    let recorded = moved();
    let legacy = Pointer::activate(Some(recorded), entry("0.3.0"));
    assert!(legacy.counting_move().is_none());
    let pointer = complete_move(Some(legacy), entry("0.4.0"), |_| true).unwrap();
    assert!(!pointer.counting_move().unwrap().back);
    assert_eq!(client_failure(&pointer, "0.4.0", START), Recorded::Nothing);
}

#[test]
fn a_move_whose_replaced_version_misses_a_floor_never_falls_back() {
    let first = complete_move(None, entry("0.1.0"), |_| true).unwrap();
    let mut asked = None;
    let pointer = complete_move(Some(first), entry("0.2.0"), |replaced| {
        asked = Some(replaced.clone());
        false
    })
    .unwrap();
    assert_eq!(asked, Some(entry("0.1.0")));
    assert!(!pointer.counting_move().unwrap().back);
    assert_eq!(client_failure(&pointer, "0.2.0", START), Recorded::Nothing);
}

#[test]
fn only_failures_inside_the_ten_minutes_after_the_first_record_count() {
    let pointer = written(client_failure(&moved(), "0.2.0", START));
    let window = pointer.counting_move().unwrap().client.unwrap();
    assert_eq!((window.since, window.failures), (START, 1));
    // Dated before the window opened, as after a clock change.
    assert_eq!(
        client_failure(&pointer, "0.2.0", START - 1),
        Recorded::Nothing
    );
    // From its end on, nothing more is recorded for the move.
    assert_eq!(
        client_failure(&pointer, "0.2.0", START + FALLBACK_WINDOW_MILLISECONDS),
        Recorded::Nothing
    );
    let last = written(client_failure(
        &pointer,
        "0.2.0",
        START + FALLBACK_WINDOW_MILLISECONDS - 1,
    ));
    assert_eq!(last.counting_move().unwrap().client.unwrap().failures, 2);
}

#[test]
fn a_move_falls_back_at_most_once_and_its_version_stays_held_until_a_higher_one_is_active() {
    let Recorded::FellBack { pointer, .. } = three_client_failures(moved()) else {
        panic!("three Client failures fall back");
    };
    // The restored version's move cannot fall back, so nothing more is recorded for either.
    assert_eq!(client_failure(&pointer, "0.1.0", START), Recorded::Nothing);
    assert_eq!(client_failure(&pointer, "0.2.0", START), Recorded::Nothing);
    // Every completion refuses the held version.
    assert_eq!(
        complete_move(Some(pointer.clone()), entry("0.2.0"), |_| true),
        Err(VersionHeld)
    );
    // A higher version becoming active drops it, and then nothing holds it.
    let higher = complete_move(Some(pointer), entry("0.3.0"), |_| true).unwrap();
    assert!(held_versions(&higher).is_empty());
    assert!(higher.fallback().unwrap().held.is_empty());
    let lower = complete_move(Some(higher), entry("0.2.0"), |_| true).unwrap();
    assert_eq!(lower.active, entry("0.2.0"));
}

#[test]
fn a_third_failure_without_the_previous_folder_is_recorded_and_ends_that_roles_records() {
    let version = "0.2.0";
    let once = written(client_failure(&moved(), version, START));
    let twice = written(client_failure(&once, version, START + 1));
    let missing = |_: &Entry| false;
    let third = written(record_event(
        &twice,
        "0.1.0",
        FallbackEvent::ClientFailure { version },
        START + 2,
        missing,
    ));
    assert_eq!(third.active, entry("0.2.0"));
    assert_eq!(third.counting_move().unwrap().client.unwrap().failures, 3);
    assert_eq!(
        client_failure(&third, version, START + 3),
        Recorded::Nothing
    );
}

#[test]
fn a_session_failure_counts_once_and_only_for_a_generation_the_moved_version_started() {
    let start = |pointer: &Pointer, own: &str, number: u32| {
        record_event(
            pointer,
            own,
            FallbackEvent::SessionStart {
                activation: &activation(number),
            },
            START,
            |_| true,
        )
    };
    let fail = |pointer: &Pointer, number: u32| {
        record_event(
            pointer,
            "0.1.0",
            FallbackEvent::SessionFailure {
                activation: &activation(number),
            },
            START + 1,
            |_| true,
        )
    };
    let pointer = moved();
    // Another version's activator records no start, and no failure counts before a start.
    assert_eq!(start(&pointer, "0.1.0", 1), Recorded::Nothing);
    assert_eq!(fail(&pointer, 1), Recorded::Nothing);
    let started = written(start(&pointer, "0.2.0", 1));
    assert_eq!(fail(&started, 2), Recorded::Nothing);
    let failed = written(fail(&started, 1));
    // Several observers may report the same generation.
    assert_eq!(fail(&failed, 1), Recorded::Nothing);
    let window = failed.counting_move().unwrap().session.unwrap();
    assert_eq!(window.failed, [activation(1)]);
}

/// A private store root with the version folders `versions` and a pointer moved to 0.2.0.
struct Store(PathBuf);

impl Store {
    fn new(versions: &[&str]) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = crate::test_temporary_directory().join(format!(
            "orchbee-release-fallback-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        permissions::protect_new(&mut permissions::open_directory(&root, true).unwrap()).unwrap();
        for version in versions {
            fs::create_dir_all(root.join(RELEASE_VERSIONS).join(version)).unwrap();
        }
        let mut lock = PrivateCache::acquire_with_lock(&root, "install", "active").unwrap();
        lock.replace(RELEASE_POINTER, &moved().encode()).unwrap();
        Self(root)
    }

    fn pointer(&self) -> Pointer {
        let bytes = PrivateCache::read_existing(&self.0, RELEASE_POINTER, RELEASE_POINTER_LIMIT)
            .unwrap()
            .unwrap();
        Pointer::parse(&bytes).unwrap()
    }

    /// The previous version's recorder, as its Client-failure helper runs it.
    fn recorder(&self) -> FallbackRecorder {
        FallbackRecorder::for_executable(
            &self
                .0
                .join(RELEASE_VERSIONS)
                .join("0.1.0")
                .join("orchbee.exe"),
        )
        .unwrap()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(
                crate::test_temporary_directory()
                    .canonicalize()
                    .unwrap()
                    .as_path()
            )
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-release-fallback-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}

#[test]
fn the_recorder_commits_under_install_lock_and_drops_a_record_it_cannot_lock() {
    let store = Store::new(&["0.1.0", "0.2.0"]);
    let recorder = store.recorder();
    assert_eq!(recorder.version(), "0.1.0");
    assert_eq!(
        FallbackRecorder::for_executable(&store.0.join("orchbee.exe")),
        None
    );
    let failure = FallbackEvent::ClientFailure { version: "0.2.0" };
    assert_eq!(
        recorder.record_at(failure, START),
        FallbackOutcome::Recorded
    );

    // A writer that holds the lock for the whole bound loses the record.
    let held = PrivateCache::acquire_with_lock(&store.0, "install", "active").unwrap();
    assert_eq!(
        recorder.record_at(failure, START + 1),
        FallbackOutcome::Dropped
    );
    drop(held);
    assert_eq!(
        store
            .pointer()
            .counting_move()
            .unwrap()
            .client
            .unwrap()
            .failures,
        1
    );

    assert_eq!(
        recorder.record_at(failure, START + 2),
        FallbackOutcome::Recorded
    );
    assert_eq!(
        recorder.record_at(failure, START + 3),
        FallbackOutcome::FellBack {
            from: entry("0.2.0"),
            to: entry("0.1.0"),
        }
    );
    let pointer = store.pointer();
    assert_eq!((pointer.active, pointer.previous), (entry("0.1.0"), None));
    assert_eq!(
        recorder.record_at(failure, START + 4),
        FallbackOutcome::Skipped
    );
}
