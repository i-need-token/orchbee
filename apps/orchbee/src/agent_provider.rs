//! Recognized interactive Agent identities and their Windows command names.

pub(crate) const CANONICAL_IDS: &[&str] = &[
    "pi",
    "claude-code",
    "codex",
    "gemini",
    "cursor",
    "devin",
    "antigravity",
    "cline",
    "omp",
    "mastracode",
    "opencode",
    "github-copilot",
    "kimi",
    "kiro",
    "droid",
    "amp",
    "grok",
    "hermes",
    "kilo",
    "qodercli",
    "qwen-code",
    "letta",
    "maki",
    "muse",
];

pub(crate) use orchbee_agent_observation_protocol::{
    native_agent_basename as basename, native_agent_provider as provider,
    normalize_agent_report_label as reported_label,
};

/// Only canonical identities select a command; aliases remain read-only recognition.
pub(crate) fn executable(canonical: &str) -> Option<&'static str> {
    let id = provider(canonical).filter(|id| *id == canonical)?;
    Some(match id {
        "claude-code" => "claude",
        "cursor" => "agent",
        "qodercli" => "qoder",
        "antigravity" => "agy",
        "github-copilot" => "copilot",
        "kiro" => "kiro-cli",
        "qwen-code" => "qwen",
        _ => id,
    })
}

#[cfg(test)]
mod tests {
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
}
