//! Closed vocabularies for the Agent integration surface. The canonical
//! contract is `packages/desktop-client-protocol/src/
//! desktop-agent-integration-management-contract.ts`; this port adds the two
//! CLI-local members `stale-side` (newer-sibling entries the CLI refuses to
//! move in either direction) and `capsule-unsupported-version` (the payload's
//! declared version exceeds the port's compiled support — distinct from the
//! entry-level `unsupported-version`). Kilo extends the native registry with
//! Windows-only managed onboarding; Desktop management keeps its own registry.

/// Canonical integration ids, in the registry's canonical order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationId {
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
    Pi,
    Omp,
    Gemini,
    GitHubCopilot,
    Cline,
    QwenCode,
}

impl IntegrationId {
    pub(crate) const ALL: [Self; 19] = [
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
        Self::Pi,
        Self::Omp,
        Self::Gemini,
        Self::GitHubCopilot,
        Self::Cline,
        Self::QwenCode,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
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
            Self::Pi => "pi",
            Self::Omp => "omp",
            Self::Gemini => "gemini",
            Self::GitHubCopilot => "github-copilot",
            Self::Cline => "cline",
            Self::QwenCode => "qwen-code",
        }
    }

    /// Exact closed-vocabulary spelling; unknown spellings are usage errors,
    /// never fuzzy matches.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|id| id.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) enum State {
    Conflict,
    Disabled,
    Enabled,
    NotInstalled,
    /// The CLI-local member: the installed entry belongs to a NEWER payload
    /// version than this build supports, so ownership cannot be proven and
    /// both enable and disable refuse. Canonical surfaces classify the same
    /// shape `update-required`; the divergence is registered.
    StaleSide,
    UpdateRequired,
    Unknown,
}

#[cfg(any(unix, windows))]
impl State {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::NotInstalled => "not-installed",
            Self::StaleSide => "stale-side",
            Self::UpdateRequired => "update-required",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) enum Issue {
    CapsuleUnsupportedVersion,
    InspectionFailed,
    /// Canonical vocabulary member — a manual-only mutation outcome on
    /// unsupported platforms; the unix port never attaches it to a row.
    #[allow(dead_code)]
    ManualActionRequired,
    /// Attached when the platform's management mode is `manual-only` — the
    /// platform issue overrides `inspection-failed` in the issue slot.
    UnsupportedPlatform,
    /// Canonical vocabulary member — an installed entry declaring an
    /// unsupported entry-level version; no current classifier produces it.
    #[allow(dead_code)]
    UnsupportedVersion,
}

#[cfg(any(unix, windows))]
impl Issue {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CapsuleUnsupportedVersion => "capsule-unsupported-version",
            Self::InspectionFailed => "inspection-failed",
            Self::ManualActionRequired => "manual-action-required",
            Self::UnsupportedPlatform => "unsupported-platform",
            Self::UnsupportedVersion => "unsupported-version",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) enum Availability {
    Available,
    NotFound,
    /// Canonical vocabulary member — the uninspected/fallback availability;
    /// this port always resolves PATH to a definite answer.
    #[allow(dead_code)]
    Unknown,
}

#[cfg(any(unix, windows))]
impl Availability {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NotFound => "not-found",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) enum ManagementMode {
    ManagedLocal,
    ManualOnly,
}

#[cfg(any(unix, windows))]
impl ManagementMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ManagedLocal => "managed-local",
            Self::ManualOnly => "manual-only",
        }
    }
}

/// One provider's inspected snapshot row (canonical item shape).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) struct SnapshotItem {
    pub(crate) availability: Availability,
    pub(crate) integration_id: IntegrationId,
    pub(crate) issue: Option<Issue>,
    pub(crate) management_mode: ManagementMode,
    pub(crate) state: State,
    /// A provider read timed out while this row was assembled — the
    /// mutation layer upgrades the unclassifiable result into the typed
    /// deadline verdict (exit 7) instead of `inspection-failed`.
    pub(crate) timed_out: bool,
}

/// The observed value's provenance — never fabricate a post-effect snapshot
/// a skipped or failed re-inspection did not produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub(crate) enum Provenance {
    /// The admission-time inspection snapshot.
    PreEffect,
    /// A fresh re-inspection after the effect frontier.
    PostEffect,
    /// Re-inspection ran but some reads failed or were unbudgeted.
    Partial,
    /// No re-inspection was possible; attempted[] stands alone.
    Unavailable,
}

#[cfg(any(unix, windows))]
impl Provenance {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::PreEffect => "pre-effect",
            Self::PostEffect => "post-effect",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}
