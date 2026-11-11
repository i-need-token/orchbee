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
    "atomcode",
    "codebuddy",
    "command-code",
    "deveco",
    "mimo",
    "reasonix",
    "snow",
    "minimax-code",
    "mothx",
    "kirocrew",
    "qwenpaw",
    "jcode",
    "openclaw",
    "penguin",
    "prime-agent",
];

pub use orchbee_agent_observation_protocol::normalize_agent_report_label as reported_label;
use orchbee_agent_observation_protocol::{AgentAliasClass, AgentReportProvider};

/// Exact canonical provider ID; aliases, report labels, and process names stay absent.
pub fn provider(canonical: &str) -> Option<&'static str> {
    AgentReportProvider::from_native_provider_id(canonical)
        .map(AgentReportProvider::native_provider_id)
}

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
        "minimax-code" => "mcode",
        _ => id,
    })
}

/// The fixed arguments that open a provider's interactive session when its bare command does
/// not; caller arguments follow them.
pub fn launch_arguments(canonical: &str) -> &'static [&'static str] {
    match provider(canonical) {
        Some("kirocrew" | "openclaw" | "penguin") => &["chat"],
        Some("qwenpaw") => &["tui"],
        _ => &[],
    }
}

/// A command that the registry classes as generic, such as `agent` or `snow`, is not enough
/// evidence to advertise a provider as installed.
pub(crate) fn detection_executable(canonical: &str) -> Option<&'static str> {
    let command = executable(canonical)?;
    let generic = AgentReportProvider::ALL.iter().any(|provider| {
        provider
            .registry_entry()
            .aliases
            .iter()
            .any(|alias| alias.name == command && alias.class == AgentAliasClass::Generic)
    });
    (!generic).then_some(command)
}

/// The `agent integration --provider` identity that manages a canonical
/// interactive identity, when a managed integration exists for it.
pub fn integration_id(canonical: &str) -> Option<&'static str> {
    let id = match canonical {
        "antigravity" => "antigravity-cli",
        other => other,
    };
    IntegrationId::parse(id).map(|id| id.as_str())
}

pub use orchbee_agent_integration::IntegrationId;

#[cfg(test)]
mod tests;
