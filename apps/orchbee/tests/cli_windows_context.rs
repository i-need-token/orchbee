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
fn missing_and_invalid_context_fail_before_runtime_without_value_echo() {
    for verb in ["current", "process-info"] {
        for (terminal, worker) in [
            (None, None),
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
    assert_eq!(document["version"], 18);
    assert_eq!(schemas.len(), 53);
    let validator =
        jsonschema::draft202012::new(&schemas["orchbee.api.schema"]).expect("catalog schema");
    validator.validate(&document).expect("valid catalog");
    for schema in schemas.values() {
        jsonschema::draft202012::new(schema).expect("schema compiles");
    }
    let mut stale = document.clone();
    stale["version"] = serde_json::json!(17);
    assert!(!validator.is_valid(&stale));
    let mut missing = document.clone();
    missing["result"]["schemas"]
        .as_object_mut()
        .expect("dictionary")
        .remove("orchbee.panel.current");
    assert!(!validator.is_valid(&missing));
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
