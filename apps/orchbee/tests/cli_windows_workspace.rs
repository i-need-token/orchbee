//! Windows argv and localized help evidence for Workspace creation and group closure.
//! Invalid runtime configuration proves invalid launch input never reaches a Session.
#![cfg(windows)]
use std::process::{Output, Stdio};
fn run(words: &[&str], locale: &str) -> Output {
    crate::staged_client::command()
        .args(words)
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .env("LC_ALL", locale)
        .stdin(Stdio::null())
        .output()
        .expect("CLI process")
}
#[test]
fn launch_usage_is_checked_before_runtime_without_environment_value_echo() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for tail in [
            vec!["--cwd", "C:relative"],
            vec!["--cwd", r"C:\repo", "--env", "ORCHBEE_SECRET=private-value"],
            vec![
                "--cwd",
                r"C:\repo",
                "--profile",
                "dev",
                "--exec",
                r"C:\program.exe",
            ],
        ] {
            let mut words = vec![
                "workspace",
                "create",
                "--project",
                "P",
                "--label",
                "W",
                "--format",
                "json",
            ];
            words.extend(tail);
            let output = run(&words, locale);
            assert_eq!(output.status.code(), Some(2));
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["failure"]["kind"], "invalid-usage");
            assert!(document.get("source").is_none());
            assert!(!String::from_utf8_lossy(&output.stdout).contains("private-value"));
            assert!(!String::from_utf8_lossy(&output.stderr).contains("private-value"));
        }
        let output = run(
            &[
                "workspace",
                "create",
                "--project",
                "P",
                "--label",
                "开发",
                "--cwd",
                r"C:\repo space",
                "--env",
                "TOKEN=a=b $literal",
                "--format",
                "json",
            ],
            locale,
        );
        assert_ne!(output.status.code(), Some(2));
        assert!(!output.status.success());
    }
}
#[test]
fn help_discovers_directory_launch_and_its_recovery_contract() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = run(&["workspace", "create", "--help"], locale);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for flag in [
            "--workspace",
            "--client",
            "--focus",
            "--no-focus",
            "--timeout",
            "--cwd",
            "--env",
            "--exec",
            "--arg",
            "--profile",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
        ] {
            assert!(text.contains(flag));
        }
        let unwrapped = text.split_whitespace().collect::<String>();
        assert!(unwrapped.contains("pending/started/failed/unavailable"));
    }
}

#[test]
fn group_close_confirmation_and_scope_conflicts_fail_before_runtime_access() {
    let id = "00000000000000000000000000000001";
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for words in [
            vec!["workspace", "close", "demo", "--group"],
            vec![
                "workspace",
                "close",
                "demo",
                "--group",
                "--confirm",
                "--project",
                id,
            ],
            vec![
                "workspace",
                "close-status",
                "--group",
                "--request",
                id,
                "--workspace",
                id,
            ],
            vec![
                "workspace",
                "close-resume",
                "--group",
                "--request",
                id,
                "--generation",
                "1",
            ],
        ] {
            let mut words = words;
            words.push("--format=json");
            let output = run(&words, locale);
            assert_eq!(output.status.code(), Some(2), "{words:?}");
            let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(document["failure"]["kind"], "invalid-usage");
            assert!(document.get("source").is_none());
        }
        for verb in ["close-all", "close-status", "close-resume"] {
            let output = run(&["help", "project", verb], locale);
            assert!(output.status.success());
            assert!(String::from_utf8(output.stdout).unwrap().contains(
                if verb == "close-status" {
                    "--request"
                } else {
                    "--confirm"
                }
            ));
        }
    }
}

#[test]
fn organization_grammar_is_complete_before_effects_and_discoverable_in_both_locales() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = run(&["--help"], locale);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for (family, verbs) in [
            (
                "workspace",
                &["set-project", "create-project", "ungroup", "move-block"][..],
            ),
            (
                "project",
                &["dissolve", "close-all", "close-status", "close-resume"][..],
            ),
        ] {
            assert!(text.contains(family), "{locale}: {family}");
            let family_output = run(&[family, "--help"], locale);
            assert!(family_output.status.success());
            assert!(family_output.stderr.is_empty());
            let directory = String::from_utf8(family_output.stdout).unwrap();
            for verb in verbs {
                assert!(directory.contains(verb), "{locale}: {family} {verb}");
            }
        }
        for words in [
            vec!["workspace", "set-project", "work"],
            vec![
                "workspace",
                "set-project",
                "work",
                "--project=p",
                "--cwd=C:/repo",
            ],
            vec!["workspace", "create-project", "work"],
            vec![
                "workspace",
                "create-project",
                "work",
                "--label=p",
                "--project=p",
            ],
            vec!["workspace", "ungroup", "work", "--project=p"],
            vec!["project", "dissolve"],
            vec!["project", "dissolve", "p", "--confirm"],
            vec!["project", "close-all", "p"],
            vec!["project", "close-status", "--workspace=work"],
            vec!["worktree", "create", "--workspace=w", "--project=p"],
            vec![
                "worktree",
                "remove",
                "--workspace=w",
                "--project=p",
                "--confirm",
            ],
        ] {
            let mut words = words;
            words.push("--format=json");
            let output = run(&words, locale);
            assert_eq!(output.status.code(), Some(2), "{words:?}");
            let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(doc["failure"]["kind"], "invalid-usage");
            assert!(doc.get("source").is_none());
        }
        for (family, verb) in [
            ("workspace", "set-project"),
            ("workspace", "create-project"),
            ("workspace", "ungroup"),
            ("project", "dissolve"),
            ("project", "close-all"),
            ("project", "close-status"),
            ("project", "close-resume"),
        ] {
            let output = run(&["help", family, verb], locale);
            assert!(output.status.success(), "{family} {verb}");
            assert!(output.stderr.is_empty());
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains(&format!("orchbee {family} {verb}"))
            );
        }
    }
}
