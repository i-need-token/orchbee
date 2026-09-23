//! Windows argv and localized help evidence for directory Workspace creation.
//! Invalid runtime configuration proves invalid launch input never reaches a Site.
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
fn launch_usage_is_checked_before_runtime_without_environment_value_echo() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for tail in [
            vec!["--env", "TOKEN=private-value"],
            vec!["--cwd", "relative"],
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
        assert!(text.contains("pending/started/failed/unavailable"));
    }
}
