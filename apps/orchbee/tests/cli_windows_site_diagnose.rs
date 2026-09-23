//! Public Windows diagnostics without a cohort, Site or provider process.
#![cfg(windows)]

use orchbee_local_ipc::{LocalListener, LocalStream};
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

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-diagnose-中文-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn scope(&self) -> String {
        let sid = orchbee_local_ipc::current_user_sid().unwrap();
        format!("orchbee-native-{:x}", Sha256::digest(sid.as_bytes()))[..47].to_owned()
    }

    fn run(&self, format: &str, locale: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args(["site", "diagnose", "--format", format])
            .env("ORCHBEE_RUNTIME_BASE", &self.0)
            .env("LC_ALL", locale)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn document(&self) -> Value {
        let output = self.run("json", "en_US.UTF-8");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "../src/cli/schemas/orchbee.site.diagnose.windows.json"
        ))
        .unwrap();
        let validator = jsonschema::draft202012::new(&schema).unwrap();
        validator.validate(&document).unwrap();
        let mut invalid = document.clone();
        invalid["result"]["endpoints"]["client"]["mode"] = "0600".into();
        assert!(!validator.is_valid(&invalid));
        document
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let root = self.0.canonicalize().unwrap();
        assert_eq!(
            root.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-diagnose-中文-")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn missing_and_invalid_scope_never_creates_or_repairs_descendants() {
    let fixture = Fixture::new();
    let document = fixture.document();
    assert_eq!(document["result"]["complete"], true);
    assert_eq!(document["result"]["runtimeDirectory"]["state"], "absent");
    assert_eq!(document["result"]["marker"]["reason"], "parent-unavailable");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    let scope_path = fixture.0.join(fixture.scope());
    fs::write(&scope_path, b"user-owned obstruction").unwrap();
    let document = fixture.document();
    assert_eq!(
        document["result"]["runtimeDirectory"]["state"],
        "uninspectable"
    );
    assert_eq!(
        document["result"]["endpoints"]["client"]["reason"],
        "parent-unavailable"
    );
    assert_eq!(fs::read(scope_path).unwrap(), b"user-owned obstruction");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn marker_lock_pipe_and_localization_are_facts_without_a_connection() {
    let fixture = Fixture::new();
    let site =
        Rendezvous::prepare_mode_checked(&fixture.0, SiteScope::parse(&fixture.scope()).unwrap())
            .unwrap();
    let document = fixture.document();
    assert_eq!(document["result"]["runtimeDirectory"]["private"], true);
    assert_eq!(
        document["result"]["locks"]["siteLifetime"]["state"],
        "absent"
    );
    assert_eq!(fs::read_dir(site.runtime_directory()).unwrap().count(), 0);
    let held = match site.try_acquire_site_lifetime_lock().unwrap() {
        LockAttempt::Acquired(held) => held,
        LockAttempt::Contended => panic!("isolated fixture"),
    };
    site.commit_activation().unwrap();
    let listener = LocalListener::bind(site.client_socket_path()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let document = fixture.document();
    assert_eq!(document["result"]["locks"]["siteLifetime"]["state"], "held");
    assert_eq!(document["result"]["marker"]["state"], "known");
    assert_eq!(document["result"]["endpoints"]["client"]["state"], "known");
    assert!(document["result"].get("hostScope").is_none());
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    let chinese = fixture.run("json", "zh_CN.UTF-8");
    assert_eq!(
        serde_json::from_slice::<Value>(&chinese.stdout).unwrap(),
        document
    );
    let chinese = fixture.run("text", "zh_CN.UTF-8");
    assert!(chinese.status.success());
    let text = String::from_utf8(chinese.stdout).unwrap();
    assert!(text.contains("管道名称"));
    assert!(text.contains("中文"));
    let client = LocalStream::connect(site.client_socket_path()).unwrap();
    let (server, ()) = listener.accept().unwrap();
    drop((client, server, listener, held));
    let document = fixture.document();
    assert_eq!(document["result"]["locks"]["siteLifetime"]["state"], "free");
    assert_eq!(document["result"]["endpoints"]["client"]["state"], "absent");
    fs::write(
        site.runtime_directory().join("activation.started"),
        b"do not repair",
    )
    .unwrap();
    let document = fixture.document();
    assert_eq!(document["result"]["marker"]["state"], "uninspectable");
    assert_eq!(
        fs::read(site.runtime_directory().join("activation.started")).unwrap(),
        b"do not repair"
    );
    assert!(!site.runtime_directory().join("activation.lock").exists());
}
