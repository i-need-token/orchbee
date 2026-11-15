//! The install engine on macOS and Linux, on a local release directory signed with the test key
//! whose files carry no modes: the private stage makes the cohort's programs executable for their
//! owner alone, the cohort admission and the staged Client's check run, and the version is published
//! and activated through the pointer. A Client that reports another version leaves no stage and no
//! version. The PATH step's link has its own evidence beside it.
use super::super::{Error, Report, engine, install::Completion, signature::TEST_TABLE};
use super::install_fixture::{
    Fixture, context, entry_for, in_process_completion, manifest, pointer, print_probe,
    publish_manifest, stage_cohort,
};
use super::sign;
use orchbee_local_runtime::{Cohort, CohortArtifact};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};

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
    (result, report)
}

#[test]
fn install_publishes_runnable_programs_and_activates_them() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    // A missing folder above the store root is created private as well.
    let root = fixture.0.join("share").join("install");
    stage_cohort(&source, "0.1.0");
    let bytes = manifest(&source, |_| {});
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));

    let (result, report) = install(&root, &source);
    assert!(result.is_ok(), "{:?}", result.err().map(Error::code));
    let published = report.published.clone().unwrap();
    assert_eq!(published, root.join("versions").join("0.1.0"));
    assert!(report.activated && !report.reused);
    assert_eq!(pointer(&root).unwrap().active, entry_for("0.1.0", &bytes));
    for program in [
        CohortArtifact::Client,
        CohortArtifact::Server,
        CohortArtifact::Worker,
        CohortArtifact::TerminalHost,
    ] {
        let mode = fs::metadata(published.join(program.file_name()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "{}", program.file_name());
    }
    let mode = fs::metadata(published.join("cohort.json"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0, "a document is never executable");
    Cohort::load(&published, Instant::now() + Duration::from_secs(60)).unwrap();
    for folder in [&root, &root.parent().unwrap().to_owned()] {
        let mode = fs::metadata(folder).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{}", folder.display());
    }

    // The same release again is reused and activated without a copy.
    let (result, report) = install(&root, &source);
    assert!(result.is_ok() && report.reused && report.activated);

    // A Client that reports another version fails its check; nothing of it stays.
    let changed = manifest(&source, |value| value["version"] = json!("0.2.0"));
    publish_manifest(&source, &changed, Some(&sign(&changed)));
    let (result, report) = install(&root, &source);
    assert_eq!(result.err().map(Error::code), Some("smoke-failed"));
    assert!(report.published.is_none() && report.cleanup_pending.is_none());
    assert!(!root.join("versions").join("0.2.0").exists());
    let leftovers = fs::read_dir(&root)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".stage-")
        })
        .count();
    assert_eq!(leftovers, 0);
    assert_eq!(pointer(&root).unwrap().active, entry_for("0.1.0", &bytes));
}
