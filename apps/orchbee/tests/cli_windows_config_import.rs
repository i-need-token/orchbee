//! Real Windows offline import processes; no Site/cohort activation is permitted.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod support;
use serde_json::Value;
use std::{ffi::OsStr, fs};
use support::{Fixture, validate};

const SCHEMA: &str = include_str!("../src/cli/schemas/orchbee.config.import-herdr.json");
fn args<'a>(source: &'a str, target: &'a str, preview: bool) -> Vec<&'a str> {
    let mut args = vec![
        "config",
        "import-herdr",
        "--input-file",
        source,
        "--output-file",
        target,
        "--format=json",
    ];
    if preview {
        args.push("--dry-run");
    }
    args
}
fn document(output: &std::process::Output) -> Value {
    let document = serde_json::from_slice(&output.stdout).unwrap();
    validate(&document, SCHEMA);
    document
}

#[test]
fn import_preview_is_offline_value_free_in_text_and_equal_across_locales() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source.toml");
    let target = fixture.0.join("does-not-exist/candidate.toml");
    let source_text = "[ui]\nwindow_title='private-title'\n[terminal]\nnew_cwd='project'\n[update]\nchannel='preview'\n";
    fs::write(&source, source_text).unwrap();
    let mut last = None;
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = fixture.run_with_env(
            &args(source.to_str().unwrap(), target.to_str().unwrap(), true),
            None,
            None,
            &[
                ("LC_ALL", OsStr::new(locale)),
                ("ORCHBEE_CONFIG_FILE", OsStr::new("relative-invalid")),
                ("ORCHBEE_RESUME_AGENTS_ON_RESTORE", OsStr::new("false")),
            ],
        );
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let doc = document(&output);
        assert_eq!(doc["result"]["created"], false);
        assert!(
            doc["result"]["candidateToml"]
                .as_str()
                .unwrap()
                .contains("private-title")
        );
        if let Some(last) = last {
            assert_eq!(doc, last);
        }
        last = Some(doc);
        let text = fixture.run_with_env(
            &[
                "config",
                "import-herdr",
                "--input-file",
                source.to_str().unwrap(),
                "--output-file",
                target.to_str().unwrap(),
                "--dry-run",
            ],
            None,
            None,
            &[("LC_ALL", OsStr::new(locale))],
        );
        assert_eq!(text.status.code(), Some(0));
        assert!(
            !String::from_utf8(text.stdout)
                .unwrap()
                .contains("private-title")
        );
    }
    assert_eq!(fs::read_to_string(&source).unwrap(), source_text);
    assert!(!target.parent().unwrap().exists());
    assert!(!fixture.0.join("runtime").exists());
    assert!(!fixture.0.join("appdata").exists());
}

#[test]
fn import_creates_one_valid_file_then_refuses_every_overwrite_without_reloading() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source.toml");
    let target = fixture.0.join("candidate.toml");
    let text = "[session]\nresume_agents_on_restore=false\n[ui.toast]\nenabled=false\n[keys]\nnew_tab='prefix+t'\nclose_pane='ctrl+'\n";
    fs::write(&source, text).unwrap();
    let output = fixture.run(&args(
        source.to_str().unwrap(),
        target.to_str().unwrap(),
        false,
    ));
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc = document(&output);
    assert_eq!(doc["result"]["state"], "created");
    assert!(doc["result"].get("candidateToml").is_none());
    let bytes = fs::read(&target).unwrap();
    assert_eq!(
        doc["result"]["candidateBytes"].as_u64().unwrap(),
        bytes.len() as u64
    );
    let admitted = orchbee_local_site_runtime::read_windows_user_settings(|name| {
        (name == "ORCHBEE_CONFIG_FILE").then(|| target.clone().into_os_string())
    })
    .unwrap();
    assert!(!admitted.settings.resume_agents_on_restore);
    assert!(
        admitted
            .settings
            .keys
            .overrides
            .iter()
            .any(|item| item.action == "close_pane" && item.triggers.is_empty())
    );
    let refused = fixture.run(&args(
        source.to_str().unwrap(),
        target.to_str().unwrap(),
        false,
    ));
    assert_eq!(refused.status.code(), Some(1));
    let refused = document(&refused);
    assert_eq!(refused["result"]["error"]["reason"], "already-exists");
    assert_eq!(refused["result"]["created"], false);
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read_to_string(&source).unwrap(), text);
    assert!(!fixture.0.join("runtime").exists());
}

#[test]
fn import_rejects_bad_sources_and_usage_before_creating_any_destination() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source.toml");
    let target = fixture.0.join("candidate.toml");
    for (bytes, reason) in [
        (b"[invalid".to_vec(), "invalid-toml"),
        (vec![0xff], "invalid-utf8"),
        (vec![b' '; 65537], "too-large"),
    ] {
        fs::write(&source, &bytes).unwrap();
        let output = fixture.run(&args(
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            false,
        ));
        assert_eq!(output.status.code(), Some(1));
        let doc = document(&output);
        assert_eq!(doc["result"]["error"]["reason"], reason);
        assert_eq!(doc["result"]["created"], false);
        assert!(!target.exists());
        assert_eq!(fs::read(&source).unwrap(), bytes);
    }
    for prefix in [vec!["--session", "work"], vec!["--machine", "remote"]] {
        let mut invalid = prefix;
        invalid.extend(args(
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            false,
        ));
        let output = fixture.run(&invalid);
        assert_eq!(output.status.code(), Some(2));
        assert!(!target.exists());
    }
    let mut missing_parent = args(source.to_str().unwrap(), target.to_str().unwrap(), false);
    fs::write(&source, "").unwrap();
    let absent = fixture.0.join("missing/candidate.toml");
    missing_parent[5] = absent.to_str().unwrap();
    assert_eq!(fixture.run(&missing_parent).status.code(), Some(1));
    assert!(!absent.parent().unwrap().exists());
    assert!(!fixture.0.join("runtime").exists());
}
