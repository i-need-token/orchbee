use super::*;

#[test]
fn reported_identities_preserve_custom_labels_and_use_report_alias_rules() {
    for (input, expected) in [
        (" CLAUDE-Code.EXE ", "claude"),
        (r"C:\tools\GitHub-Copilot.CMD", "copilot"),
        ("/tools/Qwen Code.bat", "qwen"),
        ("Antigravity-CLI.ps1", "agy"),
        ("cursor-agent.js", "cursor"),
        ("muse-bin-1.2.3.exe", "muse"),
        ("/tools/codex/", "codex"),
        ("codex.exe/", "codex.exe/"),
        ("agent", "agent"),
        ("codex.mjs", "codex.mjs"),
        ("codex.com", "codex.com"),
        (" 团队 Agent ", "团队 Agent"),
        (r" C:\Unknown\Tool.EXE ", r"C:\Unknown\Tool.EXE"),
        ("foo\tbar", "foo\tbar"),
    ] {
        assert_eq!(
            reported_label(input).as_deref(),
            Some(expected),
            "{input:?}"
        );
    }
    let shared_ids: Vec<_> = orchbee_agent_observation_protocol::AgentReportProvider::ALL
        .iter()
        .map(|provider| provider.native_provider_id())
        .collect();
    assert_eq!(shared_ids, CANONICAL_IDS);
    for id in CANONICAL_IDS {
        assert!(reported_label(id).is_some());
    }
    assert_eq!(reported_label(" \t\n"), None);
    assert_eq!(reported_label(""), None);
    assert_eq!(provider("agent"), Some("cursor"));
    assert_eq!(provider("codex.mjs"), Some("codex"));
    assert_eq!(provider("codex.com"), Some("codex"));
    assert_eq!(provider("codex.bat"), None);
}

#[cfg(windows)]
#[test]
fn startup_catalog_uses_canonical_command_mapping_and_reports_only_real_files() {
    use std::{ffi::OsString, fs, path::PathBuf, time::SystemTime};

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            for command in ["codex.cmd", "agent.exe", "amp.ps1", "maki.bat", "muse.com"] {
                let _ = fs::remove_file(self.0.join(command));
            }
            let _ = fs::remove_dir(self.0.join("claude.exe"));
            let _ = fs::remove_dir(&self.0);
        }
    }

    let root = std::env::temp_dir().join(format!(
        "orchbee-agent-start-catalog-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let fixture = Fixture(root);
    for command in ["codex.cmd", "agent.exe", "amp.ps1", "maki.bat", "muse.com"] {
        fs::write(fixture.0.join(command), []).unwrap();
    }
    fs::create_dir(fixture.0.join("claude.exe")).unwrap();
    let path = OsString::from(fixture.0.as_os_str());
    assert!(installed_start_providers_while(Some(&path), || false).is_empty());
    let found = installed_start_providers(Some(&path));
    assert_eq!(found, ["codex", "cursor", "amp", "maki", "muse"]);
}
