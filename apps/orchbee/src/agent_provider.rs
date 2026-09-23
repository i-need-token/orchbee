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
    match name.as_str() {
        "pi" => Some("pi"),
        "claude" | "claude-code" => Some("claude-code"),
        "codex" => Some("codex"),
        "gemini" => Some("gemini"),
        "cursor" | "cursor-agent" => Some("cursor"),
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
        "cursor" => "cursor-agent",
        "antigravity" => "agy",
        "github-copilot" => "copilot",
        "kiro" => "kiro-cli",
        "qwen-code" => "qwen",
        _ => id,
    })
}
