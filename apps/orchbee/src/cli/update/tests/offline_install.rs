//! The install engine on a local feed signed with the test key: synthetic cohort peers, a probe
//! that prints the staged Client's bytes, and an activation that only reads the published release.
//! This proves verification, staging, publication and reuse, not Server, Worker or Host behavior
//! or user PATH activation.
use super::super::{
    Error, Report, engine,
    signature::{TEST_TABLE, TrustedKey},
};
use super::install_fixture::{Fixture, context, print_probe, reading_activation, report};
use super::{entry, sign, signed_bytes, stranger};
use orchbee_local_runtime::{Cohort, CohortArtifact};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
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

/// Stages a complete cohort feed in `source`; the Client is a text file holding its version line.
fn stage_cohort(source: &Path) {
    fs::create_dir(source).unwrap();
    fs::write(
        source.join(CohortArtifact::Client.file_name()),
        b"orchbee 0.1.0\r\n",
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
}

fn run(fixture: &Fixture, keys: &[TrustedKey]) -> Run {
    let data = fixture.0.join("data");
    let state = move |name: &str| (name == "ORCHBEE_HOME").then(|| data.clone().into_os_string());
    let inputs = engine::Inputs {
        channel: None,
        state: &state,
        manifest_source: Some(
            fixture
                .0
                .join("source/release.json")
                .to_string_lossy()
                .into_owned(),
        ),
        keys,
        install_root: Ok(fixture.0.join("install")),
        probe: print_probe,
        activation: reading_activation,
    };
    let mut context = context();
    let mut report = report();
    let result = engine::run(&inputs, &mut context, &mut report);
    Run { result, report }
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
    stage_cohort(&source);
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
    assert_eq!(
        published,
        root.join(format!("{:x}", Sha256::digest(&bytes)))
    );
    assert!(first.report.activated && !first.report.reused);
    assert!(first.report.cleanup_pending.is_none());
    assert_eq!(fs::read(published.join("release.json")).unwrap(), bytes);
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
            Box::new(|value: &mut Value| value["files"][0]["sha256"] = json!("0".repeat(64)))
                as Box<dyn FnOnce(&mut Value)>,
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
        assert!(
            !root
                .join(format!("{:x}", Sha256::digest(&changed)))
                .exists()
        );
        assert_eq!(fs::read(&client).unwrap(), executable);
        assert_eq!(stages(&root), 0);
    }
    fs::write(published.join("release.json"), b"corrupt metadata").unwrap();
    assert_eq!(
        crate::cli::installation::inspect(&client, Some(&admitted), "0.1.0")
            .expect("Windows installation observation")
            .delivery(),
        "unknown"
    );
}
