//! Unit tests for global Orchbee skill path resolution, ownership markers, state folding, and
//! mutation lifecycle.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use super::{
    SkillState, combine_state, combine_unread, inspect, skill_bytes, skill_root, skill_target,
};
use crate::{
    capsule,
    exec_budget::now,
    inspect::ResourceOptions,
    model::{IntegrationId, ManagementMode, State},
    mutation::{Direction, Refusal, Verdict, mutate},
    providers::{self, ProviderContext},
};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "orchbee-skill-test-{:032x}",
            u128::from_le_bytes(nonce)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_capsule_root(base: &Path, stem: &str, version: &str) -> PathBuf {
    base.join(stem)
        .join("internal/integrations/agent-observation")
        .join(version)
}

fn materialize_valid_capsule(root: &Path, adapter_source: &Path) {
    let required = [
        ("bundle/package.json", b"{\"name\":\"test\"}\n".as_slice()),
        ("bundle/scripts/forward-hook.mjs", b"// forward\n"),
        ("bundle/scripts/hook-sanitizers.mjs", b"// sanitizers\n"),
        (
            "bundle/scripts/observation-transport.mjs",
            b"// transport\n",
        ),
        ("bundle/pi/orchbee-agent-observation.cjs", b"// pi cjs\n"),
        ("bundle/pi/orchbee-agent-observation.mjs", b"// pi mjs\n"),
    ];
    for (relative, contents) in required {
        let src = adapter_source.join(relative);
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        fs::write(src, contents).unwrap();
    }
    let digest = capsule::payload_digest(adapter_source).unwrap();
    let manifest = serde_json::json!({
        "contractVersion": 1,
        "payloadSha256": digest,
    });
    fs::write(adapter_source.join("capsule.json"), manifest.to_string()).unwrap();

    for (relative, contents) in required {
        let dst = root.join(relative);
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::write(dst, contents).unwrap();
    }
    fs::write(root.join("capsule.json"), manifest.to_string()).unwrap();
    fs::write(root.join(capsule::OWNER_RECORD), capsule::owner_record()).unwrap();
}

#[test]
fn env_overrides_and_invalid_values_behave_strictly() {
    let temp = TempDir::new();
    let home = temp.path();

    let override_vars = |name: &str| -> Option<OsString> {
        let dir = std::env::temp_dir().join(format!("orchbee-skill-var-{name}"));
        match name {
            "CLAUDE_CONFIG_DIR"
            | "CODEX_HOME"
            | "GEMINI_CLI_HOME"
            | "QWEN_HOME"
            | "XDG_CONFIG_HOME"
            | "CURSOR_CONFIG_DIR"
            | "COPILOT_HOME"
            | "QODER_CONFIG_DIR"
            | "PI_CODING_AGENT_DIR"
            | "KIMI_CODE_HOME"
            | "HERMES_HOME" => Some(OsString::from(dir)),
            _ => None,
        }
    };
    assert_eq!(
        skill_target(IntegrationId::ClaudeCode, home, override_vars),
        Some(
            std::env::temp_dir()
                .join("orchbee-skill-var-CLAUDE_CONFIG_DIR")
                .join("skills/orchbee/SKILL.md")
        )
    );
    assert_eq!(
        skill_target(IntegrationId::Gemini, home, override_vars),
        Some(
            std::env::temp_dir()
                .join("orchbee-skill-var-GEMINI_CLI_HOME")
                .join(".gemini/skills/orchbee/SKILL.md")
        )
    );
    assert_eq!(
        skill_target(IntegrationId::Opencode, home, override_vars),
        Some(
            std::env::temp_dir()
                .join("orchbee-skill-var-XDG_CONFIG_HOME")
                .join("opencode/skills/orchbee/SKILL.md")
        )
    );
    assert_eq!(
        skill_target(IntegrationId::Kilo, home, override_vars),
        Some(
            std::env::temp_dir()
                .join("orchbee-skill-var-XDG_CONFIG_HOME")
                .join("kilo/skills/orchbee/SKILL.md")
        )
    );
    assert_eq!(
        skill_target(IntegrationId::Hermes, home, override_vars),
        Some(
            std::env::temp_dir()
                .join("orchbee-skill-var-HERMES_HOME")
                .join("skills/orchbee/SKILL.md")
        )
    );

    // Relative, unexpanded, or traversal overrides reject instead of falling back.
    assert_eq!(
        skill_target(IntegrationId::ClaudeCode, home, |k| {
            (k == "CLAUDE_CONFIG_DIR").then(|| OsString::from("relative/claude"))
        }),
        None
    );
    assert_eq!(
        skill_target(IntegrationId::Codex, home, |k| {
            (k == "CODEX_HOME").then(|| OsString::from("$HOME/.codex"))
        }),
        None
    );
    assert_eq!(
        skill_target(IntegrationId::Cursor, home, |k| {
            (k == "CURSOR_CONFIG_DIR").then(|| OsString::from("~/.cursor"))
        }),
        None
    );
    assert_eq!(
        skill_target(IntegrationId::Kilo, home, |k| match k {
            "KILO_CONFIG_DIR" => Some(OsString::from("relative-kilo")),
            "XDG_CONFIG_HOME" => Some(std::env::temp_dir().join("valid-xdg").into_os_string()),
            _ => None,
        }),
        None
    );
    // An empty value is unset and takes the default. Whitespace is a nonempty relative value, so it
    // rejects, as the hook-configuration paths do.
    assert_eq!(
        skill_target(IntegrationId::ClaudeCode, home, |k| {
            (k == "CLAUDE_CONFIG_DIR").then(|| OsString::from("   "))
        }),
        None
    );
    assert_eq!(
        skill_target(IntegrationId::ClaudeCode, home, |k| {
            (k == "CLAUDE_CONFIG_DIR").then(OsString::new)
        }),
        Some(home.join(".claude/skills/orchbee/SKILL.md"))
    );
}

#[test]
fn marker_roundtrip_and_tamper_detection() {
    let temp = TempDir::new();
    let current = make_capsule_root(temp.path(), "orchbee-dev", "0.5.0");
    let bytes = skill_bytes(&current).unwrap();
    assert_eq!(skill_root(&bytes).as_deref(), Some(current.as_path()));

    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains("ORCHBEE_TERMINAL_ID"));
    assert!(text.contains("orchbee --skill"));

    // The marker alone decides ownership, so a body edit keeps the file owned by this capsule root
    // and a later template change still recognizes it. Two markers make the file ambiguous.
    let edited = format!("{text}\n# custom note\n");
    assert_eq!(
        skill_root(edited.as_bytes()).as_deref(),
        Some(current.as_path())
    );
    assert_eq!(skill_root(format!("{text}{text}").as_bytes()), None);
}

#[test]
fn inspect_classifies_current_outdated_elsewhere_foreign_and_symlink() {
    let temp = TempDir::new();
    let home = temp.path().join("home");
    let data = temp.path().join("data");
    let current_root = make_capsule_root(&data, "orchbee-dev-a", "0.5.0");
    let outdated_root = make_capsule_root(&data, "orchbee-dev-a", "0.4.0");
    let other_env_root = make_capsule_root(&data, "orchbee", "0.5.0");

    let target = skill_target(IntegrationId::ClaudeCode, &home, |_| None).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let options = ResourceOptions {
        resource_root: &current_root,
    };

    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Missing
    );

    fs::write(&target, skill_bytes(&current_root).unwrap()).unwrap();
    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Current
    );

    fs::write(&target, skill_bytes(&outdated_root).unwrap()).unwrap();
    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Outdated
    );

    let other_owner = other_env_root.join(capsule::OWNER_RECORD);
    fs::create_dir_all(other_owner.parent().unwrap()).unwrap();
    fs::write(&other_owner, capsule::owner_record()).unwrap();
    fs::write(&target, skill_bytes(&other_env_root).unwrap()).unwrap();
    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Elsewhere
    );

    // A body edit keeps the marker: the file stays this environment's, reads Outdated, and enable
    // rewrites it.
    let edited = format!(
        "{}\nuser edit\n",
        String::from_utf8(skill_bytes(&current_root).unwrap()).unwrap()
    );
    fs::write(&target, edited).unwrap();
    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Outdated
    );

    fs::write(&target, b"# user custom skill").unwrap();
    assert_eq!(
        inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
        SkillState::Foreign
    );

    #[cfg(unix)]
    {
        fs::remove_file(&target).unwrap();
        let outside = temp.path().join("outside.md");
        fs::write(&outside, skill_bytes(&current_root).unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &target).unwrap();
        assert_eq!(
            inspect(IntegrationId::ClaudeCode, &home, |_| None, &options),
            SkillState::Foreign
        );
    }
}

#[test]
fn combine_helpers_treat_foreign_and_elsewhere_skills_as_accessories() {
    assert_eq!(
        combine_state(State::Enabled, SkillState::Missing),
        State::UpdateRequired
    );
    assert_eq!(
        combine_state(State::NotInstalled, SkillState::Current),
        State::UpdateRequired
    );
    assert_eq!(
        combine_state(State::Disabled, SkillState::Current),
        State::Disabled
    );
    assert_eq!(
        combine_state(State::Disabled, SkillState::Missing),
        State::Disabled
    );
    assert_eq!(
        combine_state(State::Enabled, SkillState::Current),
        State::Enabled
    );
    assert_eq!(
        combine_state(State::Enabled, SkillState::Outdated),
        State::UpdateRequired
    );

    // A skill this environment does not own neither changes nor blocks the primary row or ownership.
    for primary in [
        State::NotInstalled,
        State::Enabled,
        State::Disabled,
        State::Conflict,
        State::Unknown,
    ] {
        assert_eq!(combine_state(primary, SkillState::Foreign), primary);
        assert_eq!(combine_state(primary, SkillState::Elsewhere), primary);
    }

    assert_eq!(
        combine_unread(State::Unknown, State::Unknown, SkillState::Current),
        (State::Unknown, State::UpdateRequired)
    );
    assert_eq!(
        combine_unread(State::Unknown, State::Unknown, SkillState::Elsewhere),
        (State::Unknown, State::Unknown)
    );
    assert_eq!(
        combine_unread(State::Unknown, State::Unknown, SkillState::Foreign),
        (State::Unknown, State::Unknown)
    );
}

#[test]
fn mutate_installs_repairs_and_cleanly_removes_empty_or_nonempty_skill_dirs() {
    let temp = TempDir::new();
    let home = temp.path().join("home");
    let bin = temp.path().join("bin");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    #[cfg(windows)]
    fs::write(bin.join("pi.cmd"), b"@exit /b 0\r\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let exe = bin.join("pi");
        fs::write(&exe, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let data = temp.path().join("data");
    let capsule_root = make_capsule_root(&data, "orchbee-dev-a", "0.5.0");
    let adapter_source = temp.path().join("adapter-source");
    materialize_valid_capsule(&capsule_root, &adapter_source);

    let slowest = std::cell::Cell::new(None);
    let context = ProviderContext {
        home: &home,
        capsule_root: &capsule_root,
        adapter_source: Some(&adapter_source),
        path_env: Some(bin.as_os_str()),
        deadline: now() + Duration::from_secs(10),
        slowest_command: &slowest,
        cancel: None,
        management_mode: ManagementMode::ManagedLocal,
        variables: |_| None,
    };

    let skill_path = skill_target(IntegrationId::Pi, &home, |_| None).unwrap();
    let orchbee_dir = skill_path.parent().unwrap().to_path_buf();

    // Enable installs both extension and SKILL.md.
    let report = mutate(IntegrationId::Pi, Direction::Enable, &context);
    assert_eq!(report.verdict, Verdict::Completed);
    assert!(skill_path.is_file());
    assert_eq!(
        providers::inspect_provider(IntegrationId::Pi, &context).state,
        State::Enabled
    );

    // Deleting SKILL.md surfaces UpdateRequired and enable repairs it.
    fs::remove_file(&skill_path).unwrap();
    assert_eq!(
        providers::inspect_provider(IntegrationId::Pi, &context).state,
        State::UpdateRequired
    );
    let repair = mutate(IntegrationId::Pi, Direction::Enable, &context);
    assert_eq!(repair.verdict, Verdict::Completed);
    assert!(skill_path.is_file());

    // Disable removes SKILL.md AND the now-empty `orchbee/` directory.
    let disable = mutate(IntegrationId::Pi, Direction::Disable, &context);
    assert_eq!(disable.verdict, Verdict::Completed);
    assert!(!skill_path.exists());
    assert!(!orchbee_dir.exists());

    // Re-enable, add a user sibling file in `orchbee/`, then disable: preserves sibling file.
    assert_eq!(
        mutate(IntegrationId::Pi, Direction::Enable, &context).verdict,
        Verdict::Completed
    );
    let user_note = orchbee_dir.join("notes.txt");
    fs::write(&user_note, b"keep me").unwrap();
    let disable_nonempty = mutate(IntegrationId::Pi, Direction::Disable, &context);
    assert_eq!(disable_nonempty.verdict, Verdict::Completed);
    assert!(!skill_path.exists());
    assert_eq!(fs::read(&user_note).unwrap(), b"keep me");

    // A SKILL.md this environment does not own is an accessory: enable and disable leave it alone,
    // the primary integration still completes, and no attempted step names it.
    fs::write(&skill_path, b"custom user skill").unwrap();
    let enable_foreign = mutate(IntegrationId::Pi, Direction::Enable, &context);
    assert_eq!(enable_foreign.verdict, Verdict::Completed);
    assert!(enable_foreign.attempted.iter().all(|step| !matches!(
        &step.step,
        orchbee_agent_integration::StepKind::Filesystem { path, .. } if path == &skill_path
    )));
    assert_eq!(
        mutate(IntegrationId::Pi, Direction::Disable, &context).verdict,
        Verdict::Completed
    );
    assert_eq!(fs::read(&skill_path).unwrap(), b"custom user skill");

    // Another environment's SKILL.md is skipped by enable and left intact by disable.
    let other_root = make_capsule_root(&data, "orchbee", "0.5.0");
    let other_owner = other_root.join(capsule::OWNER_RECORD);
    fs::create_dir_all(other_owner.parent().unwrap()).unwrap();
    fs::write(&other_owner, capsule::owner_record()).unwrap();
    let other_bytes = skill_bytes(&other_root).unwrap();
    fs::write(&skill_path, &other_bytes).unwrap();
    let enable_elsewhere = mutate(IntegrationId::Pi, Direction::Enable, &context);
    assert_eq!(enable_elsewhere.verdict, Verdict::Completed);
    assert!(enable_elsewhere.attempted.iter().all(|step| !matches!(
        &step.step,
        orchbee_agent_integration::StepKind::Filesystem { path, .. } if path == &skill_path
    )));
    let disable_elsewhere = mutate(IntegrationId::Pi, Direction::Disable, &context);
    assert_eq!(disable_elsewhere.verdict, Verdict::Completed);
    assert_eq!(fs::read(&skill_path).unwrap(), other_bytes);
}

#[test]
fn disable_removes_an_owned_skill_even_when_the_program_is_missing() {
    let temp = TempDir::new();
    let home = temp.path().join("home");
    let bin = temp.path().join("empty-bin");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let capsule_root = make_capsule_root(&temp.path().join("data"), "orchbee-dev-a", "0.5.0");

    let slowest = std::cell::Cell::new(None);
    let context = ProviderContext {
        home: &home,
        capsule_root: &capsule_root,
        adapter_source: None,
        path_env: Some(bin.as_os_str()),
        deadline: now() + Duration::from_secs(10),
        slowest_command: &slowest,
        cancel: None,
        management_mode: ManagementMode::ManagedLocal,
        variables: |_| None,
    };

    // The program is absent, so the primary integration cannot be inspected. The owned skill file
    // needs no program, so disable still removes it along with its now-empty folder.
    let skill_path = skill_target(IntegrationId::ClaudeCode, &home, |_| None).unwrap();
    let orchbee_dir = skill_path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&orchbee_dir).unwrap();
    fs::write(&skill_path, skill_bytes(&capsule_root).unwrap()).unwrap();

    let disable = mutate(IntegrationId::ClaudeCode, Direction::Disable, &context);
    assert_eq!(disable.verdict, Verdict::Refused(Refusal::InspectionFailed));
    assert!(!skill_path.exists());
    assert!(!orchbee_dir.exists());
}

#[test]
fn conformance_skill_payloads_and_paths_match_writer() {
    use base64::Engine as _;

    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/provider-conformance");
    let payloads_raw = fs::read_to_string(fixtures.join("payloads.json")).unwrap();
    let payloads: serde_json::Value = serde_json::from_str(&payloads_raw).unwrap();

    let temp = TempDir::new();
    let capsule_root = make_capsule_root(temp.path(), "orchbee-dev-a", "0.5.0");
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(capsule_root.to_str().unwrap().as_bytes());
    let expected = skill_bytes(&capsule_root).unwrap();

    for (key, token) in [
        ("orchbee.skill.current", "${CAPSULE_ROOT_B64}"),
        ("orchbee.skill.outdated", "${OUTDATED_ROOT_B64}"),
        (
            "orchbee.skill.otherEnvironment",
            "${OTHER_CAPSULE_ROOT_B64}",
        ),
    ] {
        let template = payloads["payloads"][key].as_str().unwrap();
        let expanded = template.replace(token, &encoded);
        assert_eq!(expanded.as_bytes(), expected.as_slice(), "payload {key}");
    }

    for (name, id) in [
        ("claude-code", IntegrationId::ClaudeCode),
        ("codex", IntegrationId::Codex),
        ("gemini", IntegrationId::Gemini),
        ("github-copilot", IntegrationId::GitHubCopilot),
        ("omp", IntegrationId::Omp),
        ("opencode", IntegrationId::Opencode),
        ("pi", IntegrationId::Pi),
        ("qwen-code", IntegrationId::QwenCode),
    ] {
        let expected_rel = skill_target(id, temp.path(), |_| None)
            .unwrap()
            .strip_prefix(temp.path())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let case_raw =
            fs::read_to_string(fixtures.join("cases").join(format!("{name}.cases.json"))).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&case_raw).unwrap();
        for case in doc["cases"].as_array().unwrap() {
            if let Some(entries) = case["given"]["fs"].as_array() {
                for entry in entries {
                    let path = entry["path"].as_str().unwrap();
                    if path.contains("skills/orchbee") {
                        assert_eq!(path, expected_rel, "{}", case["id"]);
                    }
                }
            }
        }
    }
}
