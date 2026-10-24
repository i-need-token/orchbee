//! Exercise the real process boundary without inheriting any user's preference scope.
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "orchbee-settings-process-{}-{nonce}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(root)
    }
    fn exchange(&self, request: &Value) -> Value {
        let mut child = Command::new(env!("CARGO_BIN_EXE_orchbee-settings"))
            .arg("--stdio-v1")
            .env("ORCHBEE_CONFIG_FILE", self.0.join("config.toml"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input
            .write_all(&serde_json::to_vec(request).unwrap())
            .unwrap();
        drop(input);
        let until = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= until {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("settings bridge exceeded bounded exchange");
            }
            thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.success(), response["kind"] == "snapshot");
        response
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn malformed_requests_have_no_effect_and_separate_invocations_observe_one_committed_record() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.exchange(&json!({"operation": "read", "path": "elsewhere"}))["kind"],
        "refused"
    );
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    let initial = fixture.exchange(&json!({"operation": "read"}));
    let committed = fixture.exchange(
        &json!({"operation": "update", "source": initial["snapshot"]["source"],
        "changes": {"agentNotificationMode": "hidden"}}),
    );
    assert_eq!(
        committed["snapshot"]["values"]["agentNotificationMode"],
        "hidden"
    );
    let stale = fixture.exchange(
        &json!({"operation": "update", "source": initial["snapshot"]["source"],
        "changes": {"soundEnabled": true}}),
    );
    assert_eq!(stale["reason"], "source-changed");
    let reopened = fixture.exchange(&json!({"operation": "read"}));
    assert_eq!(reopened["snapshot"], committed["snapshot"]);
}

#[test]
fn competing_processes_cannot_overwrite_and_explicit_reset_is_durable() {
    let fixture = Fixture::new();
    let initial = fixture.exchange(&json!({"operation": "read"}));
    let barrier = std::sync::Barrier::new(2);
    let results = thread::scope(|scope| {
        let run = |locale| {
            barrier.wait();
            fixture.exchange(
                &json!({"operation": "update", "source": initial["snapshot"]["source"],
                "changes": {"locale": locale}}),
            )
        };
        let first = scope.spawn(move || run("zh-CN"));
        let second = scope.spawn(move || run("en-US"));
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(
        results
            .iter()
            .filter(|result| result["kind"] == "snapshot")
            .count(),
        1
    );
    let refused = results
        .iter()
        .find(|result| result["kind"] == "refused")
        .unwrap();
    assert!(refused["reason"] == "source-changed" || refused["reason"] == "busy");
    let winner = results
        .iter()
        .find(|result| result["kind"] == "snapshot")
        .unwrap();
    let reset = fixture.exchange(
        &json!({"operation": "update", "source": winner["snapshot"]["source"],
        "changes": {"locale": null}}),
    );
    assert_eq!(reset["snapshot"]["values"]["locale"], "system");
    assert_eq!(
        fixture.exchange(&json!({"operation": "read"}))["snapshot"],
        reset["snapshot"]
    );
}
