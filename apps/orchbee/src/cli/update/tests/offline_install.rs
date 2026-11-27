//! The install engine on a local release directory signed with the test key: synthetic cohort
//! peers, a probe that prints the staged Client's bytes, and an in-process completion that moves
//! the pointer. This proves verification, staging, publication, reuse and activation through the
//! pointer, not Server, Worker or Host behavior, the completion helper's own signature check, or a
//! PATH step.
use super::super::{
    Context, Error, Report, StorageCause, StorageReason, complete, engine,
    install::{Activation, Completion},
    signature::{TEST_TABLE, TrustedKey},
};
use super::install_fixture::{
    Fixture, context, entry_for, in_process_completion, manifest, pointer, print_probe,
    publish_manifest, stage_cohort,
};
use super::{entry, sign, signed_bytes, stranger};
use orchbee_local_runtime::{Cohort, CohortArtifact, RefusalReason};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

struct Run {
    result: Result<(), Error>,
    report: Report,
    /// Where and why a `storage` failure happened, which only standard error reports.
    storage_cause: Option<StorageCause>,
}

/// One `install` of the signed release directory `source`.
fn install(root: &Path, source: &Path, allow_downgrade: bool) -> Run {
    install_with(root, source, TEST_TABLE, allow_downgrade)
}

/// One `install` of `source` that trusts `keys`, completing with `activation` and asking for the
/// PATH step as `path` says.
fn install_as(
    root: &Path,
    source: &Path,
    keys: &[TrustedKey],
    allow_downgrade: bool,
    activation: Activation,
    path: bool,
) -> Run {
    let inputs = engine::InstallInputs {
        directory: Ok(source.to_path_buf()),
        keys,
        install_root: Ok(root.to_path_buf()),
        probe: print_probe,
        activation,
        window_store: false,
        completion: Completion {
            path,
            allow_downgrade,
        },
    };
    let mut context = context();
    let mut report = Report::new();
    let result = engine::install(&inputs, &mut context, &mut report);
    Run {
        result,
        report,
        storage_cause: context.storage_cause.take(),
    }
}

/// One `install` of `source` that trusts `keys`, completing in process without the PATH step.
fn install_with(root: &Path, source: &Path, keys: &[TrustedKey], allow_downgrade: bool) -> Run {
    install_as(
        root,
        source,
        keys,
        allow_downgrade,
        in_process_completion,
        false,
    )
}

fn code(run: &Run) -> &'static str {
    run.result.err().map_or("complete", Error::code)
}

fn stages(root: &Path) -> usize {
    fs::read_dir(root)
        .map(|entries| {
            entries
                .filter(|entry| {
                    entry
                        .as_ref()
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".stage-")
                })
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn offline_install_verifies_before_probe_reuses_without_overwrite_and_cleans_failures() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    let root = fixture.0.join("install");
    stage_cohort(&source, "0.1.0");
    let bytes = manifest(&source, |_| {});

    // Refusals before parsing: no trusted key, an unsigned feed, or an unknown signer install
    // nothing and never report a release.
    publish_manifest(&source, &bytes, None);
    for (keys, expected) in [
        (&[][..], "signing-keys-unavailable"),
        (TEST_TABLE, "signature-invalid"),
    ] {
        let refused = install_with(&root, &source, keys, false);
        assert_eq!(code(&refused), expected);
        assert!(refused.report.version.is_none() && !root.exists());
    }
    let foreign =
        serde_json::to_vec(&json!({"signatures": [entry(&stranger(), &signed_bytes(&bytes))]}))
            .unwrap();
    publish_manifest(&source, &bytes, Some(&foreign));
    let refused = install(&root, &source, false);
    assert_eq!(code(&refused), "signature-untrusted");
    assert!(refused.report.version.is_none() && !root.exists());
    // A correctly signed document that is not a manifest is refused after verification.
    publish_manifest(&source, b"{}", Some(&sign(b"{}")));
    assert_eq!(code(&install(&root, &source, false)), "manifest-invalid");
    assert!(!root.exists());

    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    let first = install(&root, &source, false);
    assert_eq!(code(&first), "complete");
    let published = first.report.published.clone().unwrap();
    assert_eq!(published, root.join("versions").join("0.1.0"));
    assert!(first.report.activated && !first.report.reused);
    assert!(first.report.cleanup_pending.is_none());
    assert_eq!(fs::read(published.join("release.json")).unwrap(), bytes);
    assert_eq!(pointer(&root).unwrap().active, entry_for("0.1.0", &bytes));
    let second = install(&root, &source, false);
    assert_eq!(code(&second), "complete");
    assert_eq!(second.report.published.as_ref(), Some(&published));
    assert!(second.report.reused && second.report.activated);

    let client = published.join(CohortArtifact::Client.file_name());
    let deadline = Instant::now() + Duration::from_secs(60);
    let admitted = Cohort::load(&published, deadline).unwrap();
    let installed = crate::cli::installation::inspect(&client, Some(&admitted), "0.1.0")
        .expect("Windows installation observation");
    assert_eq!(installed.delivery(), "native-release");
    assert_eq!(installed.channel(), Some("stable"));
    assert_eq!(installed.json()["buildId"], "test");
    for (cohort, version) in [(None, "0.1.0"), (Some(&admitted), "9.0.0")] {
        assert_eq!(
            crate::cli::installation::inspect(&client, cohort, version)
                .expect("Windows installation observation")
                .delivery(),
            "unknown"
        );
    }
    let server = published.join(CohortArtifact::Server.file_name());
    let cohort_before = fs::read(published.join("cohort.json")).unwrap();
    let peer_before = fs::read(&server).unwrap();
    fs::write(&server, b"another cohort peer").unwrap();
    let mut different: Value = serde_json::from_slice(&cohort_before).unwrap();
    different["artifacts"]["orchbee-server"] =
        json!(crate::hex::encode(&Sha256::digest(b"another cohort peer")));
    fs::write(published.join("cohort.json"), different.to_string()).unwrap();
    let stale = Cohort::load(&published, deadline).unwrap();
    fs::write(published.join("cohort.json"), cohort_before).unwrap();
    fs::write(&server, peer_before).unwrap();
    assert_eq!(
        crate::cli::installation::inspect(&client, Some(&stale), "0.1.0")
            .expect("Windows installation observation")
            .delivery(),
        "unknown"
    );

    // A wrong digest fails before the probe runs, and a Client that reports another version fails
    // the probe; neither overwrites the published release nor leaves a stage behind.
    let executable = fs::read(&client).unwrap();
    for (change, expected) in [
        (
            Box::new(|value: &mut Value| {
                value["version"] = json!("0.1.1");
                value["files"][0]["sha256"] = json!("0".repeat(64));
            }) as Box<dyn FnOnce(&mut Value)>,
            "checksum",
        ),
        (
            Box::new(|value: &mut Value| value["version"] = json!("0.2.0")),
            "smoke-failed",
        ),
    ] {
        let changed = manifest(&source, change);
        publish_manifest(&source, &changed, Some(&sign(&changed)));
        let failed = install(&root, &source, false);
        assert_eq!(code(&failed), expected);
        assert!(failed.report.published.is_none() && failed.report.cleanup_pending.is_none());
        assert!(!failed.report.activated);
        let version = failed.report.version.clone().unwrap();
        assert!(!root.join("versions").join(version).exists());
        assert_eq!(fs::read(&client).unwrap(), executable);
        assert_eq!(stages(&root), 0);
        assert_eq!(pointer(&root).unwrap().active, entry_for("0.1.0", &bytes));
    }

    // An existing version directory is never overwritten: another manifest under the same version
    // is a conflict.
    let other = manifest(&source, |value| value["buildId"] = json!("other"));
    publish_manifest(&source, &other, Some(&sign(&other)));
    assert_eq!(code(&install(&root, &source, false)), "version-conflict");
    assert_eq!(fs::read(published.join("release.json")).unwrap(), bytes);
    fs::write(published.join("release.json"), b"corrupt metadata").unwrap();
    assert_eq!(
        crate::cli::installation::inspect(&client, Some(&admitted), "0.1.0")
            .expect("Windows installation observation")
            .delivery(),
        "unknown"
    );
}

#[test]
fn install_refuses_a_lower_version_unless_allowed_and_repairs_an_unreadable_pointer() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let releases = [
        ("0.1.0", fixture.0.join("older")),
        ("0.2.0", fixture.0.join("newer")),
    ];
    let mut manifests = Vec::new();
    for (version, source) in &releases {
        stage_cohort(source, version);
        let bytes = manifest(source, |value| value["version"] = json!(version));
        publish_manifest(source, &bytes, Some(&sign(&bytes)));
        manifests.push(bytes);
    }
    let older = entry_for("0.1.0", &manifests[0]);
    let newer = entry_for("0.2.0", &manifests[1]);

    let first = install(&root, &releases[1].1, false);
    assert_eq!(code(&first), "complete");
    assert!(first.report.activated && first.report.active_before.is_none());

    // A lower version is refused before anything is copied, and the completing Client refuses it
    // again; the pointer stays where it was.
    let refused = install(&root, &releases[0].1, false);
    assert_eq!(code(&refused), "downgrade-refused");
    assert_eq!(refused.report.active_before.as_deref(), Some("0.2.0"));
    assert!(refused.report.published.is_none() && !refused.report.activated);
    assert!(!root.join("versions/0.1.0").exists());
    let lower = Completion {
        path: false,
        allow_downgrade: false,
    };
    assert_eq!(
        complete::activate(&root, &older, lower)
            .err()
            .map(Error::code),
        Some("downgrade-refused")
    );
    assert_eq!(pointer(&root).unwrap().active, newer);

    // `--allow-downgrade` lifts the refusal, and the replaced version becomes the rollback target.
    let allowed = install(&root, &releases[0].1, true);
    assert_eq!(code(&allowed), "complete");
    let moved = pointer(&root).unwrap();
    assert_eq!(
        (&moved.active, moved.previous.as_ref()),
        (&older, Some(&newer))
    );

    // A missing or unreadable pointer names no active version; install repairs it.
    let client = root
        .join("versions/0.1.0")
        .join(CohortArtifact::Client.file_name());
    let admitted = Cohort::load(
        &root.join("versions/0.1.0"),
        Instant::now() + Duration::from_secs(60),
    )
    .unwrap();
    for damage in [Some(b"{\"version\":1".as_slice()), None] {
        match damage {
            Some(bytes) => fs::write(root.join("active.json"), bytes).unwrap(),
            None => fs::remove_file(root.join("active.json")).unwrap(),
        }
        assert!(pointer(&root).is_none());
        assert_eq!(
            crate::cli::installation::inspect(&client, Some(&admitted), "0.1.0")
                .expect("Windows installation observation")
                .delivery(),
            "unknown"
        );
        let repaired = install(&root, &releases[0].1, false);
        assert_eq!(code(&repaired), "complete");
        assert!(repaired.report.reused && repaired.report.active_before.is_none());
        assert_eq!(pointer(&root).unwrap().active, older);
        assert_eq!(
            crate::cli::installation::inspect(&client, Some(&admitted), "0.1.0")
                .expect("Windows installation observation")
                .delivery(),
            "native-release"
        );
    }
}

/// Lets the built-in Users group change `folder` and everything below it, as a folder another
/// account shares would.
fn open_to_users(folder: &Path) {
    let output = Command::new("icacls.exe")
        .arg(folder)
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)M", "/q"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn install_refuses_and_names_a_store_others_can_change_but_reads_an_open_source() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    stage_cohort(&source, "0.1.0");
    let bytes = manifest(&source, |_| {});
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    open_to_users(&source);

    // A store root another account can change, holding something no installer wrote, is refused
    // before anything is written, and the cause names that folder.
    let shared = fixture.0.join("shared");
    fs::create_dir(&shared).unwrap();
    fs::write(shared.join("notes.txt"), b"another program's").unwrap();
    open_to_users(&shared);
    let refused = install(&shared, &source, false);
    assert_eq!((code(&refused), refused.report.phase), ("storage", "copy"));
    let cause = refused
        .storage_cause
        .expect("the cause of the storage failure");
    assert_eq!(cause.path, shared);
    assert!(matches!(
        cause.reason,
        StorageReason::Refused(RefusalReason::OthersCanRead)
    ));
    assert_eq!(stages(&shared), 0);

    // The same open source installs into a private store: only the store must be private, and
    // every copied file is checked against the signed manifest.
    let installed = install(&fixture.0.join("install"), &source, false);
    assert_eq!(code(&installed), "complete");
    assert!(installed.storage_cause.is_none());
}

/// A completion that moves the pointer and then fails, as a helper does when it stops after the
/// move.
fn moved_then_failed(
    root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error> {
    in_process_completion(root, version, completion, context)?;
    Err(Error::Activation)
}

/// A completion that fails before it moves the pointer.
fn failed_before_move(
    _root: &Path,
    _version: &Path,
    _completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    Err(Error::Activation)
}

/// A completion that moves the pointer and then is cancelled.
fn moved_then_cancelled(
    root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error> {
    in_process_completion(root, version, completion, context)?;
    Err(Error::Cancelled)
}

/// A completion that moves the pointer and then times out.
fn moved_then_timed_out(
    root: &Path,
    version: &Path,
    completion: Completion,
    context: &mut Context,
) -> Result<(), Error> {
    in_process_completion(root, version, completion, context)?;
    Err(Error::Timeout)
}

/// A completion that is cancelled before it moves the pointer.
fn cancelled_before_move(
    _root: &Path,
    _version: &Path,
    _completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    Err(Error::Cancelled)
}

/// A completion that times out before it moves the pointer.
fn timed_out_before_move(
    _root: &Path,
    _version: &Path,
    _completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    Err(Error::Timeout)
}

#[test]
fn a_stopped_completion_is_unknown_only_when_it_moved_the_pointer() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    stage_cohort(&source, "0.1.0");
    let bytes = manifest(&source, |_| {});
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    let cases: [(&str, Activation, &str, bool); 6] = [
        (
            "failed-before-move",
            failed_before_move,
            "activation",
            false,
        ),
        (
            "failed-after-move",
            moved_then_failed,
            "activation-unknown",
            true,
        ),
        (
            "cancelled-before-move",
            cancelled_before_move,
            "cancelled",
            false,
        ),
        (
            "cancelled-after-move",
            moved_then_cancelled,
            "activation-unknown",
            true,
        ),
        (
            "timed-out-before-move",
            timed_out_before_move,
            "timeout",
            false,
        ),
        (
            "timed-out-after-move",
            moved_then_timed_out,
            "activation-unknown",
            true,
        ),
    ];
    for (name, activation, expected, moved) in cases {
        let root = fixture.0.join(name);
        let run = install_as(&root, &source, TEST_TABLE, false, activation, false);
        assert_eq!(code(&run), expected, "{name}");
        assert_eq!(pointer(&root).is_some(), moved, "{name}");
    }
}

#[test]
fn an_install_records_a_no_path_install_and_one_with_the_path_step_clears_the_record() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    let root = fixture.0.join("install");
    stage_cohort(&source, "0.1.0");
    let bytes = manifest(&source, |_| {});
    publish_manifest(&source, &bytes, Some(&sign(&bytes)));
    // An install with the PATH step writes no member. A test build's pointer also carries the
    // fallback record, so only the `noPath` member is checked.
    let with_path = install_as(
        &root,
        &source,
        TEST_TABLE,
        false,
        in_process_completion,
        true,
    );
    assert_eq!(code(&with_path), "complete");
    let raw = fs::read(root.join("active.json")).unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("noPath"));
    assert!(!pointer(&root).unwrap().no_path());
    let no_path = install_as(
        &root,
        &source,
        TEST_TABLE,
        false,
        in_process_completion,
        false,
    );
    assert_eq!(code(&no_path), "complete");
    assert!(pointer(&root).unwrap().no_path());
    let again = install_as(
        &root,
        &source,
        TEST_TABLE,
        false,
        in_process_completion,
        true,
    );
    assert_eq!(code(&again), "complete");
    assert!(!pointer(&root).unwrap().no_path());
}
