//! Installed rule snapshots and Site-lifetime reload receipts.
use crate::{
    AgentDetectionManifest, ContractError, ProtocolVersion, SiteId, WorkbenchRequestId,
    WorkbenchRevision,
};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_ENCODED_AGENT_MANIFESTS_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestSummary {
    pub agent: String,
    #[serde(deserialize_with = "nullable")]
    pub version: Option<String>,
    pub manifest: AgentDetectionManifest,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestsSnapshot {
    pub revision: WorkbenchRevision,
    pub engine_version: u32,
    #[serde(deserialize_with = "nullable")]
    pub configuration_root: Option<String>,
    pub manifests: Vec<AgentManifestSummary>,
    #[serde(
        default,
        deserialize_with = "present_maintenance",
        skip_serializing_if = "Option::is_none"
    )]
    pub maintenance: Option<crate::AgentManifestMaintenance>,
}
pub(super) fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestsFailure {
    Denied,
    Unavailable,
    SiteMismatch,
    Busy,
    ResourceLimit,
    NotFound,
    LoadFailed,
    RevisionExhausted,
    Cancelled,
    Conflict,
    UpdateUnavailable,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentManifestsAction {
    Current {},
    Reload { operation_id: WorkbenchRequestId },
    Update { operation_id: WorkbenchRequestId },
    Operation { operation_id: WorkbenchRequestId },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestsRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: AgentManifestsAction,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentManifestsOperation {
    Pending {},
    Updated {
        report: Box<crate::AgentManifestUpdateReport>,
        activation: AgentManifestActivation,
    },
    Unknown {
        reason: AgentManifestsFailure,
    },
    Applied {
        revision: WorkbenchRevision,
    },
    Failed {
        reason: AgentManifestsFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentManifestsOutcome {
    Current {
        snapshot: AgentManifestsSnapshot,
    },
    Operation {
        operation_id: WorkbenchRequestId,
        status: AgentManifestsOperation,
    },
    Failed {
        reason: AgentManifestsFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestsResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: AgentManifestsOutcome,
}
impl AgentManifestsResult {
    pub fn answers(&self, request: &AgentManifestsRequest) -> bool {
        self.request_id == request.request_id
            && self.site_id == request.site_id
            && match (&request.action, &self.outcome) {
                (_, AgentManifestsOutcome::Failed { .. }) => true,
                (AgentManifestsAction::Current {}, AgentManifestsOutcome::Current { .. }) => true,
                (
                    AgentManifestsAction::Reload {
                        operation_id: expected,
                    }
                    | AgentManifestsAction::Update {
                        operation_id: expected,
                    }
                    | AgentManifestsAction::Operation {
                        operation_id: expected,
                    },
                    AgentManifestsOutcome::Operation {
                        operation_id,
                        status,
                    },
                ) => {
                    expected == operation_id
                        && !matches!(
                            (&request.action, status),
                            (
                                AgentManifestsAction::Reload { .. },
                                AgentManifestsOperation::Updated { .. }
                                    | AgentManifestsOperation::Unknown { .. }
                            ) | (
                                AgentManifestsAction::Update { .. },
                                AgentManifestsOperation::Applied { .. }
                            )
                        )
                }
                _ => false,
            }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if let AgentManifestsOutcome::Current { snapshot } = &self.outcome
            && (snapshot.engine_version != 3
                || snapshot.revision.value() == 0
                || snapshot.configuration_root.as_ref().is_some_and(|p| {
                    p.is_empty() || p.len() > 32768 || p.chars().any(char::is_control)
                })
                || snapshot.manifests.len() > 24
                || snapshot.manifests.iter().enumerate().any(|(i, m)| {
                    m.agent.is_empty()
                        || m.agent.len() > 64
                        || !m
                            .agent
                            .bytes()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                        || snapshot.manifests[..i]
                            .iter()
                            .any(|old| old.agent == m.agent)
                        || !m.manifest.valid()
                        || m.version
                            .as_ref()
                            .is_some_and(|v| !super::panel_agent::manifest::numeric_version(v))
                        || (m.manifest.source == crate::AgentManifestSource::RemoteCache
                            && m.version != m.manifest.cached_remote_version)
                }))
        {
            return Err(ContractError::InvalidDocument);
        }
        if let AgentManifestsOutcome::Current { snapshot } = &self.outcome
            && let Some(maintenance) = &snapshot.maintenance
        {
            maintenance.validate()?;
        }
        if let AgentManifestsOutcome::Operation { status, .. } = &self.outcome {
            status.validate()?;
        }

        Ok(())
    }
}
fn present_maintenance<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<crate::AgentManifestMaintenance>, D::Error> {
    crate::AgentManifestMaintenance::deserialize(d).map(Some)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentManifestActivation {
    NotAttempted {},
    Applied { revision: WorkbenchRevision },
    Failed { reason: AgentManifestsFailure },
}

impl AgentManifestsOperation {
    pub fn validate(&self) -> Result<(), ContractError> {
        if matches!(self,Self::Applied {revision} if revision.value()==0) {
            return Err(ContractError::InvalidDocument);
        }
        if let Self::Updated { report, activation } = self {
            report.validate()?;
            if matches!(activation,AgentManifestActivation::Applied {revision} if revision.value()==0)
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        Ok(())
    }
}
