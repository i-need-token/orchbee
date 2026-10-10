//! Real Windows process evidence for caller-context admission and offline
//! schemas. Invalid runtime configuration prevents any Site access.
#![cfg(windows)]

use std::process::{Command, Output, Stdio};

fn run(arguments: &[&str], terminal: Option<&str>, worker: Option<&str>, locale: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orchbee"));
    command
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
        .env("LC_ALL", locale)
        .env_remove("ORCHBEE_TERMINAL_ID")
        .env_remove("ORCHBEE_WORKER_ID")
        .stdin(Stdio::null());
    if let Some(value) = terminal {
        command.env("ORCHBEE_TERMINAL_ID", value);
    }
    if let Some(value) = worker {
        command.env("ORCHBEE_WORKER_ID", value);
    }
    command.output().expect("CLI process")
}

#[test]
fn attach_requires_inherited_console_before_runtime_or_agent_resolution() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for args in [
            vec!["terminal", "attach", "@terminal:01"],
            vec!["agent", "attach", "--name=alpha", "--format=json"],
        ] {
            let output = run(&args, None, None, locale);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty(), "attach is always interactive");
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("terminal session control"), "{stderr}");
            assert!(!stderr.contains("invalid-relative-runtime"));
        }
    }
}

#[test]
fn creation_checks_caller_metadata_before_runtime_and_explicit_client_bypasses_it() {
    for base in [
        vec!["tab", "create"],
        vec!["panel", "split", "--direction=right"],
    ] {
        for (terminal, worker) in [
            (Some("untrusted-secret-value"), Some("bad")),
            (Some("00000000000000000000000000000001"), None),
        ] {
            let mut args = base.clone();
            args.push("--format=json");
            let rejected = run(&args, terminal, worker, "en_US.UTF-8");
            assert_eq!(rejected.status.code(), Some(2));
            assert!(!String::from_utf8_lossy(&rejected.stderr).contains("untrusted-secret-value"));
            args.extend(["--client", "00000000000000000000000000000007"]);
            let explicit = run(&args, terminal, worker, "en_US.UTF-8");
            let clean = run(&args, None, None, "en_US.UTF-8");
            assert_eq!(explicit.status.code(), clean.status.code());
            assert_eq!(explicit.stdout, clean.stdout);
            assert_ne!(explicit.status.code(), Some(2));
        }
    }
}

#[test]
fn partial_and_invalid_context_fail_before_runtime_without_value_echo() {
    for verb in ["current", "process-info"] {
        for (terminal, worker) in [
            (Some("untrusted-secret-value"), Some("invalid")),
            (Some("00000000000000000000000000000001"), None),
        ] {
            for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
                let output = run(
                    &["panel", verb, "--format", "json"],
                    terminal,
                    worker,
                    locale,
                );
                assert_eq!(output.status.code(), Some(2));
                let document: serde_json::Value =
                    serde_json::from_slice(&output.stdout).expect("document");
                assert_eq!(document["failure"]["kind"], "invalid-usage");
                assert!(document.get("source").is_none());
                let diagnostic = String::from_utf8(output.stderr).expect("diagnostic");
                assert!(diagnostic.contains("panel current --panel"));
                assert!(!diagnostic.contains("untrusted-secret-value"));
                let text = run(&["panel", verb], terminal, worker, locale);
                assert_eq!(text.status.code(), Some(2));
                assert!(text.stdout.is_empty());
            }
        }
    }
}

#[test]
fn explicit_target_bypasses_invalid_caller_context_and_reaches_runtime_validation() {
    let output = run(
        &[
            "panel",
            "current",
            "--panel",
            "@panel:01",
            "--format",
            "json",
        ],
        Some("invalid"),
        None,
        "en_US.UTF-8",
    );
    let control = run(
        &["panel", "get", "@panel:01", "--format", "json"],
        None,
        None,
        "en_US.UTF-8",
    );
    assert_ne!(output.status.code(), Some(2));
    assert!(!output.status.success());
    assert_eq!(output.status.code(), control.status.code());
    assert_eq!(output.stdout, control.stdout);
}

#[test]
fn offline_catalog_is_closed_and_current_panel_schema_is_discoverable() {
    let output = run(&["api", "schema"], None, None, "en_US.UTF-8");
    assert!(output.status.success());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).expect("catalog");
    let schemas = document["result"]["schemas"]
        .as_object()
        .expect("dictionary");
    let validator =
        jsonschema::draft202012::new(&schemas["orchbee.api.schema"]).expect("catalog schema");
    assert!(
        validator.is_valid(&document),
        "catalog must satisfy its published closed schema"
    );
    let english = run(
        &["api", "schema", "orchbee.panel.current"],
        None,
        None,
        "en_US.UTF-8",
    );
    let chinese = run(
        &["api", "schema", "orchbee.panel.current"],
        None,
        None,
        "zh_CN.UTF-8",
    );
    assert_eq!(english.stdout, chinese.stdout);
    for arguments in [
        &["help", "panel", "current"][..],
        &["panel", "current", "--help"][..],
    ] {
        for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
            let help = run(arguments, None, None, locale);
            assert!(help.status.success());
            assert!(
                String::from_utf8(help.stdout)
                    .expect("help")
                    .contains("--panel")
            );
        }
    }
}

#[test]
fn panel_input_current_rejects_untrusted_metadata_before_admission_in_both_locales() {
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        for (terminal, worker) in [
            (None, None),
            (Some("untrusted-secret-value"), Some("bad")),
            (Some("00000000000000000000000000000001"), None),
        ] {
            let result = run(
                &[
                    "panel",
                    "input",
                    "--current",
                    "--right-click=panel",
                    "--format=json",
                ],
                terminal,
                worker,
                locale,
            );
            assert_eq!(result.status.code(), Some(2));
            let doc: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(doc["failure"]["kind"], "invalid-usage");
            assert!(!String::from_utf8_lossy(&result.stderr).contains("untrusted-secret-value"));
        }
        for args in [["help", "panel", "input"], ["panel", "input", "--help"]] {
            let result = run(&args, None, None, locale);
            assert!(result.status.success());
            let text = String::from_utf8(result.stdout).unwrap();
            assert!(text.contains("--right-click orchbee|panel") && text.contains("--current"));
        }
    }
}

#[test]
fn terminal_observe_rejects_invalid_grammar_before_effect_and_closes_missing_runtime() {
    let help = run(
        &["help", "terminal", "session", "observe"],
        None,
        None,
        "en_US.UTF-8",
    );
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("orchbee.terminal.stream"));
    let invalid = run(
        &["terminal", "session", "observe", "@terminal:ab", "--cols=0"],
        None,
        None,
        "en_US.UTF-8",
    );
    assert_eq!(invalid.status.code(), Some(2));
    let schema = run(
        &["api", "schema", "orchbee.terminal.stream"],
        None,
        None,
        "en_US.UTF-8",
    );
    let document: serde_json::Value =
        serde_json::from_slice(&schema.stdout).expect("schema document");
    let validator = jsonschema::draft202012::new(&document["result"]["schema"]["document"])
        .expect("stream schema");
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = run(
            &[
                "terminal",
                "session",
                "observe",
                "@terminal:ab",
                "--timeout=3000",
            ],
            None,
            None,
            locale,
        );
        assert!(!output.status.success());
        let record: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("one closed record");
        assert_eq!(record["kind"], "closed");
        assert_eq!(record["cleanup"], "not-attached");
        assert!(!output.stderr.is_empty());
        validator.validate(&record).expect("closed record conforms");
    }
}

#[test]
fn terminal_control_rejects_invalid_grammar_before_effect_and_closes_missing_runtime() {
    let help = run(
        &["help", "terminal", "session", "control"],
        None,
        None,
        "en_US.UTF-8",
    );
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("orchbee.terminal.control.input"));
    let invalid = run(
        &["terminal", "session", "control", "@terminal:ab", "--cols=0"],
        None,
        None,
        "en_US.UTF-8",
    );
    assert_eq!(invalid.status.code(), Some(2));
    let schema = run(
        &["api", "schema", "orchbee.terminal.stream"],
        None,
        None,
        "en_US.UTF-8",
    );
    let document: serde_json::Value =
        serde_json::from_slice(&schema.stdout).expect("schema document");
    let validator = jsonschema::draft202012::new(&document["result"]["schema"]["document"])
        .expect("stream schema");
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = run(
            &[
                "terminal",
                "session",
                "control",
                "@terminal:ab",
                "--timeout=3000",
            ],
            None,
            None,
            locale,
        );
        assert!(!output.status.success());
        let record: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("one closed record");
        assert_eq!(record["kind"], "closed");
        assert_eq!(record["cleanup"], "not-attached");
        assert!(!output.stderr.is_empty());
        validator.validate(&record).expect("closed record conforms");
    }
}
