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

#[test]
fn startup_commands_come_only_from_the_canonical_mapping() {
    for id in CANONICAL_IDS {
        assert!(executable(id).is_some(), "missing startup command for {id}");
    }
    assert_eq!(executable("claude-code"), Some("claude"));
    assert_eq!(executable("cursor"), Some("agent"));
    assert_eq!(executable("github-copilot"), Some("copilot"));
    assert_eq!(executable("amp"), Some("amp"));
    assert_eq!(executable("maki"), Some("maki"));
    assert_eq!(executable("muse"), Some("muse"));
    assert_eq!(executable("agent"), None);
    assert_eq!(executable("unknown"), None);
}

#[test]
fn every_managed_integration_is_reachable_from_exactly_one_start_kind() {
    let mut managed: Vec<_> = CANONICAL_IDS
        .iter()
        .filter_map(|id| integration_id(id))
        .collect();
    assert_eq!(integration_id("antigravity"), Some("antigravity-cli"));
    assert_eq!(integration_id("codex"), Some("codex"));
    assert_eq!(integration_id("amp"), None);
    managed.sort_unstable();
    let mut all: Vec<_> = crate::agent_integrations::IntegrationId::ALL
        .iter()
        .map(|id| id.as_str())
        .collect();
    all.sort_unstable();
    assert_eq!(managed, all);
}
