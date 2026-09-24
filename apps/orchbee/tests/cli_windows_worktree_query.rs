//! Source-query grammar and offline discoverability never activate a Site.
#![cfg(windows)]
use std::process::{Command, Output, Stdio};

fn run(words: &[&str], locale: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(words)
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .env("LC_ALL", locale)
        .stdin(Stdio::null())
        .output()
        .expect("CLI process")
}

#[test]
fn conflicting_sources_fail_before_runtime_and_help_describes_both_contracts() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for tail in [
            vec!["--workspace=w", "--cwd=C:/repo"],
            vec!["--project=p", "--cwd=C:/repo"],
            vec!["--project=p", "--trust-repository"],
            vec!["--cwd=relative"],
            vec!["--detach"],
        ] {
            let words = [vec!["worktree", "list", "--format=json"], tail].concat();
            let output = run(&words, locale);
            assert_eq!(output.status.code(), Some(2), "{words:?}");
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["failure"]["kind"], "invalid-usage");
            assert!(document.get("source").is_none());
        }
        let output = run(&["help", "worktree", "list"], locale);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for token in [
            "--workspace",
            "--cwd",
            "--trust-repository",
            "--project",
            "--directory",
            "orchbee.worktree.query",
            "orchbee.worktree.list",
        ] {
            assert!(text.contains(token), "{token}");
        }
    }
    let output = run(
        &["api", "schema", "orchbee.worktree.query", "--format=json"],
        "en_US.UTF-8",
    );
    assert!(output.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["version"], 21);
    assert_eq!(
        schema["result"]["schema"]["schemaId"],
        "orchbee.worktree.query"
    );
}
