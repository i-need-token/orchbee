//! The completion's side of a fallback after a failed move, through the install and update engine
//! on local releases signed with the test key: whether a move may fall back, which the target's own
//! code settles from the replaced version's record, its intact `release.json` and the target's
//! component floors; and a version held after a fallback, which no install, update or completion
//! makes active until a higher version has been.
use super::super::{
    Error, PathStep, Report, complete, engine, install::Completion, install_document,
    signature::TEST_TABLE,
};
use super::install_fixture::{
    Fixture, context, entry_for, in_process_completion, manifest, never_unpack, pointer,
    print_probe, publish_manifest, sha, stage_cohort,
};
use super::{sign, sign_feed};
use orchbee_local_runtime::{
    CohortArtifact, FallbackEvent, FallbackOutcome, FallbackRecorder, UpdateChannel, held_versions,
};
use orchbee_release_records::Pointer;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A signed release directory of `version` whose components carry its version, with `change`
/// applied to its manifest.
fn release(fixture: &Fixture, version: &str, change: impl FnOnce(&mut Value)) -> PathBuf {
    let source = fixture.0.join(version);
    stage_cohort(&source, version);
    let bytes = manifest(&source, |value| {
        value["version"] = json!(version);
        value["components"][0]["version"] = json!(version);
        change(value);
    });
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    source
}

/// A component `id` of `version` with the security floor `floor`, listing the Server program.
fn floored(id: &str, version: &str, floor: &str) -> Value {
    json!({"id": id, "version": version, "securityFloor": floor,
        "files": [CohortArtifact::Server.file_name()], "connections": {}})
}

/// One `install` of `source` into `root`; its `orchbee.install` document must meet its schema.
fn install(root: &Path, source: &Path) -> (Result<(), Error>, Report) {
    let inputs = engine::InstallInputs {
        directory: Ok(source.to_path_buf()),
        keys: TEST_TABLE,
        install_root: Ok(root.to_path_buf()),
        probe: print_probe,
        activation: in_process_completion,
        completion: Completion {
            path: false,
            allow_downgrade: false,
        },
    };
    let mut report = Report::new();
    let result = engine::install(&inputs, &mut context(), &mut report);
    let document = install_document(&report, PathStep::NotRequested, result.err(), false);
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.install.json")).unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&document),
        "{document}"
    );
    (result, report)
}

fn installed(root: &Path, source: &Path) {
    assert_eq!(install(root, source).0.err().map(Error::code), None);
}

/// Whether the move that made the active entry active may fall back.
fn back(root: &Path) -> bool {
    pointer(root).unwrap().counting_move().unwrap().back
}

#[test]
fn a_move_may_fall_back_only_from_a_recording_version_that_meets_every_floor_of_the_target() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let first = release(&fixture, "0.1.0", |_| {});
    // A floor on a component the replaced release lacks imposes nothing.
    let second = release(&fixture, "0.2.0", |value| {
        value["components"]
            .as_array_mut()
            .unwrap()
            .push(floored("server", "0.2.0", "9.9.9"));
    });
    let third = release(&fixture, "0.3.0", |value| {
        value["components"][0]["securityFloor"] = json!("0.3.0");
    });
    let fourth = release(&fixture, "0.4.0", |_| {});
    let fifth = release(&fixture, "0.5.0", |_| {});

    // Nothing was known to record before the first move.
    installed(&root, &first);
    assert!(!back(&root));
    installed(&root, &second);
    assert!(back(&root));
    // The replaced Client 0.2.0 is below the target's own floor 0.3.0.
    installed(&root, &third);
    assert!(!back(&root));

    // A release before slice 1 writes no record, so the move after it proves nothing.
    let unrecorded = Pointer::activate(None, pointer(&root).unwrap().active);
    fs::write(root.join("active.json"), unrecorded.encode()).unwrap();
    installed(&root, &fourth);
    assert!(!back(&root));

    // The replaced version's `release.json` must still hash to its entry.
    installed(&root, &fifth);
    let sixth = release(&fixture, "0.6.0", |_| {});
    fs::write(root.join("versions/0.5.0/release.json"), b"{}").unwrap();
    installed(&root, &sixth);
    assert!(!back(&root));
}

/// Signs and writes the fixture's feed offering the release in `source`; no archive is ever
/// fetched.
fn offer(fixture: &Fixture, source: &Path) {
    let manifest = fs::read(source.join("release.json")).unwrap();
    let release: Value = serde_json::from_slice(&manifest).unwrap();
    let folder = fixture.0.join("feed");
    fs::create_dir_all(&folder).unwrap();
    let feed = serde_json::to_vec(&json!({
        "schemaVersion": 1, "product": "orchbee", "channel": "stable",
        "platform": "windows-x86_64", "version": release["version"], "buildId": release["buildId"],
        "manifest": {"sha256": sha(&manifest)},
        "archive": {"format": "zip", "url": "release.zip", "size": 1, "sha256": sha(b"-")},
    }))
    .unwrap();
    fs::write(folder.join("feed.json"), &feed).unwrap();
    fs::write(folder.join("feed.json.sig"), sign_feed(&feed)).unwrap();
}

/// One update from the fixture's feed that must end before any archive request.
fn update(fixture: &Fixture, channel: Option<UpdateChannel>) -> Report {
    let data = fixture.0.join("data");
    let state = move |name: &str| (name == "ORCHBEE_HOME").then(|| data.clone().into_os_string());
    let announce = |_: &str, _: u64| panic!("a held version is never downloaded");
    let inputs = engine::Inputs {
        channel,
        state: &state,
        feed_source: Some(
            fixture
                .0
                .join("feed/feed.json")
                .to_string_lossy()
                .into_owned(),
        ),
        keys: TEST_TABLE,
        install_root: Ok(fixture.0.join("install")),
        loopback: false,
        path: engine::PathPolicy::Step,
        probe: print_probe,
        activation: in_process_completion,
        unpack: &never_unpack,
        announce: &announce,
    };
    let mut report = Report::new();
    let result = engine::run(&inputs, &mut context(), &mut report);
    assert_eq!(result.err().map(Error::code), None);
    report
}

#[test]
fn a_held_version_is_neither_installed_nor_offered_until_a_higher_one_is_active() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let first = release(&fixture, "0.1.0", |_| {});
    let second = release(&fixture, "0.2.0", |_| {});
    let third = release(&fixture, "0.3.0", |_| {});
    installed(&root, &first);
    installed(&root, &second);
    assert!(back(&root));

    // The previous version's helper records three Client failures of 0.2.0.
    let recorder =
        FallbackRecorder::for_executable(&root.join("versions/0.1.0/orchbee.exe")).unwrap();
    let failure = FallbackEvent::ClientFailure { version: "0.2.0" };
    assert_eq!(recorder.record(failure), FallbackOutcome::Recorded);
    assert_eq!(recorder.record(failure), FallbackOutcome::Recorded);
    assert!(matches!(
        recorder.record(failure),
        FallbackOutcome::FellBack { .. }
    ));
    let restored = pointer(&root).unwrap();
    let held_entry = entry_for("0.2.0", &fs::read(second.join("release.json")).unwrap());
    assert_eq!(restored.active.product, "0.1.0");
    assert_eq!(held_versions(&restored), ["0.2.0"]);

    // `install` refuses it before copying anything, with the lower-version refusal's status.
    let (result, report) = install(&root, &second);
    assert_eq!(result.err().map(Error::code), Some("version-held"));
    assert_eq!(result.err().map(Error::status), Some(3));
    assert!(report.published.is_none() && !report.activated);
    // Every completion refuses it.
    let completion = Completion {
        path: false,
        allow_downgrade: true,
    };
    assert_eq!(
        complete::activate(&root, &held_entry, completion)
            .err()
            .map(Error::code),
        Some("version-held")
    );
    // An update reads it as up to date, even on a channel switch, and downloads nothing.
    offer(&fixture, &second);
    for channel in [None, Some(UpdateChannel::Stable)] {
        let report = update(&fixture, channel);
        assert!(report.up_to_date && !report.activated);
    }
    assert_eq!(pointer(&root).unwrap(), restored);

    // A higher version becoming active ends the hold.
    installed(&root, &third);
    assert!(held_versions(&pointer(&root).unwrap()).is_empty());
}
