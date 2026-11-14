//! Engine fixtures shared by the Windows install, update and channel tests: a private temporary
//! root, a console-interrupt context, synthetic signed release directories, a probe command, and an
//! in-process completion that never touches a user PATH.
use super::super::{Context, Error, complete, install::Completion};
use crate::{cli::console_interrupt::ArmedSignals, release_store::read_pointer};
use orchbee_local_runtime::{CohortArtifact, PrivateCache};
use orchbee_release_records::{Entry, Pointer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const PEERS: [CohortArtifact; 3] = [
    CohortArtifact::Server,
    CohortArtifact::Worker,
    CohortArtifact::TerminalHost,
];

/// Copies every file below `source` into the folder `target`, which may already exist.
pub(super) fn copy_into(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_into(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

pub(super) fn digest(path: &Path) -> String {
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

/// Stages a complete cohort in `source`; the Client is a text file holding the version line of
/// `version`.
pub(super) fn stage_cohort(source: &Path, version: &str) {
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
    copy_into(&adapters, &source.join("agent-observation-adapters"));
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
pub(super) fn manifest(source: &Path, change: impl FnOnce(&mut Value)) -> Vec<u8> {
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

/// Writes `manifest` and `signature` as the release directory's `release.json` and its document.
pub(super) fn publish_manifest(source: &Path, manifest: &[u8], signature: Option<&[u8]>) {
    fs::write(source.join("release.json"), manifest).unwrap();
    let document = source.join("release.json.sig");
    match signature {
        Some(signature) => fs::write(&document, signature).unwrap(),
        None => {
            let _ = fs::remove_file(&document);
        }
    }
}

pub(super) fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn entry_for(version: &str, manifest: &[u8]) -> Entry {
    Entry {
        product: version.into(),
        manifest: sha(manifest),
    }
}

/// A private directory under the test process's temporary folder, removed on drop.
pub(super) struct Fixture(pub(super) PathBuf);

impl Fixture {
    pub(super) fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "orchbee-release-test-{}-{nanos}",
            std::process::id()
        ));
        PrivateCache::prepare(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-release-test-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}

/// A context with a minute's deadline. The console interrupt has one owner per process, so this
/// waits while another test holds it.
pub(super) fn context() -> Context {
    let give_up = Instant::now() + Duration::from_secs(30);
    loop {
        match ArmedSignals::arm_interrupts() {
            Ok(signals) => {
                return Context {
                    signals,
                    deadline: Instant::now() + Duration::from_secs(60),
                    curl_checked: false,
                    completion_diagnostic: None,
                    storage_cause: None,
                };
            }
            Err(_) if Instant::now() < give_up => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => panic!("console interrupt: {error}"),
        }
    }
}

/// Prints the staged Client's bytes, which this fixture writes as the Client's version line.
pub(super) fn print_probe(client: &Path) -> Command {
    let mut command = Command::new("cmd.exe");
    command.args(["/d", "/c", "type"]).arg(client);
    command
}

/// Completes the install of a published version directory in process, as its Client's completion
/// helper would after verifying its own signature, so it fails unless it receives a published
/// release. It never performs the PATH step, which would edit this user's real `Path`.
pub(super) fn in_process_completion(
    root: &Path,
    version: &Path,
    completion: Completion,
    _context: &mut Context,
) -> Result<(), Error> {
    let bytes = fs::read(version.join("release.json")).map_err(|_| Error::NotReleaseDirectory)?;
    let entry = Entry {
        product: version.file_name().unwrap().to_string_lossy().into_owned(),
        manifest: format!("{:x}", Sha256::digest(&bytes)),
    };
    complete::activate(
        root,
        &entry,
        Completion {
            path: false,
            ..completion
        },
    )
}

/// The unpack step of a run that must end before any archive is unpacked.
pub(super) fn never_unpack(_unpacked: &Path, _context: &mut Context) -> Result<(), Error> {
    unreachable!("this run never unpacks an archive")
}

/// The pointer under `root`, or `None` when it is missing or names no valid record.
pub(super) fn pointer(root: &Path) -> Option<Pointer> {
    read_pointer(root)
        .unwrap()
        .and_then(|bytes| Pointer::parse(&bytes))
}
