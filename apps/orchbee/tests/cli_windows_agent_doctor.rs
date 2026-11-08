//! Offline Windows doctor: real argv, filesystem, locale and schema evidence.
#![cfg(windows)]

use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        // Windows wall-clock samples can be equal in concurrent test threads.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "orchbee-windows-doctor-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        for directory in ["用户 home/.codex", "explicit Codex home", "bin"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        // If doctor accidentally invokes a provider, this script leaves evidence.
        fs::write(
            root.join("bin/codex.cmd"),
            "@echo off\r\necho executed>\"%USERPROFILE%\\executed\"\r\n",
        )
        .unwrap();
        fs::write(
            root.join("bin/node.exe"),
            b"not an executable; observation only",
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, words: &[&str], locale: &str, codex_home: Option<&Path>) -> Output {
        self.run_binary(crate::staged_client::client(), words, locale, codex_home)
    }
    fn run_binary(
        &self,
        binary: &Path,
        words: &[&str],
        locale: &str,
        codex_home: Option<&Path>,
    ) -> Output {
        let mut command = Command::new(binary);
        command
            .args(words)
            .env("USERPROFILE", self.0.join("用户 home"))
            .env("APPDATA", self.0.join("roaming"))
            .env("LOCALAPPDATA", self.0.join("local"))
            .env_remove("ORCHBEE_HOME")
            .env_remove("ORCHBEE_ENVIRONMENT")
            .env_remove("HERMES_HOME")
            .env_remove("HERMES_DATA_DIR_SUFFIX")
            .env("HOME", self.0.join("wrong-posix-home"))
            .env("PATH", self.0.join("bin"))
            .env("LC_ALL", locale)
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .env_remove("CODEX_HOME")
            .env_remove("ORCHBEE_AGENT_OBSERVATION_SOCKET")
            .current_dir(&self.0)
            .stdin(Stdio::null());
        if let Some(home) = codex_home {
            command.env("CODEX_HOME", home);
        }
        command.output().expect("doctor process")
    }
    fn document(&self, codex_home: Option<&Path>) -> Value {
        let before = snapshot(&self.0);
        let output = self.run(
            &["agent", "doctor", "--provider", "codex", "--format", "json"],
            "en-US",
            codex_home,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(
            snapshot(&self.0),
            before,
            "offline diagnosis must not write or execute"
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.agent.doctor.json"))
                .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(
            validator.is_valid(&document),
            "{:?}",
            validator
                .iter_errors(&document)
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(document["version"], 1);
        assert_eq!(
            document["result"]["endpoint"]["socket"],
            serde_json::json!({"kind":"unsupported"})
        );
        document
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let target = fs::canonicalize(&self.0).unwrap();
        assert!(target.starts_with(fs::canonicalize(std::env::temp_dir()).unwrap()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-windows-doctor-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    Vec::new(),
                );
                walk(root, &entry.path(), entries);
            } else {
                entries.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries);
    entries
}
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
/// This test build's Capsule folder in its data root under the fixture home.
fn capsule_folder(fixture: &Fixture) -> PathBuf {
    orchbee_local_runtime::DataRoot::under_home(&fixture.0.join("用户 home"))
        .agent_observation_capsules()
}
fn config(root: &Path, enabled: bool) -> String {
    format!(
        "unrelated_user_setting = 'preserve me'\n[marketplaces.orchbee-local]\nsource_type = 'local'\nsource = '{}'\n[plugins.\"orchbee-agent-observation@orchbee-local\"]\nenabled = {enabled}\n",
        root.display()
    )
}

#[test]
fn native_home_and_script_discovery_are_offline_and_localized() {
    let fixture = Fixture::new();
    let document = fixture.document(None);
    let row = &document["result"]["providers"][0];
    assert_eq!(row["id"], "codex");
    assert_eq!(row["executable"]["state"], "present");
    assert!(
        row["executable"]["detail"]["path"]
            .as_str()
            .unwrap()
            .ends_with("codex.cmd")
    );
    assert_eq!(row["runtime"]["node"]["state"], "present");
    assert_eq!(row["hook"]["reason"], "target-missing");
    assert_eq!(document["result"]["complete"], true);
    let output = fixture.run(&["agent", "doctor", "--provider", "codex"], "zh-CN", None);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("不检查 Windows 端点投递"), "{text}");
    let empty = fixture.document(Some(Path::new("")));
    assert_eq!(
        empty["result"]["providers"][0]["hook"]["reason"],
        "target-missing"
    );
    let missing = fixture.document(Some(&fixture.0.join("missing-explicit-home")));
    assert_eq!(
        missing["result"]["providers"][0]["hook"]["state"],
        "unknown"
    );
    let all = fixture.run(&["agent", "doctor", "--format", "json"], "en-US", None);
    assert!(all.status.success());
    let all: Value = serde_json::from_slice(&all.stdout).unwrap();
    assert_eq!(all["result"]["providers"].as_array().unwrap().len(), 20);
    let invalid = fixture.document(Some(Path::new("relative-codex-home")));
    assert_eq!(
        invalid["result"]["providers"][0]["hook"]["detail"]["configurationRoot"],
        "invalid-CODEX_HOME"
    );
}

#[test]
fn codex_installation_disabled_and_corrupt_capsule_facts_do_not_claim_delivery() {
    let fixture = Fixture::new();
    let assets = fixture.0.join("中文 capsule");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/agent-integration-writer/agent-observation-adapters"),
        &assets,
    );
    let supplied = fixture.run(
        &[
            "agent",
            "doctor",
            "--provider",
            "codex",
            "--asset-root",
            assets.to_str().unwrap(),
            "--format",
            "json",
        ],
        "en-US",
        None,
    );
    assert!(
        supplied.status.success(),
        "{}",
        String::from_utf8_lossy(&supplied.stderr)
    );
    let supplied: Value = serde_json::from_slice(&supplied.stdout).unwrap();
    assert_eq!(
        supplied["result"]["asset"]["roots"][0]["origin"],
        "supplied"
    );
    assert_eq!(
        supplied["result"]["asset"]["roots"][0]["state"],
        "consistent"
    );
    for invalid in [
        r"C:relative",
        r"\rooted-on-current-drive",
        "/posix-only",
        "relative",
    ] {
        let output = fixture.run(
            &[
                "agent",
                "doctor",
                "--asset-root",
                invalid,
                "--format",
                "json",
            ],
            "en-US",
            None,
        );
        assert_eq!(output.status.code(), Some(2), "{invalid}");
    }
    let explicit = fixture.0.join("explicit Codex home");
    // The wrong root contains a contradiction: CODEX_HOME must win.
    fs::write(
        fixture.0.join("用户 home/.codex/config.toml"),
        "[plugins.\"orchbee-agent-observation@orchbee-local\"]\nenabled = true\n",
    )
    .unwrap();
    for (enabled, expected) in [(true, "present"), (false, "disabled")] {
        fs::write(explicit.join("config.toml"), config(&assets, enabled)).unwrap();
        let document = fixture.document(Some(&explicit));
        assert_eq!(
            document["result"]["providers"][0]["hook"]["state"], expected,
            "{document}"
        );
        assert_eq!(
            document["result"]["asset"]["roots"][0]["state"],
            "consistent"
        );
    }
    fs::write(explicit.join("config.toml"), config(&assets, true)).unwrap();
    let manifest_path = assets.join("capsule.json");
    let current_manifest = fs::read(&manifest_path).unwrap();
    for contract_version in [2, 3, 4] {
        let mut manifest: Value = serde_json::from_slice(&current_manifest).unwrap();
        manifest["contractVersion"] = contract_version.into();
        manifest["retired"] = true.into();
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        let document = fixture.document(Some(&explicit));
        assert_eq!(
            document["result"]["asset"]["roots"][0]["state"], "update-required",
            "{document}"
        );
        assert_eq!(
            document["result"]["providers"][0]["hook"]["state"], "update-required",
            "{document}"
        );
        let output = fixture.run(
            &["agent", "doctor", "--provider", "codex"],
            "zh-CN",
            Some(&explicit),
        );
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("orchbee agent integration enable --provider codex")
        );
    }
    fs::write(&manifest_path, current_manifest).unwrap();
    fs::write(assets.join("bundle/scripts/forward-hook.mjs"), "corrupt").unwrap();
    let document = fixture.document(Some(&explicit));
    assert_eq!(
        document["result"]["providers"][0]["hook"]["state"],
        "conflict"
    );
    assert_eq!(
        document["result"]["asset"]["roots"][0]["state"],
        "inconsistent"
    );
}

/// An entry naming the Capsule folder every environment shared before each kept its own reads as
/// stale, and one naming another environment's Capsules as that environment's; the doctor changes
/// neither.
#[test]
fn shared_folder_and_other_environment_entries_are_reported_unchanged() {
    let fixture = Fixture::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/agent-integration-writer/agent-observation-adapters");
    let home = fixture.0.join("用户 home");
    let explicit = fixture.0.join("explicit Codex home");
    let other =
        home.join(".ob-dev-0123456789abcdef/internal/integrations/agent-observation/0.4.17");
    for (root, root_state, reason, hint) in [
        (
            home.join(".orchbee/agent-observation-adapters")
                .join("b".repeat(64)),
            "stale",
            "recorded-root-stale",
            "root-stale",
        ),
        (
            other,
            "other-environment",
            "other-environment",
            "hook-other-environment",
        ),
    ] {
        fs::create_dir_all(root.parent().unwrap()).unwrap();
        copy_tree(&source, &root);
        fs::write(explicit.join("config.toml"), config(&root, true)).unwrap();
        // The document helper also proves that diagnosis writes nothing.
        let document = fixture.document(Some(&explicit));
        let result = &document["result"];
        assert_eq!(
            result["asset"]["roots"][0]["state"], root_state,
            "{document}"
        );
        assert_eq!(
            result["providers"][0]["hook"]["state"], "conflict",
            "{document}"
        );
        assert_eq!(
            result["providers"][0]["hook"]["reason"], reason,
            "{document}"
        );
        let hints: Vec<&str> = result["hints"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["id"].as_str())
            .collect();
        assert!(hints.contains(&hint), "{document}");
        assert!(!hints.contains(&"hook-conflict"), "{document}");
    }
}

#[test]
fn changed_sidecar_bytes_require_reinstall_even_with_refreshed_manifests() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/agent-integration-writer/agent-observation-adapters");
    let manifest: Value =
        serde_json::from_slice(&fs::read(source.join("capsule.json")).unwrap()).unwrap();
    let installed = capsule_folder(&fixture).join("0.4.17");
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    copy_tree(&source, &installed);
    let explicit = fixture.0.join("explicit Codex home");
    fs::write(explicit.join("config.toml"), config(&installed, true)).unwrap();
    let distribution = fixture.0.join("isolated distribution");
    fs::create_dir(&distribution).unwrap();
    let binary = distribution.join(orchbee_local_runtime::CohortArtifact::Client.file_name());
    fs::copy(env!("CARGO_BIN_EXE_orchbee"), &binary).unwrap();
    let current = distribution.join("agent-observation-adapters");
    copy_tree(&source, &current);
    let words = ["agent", "doctor", "--provider", "codex", "--format", "json"];
    let before = snapshot(&installed);
    let initial = fixture.run_binary(&binary, &words, "en-US", Some(&explicit));
    assert!(initial.status.success());
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(
        initial["result"]["providers"][0]["hook"]["state"],
        "present"
    );

    // Runtime and documentation bytes both participate in the same identity.
    // The staging script's verifier supplies an independent digest oracle. The
    // Capsule path comes first so the script's entry guard never runs staging.
    for relative in ["bundle/scripts/forward-hook.mjs", "README.md"] {
        fs::write(
            current.join(relative),
            [fs::read(source.join(relative)).unwrap(), b"\n".to_vec()].concat(),
        )
        .unwrap();
        let output = Command::new("node.exe")
            .args(["--input-type=module", "--eval",
                "import {pathToFileURL} from 'node:url'; const {capsulePayloadDigest}=await import(pathToFileURL(process.argv[2]).href); process.stdout.write(capsulePayloadDigest(process.argv[1]) ?? '');"])
            .arg(&current)
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/stage-local-runtime.mjs"))
            .creation_flags(0x0800_0000)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let digest = String::from_utf8(output.stdout).unwrap();
        assert_eq!(digest.len(), 64);
        let refreshed = serde_json::json!({"contractVersion":1,"payloadSha256":digest});
        fs::write(current.join("capsule.json"), refreshed.to_string()).unwrap();
        let output = fixture.run_binary(&binary, &words, "en-US", Some(&explicit));
        assert!(output.status.success());
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let root = &document["result"]["asset"]["roots"][0];
        assert_eq!(root["state"], "update-required", "{relative}: {document}");
        assert_eq!(root["detail"]["payloadSha256"], manifest["payloadSha256"]);
        assert_eq!(
            root["detail"]["expectedPayloadSha256"],
            refreshed["payloadSha256"]
        );
        assert_eq!(
            document["result"]["providers"][0]["hook"]["state"],
            "update-required"
        );
        let schema: Value =
            serde_json::from_str(include_str!("../src/cli/schemas/orchbee.agent.doctor.json"))
                .unwrap();
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&document)
        );
        let localized = fixture.run_binary(&binary, &words[..4], "zh-CN", Some(&explicit));
        assert!(localized.status.success());
        assert!(
            String::from_utf8(localized.stdout)
                .unwrap()
                .contains("orchbee agent integration enable --provider codex")
        );
        assert_eq!(
            snapshot(&installed),
            before,
            "doctor must retain the installed bytes"
        );
        fs::copy(source.join(relative), current.join(relative)).unwrap();
    }
}
