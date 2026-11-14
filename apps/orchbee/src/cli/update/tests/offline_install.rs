//! The install engine on a local feed signed with the test key: synthetic cohort peers, a probe
//! that prints the staged Client's bytes, and an in-process completion that moves the pointer.
//! This proves verification, staging, publication, reuse and activation through the pointer, not
//! Server, Worker or Host behavior, the completion helper's own signature check, or a PATH step.
use super::super::{
    Context, Error, Report, StorageCause, StorageReason, complete, document, engine,
    install::{Activation, Completion},
    signature::{TEST_TABLE, TrustedKey},
};
use super::install_fixture::{Fixture, context, in_process_completion, pointer, print_probe};
use super::{entry, sign, signed_bytes, stranger};
use orchbee_local_runtime::{Cohort, CohortArtifact, RefusalReason};
use orchbee_release_records::Entry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

const PEERS: [CohortArtifact; 3] = [
    CohortArtifact::Server,
    CohortArtifact::Worker,
    CohortArtifact::TerminalHost,
];

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

fn digest(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn inventory(directory: &Path, base: &Path, files: &mut Vec<Value>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            inventory(&entry.path(), base, files);
        } else {
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            if relative.starts_with("release.json") {
                continue;
            }
            let size = fs::metadata(&path).unwrap().len();
            files.push(
                json!({"path": relative, "url": relative, "size": size, "sha256": digest(&path)}),
            );
        }
    }
}

/// Stages a complete cohort feed in `source`; the Client is a text file holding the version line
/// of `version`.
fn stage_cohort(source: &Path, version: &str) {
    fs::create_dir(source).unwrap();
    fs::write(
        source.join(CohortArtifact::Client.file_name()),
        format!("orchbee {version}\r\n"),
    )
    .unwrap();
    let mut artifacts = serde_json::Map::new();
    artifacts.insert(
        "orchbee".into(),
        json!(digest(&source.join(CohortArtifact::Client.file_name()))),
    );
    for (peer, key) in
        PEERS
            .into_iter()
            .zip(["orchbee-server", "orchbee-worker", "orchbee-terminal-host"])
    {
        let path = source.join(peer.file_name());
        fs::write(&path, format!("synthetic {key}; never executed")).unwrap();
        artifacts.insert(key.into(), json!(digest(&path)));
    }
    let adapters = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/agent-integration-writer/agent-observation-adapters");
    copy_tree(&adapters, &source.join("agent-observation-adapters"));
    let capsule: Value = serde_json::from_slice(
        &fs::read(source.join("agent-observation-adapters/capsule.json")).unwrap(),
    )
    .unwrap();
    artifacts.insert(
        "agent-observation-adapters".into(),
        capsule["payloadSha256"].clone(),
    );
    fs::write(
        source.join("cohort.json"),
        json!({"schemaVersion": 1, "artifacts": artifacts}).to_string(),
    )
    .unwrap();
}

/// A version 2 manifest for every file in `source`, with `change` applied.
fn manifest(source: &Path, change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut files = Vec::new();
    inventory(source, source, &mut files);
    let client = CohortArtifact::Client.file_name();
    let mut value = json!({
        "schemaVersion": 2, "product": "orchbee", "channel": "stable",
        "platform": "windows-x86_64", "version": "0.1.0", "buildId": "test", "files": files,
        "components": [{"id": "client", "version": "0.1.0", "files": [client],
            "connections": {"client-server": {"generations": {"lowest": 1, "highest": 1}}}}],
    });
    change(&mut value);
    serde_json::to_vec(&value).unwrap()
}

/// Publishes `manifest` and `signature` as the local feed's `release.json` and its document.
fn publish_feed(source: &Path, manifest: &[u8], signature: Option<&[u8]>) {
    fs::write(source.join("release.json"), manifest).unwrap();
    let document = source.join("release.json.sig");
    match signature {
        Some(signature) => fs::write(&document, signature).unwrap(),
        None => {
            let _ = fs::remove_file(&document);
        }
    }
}

struct Run {
    result: Result<(), Error>,
    report: Report,
    /// Where and why a `storage` failure happened, which only standard error reports.
    storage_cause: Option<StorageCause>,
}

fn run(fixture: &Fixture, keys: &[TrustedKey]) -> Run {
    update(
        fixture,
        &fixture.0.join("source"),
        keys,
        in_process_completion,
    )
}

/// One update from the local feed `source` into the fixture's store, completing with `activation`.
fn update(fixture: &Fixture, source: &Path, keys: &[TrustedKey], activation: Activation) -> Run {
    let data = fixture.0.join("data");
    let state = move |name: &str| (name == "ORCHBEE_HOME").then(|| data.clone().into_os_string());
    let inputs = engine::Inputs {
        channel: None,
        state: &state,
        manifest_source: Some(source.join("release.json").to_string_lossy().into_owned()),
        keys,
        install_root: Ok(fixture.0.join("install")),
        probe: print_probe,
        activation,
    };
    let mut context = context();
    let mut report = Report::new();
    let result = engine::run(&inputs, &mut context, &mut report);
    Run {
        result,
        report,
        storage_cause: context.storage_cause.take(),
    }
}

/// One `install` of the signed release directory `source`.
fn install(root: &Path, source: &Path, allow_downgrade: bool) -> Run {
    let inputs = engine::InstallInputs {
        directory: Ok(source.to_path_buf()),
        keys: TEST_TABLE,
        install_root: Ok(root.to_path_buf()),
        probe: print_probe,
        activation: in_process_completion,
        completion: Completion {
            path: false,
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

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn entry_for(version: &str, manifest: &[u8]) -> Entry {
    Entry {
        product: version.into(),
        manifest: sha(manifest),
    }
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
    publish_feed(&source, &bytes, None);
    for (keys, expected) in [
        (&[][..], "signing-keys-unavailable"),
        (TEST_TABLE, "signature-invalid"),
    ] {
        let refused = run(&fixture, keys);
        assert_eq!(code(&refused), expected);
        assert!(refused.report.version.is_none() && !root.exists());
    }
    let foreign =
        serde_json::to_vec(&json!({"signatures": [entry(&stranger(), &signed_bytes(&bytes))]}))
            .unwrap();
    publish_feed(&source, &bytes, Some(&foreign));
    let refused = run(&fixture, TEST_TABLE);
    assert_eq!(code(&refused), "signature-untrusted");
    assert!(refused.report.version.is_none() && !root.exists());
    // A correctly signed document that is not a manifest is refused after verification.
    publish_feed(&source, b"{}", Some(&sign(b"{}")));
    assert_eq!(code(&run(&fixture, TEST_TABLE)), "manifest-invalid");
    assert!(!root.exists());

    publish_feed(&source, &bytes, Some(&sign(&bytes)));
    let first = run(&fixture, TEST_TABLE);
    assert_eq!(code(&first), "complete");
    let published = first.report.published.clone().unwrap();
    assert_eq!(published, root.join("versions").join("0.1.0"));
    assert!(first.report.activated && !first.report.reused);
    assert!(first.report.cleanup_pending.is_none());
    assert_eq!(fs::read(published.join("release.json")).unwrap(), bytes);
    assert_eq!(pointer(&root).unwrap().active, entry_for("0.1.0", &bytes));
    let second = run(&fixture, TEST_TABLE);
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
        json!(format!("{:x}", Sha256::digest(b"another cohort peer")));
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
        publish_feed(&source, &changed, Some(&sign(&changed)));
        let failed = run(&fixture, TEST_TABLE);
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
    publish_feed(&source, &other, Some(&sign(&other)));
    assert_eq!(code(&run(&fixture, TEST_TABLE)), "version-conflict");
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
        publish_feed(source, &bytes, Some(&sign(&bytes)));
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

/// A completion that never activates, for runs that must not complete anything.
fn refused_completion(
    _root: &Path,
    _version: &Path,
    _completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    Err(Error::Activation)
}

/// The rendered `orchbee.update` document of a run, checked against its schema.
fn rendered(run: &Run) -> Value {
    let rendered = document(None, &run.report, run.result.err(), false);
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.update.json")).unwrap();
    assert!(
        jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&rendered),
        "{rendered}"
    );
    rendered
}

#[test]
fn update_is_up_to_date_on_the_active_release_and_on_a_lower_feed() {
    let fixture = Fixture::new();
    let root = fixture.0.join("install");
    let mut feeds = Vec::new();
    for (version, name) in [("0.1.0", "older"), ("0.2.0", "newer")] {
        let source = fixture.0.join(name);
        stage_cohort(&source, version);
        let bytes = manifest(&source, |value| value["version"] = json!(version));
        publish_feed(&source, &bytes, Some(&sign(&bytes)));
        feeds.push((source, bytes));
    }
    let newer = entry_for("0.2.0", &feeds[1].1);
    let installed = update(&fixture, &feeds[1].0, TEST_TABLE, in_process_completion);
    assert_eq!(code(&installed), "complete");
    assert_eq!(rendered(&installed)["result"]["upToDate"], false);

    // The active release again: nothing is hashed or completed, so a damaged file goes unread and
    // a completion that would fail never runs.
    let server = root
        .join("versions/0.2.0")
        .join(CohortArtifact::Server.file_name());
    fs::write(&server, b"damaged").unwrap();
    let again = update(&fixture, &feeds[1].0, TEST_TABLE, refused_completion);
    assert_eq!(code(&again), "complete");
    let result = &rendered(&again)["result"];
    assert_eq!(
        (&result["upToDate"], &result["activated"], &result["reused"]),
        (&json!(true), &json!(true), &json!(true)),
        "{result}"
    );
    assert_eq!(pointer(&root).unwrap().active, newer);

    // A feed pointed back to a lower version is nothing to do rather than a refusal: nothing is
    // copied and the pointer stays.
    let lower = update(&fixture, &feeds[0].0, TEST_TABLE, refused_completion);
    assert_eq!(code(&lower), "complete");
    let result = &rendered(&lower)["result"];
    assert_eq!(
        (
            &result["upToDate"],
            &result["activated"],
            &result["releasePath"]
        ),
        (&json!(true), &json!(false), &Value::Null),
        "{result}"
    );
    assert!(!root.join("versions/0.1.0").exists());
    assert_eq!(pointer(&root).unwrap().active, newer);
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
    publish_feed(&source, &bytes, Some(&sign(&bytes)));
    open_to_users(&source);

    // A store root another account can change is refused before anything is written, and the
    // cause names that folder.
    let shared = fixture.0.join("shared");
    fs::create_dir(&shared).unwrap();
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
