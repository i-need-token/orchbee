//! Report input normalization, source policy and validated provider references.
//! These values create no canonical Session, authority, process, deadline or side effect.

mod provider;
pub use provider::{AgentReportProvider, normalize_agent_report_label};

use crate::AgentIdentityError;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Maximum UTF-8 byte length of an admitted provider Session ID.
pub const MAXIMUM_PROVIDER_SESSION_ID_BYTES: usize = 512;
/// Maximum UTF-8 byte length of an admitted provider Session path.
pub const MAXIMUM_PROVIDER_SESSION_PATH_BYTES: usize = 4096;

/// How an exact official source/Agent pair participates in reported state.
/// This classification is not an authentication decision or proof of a running process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentReportSourcePolicy {
    Custom,
    NativeState,
    FullLifecycle,
    IdentityOnly,
}

/// Classifies canonical report labels and exact, case-sensitive source strings.
pub fn agent_report_source_policy(source: &str, agent: &str) -> AgentReportSourcePolicy {
    use AgentReportSourcePolicy::{Custom, FullLifecycle, IdentityOnly, NativeState};
    match (source, agent) {
        ("herdr:pi", "pi")
        | ("herdr:omp", "omp")
        | ("herdr:mastracode", "mastracode")
        | ("herdr:opencode", "opencode")
        | ("herdr:kilo", "kilo")
        | ("herdr:kimi", "kimi") => FullLifecycle,
        ("herdr:hermes", "hermes")
        | ("herdr:qwen", "qwen")
        | ("herdr:letta", "letta")
        | ("herdr:antigravity_cli", "agy") => IdentityOnly,
        ("herdr:claude", "claude")
        | ("herdr:codex", "codex")
        | ("herdr:copilot", "copilot")
        | ("herdr:devin", "devin")
        | ("herdr:droid", "droid")
        | ("herdr:qodercli", "qodercli")
        | ("herdr:cursor", "cursor")
        | ("herdr:grok", "grok") => NativeState,
        _ => Custom,
    }
}

/// Native-state reservation is independent of report mode: Qwen reserves native
/// state while its reports provide identity only. This is not source authentication.
pub fn agent_report_reserves_native_state(source: &str, agent: &str) -> bool {
    agent_report_source_policy(source, agent) == AgentReportSourcePolicy::NativeState
        || (source, agent) == ("herdr:qwen", "qwen")
}

/// Provider/start-source eligibility only. The Server must still prove current owner,
/// process, generation and sequence guards before replacing any Session reference.
pub fn agent_report_allows_session_replacement(
    source: &str,
    agent: &str,
    start: Option<AgentSessionStartSource>,
) -> bool {
    use AgentSessionStartSource::{Branch, Clear, Compact, Fork, New, Resume, Select, Startup};
    matches!(
        (source, agent, start),
        ("herdr:claude", "claude", Some(Clear | Resume | Compact))
            | (
                "herdr:codex",
                "codex",
                Some(Startup | Clear | Resume | Compact)
            )
            | ("herdr:mastracode", "mastracode", Some(Startup))
            | ("herdr:hermes", "hermes", Some(Startup | New | Resume))
            | ("herdr:opencode", "opencode", Some(Select))
            | ("herdr:pi", "pi", Some(New | Resume | Fork))
            | ("herdr:grok", "grok", Some(New))
            | ("herdr:omp", "omp", Some(Startup | New | Resume | Fork))
            | (
                "herdr:qwen",
                "qwen",
                Some(Startup | Clear | Resume | Compact | Branch)
            )
            | ("herdr:antigravity_cli", "agy", None)
    )
}

/// The provider's reference category, never an Orchbee observation Session ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentProviderSessionReferenceKind {
    Id,
    Path,
}

/// Provider identity admitted through text bounds and, for paths, the destination's
/// absolute-path capability. It intentionally has no context-free Deserialize implementation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSessionReference {
    kind: AgentProviderSessionReferenceKind,
    value: String,
}

fn valid_reference_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

impl AgentProviderSessionReference {
    /// Accepts an exact provider ID without trimming or interpreting it.
    pub fn id(value: &str) -> Option<Self> {
        valid_reference_text(value, MAXIMUM_PROVIDER_SESSION_ID_BYTES).then(|| Self {
            kind: AgentProviderSessionReferenceKind::Id,
            value: value.to_owned(),
        })
    }

    /// The caller supplies the destination platform's pure absolute-path predicate.
    /// No filesystem existence or canonicalization is required for a report.
    pub fn path(value: &str, is_absolute: impl FnOnce(&str) -> bool) -> Option<Self> {
        (valid_reference_text(value, MAXIMUM_PROVIDER_SESSION_PATH_BYTES) && is_absolute(value))
            .then(|| Self {
                kind: AgentProviderSessionReferenceKind::Path,
                value: value.to_owned(),
            })
    }

    /// Selects the reference allowed by the exact source/Agent pair. pi/omp prefer
    /// a valid path and fall back to ID; unsupported or invalid references stay absent.
    pub fn from_report(
        source: &str,
        agent: &str,
        id: Option<&str>,
        path: Option<&str>,
        is_absolute: impl FnOnce(&str) -> bool,
    ) -> Option<Self> {
        if agent_report_source_policy(source, agent) == AgentReportSourcePolicy::Custom {
            return None;
        }
        if matches!(agent, "pi" | "omp")
            && let Some(path) = path
            && let Some(reference) = Self::path(path, is_absolute)
        {
            return Some(reference);
        }
        id.and_then(Self::id)
    }

    /// Returns the admitted reference category.
    pub const fn kind(&self) -> AgentProviderSessionReferenceKind {
        self.kind
    }
    /// Returns the exact provider identity/path, without shell escaping or normalization.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Build a Windows provider's literal resume argv. No process or shell is invoked.
/// A foreign source/Agent pair or unsupported reference kind has no resume plan.
pub fn windows_agent_resume_argv(
    source: &str,
    agent: &str,
    reference: &AgentProviderSessionReference,
) -> Option<Vec<String>> {
    if agent_report_source_policy(source, agent) == AgentReportSourcePolicy::Custom
        || (reference.kind == AgentProviderSessionReferenceKind::Path
            && !matches!(agent, "pi" | "omp"))
    {
        return None;
    }
    let value = reference.value();
    let arguments: Vec<String> = match agent {
        "claude" | "devin" | "droid" | "hermes" | "qodercli" | "qwen" | "grok" => {
            vec![agent.to_owned(), "--resume".into(), value.into()]
        }
        "codex" => vec!["codex".into(), "resume".into(), value.into()],
        "copilot" => vec!["copilot".into(), format!("--resume={value}")],
        "kimi" | "pi" | "opencode" | "kilo" => {
            vec![agent.to_owned(), "--session".into(), value.into()]
        }
        "omp" => vec!["omp".into(), format!("--resume={value}")],
        "mastracode" => vec!["mastracode".into(), "--thread".into(), value.into()],
        "cursor" => vec!["cursor-agent.cmd".into(), "--resume".into(), value.into()],
        "agy" => vec!["agy".into(), "--conversation".into(), value.into()],
        "letta" => match value.strip_prefix("default:") {
            Some("") => return None,
            Some(id) => vec![
                "letta".into(),
                "--conversation".into(),
                "default".into(),
                "--agent".into(),
                id.into(),
            ],
            None => vec!["letta".into(), "--conversation".into(), value.into()],
        },
        _ => return None,
    };
    Some(arguments)
}

/// Recognized provider session-start reasons; unknown reasons are not generation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSessionStartSource {
    Startup,
    Resume,
    Clear,
    Compact,
    Branch,
    New,
    Fork,
    Select,
}
impl AgentSessionStartSource {
    /// Branch is a recognized identity start but does not establish full-lifecycle
    /// reanchor eligibility; process/generation/sequence confirmation remains separate.
    pub const fn is_full_lifecycle_start(self) -> bool {
        !matches!(self, Self::Branch)
    }

    /// Trims input, accepts the exact recognized lowercase values, otherwise returns absence.
    pub fn normalize(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "startup" => Self::Startup,
            "resume" => Self::Resume,
            "clear" => Self::Clear,
            "compact" => Self::Compact,
            "branch" => Self::Branch,
            "new" => Self::New,
            "fork" => Self::Fork,
            "select" => Self::Select,
            _ => return None,
        })
    }
}

/// Full-u64 report sequence, serialized as canonical decimal text rather than a JS number.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AgentReportSequence(u64);
impl AgentReportSequence {
    /// Constructs a sequence with no truncation to the canonical runtime's safe-integer domain.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Parses only canonical decimal text: no sign, whitespace, leading zero or exponent.
    pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
        if value.is_empty()
            || value.len() > 20
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(AgentIdentityError::InvalidFormat);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| AgentIdentityError::InvalidFormat)
    }
    /// The original unsigned sequence; ordering and acceptance belong to the state owner.
    pub const fn get(self) -> u64 {
        self.0
    }
}
impl Serialize for AgentReportSequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for AgentReportSequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}
