//! Bounded launch reservations, distinct from canonical Agent observations.
//! The Server owns transitions; this module owns only closed wire documents.

use serde::{Deserialize, Serialize};

use crate::identity::{
    AgentAttachmentGeneration, AgentHostEpoch, AgentHostTerminalId, AgentName, AgentPresentationId,
    AgentProviderId, AgentSafeInteger, AgentSessionId, ObapRequestId,
};
use crate::model::deserialize_required_nullable;
use crate::read::AgentUnavailableReason;

pub const MAXIMUM_LAUNCH_TTL_MS: u64 = 300_000;
pub const LAUNCH_SETTLE_MS: u64 = 3000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentLaunchBinding {
    pub presentation_id: AgentPresentationId,
    pub generation: AgentAttachmentGeneration,
    pub host_epoch: AgentHostEpoch,
    pub host_terminal_id: AgentHostTerminalId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentLaunchReservation {
    /// The reserve request's identity is also the reservation identity.
    pub reservation_id: ObapRequestId,
    pub name: AgentName,
    pub provider_id: AgentProviderId,
    pub binding: AgentLaunchBinding,
    pub remaining_ms: AgentSafeInteger,
    pub settle_remaining_ms: AgentSafeInteger,
}

/// Startup ownership is independent of current semantic activity and Client focus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentManagement {
    Unmanaged {},
    Pending {
        reservation: AgentLaunchReservation,
        blocked: bool,
    },
    Active {
        reservation_id: ObapRequestId,
        provider_id: AgentProviderId,
        binding: AgentLaunchBinding,
    },
}

/// A restored physical Agent has startup ownership, but no retained canonical session or reservation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentTerminalManagement {
    Unmanaged {},
    Active { provider_id: AgentProviderId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentLaunchRequest {
    #[serde(rename = "agent.launch.terminal-status.request")]
    TerminalStatus {
        request_id: ObapRequestId,
        binding: AgentLaunchBinding,
    },
    #[serde(rename = "agent.launch.status.request")]
    Status {
        request_id: ObapRequestId,
        session_id: AgentSessionId,
    },
    #[serde(rename = "agent.launch.reserve.request")]
    Reserve {
        request_id: ObapRequestId,
        name: AgentName,
        provider_id: AgentProviderId,
        presentation_id: AgentPresentationId,
        host_epoch: AgentHostEpoch,
        host_terminal_id: AgentHostTerminalId,
        ttl_ms: AgentSafeInteger,
    },
    #[serde(rename = "agent.launch.inspect.request")]
    Inspect {
        request_id: ObapRequestId,
        reservation_id: ObapRequestId,
    },
    #[serde(rename = "agent.launch.complete.request")]
    Complete {
        request_id: ObapRequestId,
        reservation_id: ObapRequestId,
        session_id: AgentSessionId,
    },
    #[serde(rename = "agent.launch.cancel.request")]
    Cancel {
        request_id: ObapRequestId,
        reservation_id: ObapRequestId,
    },
}

impl AgentLaunchRequest {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::TerminalStatus { request_id, .. }
            | Self::Status { request_id, .. }
            | Self::Reserve { request_id, .. }
            | Self::Inspect { request_id, .. }
            | Self::Complete { request_id, .. }
            | Self::Cancel { request_id, .. } => request_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentLaunchRejectionReason {
    SessionNotFound,
    NameInUse,
    PresentationBusy,
    CapacityExceeded,
    BindingChanged,
    ReservationNotFound,
    CandidateUnavailable,
    CandidateAmbiguous,
    AlreadyNamed,
    NotReady,
    ReadDenied,
    ManageDenied,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentLaunchOutcome {
    TerminalStatus {
        binding: AgentLaunchBinding,
        management: AgentTerminalManagement,
    },
    Status {
        session_id: AgentSessionId,
        revision: AgentSafeInteger,
        management: AgentManagement,
    },
    Reserved {
        reservation: AgentLaunchReservation,
    },
    Pending {
        reservation: AgentLaunchReservation,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        candidate_session_id: Option<AgentSessionId>,
    },
    Completed {
        reservation_id: ObapRequestId,
        name: AgentName,
        session_id: AgentSessionId,
    },
    Released {
        reservation_id: ObapRequestId,
    },
    Rejected {
        reason: AgentLaunchRejectionReason,
    },
    Unavailable {
        reason: AgentUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentLaunchOutcomeEnvelope {
    pub outcome: AgentLaunchOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum AgentLaunchResult {
    #[serde(rename = "agent.launch.terminal-status.result")]
    TerminalStatus {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
    #[serde(rename = "agent.launch.status.result")]
    Status {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
    #[serde(rename = "agent.launch.reserve.result")]
    Reserve {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
    #[serde(rename = "agent.launch.inspect.result")]
    Inspect {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
    #[serde(rename = "agent.launch.complete.result")]
    Complete {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
    #[serde(rename = "agent.launch.cancel.result")]
    Cancel {
        request_id: ObapRequestId,
        result: AgentLaunchOutcomeEnvelope,
    },
}

impl AgentLaunchResult {
    #[must_use]
    pub fn request_id(&self) -> &ObapRequestId {
        match self {
            Self::TerminalStatus { request_id, .. }
            | Self::Status { request_id, .. }
            | Self::Reserve { request_id, .. }
            | Self::Inspect { request_id, .. }
            | Self::Complete { request_id, .. }
            | Self::Cancel { request_id, .. } => request_id,
        }
    }

    #[must_use]
    pub fn outcome(&self) -> &AgentLaunchOutcome {
        match self {
            Self::TerminalStatus { result, .. }
            | Self::Status { result, .. }
            | Self::Reserve { result, .. }
            | Self::Inspect { result, .. }
            | Self::Complete { result, .. }
            | Self::Cancel { result, .. } => &result.outcome,
        }
    }
}
