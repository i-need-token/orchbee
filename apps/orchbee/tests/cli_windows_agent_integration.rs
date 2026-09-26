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

#[test]
fn cursor_json_hooks_preserve_user_config_and_execute_on_windows() {
    check_json_hooks("cursor");
}

#[test]
fn qoder_json_hooks_preserve_user_config_and_execute_on_windows() {
    check_json_hooks("qodercli");
}

fn check_json_hooks(provider: &str) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use std::os::windows::process::CommandExt;
    let (executable, variable, event, config_relative) = match provider {
        "cursor" => (
            "agent",
            "CURSOR_CONFIG_DIR",
            "sessionStart",
            ".cursor/hooks.json",
        ),
        "qodercli" => (
            "qoder",
            "QODER_CONFIG_DIR",
            "SessionStart",
            ".qoder/settings.json",
        ),
        _ => panic!("unexpected test provider"),
    };
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join(format!("bin/{executable}.cmd")),
        b"@exit /b 99\r\n",
    )
    .unwrap();
    let target = fixture.home().join(config_relative);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let user = if provider == "cursor" {
        r#"{"number":123456789012345678901234567890,"theme":"dark","hooks":{"stop":[{"command":"echo user"}],"sessionStart":[{"command":"echo user-start"}]}}"#
    } else {
        r#"{"version":"user-format","number":123456789012345678901234567890,"theme":"dark","hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo user"}]}],"SessionStart":[{"matcher":"resume","hooks":[{"type":"command","command":"echo user-start"}]}]}}"#
    };
    fs::write(&target, user).unwrap();
    let acl = |protect| {
        let script = if protect {
            "$acl=Get-Acl -LiteralPath $env:ORCHBEE_TEST_CONFIG; $acl.SetAccessRuleProtection($true,$true); Set-Acl -LiteralPath $env:ORCHBEE_TEST_CONFIG -AclObject $acl; (Get-Acl -LiteralPath $env:ORCHBEE_TEST_CONFIG).Sddl"
        } else {
            "(Get-Acl -LiteralPath $env:ORCHBEE_TEST_CONFIG).Sddl"
        };
        let result = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("ORCHBEE_TEST_CONFIG", &target)
            .env_remove("PSModulePath")
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().to_owned()
    };
    let original_acl = acl(true);
    let stream = PathBuf::from(format!("{}:orchbee-test", target.display()));
    fs::write(&stream, b"USER_ALTERNATE_STREAM").unwrap();

    assert_eq!(
        fixture.provider_document("explain", provider, 0)["result"]["state"],
        "not-installed"
    );
    fixture.provider_document("enable", provider, 0);
    let installed = fs::read_to_string(&target).unwrap();
    assert_eq!(acl(false), original_acl);
    assert_eq!(fs::read(&stream).unwrap(), b"USER_ALTERNATE_STREAM");

    assert!(installed.contains("123456789012345678901234567890"));
    assert!(installed.contains("echo user-start") && installed.contains("echo user"));
    fixture.provider_document("enable", provider, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), installed);
    exercise_installed_json_hooks(provider, &target);
    let doc: Value = serde_json::from_str(&installed).unwrap();
    let entry = if provider == "cursor" {
        &doc["hooks"][event][1]
    } else {
        &doc["hooks"][event][1]["hooks"][0]
    };
    let command = entry["command"].as_str().unwrap();
    let encoded = command.rsplit(' ').next().unwrap();
    let bytes = STANDARD.decode(encoded).unwrap();
    let decoded = String::from_utf16(
        &bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for (version, expected) in [("0.4.2", "update-required"), ("9.0.0", "stale-side")] {
        let old_root = decoded.lines().nth(1).unwrap().split('\'').nth(1).unwrap();
        let current_root =
            PathBuf::from(String::from_utf8(STANDARD.decode(old_root).unwrap()).unwrap());
        let new_root = STANDARD.encode(current_root.with_file_name(version).to_str().unwrap());
        let altered = decoded.replace(old_root, &new_root);
        assert_ne!(altered, decoded);
        let encoded_new = STANDARD.encode(
            altered
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let modified = installed.replace(encoded, &encoded_new);
        fs::write(&target, &modified).unwrap();
        assert_eq!(
            fixture.provider_document("explain", provider, 0)["result"]["state"],
            expected
        );
        if version == "0.4.2" {
            fixture.provider_document("enable", provider, 0);
            assert_eq!(fs::read_to_string(&target).unwrap(), installed);
        } else {
            fixture.provider_document("enable", provider, 3);
            fixture.provider_document("disable", provider, 3);
            assert_eq!(fs::read_to_string(&target).unwrap(), modified);
        }
    }
    fs::write(
        &target,
        installed.replace("\"timeout\":10", "\"timeout\":99"),
    )
    .unwrap();
    fixture.provider_document("enable", provider, 3);
    fixture.provider_document("disable", provider, 3);
    fs::write(&target, &installed).unwrap();
    fixture.provider_document("disable", provider, 0);
    let disabled = fs::read_to_string(&target).unwrap();
    assert_eq!(acl(false), original_acl);
    assert_eq!(fs::read(&stream).unwrap(), b"USER_ALTERNATE_STREAM");

    assert!(
        disabled.contains("123456789012345678901234567890") && disabled.contains("echo user-start")
    );
    assert!(!disabled.contains("EncodedCommand"));
    let configured = fixture.0.join("用户 & independent cursor");
    let output = fixture
        .command("enable", provider, &fixture.config_home(), &[])
        .env(variable, &configured)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(configured.join(target.file_name().unwrap()).is_file());
    assert_eq!(fs::read_to_string(&target).unwrap(), disabled);
    let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args([
            "agent",
            "doctor",
            "--provider",
            provider,
            "--format",
            "json",
        ])
        .env("USERPROFILE", fixture.home())
        .env(variable, &configured)
        .output()
        .unwrap();
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stdout)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&doctor.stdout).unwrap()["result"]["providers"][0]["hook"]
            ["state"],
        "present"
    );
    let invalid = fixture
        .command("enable", provider, &fixture.config_home(), &[])
        .env(variable, "relative")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert_eq!(fs::read_to_string(&target).unwrap(), disabled);
    let output = fixture
        .command("disable", provider, &fixture.config_home(), &[])
        .env(variable, &configured)
        .output()
        .unwrap();
    assert!(output.status.success());
}

fn exercise_installed_json_hooks(provider: &str, target: &Path) {
    use std::os::windows::process::CommandExt;
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|path| path.join("node.exe"))
        .find(|path| path.is_file())
        .unwrap();
    for shell in ["powershell.exe", "pwsh.exe", "cmd.exe"] {
        let output = Command::new(&node)
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/windows-integration-exec-hook.mjs"),
            )
            .args([provider, target.parent().unwrap().to_str().unwrap(), shell])
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["events"],
            if provider == "mastracode" { 10 } else { 1 }
        );
    }
}

#[test]
fn antigravity_named_hooks_preserve_foreign_blocks_and_execute_on_windows() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("bin/agy.cmd"), b"@exit /b 99\r\n").unwrap();
    let target = fixture.home().join(".gemini/config/hooks.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let original = r#"{"user":{"enabled":false,"future":123456789012345678901234567890,"PreInvocation":[{"command":"echo user"}]}}"#;
    fs::write(&target, original).unwrap();
    let id = "antigravity-cli";
    assert_eq!(
        fixture.provider_document("explain", id, 0)["result"]["state"],
        "not-installed"
    );
    fixture.provider_document("enable", id, 0);
    let installed = fs::read_to_string(&target).unwrap();
    assert!(installed.contains("123456789012345678901234567890"));
    fixture.provider_document("enable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), installed);
    exercise_installed_json_hooks(id, &target);
    let disabled = installed.replace("\"enabled\":true", "\"enabled\":false");
    assert_ne!(disabled, installed);
    fs::write(&target, &disabled).unwrap();
    assert_eq!(
        fixture.provider_document("explain", id, 0)["result"]["state"],
        "disabled"
    );
    fixture.provider_document("disable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), disabled);
    let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args(["agent", "doctor", "--provider", id, "--format", "json"])
        .env("USERPROFILE", fixture.home())
        .output()
        .unwrap();
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stdout)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&doctor.stdout).unwrap()["result"]["providers"][0]["hook"]
            ["state"],
        "disabled"
    );
    fixture.provider_document("enable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), installed);
    let changed = installed.replace("\"timeout\":10", "\"timeout\":99");
    fs::write(&target, &changed).unwrap();
    fixture.provider_document("enable", id, 3);
    fixture.provider_document("disable", id, 3);
    assert_eq!(fs::read_to_string(&target).unwrap(), changed);
    fs::write(&target, &installed).unwrap();
    fixture.provider_document("disable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
}

#[test]
fn opencode_migrates_only_owned_undiscovered_entries_and_resolves_xdg() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("bin/opencode.cmd"), "@exit /b 0\r\n").unwrap();
    let inspect = |verb| fixture.provider_document(verb, "opencode", 0);
    inspect("enable");
    let current = fixture
        .home()
        .join(".config/opencode/plugins/orchbee-agent-observation.js");
    let obsolete = current.with_extension("mjs");
    let bytes = fs::read(&current).unwrap();
    fs::rename(&current, &obsolete).unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "update-required");
    let plan = inspect("explain").to_string();
    assert!(
        plan.contains("guarded-write") && plan.contains("guarded-remove"),
        "{plan}"
    );
    inspect("enable");
    assert_eq!(fs::read(&current).unwrap(), bytes);
    assert!(!obsolete.exists());
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    fs::copy(&current, &obsolete).unwrap();
    let plan = inspect("explain").to_string();
    assert!(
        !plan.contains("guarded-write") && plan.contains("guarded-remove"),
        "{plan}"
    );
    inspect("enable");
    assert!(!obsolete.exists());
    for (old, expected) in [
        ("// foreign user module".to_string(), "conflict"),
        (
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("/0.4.11/", "/9.0.0/"),
            "stale-side",
        ),
    ] {
        fs::write(&obsolete, &old).unwrap();
        assert_eq!(inspect("explain")["result"]["state"], expected);
        fixture.provider_document("enable", "opencode", 3);
        fixture.provider_document("disable", "opencode", 3);
        assert_eq!(fs::read(&current).unwrap(), bytes);
        assert_eq!(fs::read_to_string(&obsolete).unwrap(), old);
    }
    fs::write(
        &obsolete,
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace("/0.4.11/", "/0.4.2/"),
    )
    .unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "update-required");
    inspect("enable");
    assert!(!obsolete.exists());
    fs::write(&obsolete, vec![b'x'; 16 * 1024 + 1]).unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "unknown");
    fixture.provider_document("enable", "opencode", 9);
    assert_eq!(fs::read(&current).unwrap(), bytes);
    fs::write(&obsolete, &bytes).unwrap();
    inspect("disable");
    assert!(!current.exists() && !obsolete.exists());

    let xdg = fixture.0.join("独立 XDG config");
    let output = fixture
        .command("enable", "opencode", &fixture.config_home(), &[])
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let installed = xdg.join("opencode/plugins/orchbee-agent-observation.js");
    assert!(installed.is_file());
    assert!(!current.exists());
    let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .args([
            "agent",
            "doctor",
            "--provider",
            "opencode",
            "--format",
            "json",
        ])
        .env("USERPROFILE", fixture.home())
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("ORCHBEE_AGENT_OBSERVATION_SOCKET_V3")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let document: Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert_eq!(
        document["result"]["providers"][0]["hook"]["state"], "present",
        "{document}"
    );
    let output = fixture
        .command("enable", "opencode", &fixture.config_home(), &[])
        .env("XDG_CONFIG_HOME", "relative-config")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
    assert!(!current.exists());
    assert_eq!(fs::read(&installed).unwrap(), bytes);
    let output = fixture
        .command("disable", "opencode", &fixture.config_home(), &[])
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!installed.exists());
}

#[test]
fn windows_extensions_install_upgrade_and_execute_literal_env_hooks() {
    use sha2::Digest;
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("node.exe"))
        .find(|p| p.is_file())
        .unwrap();
    for (provider, executable, relative, variable, configured_relative, manifest) in [
        (
            "gemini",
            "gemini",
            ".gemini/extensions/orchbee-agent-observation",
            "GEMINI_CLI_HOME",
            ".gemini/extensions/orchbee-agent-observation",
            "gemini-extension.json",
        ),
        (
            "qwen-code",
            "qwen",
            ".qwen/extensions/orchbee-agent-observation",
            "QWEN_HOME",
            "extensions/orchbee-agent-observation",
            "qwen-extension.json",
        ),
    ] {
        fs::write(
            fixture.0.join(format!("bin/{executable}.cmd")),
            "@exit /b 99\r\n",
        )
        .unwrap();
        let target = fixture.home().join(relative);
        let inspect = |verb| fixture.provider_document(verb, provider, 0);
        assert_eq!(
            inspect("explain")["result"]["managementMode"],
            "managed-local"
        );
        inspect("enable");
        assert_eq!(
            inspect("list")["result"]["integrations"][0]["state"],
            "enabled"
        );
        let manifest_path = target.join(manifest);
        let marker_path = target.join(".orchbee-agent-observation.json");
        let original_manifest = fs::read(&manifest_path).unwrap();
        let original_marker = fs::read(&marker_path).unwrap();
        let set_version = |version: Value, contract| {
            let mut document: Value = serde_json::from_slice(&original_manifest).unwrap();
            document["version"] = version;
            let bytes = document.to_string();
            fs::write(&manifest_path, &bytes).unwrap();
            let mut marker: Value = serde_json::from_slice(&original_marker).unwrap();
            marker["contractVersion"] = Value::from(contract);
            let hash = format!("{:x}", sha2::Sha256::digest(bytes.as_bytes()));
            let entry = marker["files"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|v| v["path"] == manifest)
                .unwrap();
            entry["sha256"] = Value::from(hash);
            fs::write(&marker_path, marker.to_string()).unwrap();
        };
        set_version(Value::from("0.4.2"), 3);
        assert_eq!(inspect("explain")["result"]["state"], "update-required");
        inspect("enable");
        assert_eq!(fs::read(&manifest_path).unwrap(), original_manifest);
        set_version(Value::from("9.0.0"), 1);
        assert_eq!(inspect("explain")["result"]["state"], "stale-side");
        fixture.provider_document("enable", provider, 3);
        fixture.provider_document("disable", provider, 3);
        set_version(Value::Null, 3);
        assert_eq!(inspect("explain")["result"]["state"], "unknown");
        fixture.provider_document("enable", provider, 9);
        fs::write(&manifest_path, &original_manifest).unwrap();
        fs::write(&marker_path, &original_marker).unwrap();
        for shell in ["powershell.exe", "pwsh.exe", "cmd.exe"] {
            let output = Command::new(&node)
                .arg(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/windows-integration-exec-hook.mjs"),
                )
                .args([provider])
                .arg(&target)
                .arg(shell)
                .env("USERPROFILE", fixture.home())
                .stdin(Stdio::null())
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{provider}/{shell}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        fs::write(target.join("user-file"), "retain").unwrap();
        fixture.provider_document("disable", provider, 3);
        assert_eq!(
            fs::read_to_string(target.join("user-file")).unwrap(),
            "retain"
        );
        fs::remove_file(target.join("user-file")).unwrap();
        inspect("disable");
        assert!(!target.exists());
        let configured = fixture.0.join(format!("{provider} config & 数据"));
        let output = fixture
            .command("enable", provider, &fixture.config_home(), &[])
            .env(variable, &configured)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            configured
                .join(configured_relative)
                .join(manifest)
                .is_file()
        );
        assert!(!target.exists());
        let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args([
                "agent",
                "doctor",
                "--provider",
                provider,
                "--format",
                "json",
            ])
            .env("USERPROFILE", fixture.home())
            .env(variable, &configured)
            .env_remove("ORCHBEE_AGENT_OBSERVATION_SOCKET_V3")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let document: Value = serde_json::from_slice(&doctor.stdout).unwrap();
        assert_eq!(
            document["result"]["providers"][0]["hook"]["state"], "present",
            "{document}"
        );
        let output = fixture
            .command("enable", provider, &fixture.config_home(), &[])
            .env(variable, "relative-invalid")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(9));
        assert!(!target.exists());
        let output = fixture
            .command("disable", provider, &fixture.config_home(), &[])
            .env(variable, &configured)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!configured.join(configured_relative).exists());
    }
    assert!(fixture.calls().is_empty());
}

#[test]
fn kilo_uses_explicit_config_before_xdg_without_invalid_fallback() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("bin/kilo.cmd"), "@exit /b 99\r\n").unwrap();
    let xdg = fixture.0.join("Kilo XDG");
    let explicit = fixture.0.join("Kilo explicit");
    for use_explicit in [false, true] {
        let configure = |command: &mut Command| {
            command.env("XDG_CONFIG_HOME", &xdg);
            if use_explicit {
                command.env("KILO_CONFIG_DIR", &explicit);
            }
        };
        let target = if use_explicit {
            explicit.clone()
        } else {
            xdg.join("kilo")
        }
        .join("plugin/orchbee-agent-observation.js");
        let mut command = fixture.command("enable", "kilo", &fixture.config_home(), &[]);
        configure(&mut command);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(target.is_file());
        assert!(
            !fixture
                .home()
                .join(".config/kilo/plugin/orchbee-agent-observation.js")
                .exists()
        );
        let bytes = fs::read(&target).unwrap();
        let mut doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"));
        doctor
            .args(["agent", "doctor", "--provider", "kilo", "--format", "json"])
            .env("USERPROFILE", fixture.home())
            .env_remove("KILO_CONFIG_DIR")
            .env_remove("GEMINI_CLI_HOME")
            .env_remove("QWEN_HOME")
            .env_remove("QODER_CONFIG_DIR")
            .env_remove("CURSOR_CONFIG_DIR")
            .env_remove("ORCHBEE_AGENT_OBSERVATION_SOCKET_V3")
            .stdin(Stdio::null());
        configure(&mut doctor);
        let document: Value = serde_json::from_slice(&doctor.output().unwrap().stdout).unwrap();
        assert_eq!(
            document["result"]["providers"][0]["hook"]["state"], "present",
            "{document}"
        );
        let output = fixture
            .command("enable", "kilo", &fixture.config_home(), &[])
            .env("XDG_CONFIG_HOME", &xdg)
            .env("KILO_CONFIG_DIR", "relative-invalid")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(9));
        assert_eq!(fs::read(&target).unwrap(), bytes);
        let mut command = fixture.command("disable", "kilo", &fixture.config_home(), &[]);
        configure(&mut command);
        assert!(command.output().unwrap().status.success());
        assert!(!target.exists());
    }
    let output = fixture
        .command("enable", "kilo", &fixture.config_home(), &[])
        .env("XDG_CONFIG_HOME", "relative-invalid")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
    assert!(fixture.calls().is_empty());
}

#[test]
fn grok_inventory_joins_local_configuration_and_runs_shipped_powershell_hooks() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    fixture.plugin("grok");
    let inspect = |verb| fixture.provider_document(verb, "grok", 0);
    let calls =
        || fs::read_to_string(fixture.config_home().join("grok-calls.jsonl")).unwrap_or_default();
    assert_eq!(inspect("explain")["result"]["state"], "not-installed");
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    let before = calls();
    inspect("enable");
    assert_eq!(calls(), before);
    let config = fixture.config_home().join("config.toml");
    for configuration in [
        "[plugins]\nenabled = []\ndisabled = []\n",
        "plugins = { enabled = ['orchbee-agent-observation'], disabled = ['orchbee-agent-observation'] }\n",
    ] {
        fs::write(&config, configuration).unwrap();
        assert_eq!(inspect("explain")["result"]["state"], "disabled");
        inspect("enable");
        assert_eq!(inspect("explain")["result"]["state"], "enabled");
    }
    for malformed in [
        "[malformed",
        "plugins.enabled = true",
        "plugins.disabled = [42]",
    ] {
        fs::write(&config, malformed).unwrap();
        assert_eq!(inspect("explain")["result"]["state"], "unknown");
        let before = calls();
        fixture.provider_document("enable", "grok", 9);
        assert_eq!(calls(), before);
        assert_eq!(fs::read_to_string(&config).unwrap(), malformed);
    }
    fs::write(
        &config,
        "plugins.enabled = ['orchbee-agent-observation'] # comment\n",
    )
    .unwrap();
    let state_path = fixture.config_home().join("grok-state.json");
    let current = fs::read(&state_path).unwrap();
    for (source, version, expected, code) in [
        (fixture.capsule().join("bundle"), Value::Null, "unknown", 9),
        (
            fixture.0.join("foreign/bundle"),
            Value::from("0.4.11"),
            "conflict",
            3,
        ),
        (
            fixture.capsule().parent().unwrap().join("0.4.2/bundle"),
            Value::from("9.0.0"),
            "stale-side",
            3,
        ),
    ] {
        let mut state: Value = serde_json::from_slice(&current).unwrap();
        state["source"] = serde_json::json!(source);
        state["version"] = version;
        fs::write(&state_path, state.to_string()).unwrap();
        assert_eq!(inspect("explain")["result"]["state"], expected);
        let before = calls();
        fixture.provider_document("enable", "grok", code);
        fixture.provider_document("disable", "grok", code);
        assert_eq!(calls(), before);
    }
    let mut old: Value = serde_json::from_slice(&current).unwrap();
    old["version"] = "0.4.2".into();
    fs::write(&state_path, old.to_string()).unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "update-required");
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    for shell in ["powershell.exe", "pwsh.exe"] {
        let output = Command::new("node.exe")
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/windows-integration-exec-hook.mjs"),
            )
            .arg("grok")
            .arg(fixture.capsule().join("bundle"))
            .arg(shell)
            .env("USERPROFILE", fixture.home())
            .stdin(Stdio::null())
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["events"], 4);
        assert_eq!(result["silent"], true);
    }
    inspect("disable");
    assert_eq!(inspect("explain")["result"]["state"], "not-installed");
    let before = calls();
    inspect("disable");
    assert_eq!(calls(), before);
}

#[test]
fn windows_codex_inspects_complete_toml_without_rewriting_unrelated_settings() {
    let fixture = Fixture::new();
    fixture.document("enable", 0);
    let path = fixture.config_home().join("config.toml");
    let source = serde_json::to_string(&fixture.capsule()).unwrap();
    let configurations = [
        format!(
            "[marketplaces.'orchbee-local']\nsource_type = 'local' # comment\nsource = {source}\n[plugins.'orchbee-agent-observation@orchbee-local']\nenabled = true # comment\n"
        ),
        format!(
            "marketplaces.orchbee-local = {{ source_type = 'local', source = {source} }}\nplugins.'orchbee-agent-observation@orchbee-local'.enabled = true\n"
        ),
    ];
    for configuration in configurations {
        fs::write(&path, &configuration).unwrap();
        assert_eq!(fixture.document("explain", 0)["result"]["state"], "enabled");
        let before = fixture.calls();
        fixture.document("enable", 0);
        assert_eq!(fixture.calls(), before);
        assert_eq!(fs::read_to_string(&path).unwrap(), configuration);
    }
    for bytes in [
        b"[unrelated-malformed".as_slice(),
        b"unrelated = 1\nunrelated = 2",
        &[0xff, 0xfe],
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(fixture.document("explain", 0)["result"]["state"], "unknown");
        let before = fixture.calls();
        fixture.document("enable", 9);
        fixture.document("disable", 9);
        assert_eq!(fixture.calls(), before);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "orchbee-integration-cli-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        for path in ["用户 & ' $ ` home/.codex", "explicit Codex home", "bin"] {
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
            root.join("用户 & ' $ ` home/.codex/config.toml"),
            b"user_default = 'must remain untouched'\n",
        )
        .unwrap();
        Self(root)
    }
    fn home(&self) -> PathBuf {
        self.0.join("用户 & ' $ ` home")
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
            .join(".orchbee/agent-observation-adapters/0.4.11")
    }
    fn run(&self, verb: &str, provider: &str, config_home: &Path) -> Output {
        self.run_options(verb, provider, config_home, &[])
    }
    fn run_options(
        &self,
        verb: &str,
        provider: &str,
        config_home: &Path,
        extra: &[&str],
    ) -> Output {
        self.command(verb, provider, config_home, extra)
            .output()
            .unwrap()
    }
    fn command(&self, verb: &str, provider: &str, config_home: &Path, extra: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_orchbee"));
        command
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
            .args(extra)
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("KILO_CONFIG_DIR")
            .env_remove("GEMINI_CLI_HOME")
            .env_remove("QWEN_HOME")
            .env_remove("QODER_CONFIG_DIR")
            .env_remove("CURSOR_CONFIG_DIR")
            .env("USERPROFILE", self.home())
            .env("HOME", self.0.join("wrong-posix-home"))
            .env("CODEX_HOME", config_home)
            .env("GROK_HOME", config_home)
            .env("PATH", self.0.join("bin"))
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .env("LC_ALL", "en-US")
            .current_dir(&self.0)
            .stdin(Stdio::null());
        command
    }
    fn document(&self, verb: &str, code: i32) -> Value {
        self.provider_document(verb, "codex", code)
    }
    fn provider_document(&self, verb: &str, provider: &str, code: i32) -> Value {
        let output = self.run(verb, provider, &self.config_home());
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
    fn plugin(&self, provider: &str) {
        let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|path| path.join("node.exe"))
            .find(|path| path.is_file())
            .unwrap();
        let (relative, prefix) = if provider == "grok" {
            ("tests/fixtures/windows-integration-grok.mjs", "")
        } else {
            ("tests/fixtures/windows-integration-plugin.mjs", provider)
        };
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        fs::write(
            self.0.join(format!("bin/{provider}.cmd")),
            format!(
                "@echo off\r\n\"{}\" \"{}\" {prefix} %*\r\n",
                node.display(),
                script.display()
            ),
        )
        .unwrap();
    }
    fn claude_calls(&self) -> String {
        fs::read_to_string(self.config_home().join("claude-calls.jsonl")).unwrap_or_default()
    }
}

#[test]
fn copilot_current_commands_join_exact_origin_and_deliver_exec_hook_events() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    fixture.plugin("copilot");
    let inspect = |verb| fixture.provider_document(verb, "github-copilot", 0);
    let calls = || {
        fs::read_to_string(fixture.config_home().join("copilot-calls.jsonl")).unwrap_or_default()
    };
    assert_eq!(inspect("explain")["result"]["state"], "not-installed");
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    let original_calls = calls();
    inspect("enable");
    assert_eq!(calls(), original_calls);
    inspect("disable");
    assert_eq!(inspect("explain")["result"]["state"], "disabled");
    inspect("enable");
    let state_path = fixture.config_home().join("copilot-state.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    state["version"] = "0.4.2".into();
    fs::write(&state_path, state.to_string()).unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "update-required");
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    let current = fs::read(&state_path).unwrap();
    for (field, value, expected) in [
        ("sourceTag", Value::from("cached"), "conflict"),
        ("marketplace", Value::from("foreign"), "conflict"),
        ("version", Value::from("9.0.0"), "stale-side"),
        ("version", Value::Null, "unknown"),
        (
            "source",
            serde_json::json!(fixture.0.join("foreign")),
            "conflict",
        ),
    ] {
        let mut state: Value = serde_json::from_slice(&current).unwrap();
        state[field] = value;
        let bytes = state.to_string();
        fs::write(&state_path, &bytes).unwrap();
        assert_eq!(inspect("explain")["result"]["state"], expected);
        let previous_calls = calls();
        let refusal_code = if expected == "unknown" { 9 } else { 3 };
        fixture.provider_document("enable", "github-copilot", refusal_code);
        fixture.provider_document("disable", "github-copilot", refusal_code);
        assert_eq!(calls(), previous_calls);
        assert_eq!(fs::read_to_string(&state_path).unwrap(), bytes);
    }
    let mut crossed: Value = serde_json::from_slice(&current).unwrap();
    crossed["source"] = serde_json::json!(fixture.capsule().parent().unwrap().join("0.4.2"));
    crossed["version"] = "9.0.0".into();
    fs::write(&state_path, crossed.to_string()).unwrap();
    assert_eq!(inspect("explain")["result"]["state"], "stale-side");
    let previous_calls = calls();
    fixture.provider_document("enable", "github-copilot", 3);
    assert_eq!(calls(), previous_calls);
    fs::write(&state_path, current).unwrap();
    let output = Command::new("node.exe")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/windows-integration-exec-hook.mjs"),
        )
        .arg("github-copilot")
        .arg(fixture.capsule().join("bundle"))
        .env("USERPROFILE", fixture.home())
        .stdin(Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let delivery: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(delivery["events"], 4);
    inspect("disable");
}

#[test]
fn claude_plugin_lifecycle_and_shipped_exec_hooks_work_on_windows() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    fixture.plugin("claude");
    let inspect = |verb| fixture.provider_document(verb, "claude-code", 0);
    assert_eq!(
        inspect("explain")["result"]["managementMode"],
        "managed-local"
    );
    assert!(!fixture.capsule().exists());
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    let calls = fixture.claude_calls();
    inspect("enable");
    assert_eq!(fixture.claude_calls(), calls);
    inspect("disable");
    assert_eq!(inspect("explain")["result"]["state"], "disabled");
    let calls = fixture.claude_calls();
    inspect("disable");
    assert_eq!(fixture.claude_calls(), calls);
    inspect("enable");
    assert_eq!(inspect("explain")["result"]["state"], "enabled");
    let output = Command::new("node.exe")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/windows-integration-exec-hook.mjs"),
        )
        .arg("claude-code")
        .arg(fixture.capsule().join("bundle"))
        .env("USERPROFILE", fixture.home())
        .stdin(Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let delivery: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(delivery["events"], 4);
    assert_eq!(delivery["silent"], true);
    assert!(fixture.calls().is_empty());
}

#[test]
fn claude_repairs_owned_old_installations_and_preserves_foreign_or_newer_sources() {
    let fixture = Fixture::new();
    fixture.plugin("claude");
    let state_path = fixture.config_home().join("claude-state.json");
    let state = |source: &Path, version: &str| {
        serde_json::json!({
            "source": source, "installed": true, "enabled": true, "version": version
        })
        .to_string()
    };
    fs::write(
        &state_path,
        state(&fixture.capsule().parent().unwrap().join("0.4.1"), "0.4.1"),
    )
    .unwrap();
    assert_eq!(
        fixture.provider_document("explain", "claude-code", 0)["result"]["state"],
        "update-required"
    );
    fixture.provider_document("enable", "claude-code", 0);
    assert_eq!(
        fixture.provider_document("explain", "claude-code", 0)["result"]["state"],
        "enabled"
    );
    for (source, version, expected) in [
        (fixture.0.join("foreign"), "0.4.11", "conflict"),
        (
            fixture.capsule().parent().unwrap().join("9.0.0"),
            "9.0.0",
            "stale-side",
        ),
        (fixture.capsule(), "9.0.0", "stale-side"),
        (
            fixture.capsule().parent().unwrap().join("0.4.2"),
            "9.0.0",
            "stale-side",
        ),
    ] {
        let original = state(&source, version);
        fs::write(&state_path, &original).unwrap();
        let calls = fixture.claude_calls();
        assert_eq!(
            fixture.provider_document("explain", "claude-code", 0)["result"]["state"],
            expected
        );
        fixture.provider_document("enable", "claude-code", 3);
        fixture.provider_document("disable", "claude-code", 3);
        assert_eq!(fs::read_to_string(&state_path).unwrap(), original);
        assert_eq!(fixture.claude_calls(), calls);
    }
}

#[test]
fn javascript_modules_load_and_deliver_ordered_windows_observations() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|path| path.join("node.exe"))
        .find(|path| path.is_file())
        .unwrap();
    for (provider, relative) in [
        (
            "opencode",
            ".config/opencode/plugins/orchbee-agent-observation.js",
        ),
        ("kilo", ".config/kilo/plugin/orchbee-agent-observation.js"),
        ("pi", ".pi/agent/extensions/orchbee-agent-observation.cjs"),
        ("omp", ".omp/agent/extensions/orchbee-agent-observation.cjs"),
    ] {
        // Availability is a PATH fact; guarded module onboarding must not
        // execute this provider, even when its command would fail.
        fs::write(
            fixture.0.join(format!("bin/{provider}.cmd")),
            b"@exit /b 99\r\n",
        )
        .unwrap();
        let target = fixture.home().join(relative);
        let plan = fixture.provider_document("explain", provider, 0);
        assert_eq!(plan["result"]["managementMode"], "managed-local");
        assert!(!target.exists());
        fixture.provider_document("enable", provider, 0);
        let original = fs::read(&target).unwrap();
        assert_eq!(
            fixture.provider_document("list", provider, 0)["result"]["integrations"][0]["state"],
            "enabled"
        );
        fixture.provider_document("enable", provider, 0);
        assert_eq!(fs::read(&target).unwrap(), original);
        let output = Command::new(&node)
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/windows-integration-module.mjs"),
            )
            .arg(provider)
            .arg(&target)
            .env("USERPROFILE", fixture.home())
            .stdin(Stdio::null())
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            provider,
            String::from_utf8_lossy(&output.stderr)
        );
        let events: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(events["events"], 4);
        assert_eq!(events["provider"], provider);
        let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
            .args([
                "agent",
                "doctor",
                "--provider",
                provider,
                "--format",
                "json",
            ])
            .env("USERPROFILE", fixture.home())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("KILO_CONFIG_DIR")
            .env_remove("GEMINI_CLI_HOME")
            .env_remove("QWEN_HOME")
            .env_remove("QODER_CONFIG_DIR")
            .env_remove("CURSOR_CONFIG_DIR")
            .env(
                "PATH",
                std::env::join_paths([fixture.0.join("bin"), node.parent().unwrap().to_path_buf()])
                    .unwrap(),
            )
            .env_remove("ORCHBEE_AGENT_OBSERVATION_SOCKET_V3")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let inspection: Value = serde_json::from_slice(&doctor.stdout).unwrap();
        assert_eq!(
            inspection["result"]["providers"][0]["hook"]["state"], "present",
            "{inspection}"
        );

        let old = String::from_utf8(original.clone())
            .unwrap()
            .replace("/0.4.11/", "/0.4.2/");
        fs::write(&target, old).unwrap();
        assert_eq!(
            fixture.provider_document("explain", provider, 0)["result"]["state"],
            "update-required"
        );
        fixture.provider_document("enable", provider, 0);
        assert_eq!(fs::read(&target).unwrap(), original);
        let changed = [original.as_slice(), b"\n// User edit: preserve it\n"].concat();
        fs::write(&target, &changed).unwrap();
        assert_eq!(
            fixture.provider_document("explain", provider, 0)["result"]["state"],
            "conflict"
        );
        fixture.provider_document("enable", provider, 3);
        fixture.provider_document("disable", provider, 3);
        assert_eq!(fs::read(&target).unwrap(), changed);
        fs::write(&target, &original).unwrap();
        fixture.provider_document("disable", provider, 0);
        assert!(!target.exists());
        fixture.provider_document("disable", provider, 0);
        assert_eq!(
            fixture.provider_document("list", provider, 0)["result"]["integrations"][0]["state"],
            "not-installed"
        );
    }
    assert!(fixture.calls().is_empty());
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
            "[marketplaces.orchbee-local]\nsource_type = 'local'\nsource = {}\n[plugins.\"orchbee-agent-observation@orchbee-local\"]\nenabled = true\n",
            serde_json::to_string(&path).unwrap()
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
    let output = fixture.run("enable", "grok", &fixture.config_home());
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(fixture.calls(), calls);
}

#[test]
fn outdated_filter_preserves_unknown_and_blocked_inspections_without_effects() {
    for case in ["absent", "older", "newer", "foreign", "unknown"] {
        let fixture = Fixture::new();
        let config = fixture.config_home().join("config.toml");
        let content = match case {
            "absent" => "unrelated = true\n".into(),
            "unknown" => "#".repeat(512 * 1024 + 1),
            _ => {
                let root = match case {
                    "older" => fixture.capsule().parent().unwrap().join("0.3.0"),
                    "newer" => fixture.capsule().parent().unwrap().join("9.0.0"),
                    _ => fixture.0.join("foreign"),
                };
                format!(
                    "[marketplaces.orchbee-local]\nsource_type = 'local'\nsource = {}\n[plugins.\"orchbee-agent-observation@orchbee-local\"]\nenabled = true\n",
                    serde_json::to_string(&root).unwrap()
                )
            }
        };
        fs::write(&config, &content).unwrap();
        let output = fixture.run_options(
            "list",
            "codex",
            &fixture.config_home(),
            &["--outdated-only"],
        );
        assert!(output.status.success(), "{case}: {output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
        let schema: Value = serde_json::from_str(include_str!(
            "../src/cli/schemas/orchbee.agent.integrations.json"
        ))
        .unwrap();
        assert!(
            jsonschema::validator_for(&schema).unwrap().is_valid(&doc),
            "{doc}"
        );
        assert_eq!(doc["version"], 2);
        let result = &doc["result"];
        assert_eq!(result["inspectedCount"], 1);
        assert_eq!(result["outdatedOnly"], true);
        assert_eq!(result["complete"], case != "unknown", "{case}: {doc}");
        assert_eq!(
            result["integrations"].as_array().unwrap().len(),
            usize::from(case == "older")
        );
        assert_eq!(
            result["unresolvedProviderIds"].as_array().unwrap().len(),
            usize::from(case == "unknown")
        );
        assert_eq!(
            result["blockedProviderIds"].as_array().unwrap().len(),
            usize::from(matches!(case, "newer" | "foreign"))
        );
        assert_eq!(fs::read_to_string(config).unwrap(), content);
        assert!(fixture.calls().is_empty());
        assert!(!fixture.capsule().exists());
    }
}

#[test]
fn mastracode_flat_hooks_repair_missing_entries_and_execute_on_windows() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("bin/mastracode.cmd"), b"@exit /b 99\r\n").unwrap();
    let target = fixture.home().join(".mastracode/hooks.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let original = r#"{"future":{"big":123456789012345678901234567890},"SessionStart":[{"command":"echo user","type":"command"}]}"#;
    fs::write(&target, original).unwrap();
    let id = "mastracode";
    assert_eq!(
        fixture.provider_document("explain", id, 0)["result"]["state"],
        "not-installed"
    );
    fixture.provider_document("enable", id, 0);
    let installed = fs::read_to_string(&target).unwrap();
    assert!(installed.contains("123456789012345678901234567890"));
    fixture.provider_document("enable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), installed);
    exercise_installed_json_hooks(id, &target);
    // Keep the large foreign value raw; remove only the selected event using RawValue.
    let mut partial: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(&installed).unwrap();
    partial.remove("AgentEnd");
    fs::write(&target, serde_json::to_vec(&partial).unwrap()).unwrap();
    assert_eq!(
        fixture.provider_document("explain", id, 0)["result"]["state"],
        "update-required"
    );
    let doctor = Command::new(env!("CARGO_BIN_EXE_orchbee"))
        .env("USERPROFILE", fixture.home())
        .args(["agent", "doctor", "--provider", id, "--format", "json"])
        .output()
        .unwrap();
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stdout)
    );
    let result: Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert_eq!(
        result["result"]["providers"][0]["hook"]["state"],
        "update-required"
    );
    fixture.provider_document("enable", id, 0);
    assert_eq!(fs::read_to_string(&target).unwrap(), installed);
    let changed = installed.replace("\"timeout\":10000", "\"timeout\":10");
    assert_ne!(changed, installed);
    fs::write(&target, &changed).unwrap();
    fixture.provider_document("enable", id, 3);
    fixture.provider_document("disable", id, 3);
    assert_eq!(fs::read_to_string(&target).unwrap(), changed);
    fs::write(&target, &installed).unwrap();
    fixture.provider_document("disable", id, 0);
    let restored: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    let original: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(original).unwrap();
    assert_eq!(
        restored.keys().collect::<Vec<_>>(),
        original.keys().collect::<Vec<_>>()
    );
    for (key, value) in original {
        assert_eq!(restored[&key].get(), value.get());
    }
}
