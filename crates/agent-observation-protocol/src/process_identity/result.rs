//! The identity result of one Terminal generation and its closed reason lists.
//! The Server owns the state that produces it; these are the shared values.

use crate::AgentReportProvider;

/// Exactly one current result per Terminal generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProcessIdentity {
    Agent {
        provider: AgentReportProvider,
        grade: AgentIdentityGrade,
    },
    /// A successful sample with no candidate and no unresolved row that could hold one.
    None,
    /// Incomplete or contradictory evidence; inside the declared matrix it counts as a failure.
    Unknown(AgentIdentityUnknownReason),
    /// Outside the declared matrix.
    Unsupported(AgentIdentityUnsupportedReason),
}

/// How an `agent` result was established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIdentityGrade {
    /// An Orchbee-started run anchored by a launch receipt from the same Terminal generation.
    Confirmed,
    /// A typed run resolved by the registry and held for the settle window.
    Recognized,
}

/// The closed, user-visible list of reasons for `unknown`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIdentityUnknownReason {
    Settling,
    SampleFailed,
    SampleLimit,
    CommandLineUnreadable,
    AmbiguousCandidates,
    UnclassifiedArguments,
    UnverifiedProvenance,
    DetachedConsole,
}

/// Reasons for `unsupported`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIdentityUnsupportedReason {
    Platform,
    RemoteExecution,
}

impl AgentIdentityGrade {
    pub const ALL: [Self; 2] = [Self::Confirmed, Self::Recognized];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Recognized => "recognized",
        }
    }
}

impl AgentIdentityUnknownReason {
    pub const ALL: [Self; 8] = [
        Self::Settling,
        Self::SampleFailed,
        Self::SampleLimit,
        Self::CommandLineUnreadable,
        Self::AmbiguousCandidates,
        Self::UnclassifiedArguments,
        Self::UnverifiedProvenance,
        Self::DetachedConsole,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Settling => "settling",
            Self::SampleFailed => "sample-failed",
            Self::SampleLimit => "sample-limit",
            Self::CommandLineUnreadable => "command-line-unreadable",
            Self::AmbiguousCandidates => "ambiguous-candidates",
            Self::UnclassifiedArguments => "unclassified-arguments",
            Self::UnverifiedProvenance => "unverified-provenance",
            Self::DetachedConsole => "detached-console",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

impl AgentIdentityUnsupportedReason {
    pub const ALL: [Self; 2] = [Self::Platform, Self::RemoteExecution];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::RemoteExecution => "remote-execution",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

impl AgentProcessIdentity {
    /// The result kind: `agent`, `none`, `unknown`, or `unsupported`.
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Agent { .. } => "agent",
            Self::None => "none",
            Self::Unknown(_) => "unknown",
            Self::Unsupported(_) => "unsupported",
        }
    }
}
