//! Public Windows onboarding: isolated profile/configuration, a deterministic
//! provider CLI, real batch launch, capsule publication, and schema receipts.
#![cfg(windows)]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-integration-cli-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        for path in ["用户 & home/.codex", "explicit Codex home", "bin"] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("node.exe"))
            .find(|path| path.is_file())
            .unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/windows-integration-provider.mjs");
        fs::write(
            root.join("bin/codex.cmd"),
            format!(
                "@echo off\r\n\"{}\" \"{}\" %*\r\n",
                node.display(),
                script.display()
            ),
        )
        .unwrap();
        fs::write(
            root.join("用户 & home/.codex/config.toml"),
            b"user_default = 'must remain untouched'\n",
        )
        .unwrap();
        Self(root)
    }
    fn home(&self) -> PathBuf {
        self.0.join("用户 & home")
    }
    fn config_home(&self) -> PathBuf {
        self.0.join("explicit Codex home")
    }
    fn assets(&self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../desktop/resources/agent-observation-adapters")
            .canonicalize()
            .unwrap()
    }
    fn capsule(&self) -> PathBuf {
        self.home()
            .join(".orchbee/agent-observation-adapters/0.4.1")
    }
    fn run(&self, verb: &str, provider: &str, config_home: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args([
                "agent",
                "integration",
                verb,
                "--provider",
                provider,
                "--format",
                "json",
                "--adapter-source",
            ])
            .arg(self.assets())
            .env("USERPROFILE", self.home())
            .env("HOME", self.0.join("wrong-posix-home"))
            .env("CODEX_HOME", config_home)
            .env("PATH", self.0.join("bin"))
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .env("LC_ALL", "en-US")
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
    fn document(&self, verb: &str, code: i32) -> Value {
        let output = self.run(verb, "codex", &self.config_home());
        assert_eq!(
            output.status.code(),
            Some(code),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema = match verb {
            "list" => include_str!("../src/cli/schemas/orchbee.agent.integrations.json"),
            "explain" => include_str!("../src/cli/schemas/orchbee.agent.integration.plan.json"),
            _ => include_str!("../src/cli/schemas/orchbee.agent.integration.mutation.json"),
        };
        let validator = jsonschema::validator_for(&serde_json::from_str(schema).unwrap()).unwrap();
        assert!(validator.is_valid(&document), "{document}");
        document
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.config_home().join("calls.jsonl")).unwrap_or_default()
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
                .starts_with("orchbee-integration-cli-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn onboarding_is_idempotent_and_disable_preserves_user_configuration() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.document("list", 0)["result"]["integrations"][0]["state"],
        "not-installed"
    );
    let plan = fixture.document("explain", 0);
    assert_eq!(plan["result"]["managementMode"], "managed-local");
    assert!(!plan["result"]["plan"].as_array().unwrap().is_empty());
    assert!(!fixture.capsule().exists(), "inspection must not install");
    fixture.document("enable", 0);
    assert!(fixture.capsule().join("capsule.json").is_file());
    assert_eq!(
        fixture.document("list", 0)["result"]["integrations"][0]["state"],
        "enabled"
    );
    let calls = fixture.calls();
    fixture.document("enable", 0);
    assert_eq!(fixture.calls(), calls, "enabled entry must not reinstall");
    fixture.document("disable", 0);
    assert_eq!(
        fixture.document("list", 0)["result"]["integrations"][0]["state"],
        "not-installed"
    );
    let calls = fixture.calls();
    fixture.document("disable", 0);
    assert_eq!(
        fixture.calls(),
        calls,
        "absent plugin must not uninstall again"
    );
    assert_eq!(
        fs::read(fixture.home().join(".codex/config.toml")).unwrap(),
        b"user_default = 'must remain untouched'\n"
    );
    assert!(
        fs::read_to_string(fixture.config_home().join("config.toml"))
            .unwrap()
            .contains("preserve me")
    );
}

#[test]
fn foreign_newer_and_invalid_roots_are_refused_without_provider_commands() {
    let fixture = Fixture::new();
    fixture.document("enable", 0);
    let calls = fixture.calls();
    for (path, state) in [
        (fixture.0.join("foreign"), "conflict"),
        (
            fixture.capsule().parent().unwrap().join("9.0.0"),
            "stale-side",
        ),
    ] {
        let content = format!(
            "[marketplaces.orchbee-local]\nsource_type = 'local'\nsource = '{}'\n[plugins.\"orchbee-agent-observation@orchbee-local\"]\nenabled = true\n",
            path.display()
        );
        fs::write(fixture.config_home().join("config.toml"), &content).unwrap();
        assert_eq!(fixture.document("explain", 0)["result"]["state"], state);
        fixture.document("enable", 3);
        fixture.document("disable", 3);
        assert_eq!(
            fs::read_to_string(fixture.config_home().join("config.toml")).unwrap(),
            content
        );
        assert_eq!(fixture.calls(), calls);
    }
    let output = fixture.run("enable", "codex", Path::new("relative-invalid"));
    assert!(!output.status.success());
    assert_eq!(fixture.calls(), calls);
    let output = fixture.run("enable", "claude-code", &fixture.config_home());
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(fixture.calls(), calls);
}
