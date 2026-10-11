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
mod tests;
