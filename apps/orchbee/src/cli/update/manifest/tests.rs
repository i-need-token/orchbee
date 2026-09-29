//! Release trust-boundary regressions; no network, filesystem or processes.
use super::*;
use serde_json::json;

fn sample() -> Value {
    let files = [
        "orchbee.exe",
        "orchbee-server.exe",
        "orchbee-worker.exe",
        "orchbee-terminal-host.exe",
        "cohort.json",
        "agent-observation-adapters/capsule.json",
        "conpty/conpty.dll",
        "conpty/OpenConsole.exe",
        "conpty/Microsoft.ConPTY-LICENSE.txt",
    ]
    .iter()
    .map(|path| json!({"path":path,"url":path,"sha256":"a".repeat(64),"size":100}))
    .collect::<Vec<_>>();
    json!({"schemaVersion":1,"product":"orchbee","platform":"windows-x86_64","channel":"stable",
        "version":"0.1.0","buildId":"test-1","files":files})
}
fn accepted(value: &Value) -> bool {
    Release::parse(
        &serde_json::to_vec(value).unwrap(),
        WindowsUpdateChannel::Stable,
    )
    .is_some()
}
#[test]
fn incomplete_wrong_channel_aliasing_and_unbounded_releases_are_rejected() {
    let original = sample();
    assert!(accepted(&original));
    for (field, value) in [
        ("channel", json!("preview")),
        ("platform", json!("linux-x86_64")),
        ("schemaVersion", json!(2)),
        ("product", json!("herdr")),
        ("version", json!("a\nb")),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        assert!(!accepted(&changed), "{field}");
    }
    for path in [
        "release.json",
        "RELEASE.JSON",
        "../evil",
        "C:/evil",
        "a:stream",
        "a/NUL.exe",
        "a/COM1.txt",
        "a.",
        "orchbee.EXE",
        "agent-observation-adapters",
    ] {
        let mut changed = original.clone();
        changed["files"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":path,"url":"x","sha256":"a".repeat(64),"size":1}));
        assert!(!accepted(&changed), "{path}");
    }
    let mut missing = original.clone();
    missing["files"].as_array_mut().unwrap().pop();
    assert!(!accepted(&missing));
    for (field, value) in [
        ("size", json!(FILE_LIMIT + 1)),
        ("size", json!(-1)),
        ("sha256", json!("a".repeat(63))),
        ("url", json!("http://example.test/file")),
        ("url", json!("https://user:secret@example.test/file")),
    ] {
        let mut changed = original.clone();
        changed["files"][0][field] = value;
        assert!(!accepted(&changed), "{field}");
    }
    let mut extra = original;
    extra["unknown"] = json!(true);
    assert!(!accepted(&extra));
}
