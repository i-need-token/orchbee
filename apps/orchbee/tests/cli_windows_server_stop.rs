//! Public stop grammar and persisted receipt checks never activate a Session.
#![cfg(windows)]
use orchbee_local_runtime::{
    LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionScope,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

const ID: &str = "11111111111111111111111111111111";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut id = [0; 16];
        getrandom::fill(&mut id).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-session-stop-中文-{:032x}",
            u128::from_le_bytes(id)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("runtime")).unwrap();
        Self(root)
    }
    fn runtime(&self) -> PathBuf {
        self.0.join("runtime")
    }
    /// A development or test build keeps its state, including the destructive request log,
    /// under the profile.
    fn profile(&self) -> PathBuf {
        self.0.join("profile")
    }
    fn run(&self, arguments: &[&str], locale: &str) -> Output {
        Command::new(crate::staged_client::client())
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", self.runtime())
            .env("USERPROFILE", self.profile())
            .env("LC_ALL", locale)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn session(&self) -> Rendezvous {
        let scope = format!(
            "orchbee-native-{:x}",
            Sha256::digest(orchbee_local_ipc::current_user_sid().unwrap().as_bytes())
        )[..47]
            .to_owned();
        Rendezvous::prepare_mode_checked(&self.runtime(), SessionScope::parse(&scope).unwrap())
            .unwrap()
    }
    fn query(&self) -> Value {
        let output = self.run(
            &["server", "stop-status", ID, "--format=json"],
            "en_US.UTF-8",
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.server.stop.json"))
                .unwrap();
        jsonschema::draft202012::new(&schema)
            .unwrap()
            .validate(&document)
            .unwrap();
        document
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-session-stop-中文-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn version_and_nested_help_are_offline_and_stop_still_requires_confirmation() {
    let fixture = Fixture::new();
    let blocked_runtime = fixture.runtime().join("runtime-is-a-file");
    fs::write(&blocked_runtime, b"untouched").unwrap();
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let run = |arguments: &[&str]| {
            Command::new(crate::staged_client::client())
                .args(arguments)
                .env("ORCHBEE_RUNTIME_BASE", &blocked_runtime)
                .env("USERPROFILE", fixture.profile())
                .env("ORCHBEE_SESSION", "invalid/session")
                .env("LC_ALL", locale)
                .stdin(Stdio::null())
                .output()
                .unwrap()
        };
        for flag in ["-v", "-V", "--version"] {
            let output = run(&[flag]);
            assert_eq!(output.status.code(), Some(0), "{flag}: {output:?}");
            assert!(output.stderr.is_empty());
            assert_eq!(
                output.stdout,
                concat!("orchbee ", env!("CARGO_PKG_VERSION"), "\n").as_bytes()
            );
        }
        for path in [
            &["server"][..],
            &["server", "stop"][..],
            &["server", "stop-status"][..],
            &["terminal", "session", "control"][..],
        ] {
            let reference = run(&std::iter::once("help")
                .chain(path.iter().copied())
                .collect::<Vec<_>>());
            assert_eq!(reference.status.code(), Some(0), "{path:?}: {reference:?}");
            assert!(!reference.stdout.is_empty());
            assert!(reference.stderr.is_empty());
            for flag in ["-h", "--help"] {
                let output = run(&path.iter().copied().chain([flag]).collect::<Vec<_>>());
                assert_eq!(output.status.code(), Some(0), "{path:?}: {output:?}");
                assert!(output.stderr.is_empty());
                assert_eq!(output.stdout, reference.stdout);
            }
        }
        let output = run(&["server", "stop", "--format=json"]);
        assert_eq!(output.status.code(), Some(2));
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["failure"]["kind"], "invalid-usage");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("--confirm")
        );
    }
    assert_eq!(fs::read(&blocked_runtime).unwrap(), b"untouched");
    assert_eq!(fs::read_dir(fixture.runtime()).unwrap().count(), 1);
}

#[test]
fn missing_receipt_query_and_invalid_destructive_argv_have_no_runtime_effects() {
    let fixture = Fixture::new();
    assert_eq!(fixture.query()["result"]["outcome"], "not-recorded");
    for arguments in [
        vec!["server", "stop", "--format=json"],
        vec!["server", "stop", "--confirm", "--retry", "--format=json"],
    ] {
        assert_eq!(
            fixture.run(&arguments, "en_US.UTF-8").status.code(),
            Some(2)
        );
    }
    assert_eq!(fs::read_dir(fixture.runtime()).unwrap().count(), 0);
}

#[test]
fn only_matching_completed_record_and_free_lifetime_prove_stop_without_a_cohort() {
    let fixture = Fixture::new();
    let session = fixture.session();
    session.commit_activation().unwrap();
    let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime");
    };
    let mut record = session.retain_active_record().unwrap();
    record.complete([0x11; 16]).unwrap();
    assert_ne!(fixture.query()["result"]["outcome"], "stopped");
    drop(lifetime);
    assert_eq!(fixture.query()["result"]["outcome"], "stopped");
    let output = fixture.run(
        &[
            "server",
            "stop",
            "--confirm",
            "--request-id",
            ID,
            "--format=json",
        ],
        "en_US.UTF-8",
    );
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"]["outcome"],
        "stopped"
    );
    let text = fixture.run(&["server", "stop-status", ID], "zh_CN.UTF-8");
    assert!(text.status.success());
    assert!(String::from_utf8(text.stdout).unwrap().contains("已停止"));
    let other = fixture.run(
        &[
            "server",
            "stop-status",
            "22222222222222222222222222222222",
            "--format=json",
        ],
        "en_US.UTF-8",
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&other.stdout).unwrap()["result"]["outcome"],
        "not-recorded"
    );
    drop(record);
    session.commit_activation().unwrap();
    assert_ne!(fixture.query()["result"]["outcome"], "stopped");
    fs::write(
        session.runtime_directory().join("activation.started"),
        b"orchbee-stopped-v1:",
    )
    .unwrap();
    assert_eq!(fixture.query()["result"]["reason"], "activation-record");
}
