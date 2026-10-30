//! Managed Agent integration identities. The wire spelling is the canonical provider id of the
//! Agent provider registry; payload ids stay private to the node-local writer.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

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

impl Serialize for IntegrationId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.provider())
    }
}

impl<'de> Deserialize<'de> for IntegrationId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse_provider(&value)
            .ok_or_else(|| de::Error::custom("unknown managed integration provider"))
    }
}
