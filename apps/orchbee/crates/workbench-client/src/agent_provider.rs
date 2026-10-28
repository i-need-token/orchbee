//! Recognized interactive Agent identities and their canonical command names.

pub const CANONICAL_IDS: &[&str] = &[
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

pub(crate) use orchbee_agent_observation_protocol::native_agent_basename as basename;
pub use orchbee_agent_observation_protocol::{
    native_agent_provider as provider, normalize_agent_report_label as reported_label,
};

/// Only canonical identities select a command; aliases remain read-only recognition.
pub fn executable(canonical: &str) -> Option<&'static str> {
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

/// A generic command name is not enough evidence to advertise a provider as installed.
pub(crate) fn detection_executable(canonical: &str) -> Option<&'static str> {
    executable(canonical).filter(|command| *command != "agent")
}

/// The `agent integration --provider` identity that manages a canonical
/// interactive identity, when a managed integration exists for it.
pub fn integration_id(canonical: &str) -> Option<&'static str> {
    let id = match canonical {
        "antigravity" => "antigravity-cli",
        other => other,
    };
    crate::agent_integrations::IntegrationId::parse(id).map(|id| id.as_str())
}

#[cfg(test)]
mod tests;
