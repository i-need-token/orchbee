//! Actual offline CLI + Git + build processes, with no network or Session dependency.
#![cfg(windows)]
use crate::peer_support as support;
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use support::{Fixture, validate};
const SCHEMA: &str = include_str!("../src/cli/schemas/orchbee.plugin.provision.json");
struct Source(PathBuf);
impl Source {
    fn new(fixture: &Fixture, build: &str) -> Self {
        let path = fixture.0.join("repository");
        fs::create_dir(&path).unwrap();
        let source = Self(path);
        source.git(&["init", "--initial-branch=main"]);
        fs::write(source.0.join("orchbee-plugin.toml"), format!("id='local.tools'\nname='Local tools'\nversion='1'\n[[build]]\ncommand=['cmd.exe','/d','/c','{build}']\n")).unwrap();
        source.git(&["add", "."]);
        source.git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ]);
        source
    }
    fn git(&self, args: &[&str]) {
        let output = Command::new("git.exe")
            .args(["-c", "core.hooksPath=NUL"])
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn invoke(&self, fixture: &Fixture, args: &[&str]) -> std::process::Output {
        let url = format!("file:///{}/", self.0.to_str().unwrap().replace('\\', "/"));
        let key = format!("url.{url}.insteadOf");
        fixture.run_with_env(
            args,
            None,
            None,
            &[
                ("GIT_CONFIG_COUNT", OsStr::new("1")),
                ("GIT_CONFIG_KEY_0", OsStr::new(&key)),
                (
                    "GIT_CONFIG_VALUE_0",
                    OsStr::new("https://github.com/fixture/tools.git"),
                ),
            ],
        )
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        let actual = self.0.canonicalize().unwrap();
        assert!(
            actual
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("orchbee-cli-names-")
        );
        assert_eq!(actual.file_name(), Some(OsStr::new("repository")));
        clear_readonly(&actual);
        fs::remove_dir_all(actual).unwrap();
    }
}
fn clear_readonly(path: &Path) {
    use std::os::windows::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).unwrap();
    assert_eq!(metadata.file_attributes() & 0x400, 0);
    if metadata.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            clear_readonly(&entry.unwrap().path());
        }
    } else if metadata.permissions().readonly() {
        let text = path.to_str().unwrap();
        let attributes = winsafe::GetFileAttributes(text).unwrap();
        winsafe::SetFileAttributes(text, attributes & !winsafe::co::FILE_ATTRIBUTE::READONLY)
            .unwrap();
    }
}
fn receipt(output: std::process::Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice(&output.stdout).unwrap();
    validate(&value, SCHEMA);
    value
}
#[test]
fn plugin_install_real_cli_fetches_builds_lists_provenance_updates_and_uninstalls_offline() {
    let fixture = Fixture::new();
    let source = Source::new(&fixture, "echo built>artifact.txt");
    let installed = receipt(
        source.invoke(
            &fixture,
            &[
                "plugin",
                "install",
                "fixture/tools",
                "--yes",
                "--ref",
                "main",
                "--format=json",
            ],
        ),
        0,
    );
    assert_eq!(installed["result"]["report"]["changed"], true);
    let root = PathBuf::from(
        installed["result"]["report"]["plugin"]["pluginRoot"]
            .as_str()
            .unwrap(),
    );
    assert!(
        fs::read_to_string(root.join("artifact.txt"))
            .unwrap()
            .contains("built")
    );
    let source_info = &installed["result"]["report"]["plugin"]["source"];
    assert_eq!(source_info["github"]["repository"], "fixture/tools");
    assert_eq!(source_info["resolvedCommit"].as_str().unwrap().len(), 40);
    let listed: Value =
        serde_json::from_slice(&fixture.run(&["plugin", "list", "--format=json"]).stdout).unwrap();
    validate(
        &listed,
        include_str!("../src/cli/schemas/orchbee.plugin.registry.json"),
    );
    assert_eq!(listed["version"], 1);
    assert_eq!(
        &listed["result"]["catalog"]["plugins"][0]["source"],
        source_info
    );
    // Update fetches the recorded ref again, so a new commit on it replaces the generation.
    fs::write(source.0.join("notes.txt"), "next").unwrap();
    source.git(&["add", "."]);
    source.git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "-m",
        "next",
    ]);
    let update = ["plugin", "update", "local.tools", "--yes", "--format=json"];
    let updated = receipt(source.invoke(&fixture, &update), 0);
    assert_eq!(updated["result"]["operation"], "update");
    assert_eq!(updated["result"]["report"]["changed"], true);
    let updated_source = &updated["result"]["report"]["plugin"]["source"];
    assert_eq!(updated_source["requestedRef"], "main");
    assert_ne!(
        updated_source["resolvedCommit"],
        source_info["resolvedCommit"]
    );
    assert!(!root.exists());
    let root = PathBuf::from(
        updated["result"]["report"]["plugin"]["pluginRoot"]
            .as_str()
            .unwrap(),
    );
    assert!(root.join("notes.txt").exists());
    let current = receipt(source.invoke(&fixture, &update), 0);
    assert_eq!(current["result"]["report"]["changed"], false);
    assert_eq!(
        current["result"]["report"]["plugin"]["source"],
        *updated_source
    );
    receipt(
        fixture.run(&["plugin", "uninstall", "fixture/tools", "--format=json"]),
        0,
    );
    assert!(!root.exists());
    assert!(source.0.exists());
    assert!(!fixture.0.join("runtime").exists());
}
#[test]
fn plugin_install_real_cli_failed_build_keeps_output_and_removes_staged_source() {
    let fixture = Fixture::new();
    let source = Source::new(&fixture, "echo build-error 1>&2 & exit /b 7");
    let failed = receipt(
        source.invoke(
            &fixture,
            &[
                "plugin",
                "install",
                "fixture/tools",
                "--yes",
                "--format=json",
            ],
        ),
        1,
    );
    let report = &failed["result"]["report"];
    assert_eq!(report["stage"], "build");
    assert_eq!(report["changed"], false);
    assert_eq!(report["failure"]["kind"], "command");
    assert_eq!(report["output"]["exitCode"], 7);
    assert!(
        report["output"]["stderr"]
            .as_str()
            .unwrap()
            .contains("build-error")
    );
    assert!(report["cleanupPending"].as_array().unwrap().is_empty());
    let listed: Value =
        serde_json::from_slice(&fixture.run(&["plugin", "list", "--format=json"]).stdout).unwrap();
    assert!(
        listed["result"]["catalog"]["plugins"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn plugin_provision_invalid_grammar_has_no_effects_or_terminal_prompt() {
    let fixture = Fixture::new();
    for args in [
        vec!["plugin", "install", "fixture/tools"],
        vec!["plugin", "install", "fixture/tools/..", "--yes"],
        vec!["plugin", "install", "fixture/tools", "--yes", "--ref=--bad"],
        vec!["plugin", "uninstall", "tools", "--yes"],
        vec!["plugin", "update", "tools"],
        vec!["plugin", "update", "fixture/tools", "--yes"],
        vec!["plugin", "install", "fixture/tools", "--yes", "--timeout=0"],
    ] {
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!fixture.0.join("appdata").exists());
        assert!(!fixture.0.join("runtime").exists());
    }
    for language in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let output = fixture.run_with_env(
            &["plugin", "install", "--help"],
            None,
            None,
            &[("LC_ALL", OsStr::new(language))],
        );
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("--yes") && text.contains("--ref") && text.contains("600000"));
    }
    let linked = fixture.0.join("linked");
    fs::create_dir(&linked).unwrap();
    fs::write(
        linked.join("orchbee-plugin.toml"),
        "id='local.tools'
name='Local tools'
version='1'
",
    )
    .unwrap();
    assert!(
        fixture
            .run(&["plugin", "link", linked.to_str().unwrap()])
            .status
            .success()
    );
    let refused = receipt(
        fixture.run(&["plugin", "update", "local.tools", "--yes", "--format=json"]),
        1,
    );
    assert_eq!(
        refused["result"]["report"]["failure"]["kind"],
        "source-unrecorded"
    );
    assert_eq!(refused["result"]["report"]["changed"], false);
}
