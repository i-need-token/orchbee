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

pub(crate) fn provider(label: &str) -> Option<&'static str> {
    let name = basename(label);
    lookup(&name)
}

fn lookup(name: &str) -> Option<&'static str> {
    match name {
        "pi" => Some("pi"),
        "claude" | "claude-code" => Some("claude-code"),
        "codex" => Some("codex"),
        "gemini" => Some("gemini"),
        "cursor" | "cursor-agent" | "agent" => Some("cursor"),
        "devin" | "devin-cli" | "devin cli" => Some("devin"),
        "agy" | "antigravity" | "antigravity-cli" => Some("antigravity"),
        "cline" | ".cline" => Some("cline"),
        "omp" => Some("omp"),
        "mastracode" | "mastra-code" | "mastra code" => Some("mastracode"),
        "opencode" | "opencode2" | "open-code" => Some("opencode"),
        "copilot" | "github-copilot" | "ghcs" => Some("github-copilot"),
        "kimi" | "kimi-code" | "kimi code" => Some("kimi"),
        "kiro" | "kiro-cli" => Some("kiro"),
        "droid" => Some("droid"),
        "amp" | "amp-local" => Some("amp"),
        "grok" | "grok-build" => Some("grok"),
        "hermes" | "hermes-agent" => Some("hermes"),
        "kilo" | "kilo-code" | "kilo code" => Some("kilo"),
        "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Some("qodercli"),
        "qwen" | "qwen-code" | "qwen code" => Some("qwen-code"),
        "letta" | "letta-code" | "letta code" => Some("letta"),
        "maki" => Some("maki"),
        "muse" | "muse-code" | "muse-cli" => Some("muse"),
        value
            if value
                .strip_prefix("muse-bin-")
                .is_some_and(|v| v.starts_with(|c: char| c.is_ascii_digit())) =>
        {
            Some("muse")
        }
        _ => None,
    }
}

/// Normalize a reported identity without converting custom labels into display text.
/// Report labels and native provider IDs have deliberately different canonical forms.
pub(crate) fn reported_label(label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    let mut name = label.to_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".ps1", ".js"] {
        if let Some(stem) = name.strip_suffix(suffix) {
            name = stem.to_owned();
            break;
        }
    }
    let name = name
        .rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(&name);
    // `agent` is Cursor's native executable, but is an unspecified custom report identity.
    let recognized = (name != "agent").then(|| lookup(name)).flatten();
    Some(match recognized {
        Some("claude-code") => "claude".to_owned(),
        Some("antigravity") => "agy".to_owned(),
        Some("github-copilot") => "copilot".to_owned(),
        Some("qwen-code") => "qwen".to_owned(),
        Some(id) => id.to_owned(),
        None => label.to_owned(),
    })
}

pub(crate) fn basename(path: &str) -> String {
    let value = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    for suffix in [".exe", ".com", ".cmd", ".ps1", ".js", ".mjs", ".cjs", ".py"] {
        if let Some(stem) = value.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    value
}

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
