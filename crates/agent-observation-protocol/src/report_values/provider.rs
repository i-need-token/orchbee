//! Known provider names shared by report and process-input adapters. Lookup itself
//! performs no path, case or suffix normalization; each caller owns that policy.

/// Recognized providers; report labels and native canonical IDs are separate projections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentReportProvider {
    Pi,
    Claude,
    Codex,
    Gemini,
    Cursor,
    Devin,
    Antigravity,
    Cline,
    Omp,
    Mastracode,
    OpenCode,
    Copilot,
    Kimi,
    Kiro,
    Droid,
    Amp,
    Grok,
    Hermes,
    Kilo,
    Qoder,
    Qwen,
    Letta,
    Maki,
    Muse,
    AtomCode,
    CodeBuddy,
    CommandCode,
    DevEco,
    Mimo,
    Reasonix,
    Snow,
    MinimaxCode,
    Mothx,
    KiroCrew,
    QwenPaw,
    Jcode,
    OpenClaw,
    Penguin,
    PrimeAgent,
}

impl AgentReportProvider {
    /// Every recognized provider, in the native catalog's stable order.
    pub const ALL: [Self; 39] = [
        Self::Pi,
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Cursor,
        Self::Devin,
        Self::Antigravity,
        Self::Cline,
        Self::Omp,
        Self::Mastracode,
        Self::OpenCode,
        Self::Copilot,
        Self::Kimi,
        Self::Kiro,
        Self::Droid,
        Self::Amp,
        Self::Grok,
        Self::Hermes,
        Self::Kilo,
        Self::Qoder,
        Self::Qwen,
        Self::Letta,
        Self::Maki,
        Self::Muse,
        Self::AtomCode,
        Self::CodeBuddy,
        Self::CommandCode,
        Self::DevEco,
        Self::Mimo,
        Self::Reasonix,
        Self::Snow,
        Self::MinimaxCode,
        Self::Mothx,
        Self::KiroCrew,
        Self::QwenPaw,
        Self::Jcode,
        Self::OpenClaw,
        Self::Penguin,
        Self::PrimeAgent,
    ];

    /// Exact lookup of an already-preprocessed name; unknown/custom names stay absent.
    pub fn lookup(name: &str) -> Option<Self> {
        Some(match name {
            "pi" => Self::Pi,
            "claude" | "claude-code" => Self::Claude,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            "cursor" | "cursor-agent" => Self::Cursor,
            "devin" | "devin-cli" | "devin cli" => Self::Devin,
            "agy" | "antigravity" | "antigravity-cli" => Self::Antigravity,
            "cline" | ".cline" => Self::Cline,
            "omp" => Self::Omp,
            "mastracode" | "mastra-code" | "mastra code" => Self::Mastracode,
            "opencode" | "opencode2" | "open-code" => Self::OpenCode,
            "copilot" | "github-copilot" | "ghcs" => Self::Copilot,
            "kimi" | "kimi-code" | "kimi code" => Self::Kimi,
            "kiro" | "kiro-cli" => Self::Kiro,
            "droid" => Self::Droid,
            "amp" | "amp-local" => Self::Amp,
            "grok" | "grok-build" => Self::Grok,
            "hermes" | "hermes-agent" => Self::Hermes,
            "kilo" | "kilo-code" | "kilo code" => Self::Kilo,
            "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Self::Qoder,
            "qwen" | "qwen-code" | "qwen code" => Self::Qwen,
            "letta" | "letta-code" | "letta code" => Self::Letta,
            "maki" => Self::Maki,
            "muse" | "muse-code" | "muse-cli" => Self::Muse,
            "atomcode" => Self::AtomCode,
            "codebuddy" | "codebuddy-code" | "cbc" => Self::CodeBuddy,
            "command-code" | "commandcode" | "cmdc" => Self::CommandCode,
            "deveco" | "deveco-code" => Self::DevEco,
            "mimo" | "mimocode" | "mimo-code" => Self::Mimo,
            "reasonix" => Self::Reasonix,
            "snow" | "snow-cli" => Self::Snow,
            "minimax-code" | "mcode" => Self::MinimaxCode,
            "mothx" => Self::Mothx,
            "kirocrew" | "kiro-crew" => Self::KiroCrew,
            "qwenpaw" => Self::QwenPaw,
            "jcode" | "j-code" => Self::Jcode,
            "openclaw" => Self::OpenClaw,
            "penguin" | "penguin-cli" => Self::Penguin,
            "prime-agent" => Self::PrimeAgent,
            value
                if value
                    .strip_prefix("muse-bin-")
                    .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit())) =>
            {
                Self::Muse
            }
            _ => return None,
        })
    }

    /// Canonical identity used by reported labels and source/Agent pair policy.
    pub const fn report_label(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Devin => "devin",
            Self::Antigravity => "agy",
            Self::Cline => "cline",
            Self::Omp => "omp",
            Self::Mastracode => "mastracode",
            Self::OpenCode => "opencode",
            Self::Copilot => "copilot",
            Self::Kimi => "kimi",
            Self::Kiro => "kiro",
            Self::Droid => "droid",
            Self::Amp => "amp",
            Self::Grok => "grok",
            Self::Hermes => "hermes",
            Self::Kilo => "kilo",
            Self::Qoder => "qodercli",
            Self::Qwen => "qwen",
            Self::Letta => "letta",
            Self::Maki => "maki",
            Self::Muse => "muse",
            Self::AtomCode => "atomcode",
            Self::CodeBuddy => "codebuddy",
            Self::CommandCode => "command-code",
            Self::DevEco => "deveco",
            Self::Mimo => "mimo",
            Self::Reasonix => "reasonix",
            Self::Snow => "snow",
            Self::MinimaxCode => "minimax-code",
            Self::Mothx => "mothx",
            Self::KiroCrew => "kirocrew",
            Self::QwenPaw => "qwenpaw",
            Self::Jcode => "jcode",
            Self::OpenClaw => "openclaw",
            Self::Penguin => "penguin",
            Self::PrimeAgent => "prime-agent",
        }
    }

    /// Exact parse of a canonical native provider ID; aliases and report labels stay absent.
    pub fn from_native_provider_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.native_provider_id() == id)
    }

    /// Canonical ID used by native process observations, independently of report labels.
    pub const fn native_provider_id(self) -> &'static str {
        match self {
            Self::Claude => "claude-code",
            Self::Antigravity => "antigravity",
            Self::Copilot => "github-copilot",
            Self::Qwen => "qwen-code",
            Self::Pi
            | Self::Codex
            | Self::Gemini
            | Self::Cursor
            | Self::Devin
            | Self::Cline
            | Self::Omp
            | Self::Mastracode
            | Self::OpenCode
            | Self::Kimi
            | Self::Kiro
            | Self::Droid
            | Self::Amp
            | Self::Grok
            | Self::Hermes
            | Self::Kilo
            | Self::Qoder
            | Self::Letta
            | Self::Maki
            | Self::Muse
            | Self::AtomCode
            | Self::CodeBuddy
            | Self::CommandCode
            | Self::DevEco
            | Self::Mimo
            | Self::Reasonix
            | Self::Snow
            | Self::MinimaxCode
            | Self::Mothx
            | Self::KiroCrew
            | Self::QwenPaw
            | Self::Jcode
            | Self::OpenClaw
            | Self::Penguin
            | Self::PrimeAgent => self.report_label(),
        }
    }
}

/// Normalize reported identity while preserving unknown labels exactly after trimming.
/// Absence means explicit empty input, not an invitation to omit a supplied guard.
pub fn normalize_agent_report_label(label: &str) -> Option<String> {
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
    let normalized = match AgentReportProvider::lookup(name) {
        Some(provider) => provider.report_label(),
        None => label,
    };
    Some(normalized.to_owned())
}
