//! The language-neutral operation contract: one bounded request and one typed report per Agent
//! integration operation. Values carry facts only; rendering, exit statuses and localized text
//! stay with each Client, and execution stays with the node-local writer.

use crate::IntegrationId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod closed;
mod node_path;
#[cfg(test)]
mod tests;

/// Encoded request bound: one operation, one optional override path and one budget.
pub const MAXIMUM_INTEGRATION_REQUEST_BYTES: usize = 8 * 1024;
/// Encoded report bound. It leaves room for the enclosing Worker and Workbench frames.
pub const MAXIMUM_INTEGRATION_REPORT_BYTES: usize = 32 * 1024;
/// The one monotonic cooperative budget of an integration operation. A request budget only
/// shortens it.
pub const MAXIMUM_INTEGRATION_BUDGET_MILLISECONDS: u32 = 40_000;

/// Filesystem step names a mutation report may carry. A writer step outside this closed
/// vocabulary is a contract change.
pub const FILESYSTEM_OPERATIONS: [&str; 15] = [
    "publish-capsule",
    "digest-verify",
    "move-aside",
    "publish-entry",
    "restore-backup",
    "remove-backup",
    "remove-entry",
    "prepare-codex-home",
    "stage-configuration",
    "move-configuration-aside",
    "preserve-configuration-metadata",
    "publish-configuration",
    "verify-configuration",
    "remove-configuration-backup",
    "restore-configuration-backup",
];

/// Capsule source kinds a report may carry.
pub const CAPSULE_SOURCE_KINDS: [&str; 3] = ["cohort-sidecar", "adapter-source-override", "absent"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    TooLarge,
    InvalidDocument,
}

/// One requested operation. `List` without a provider inspects every managed provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum IntegrationOperation {
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider: Option<IntegrationId>,
    },
    Explain {
        provider: IntegrationId,
    },
    Enable {
        provider: IntegrationId,
    },
    Disable {
        provider: IntegrationId,
    },
}

/// One operation with its executing-node inputs. `adapter_source` is the development Capsule
/// source override, resolved on the executing node; `budget_milliseconds` lets a read-only
/// caller with a shorter budget lower the operation ceiling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegrationRequest {
    pub operation: IntegrationOperation,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "node_path::optional"
    )]
    pub adapter_source: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_milliseconds: Option<u32>,
}

impl IntegrationRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self.budget_milliseconds {
            Some(0) => Err(ContractError::InvalidDocument),
            Some(budget) if budget > MAXIMUM_INTEGRATION_BUDGET_MILLISECONDS => {
                Err(ContractError::InvalidDocument)
            }
            _ => Ok(()),
        }
    }
}

/// The typed answer of one operation. `HomeUnavailable` means the executing node has no
/// admitted home directory, so nothing was inspected or changed. Field-less variants are empty
/// struct variants because serde ignores unknown fields beside an internally tagged unit variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum IntegrationReport {
    List {
        items: Vec<SnapshotItem>,
        capsule: CapsuleStatus,
    },
    Explain {
        item: SnapshotItem,
        detail: InspectionDetail,
        capsule: CapsuleStatus,
        plan: Vec<PlannedStep>,
    },
    Mutation {
        report: MutationReport,
    },
    HomeUnavailable {},
}

pub fn encode_request(request: &IntegrationRequest) -> Result<Vec<u8>, ContractError> {
    request.validate()?;
    bounded(
        serde_json::to_vec(request).map_err(|_| ContractError::InvalidDocument)?,
        MAXIMUM_INTEGRATION_REQUEST_BYTES,
    )
}

pub fn decode_request(bytes: &[u8]) -> Result<IntegrationRequest, ContractError> {
    if bytes.len() > MAXIMUM_INTEGRATION_REQUEST_BYTES {
        return Err(ContractError::TooLarge);
    }
    let request: IntegrationRequest =
        serde_json::from_slice(bytes).map_err(|_| ContractError::InvalidDocument)?;
    request.validate()?;
    Ok(request)
}

pub fn encode_report(report: &IntegrationReport) -> Result<Vec<u8>, ContractError> {
    bounded(
        serde_json::to_vec(report).map_err(|_| ContractError::InvalidDocument)?,
        MAXIMUM_INTEGRATION_REPORT_BYTES,
    )
}

pub fn decode_report(bytes: &[u8]) -> Result<IntegrationReport, ContractError> {
    if bytes.len() > MAXIMUM_INTEGRATION_REPORT_BYTES {
        return Err(ContractError::TooLarge);
    }
    serde_json::from_slice(bytes).map_err(|_| ContractError::InvalidDocument)
}

fn bounded(bytes: Vec<u8>, maximum: usize) -> Result<Vec<u8>, ContractError> {
    if bytes.len() > maximum {
        return Err(ContractError::TooLarge);
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Conflict,
    Disabled,
    Enabled,
    NotInstalled,
    UpdateRequired,
    Unknown,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Issue {
    InspectionFailed,
    ManualActionRequired,
    UnsupportedPlatform,
    UnsupportedVersion,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    Available,
    NotFound,
    Unknown,
}

impl Availability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NotFound => "not-found",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagementMode {
    ManagedLocal,
    ManualOnly,
}

impl ManagementMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManagedLocal => "managed-local",
            Self::ManualOnly => "manual-only",
        }
    }
}

/// One provider row of a snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotItem {
    pub availability: Availability,
    pub integration_id: IntegrationId,
    pub issue: Option<Issue>,
    pub management_mode: ManagementMode,
    pub state: State,
    pub timed_out: bool,
}

/// Where a mutation report's observed state comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    PreEffect,
    PostEffect,
    Partial,
    Unavailable,
}

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

/// The capsule-facing view a snapshot or plan carries: whether the managed capsule validates and
/// stays in step with its source, plus the source's own admission state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapsuleStatus {
    pub ready: bool,
    #[serde(serialize_with = "node_path::serialize")]
    pub root: PathBuf,
    pub source_kind: &'static str,
    /// The source's declared digest does not match the capsule contract — a binding failure,
    /// surfaced separately from readiness.
    pub binding_failure: bool,
    /// Identity of the current source payload, never a semantic version.
    pub payload_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Marketplace {
    Absent,
    Current,
    #[serde(rename = "update-required")]
    Outdated,
}

impl Marketplace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Current => "current",
            Self::Outdated => "update-required",
        }
    }
}

/// Guarded-entry target states after ownership inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetState {
    Conflict,
    Current,
    Missing,
    Unknown,
    UpdateRequired,
}

impl TargetState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Current => "current",
            Self::Missing => "missing",
            Self::Unknown => "unknown",
            Self::UpdateRequired => "update-required",
        }
    }
}

/// The detailed guard dimensions an `explain` surfaces — the canonical per-provider inspection
/// internals, captured in the same read pass as the snapshot row. `Plain` is an empty struct
/// variant so its decoder refuses unknown fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum InspectionDetail {
    Cli {
        marketplace: Option<Marketplace>,
        plugin_present: Option<bool>,
    },
    Codex {
        marketplace: Marketplace,
        plugin_present: bool,
    },
    Guarded {
        target_state: TargetState,
        source_ready: bool,
        obsolete_target_state: Option<TargetState>,
    },
    Plain {},
}

/// One step of the guard-evaluated enable plan — the same vector the frontier would run for the
/// observed dimensions, projected for `explain` without any effect.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PlannedStep {
    /// The managed capsule publication precedes provider steps on `enable`.
    CapsulePublish {
        #[serde(with = "node_path")]
        target: PathBuf,
    },
    /// A provider CLI command; `executable` is the resolved absolute path when PATH resolves it,
    /// else the bare registered name.
    Command {
        executable: String,
        argv: Vec<String>,
    },
    /// A guarded filesystem effect on the provider-owned target.
    GuardedWrite {
        #[serde(with = "node_path")]
        target: PathBuf,
    },
    /// Removal of a source-proven obsolete target after current publication.
    GuardedRemove {
        #[serde(with = "node_path")]
        target: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Enable,
    Disable,
}

impl Direction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

/// One executed or refused step of the plan — the machine-readable `attempted[]` member.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum StepKind {
    /// A provider CLI command — `argv` is the literal argument vector.
    Command {
        executable: String,
        argv: Vec<String>,
    },
    /// A guarded filesystem effect on a provider-owned or capsule path.
    Filesystem {
        operation: &'static str,
        #[serde(serialize_with = "node_path::serialize")]
        path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepOutcome {
    Completed,
    Conflict,
    Failed,
    Deadline,
    TimedOut,
    Leaked,
    OutputTooLarge,
    Cancelled,
}

impl StepOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
            Self::Deadline => "deadline",
            Self::TimedOut => "timed-out",
            Self::Leaked => "leaked",
            Self::OutputTooLarge => "output-too-large",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttemptedStep {
    pub step: StepKind,
    pub outcome: StepOutcome,
}

/// Pre-effect refusal codes — nothing was mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    AgentUnavailable,
    Conflict,
    ManualActionRequired,
    InspectionFailed,
    CapsuleUnavailable,
    #[serde(rename = "timed-out")]
    Deadline,
}

impl Refusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AgentUnavailable => "agent-unavailable",
            Self::Conflict => "conflict",
            Self::ManualActionRequired => "manual-action-required",
            Self::InspectionFailed => "inspection-failed",
            Self::CapsuleUnavailable => "capsule-unavailable",
            Self::Deadline => "timed-out",
        }
    }
}

/// Post-effect failure codes — the canonical failure vocabulary plus the typed step outcomes the
/// honest frontier surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureCode {
    Conflict,
    ManualActionRequired,
    OperationFailed,
    VerificationFailed,
    TimedOut,
    OutputTooLarge,
    Leaked,
}

impl FailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::ManualActionRequired => "manual-action-required",
            Self::OperationFailed => "operation-failed",
            Self::VerificationFailed => "verification-failed",
            Self::TimedOut => "timed-out",
            Self::OutputTooLarge => "output-too-large",
            Self::Leaked => "leaked",
        }
    }
}

/// `refused` and `failed` carry their typed code in `code`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "code",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Verdict {
    /// The goal state is observed (or was already true pre-effect).
    Completed,
    /// The controller refused the goal. A nonempty `attempted` means the capsule or vector
    /// frontier was already crossed.
    Refused(Refusal),
    /// A known post-effect failure.
    Failed(FailureCode),
    /// The outcome could not be determined. Closed-vocabulary member: the frontier maps
    /// undetermined observation onto `Failed(TimedOut)`, but the verdict word stays reserved.
    Unknown,
    /// Cancellation was requested before the operation settled.
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationReport {
    pub integration_id: IntegrationId,
    pub direction: Direction,
    pub attempted: Vec<AttemptedStep>,
    /// Retained backups or staging a same-call cleanup could not remove — named, never
    /// silently swept.
    #[serde(with = "node_path::sequence")]
    pub residuals: Vec<PathBuf>,
    pub observed: Option<SnapshotItem>,
    pub provenance: Provenance,
    /// Whether any Orchbee-owned effect could already exist — capsule staging or publication, a
    /// spawned provider command, or a guarded filesystem write.
    pub effect_begun: bool,
    pub verdict: Verdict,
}
