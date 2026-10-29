//! The identity result of one Terminal generation and its closed reason lists.
//! The Server owns the state that produces it; these are the shared values.
//!
//! Wire form: one closed object discriminated by `result`. `agent` carries `providerId` (the
//! native provider ID) and `grade`; `unknown` and `unsupported` carry exactly one `reason`;
//! `none` carries nothing else. `result` avoids the CLI's reserved `kind: unsupported` shape.

use crate::AgentReportProvider;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIdentityGrade {
    /// An Orchbee-started run anchored by a launch receipt from the same Terminal generation.
    Confirmed,
    /// A typed run resolved by the registry and held for the settle window.
    Recognized,
}

/// The closed, user-visible list of reasons for `unknown`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case", deny_unknown_fields)]
enum Wire<'a> {
    Agent {
        #[serde(rename = "providerId", borrow)]
        provider_id: std::borrow::Cow<'a, str>,
        grade: AgentIdentityGrade,
    },
    None {},
    Unknown {
        reason: AgentIdentityUnknownReason,
    },
    Unsupported {
        reason: AgentIdentityUnsupportedReason,
    },
}

impl Serialize for AgentProcessIdentity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Agent { provider, grade } => Wire::Agent {
                provider_id: provider.native_provider_id().into(),
                grade,
            },
            Self::None => Wire::None {},
            Self::Unknown(reason) => Wire::Unknown { reason },
            Self::Unsupported(reason) => Wire::Unsupported { reason },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AgentProcessIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Agent { provider_id, grade } => Self::Agent {
                provider: AgentReportProvider::from_native_provider_id(&provider_id)
                    .ok_or_else(|| serde::de::Error::custom("unknown providerId"))?,
                grade,
            },
            Wire::None {} => Self::None,
            Wire::Unknown { reason } => Self::Unknown(reason),
            Wire::Unsupported { reason } => Self::Unsupported(reason),
        })
    }
}
