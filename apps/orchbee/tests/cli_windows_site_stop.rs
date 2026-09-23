//! Public stop grammar and persisted receipt checks never activate a Site.
#![cfg(windows)]
use orchbee_local_site_runtime::{
    LockAttempt, ModeCheckedLocalSiteRendezvous as Rendezvous, SiteScope,
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
            "orchbee-site-stop-中文-{:032x}",
            u128::from_le_bytes(id)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn run(&self, arguments: &[&str], locale: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", &self.0)
            .env("LC_ALL", locale)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn site(&self) -> Rendezvous {
        let scope = format!(
            "orchbee-native-{:x}",
            Sha256::digest(orchbee_local_ipc::current_user_sid().unwrap().as_bytes())
        )[..47]
            .to_owned();
        Rendezvous::prepare_mode_checked(&self.0, SiteScope::parse(&scope).unwrap()).unwrap()
    }
    fn query(&self) -> Value {
        let output = self.run(&["site", "stop-status", ID, "--format=json"], "en_US.UTF-8");
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.site.stop.json"))
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
                .starts_with("orchbee-site-stop-中文-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn missing_receipt_query_and_invalid_destructive_argv_have_no_runtime_effects() {
    let fixture = Fixture::new();
    assert_eq!(fixture.query()["result"]["outcome"], "not-recorded");
    for arguments in [
        vec!["site", "stop", "--format=json"],
        vec!["site", "stop", "--confirm", "--retry", "--format=json"],
    ] {
        assert_eq!(
            fixture.run(&arguments, "en_US.UTF-8").status.code(),
            Some(2)
        );
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[test]
fn only_matching_completed_record_and_free_lifetime_prove_stop_without_a_cohort() {
    let fixture = Fixture::new();
    let site = fixture.site();
    site.commit_activation().unwrap();
    let LockAttempt::Acquired(lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime");
    };
    let mut record = site.retain_active_record().unwrap();
    record.complete([0x11; 16]).unwrap();
    assert_ne!(fixture.query()["result"]["outcome"], "stopped");
    drop(lifetime);
    assert_eq!(fixture.query()["result"]["outcome"], "stopped");
    let output = fixture.run(
        &[
            "site",
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
    let text = fixture.run(&["site", "stop-status", ID], "zh_CN.UTF-8");
    assert!(text.status.success());
    assert!(String::from_utf8(text.stdout).unwrap().contains("已停止"));
    let other = fixture.run(
        &[
            "site",
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
    site.commit_activation().unwrap();
    assert_ne!(fixture.query()["result"]["outcome"], "stopped");
    fs::write(
        site.runtime_directory().join("activation.started"),
        b"orchbee-stopped-v1:",
    )
    .unwrap();
    assert_eq!(fixture.query()["result"]["reason"], "activation-record");
}
