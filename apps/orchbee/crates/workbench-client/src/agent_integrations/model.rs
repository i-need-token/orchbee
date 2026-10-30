//! Closed vocabularies for the Agent integration surface. The canonical
//! contract is `packages/desktop-client-protocol/src/
//! desktop-agent-integration-management-contract.ts`.
//! Kilo extends the native registry with
//! Windows-only managed onboarding; Desktop management keeps its own registry.

pub use crate::agent_provider::IntegrationId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub enum State {
    Conflict,
    Disabled,
    Enabled,
    NotInstalled,
    UpdateRequired,
    Unknown,
}

#[cfg(any(unix, windows))]
impl State {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::NotInstalled => "not-installed",
            Self::UpdateRequired => "update-required",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub enum Issue {
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectionFailed => "inspection-failed",
            Self::ManualActionRequired => "manual-action-required",
            Self::UnsupportedPlatform => "unsupported-platform",
            Self::UnsupportedVersion => "unsupported-version",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub enum Availability {
    Available,
    NotFound,
    /// Canonical vocabulary member — the uninspected/fallback availability;
    /// this port always resolves PATH to a definite answer.
    #[allow(dead_code)]
    Unknown,
}

#[cfg(any(unix, windows))]
impl Availability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NotFound => "not-found",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub enum ManagementMode {
    ManagedLocal,
    ManualOnly,
}

#[cfg(any(unix, windows))]
impl ManagementMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManagedLocal => "managed-local",
            Self::ManualOnly => "manual-only",
        }
    }
}

/// One provider's inspected snapshot row (canonical item shape).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub struct SnapshotItem {
    pub availability: Availability,
    pub integration_id: IntegrationId,
    pub issue: Option<Issue>,
    pub management_mode: ManagementMode,
    pub state: State,
    /// A provider read timed out while this row was assembled — the
    /// mutation layer upgrades the unclassifiable result into the typed
    /// deadline verdict (exit 7) instead of `inspection-failed`.
    pub timed_out: bool,
}

/// The observed value's provenance — never fabricate a post-effect snapshot
/// a skipped or failed re-inspection did not produce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(any(unix, windows))]
pub enum Provenance {
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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreEffect => "pre-effect",
            Self::PostEffect => "post-effect",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}
