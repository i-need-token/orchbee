//! Public Windows diagnostics without a cohort, Session or provider process.
#![cfg(windows)]

use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_runtime::{
    LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionScope,
};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::{Output, Stdio},
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
        fs::create_dir(root.join("runtime")).unwrap();
        Self(root)
    }

    fn runtime(&self) -> PathBuf {
        self.0.join("runtime")
    }

    /// The default Session's runtime folder, which takes the Session's name.
    fn scope(&self) -> String {
        "default".to_owned()
    }

    fn run(&self, format: &str, locale: &str) -> Output {
        crate::staged_client::isolated()
            .args(["server", "diagnose", "--format", format])
            .env("ORCHBEE_RUNTIME_BASE", self.runtime())
            // A development or test build keeps its Session catalog under the profile.
            .env("USERPROFILE", self.0.join("profile"))
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
        let validator = validator();
        validator.validate(&document).unwrap();
        // The shared schema admits POSIX modes for every host; Windows rows never claim one.
        assert_eq!(
            document["result"]["endpoints"]["client"]["mode"],
            serde_json::json!({"kind": "unsupported"})
        );
        let mut invalid = document.clone();
        invalid["result"]["endpoints"]["client"]["mode"] = "rw-------".into();
        assert!(!validator.is_valid(&invalid));
        document
    }
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(include_str!(
        "../src/cli/schemas/orchbee.server.diagnose.json"
    ))
    .unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
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
    assert_eq!(fs::read_dir(fixture.runtime()).unwrap().count(), 0);
    let scope_path = fixture.runtime().join(fixture.scope());
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
    assert_eq!(fs::read_dir(fixture.runtime()).unwrap().count(), 1);
}

#[test]
fn marker_lock_pipe_and_localization_are_facts_without_a_connection() {
    let fixture = Fixture::new();
    let session = Rendezvous::prepare_mode_checked(
        &fixture.runtime(),
        SessionScope::parse(&fixture.scope()).unwrap(),
    )
    .unwrap();
    let document = fixture.document();
    assert_eq!(document["result"]["runtimeDirectory"]["private"], true);
    assert_eq!(
        document["result"]["locks"]["sessionLifetime"]["state"],
        "absent"
    );
    assert_eq!(
        fs::read_dir(session.runtime_directory()).unwrap().count(),
        0
    );
    let held = match session.try_acquire_session_lifetime_lock().unwrap() {
        LockAttempt::Acquired(held) => held,
        LockAttempt::Contended => panic!("isolated fixture"),
    };
    session.commit_activation().unwrap();
    let listener = LocalListener::bind(session.client_socket_path()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let document = fixture.document();
    assert_eq!(
        document["result"]["locks"]["sessionLifetime"]["state"],
        "held"
    );
    assert_eq!(document["result"]["marker"]["state"], "known");
    assert_eq!(document["result"]["marker"]["activation"], "active");
    assert_eq!(document["result"]["endpoints"]["client"]["state"], "known");
    assert_eq!(
        document["result"]["hostScope"],
        serde_json::json!({"kind": "unsupported"})
    );
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
    let client = LocalStream::connect(session.client_socket_path()).unwrap();
    let (server, ()) = listener.accept().unwrap();
    drop((client, server, listener, held));
    let document = fixture.document();
    assert_eq!(
        document["result"]["locks"]["sessionLifetime"]["state"],
        "free"
    );
    assert_eq!(document["result"]["endpoints"]["client"]["state"], "absent");
    // This fixture owns no child processes; it can publish a completed stop.
    let mut record = session.retain_active_record().unwrap();
    record.complete([1; 16]).unwrap();
    let stopped = fixture.document();
    assert_eq!(stopped["result"]["marker"]["state"], "known");
    assert_eq!(stopped["result"]["marker"]["activation"], "stopped");
    assert_eq!(
        stopped["result"]["marker"]["stopRequestId"],
        "01010101010101010101010101010101"
    );
    assert_eq!(stopped["result"]["advisories"], serde_json::json!([]));
    drop(record);
    session.commit_activation().unwrap();
    fs::write(
        session.runtime_directory().join("activation.started"),
        b"do not repair",
    )
    .unwrap();
    let document = fixture.document();
    assert_eq!(document["result"]["marker"]["state"], "uninspectable");
    assert_eq!(
        fs::read(session.runtime_directory().join("activation.started")).unwrap(),
        b"do not repair"
    );
    assert!(!session.runtime_directory().join("activation.lock").exists());
}

#[test]
fn unreadable_session_records_are_reported_only_for_the_session_they_name() {
    let fixture = Fixture::new();
    let (base, home) = (fixture.0.join("base"), fixture.0.join("home"));
    fs::create_dir(&base).unwrap();
    fs::create_dir(&home).unwrap();
    // A record its admission rejects, in the Session folder of "work".
    let folder = orchbee_local_runtime::DataRoot::under_home(&home)
        .sessions()
        .join("work");
    fs::create_dir_all(&folder).unwrap();
    let record = folder.join("session.json");
    fs::write(&record, br#"{"version":1,"name":"work""#).unwrap();
    let entries = fs::read_dir(&base).unwrap().count();
    let run = |session: &str, format: &str, locale: &str| {
        crate::staged_client::isolated()
            .args([
                "--session",
                session,
                "server",
                "diagnose",
                "--format",
                format,
            ])
            .env("ORCHBEE_RUNTIME_BASE", &base)
            // A development or test build keeps its Session folders under the profile.
            .env("USERPROFILE", &home)
            .env("LC_ALL", locale)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let document = |session: &str| {
        let output = run(session, "json", "en_US.UTF-8");
        assert!(output.status.success());
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator().validate(&document).unwrap();
        document
    };
    // The Session is found by any letter case of its name.
    let work = document("WORK");
    assert_eq!(
        work["result"]["unrecognizedRecords"],
        serde_json::json!({"state": "known", "reason": null, "records": [{
            "record": "catalog-record",
            "reason": "unreadable",
            "path": record.to_str().unwrap(),
            "pathUtf16": null,
        }]})
    );
    assert_eq!(
        work["result"]["advisories"],
        serde_json::json!(["unrecognized-records"])
    );
    let other = document("other");
    assert_eq!(
        other["result"]["unrecognizedRecords"],
        serde_json::json!({"state": "known", "reason": null, "records": []})
    );
    assert_eq!(other["result"]["advisories"], serde_json::json!([]));
    let text = run("work", "text", "zh_CN.UTF-8");
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(text.contains("unrecognizedRecords.1"), "{text}");
    assert!(text.contains("unreadable"), "{text}");
    assert!(text.contains("所列记录"), "{text}");
    // Diagnosis reads only: the record keeps its bytes and nothing is created for either name.
    assert_eq!(fs::read(&record).unwrap(), br#"{"version":1,"name":"work""#);
    assert_eq!(fs::read_dir(&base).unwrap().count(), entries);
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
}
