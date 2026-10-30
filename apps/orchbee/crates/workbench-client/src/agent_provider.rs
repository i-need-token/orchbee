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

use orchbee_agent_observation_protocol::AgentReportProvider;
pub(crate) use orchbee_agent_observation_protocol::native_agent_basename as basename;
pub use orchbee_agent_observation_protocol::normalize_agent_report_label as reported_label;

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
    IntegrationId::parse(id).map(|id| id.as_str())
}

/// Canonical integration ids, in the registry's canonical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrationId {
    ClaudeCode,
    Codex,
    Grok,
    Opencode,
    Kilo,
    Cursor,
    QoderCli,
    AntigravityCli,
    Mastracode,
    Droid,
    Kimi,
    Letta,
    Devin,
    Hermes,
    Pi,
    Omp,
    Gemini,
    GitHubCopilot,
    Cline,
    QwenCode,
}

impl IntegrationId {
    pub const ALL: [Self; 20] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::Grok,
        Self::Opencode,
        Self::Kilo,
        Self::Cursor,
        Self::QoderCli,
        Self::AntigravityCli,
        Self::Mastracode,
        Self::Droid,
        Self::Kimi,
        Self::Letta,
        Self::Devin,
        Self::Hermes,
        Self::Pi,
        Self::Omp,
        Self::Gemini,
        Self::GitHubCopilot,
        Self::Cline,
        Self::QwenCode,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Opencode => "opencode",
            Self::Kilo => "kilo",
            Self::Cursor => "cursor",
            Self::QoderCli => "qodercli",
            Self::AntigravityCli => "antigravity-cli",
            Self::Mastracode => "mastracode",
            Self::Droid => "droid",
            Self::Kimi => "kimi",
            Self::Letta => "letta",
            Self::Devin => "devin",
            Self::Hermes => "hermes",
            Self::Pi => "pi",
            Self::Omp => "omp",
            Self::Gemini => "gemini",
            Self::GitHubCopilot => "github-copilot",
            Self::Cline => "cline",
            Self::QwenCode => "qwen-code",
        }
    }

    /// Public CLI providers share the interactive registry; payload IDs remain private.
    pub const fn provider(self) -> &'static str {
        match self {
            Self::AntigravityCli => "antigravity",
            other => other.as_str(),
        }
    }
    pub fn parse_provider(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.provider() == value)
    }

    /// Exact closed-vocabulary spelling; unknown spellings are usage errors,
    /// never fuzzy matches.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.as_str() == value)
    }
}

#[cfg(test)]
mod tests;
