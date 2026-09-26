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
fn user_paths_parse_before_cohort_access_and_invalid_home_is_redacted() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for path in [".", "../new checkout", "~/new checkout"] {
            for mut words in [
                vec!["worktree", "list", "--cwd", path],
                vec!["worktree", "create", "--cwd", path, "--path", path],
                vec!["worktree", "open", "--cwd", path, "--path", path],
            ] {
                words.push("--format=json");
                let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
                    .args(&words)
                    .env("USERPROFILE", "C:/nonexistent-caller-home")
                    .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
                    .env("LC_ALL", locale)
                    .stdin(Stdio::null())
                    .output()
                    .unwrap();
                let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(
                    document["failure"]["kind"], "check-failed",
                    "{words:?}: {document}"
                );
                assert_eq!(document["failure"]["details"]["stage"], "cohort");
            }
        }
        for (verb, option) in [("list", "--cwd"), ("create", "--path"), ("open", "--path")] {
            let output = Command::new(env!("CARGO_BIN_EXE_orchbee"))
                .args(["worktree", verb, option, "~", "--format=json"])
                .env("USERPROFILE", "private-invalid-home-value")
                .env("HOME", "C:/must-not-fall-back")
                .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
                .env("LC_ALL", locale)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["failure"]["kind"], "invalid-usage");
            let text = format!(
                "{}{}",
                String::from_utf8(output.stdout).unwrap(),
                String::from_utf8(output.stderr).unwrap()
            );
            assert!(text.contains("USERPROFILE"));
            assert!(!text.contains("private-invalid-home-value"));
            assert!(!text.contains("must-not-fall-back"));
        }
    }
}

#[test]
fn conflicting_sources_fail_before_runtime_and_help_describes_both_contracts() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for tail in [
            vec!["--workspace=w", "--cwd=C:/repo"],
            vec!["--project=p", "--cwd=C:/repo"],
            vec!["--project=p", "--trust-repository"],
            vec!["--cwd=C:relative"],
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
    assert_eq!(schema["version"], 62);
    assert_eq!(
        schema["result"]["schema"]["schemaId"],
        "orchbee.worktree.query"
    );
}
