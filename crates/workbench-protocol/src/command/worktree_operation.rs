//! Bounded native Worktree operation requests and independently observable partial outcomes.
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchRequestId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Deserializer, Serialize, de};

pub const MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES: usize = 96 * 1024;
pub const MAXIMUM_WORKTREE_OPERATION_ATTEMPTS: u32 = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorktreeReference(String);
impl WorktreeReference {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || value.len() > 1024
            || value.starts_with('-')
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for WorktreeReference {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeSource {
    Workspace { workspace_id: WorkbenchWorkspaceId },
    Directory { path: WorkbenchDirectoryPath },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorktreeSelector {
    Path { path: WorkbenchDirectoryPath },
    Branch { branch: WorktreeReference },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeIntent {
    Create {
        source: WorktreeSource,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        branch: Option<WorktreeReference>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        base: Option<WorktreeReference>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        path: Option<WorkbenchDirectoryPath>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        label: Option<WorkbenchLabel>,
        geometry: WorkbenchTerminalGeometry,
        trust_repository: bool,
    },
    Open {
        source: WorktreeSource,
        selector: WorktreeSelector,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_present"
        )]
        label: Option<WorkbenchLabel>,
        geometry: WorkbenchTerminalGeometry,
        trust_repository: bool,
    },
    Remove {
        workspace_id: WorkbenchWorkspaceId,
        force: bool,
        trust_repository: bool,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorktreeAction {
    Start {
        operation_id: WorkbenchRequestId,
        project_id: WorkbenchProjectId,
        intent: WorktreeIntent,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
    Resume {
        operation_id: WorkbenchRequestId,
        #[serde(deserialize_with = "deserialize_attempt")]
        expected_attempt: u32,
    },
}
impl WorkbenchWorktreeAction {
    #[must_use]
    pub const fn operation_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Start { operation_id, .. }
            | Self::Read { operation_id }
            | Self::Resume { operation_id, .. } => operation_id,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: WorkbenchWorktreeAction,
}
impl WorkbenchWorktreeRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkbenchWorktreeAction::Resume {
            expected_attempt, ..
        } = self.action
        {
            validate_attempt(expected_attempt)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreeCheckoutFacts {
    pub repository_root: WorkbenchDirectoryPath,
    pub common_directory: WorkbenchDirectoryPath,
    pub path: WorkbenchDirectoryPath,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub branch: Option<WorktreeReference>,
    pub dirty: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeWorkspaceDisposition {
    Unassociated {},
    Open {
        workspace_id: WorkbenchWorkspaceId,
    },
    Closing {
        workspace_id: WorkbenchWorkspaceId,
        close_request_id: WorkbenchRequestId,
    },
    Closed {
        workspace_id: WorkbenchWorkspaceId,
        close_request_id: WorkbenchRequestId,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeGitEffect {
    None,
    MayHaveChanged,
    Changed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreeProgress {
    /// Requested path is retained even when Git changed the checkout but verification failed.
    #[serde(deserialize_with = "deserialize_nullable")]
    pub path: Option<WorkbenchDirectoryPath>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub checkout: Option<WorktreeCheckoutFacts>,
    pub workspace: WorktreeWorkspaceDisposition,
    pub git_effect: WorktreeGitEffect,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeOperationPhase {
    ObservingSource,
    Creating,
    Opening,
    Attaching,
    InspectingRemove,
    Closing,
    Removing,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeGitStage {
    Probe,
    Select,
    CheckBranch,
    DirtyCheck,
    Mutate,
    Verify,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeGitError {
    GitFailed,
    SpawnUnavailable,
    SpawnFailed,
    InvalidRepository,
    CheckoutNotFound,
    MainCheckout,
    Dirty,
    InvalidReference,
    TooLarge,
    Timeout,
    Cancelled,
    Io,
    RepositoryChanged,
    AmbiguousCheckout,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeOperationFailure {
    Git {
        stage: WorktreeGitStage,
        error: WorktreeGitError,
        #[serde(deserialize_with = "deserialize_status")]
        status: Option<i32>,
        #[serde(deserialize_with = "deserialize_diagnostics")]
        diagnostics: Vec<u8>,
    },
    WorkerUnavailable {},
    WorkerBusy {},
    WorkerCapacity {},
    Unsupported {},
    TransportUnknown {},
    PhysicalUnknown {},
    SourceChanged {},
    CheckoutInUse {},
    AssociationFailed {},
    CloseIncomplete {},
    SiteStopping {},
    Internal {},
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeOperationStatus {
    Pending {
        phase: WorktreeOperationPhase,
        progress: WorktreeProgress,
    },
    Opened {
        checkout: WorktreeCheckoutFacts,
        workspace_id: WorkbenchWorkspaceId,
        reused: bool,
        #[serde(deserialize_with = "deserialize_nullable")]
        terminal_outcome: Option<WorkbenchTerminalOutcome>,
    },
    Removed {
        path: WorkbenchDirectoryPath,
        workspace_id: WorkbenchWorkspaceId,
    },
    Failed {
        reason: WorktreeOperationFailure,
        progress: WorktreeProgress,
        resumable: bool,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeAdmissionFailure {
    Busy,
    Capacity,
    WorkerUnavailable,
    SourceUnavailable,
    ServiceUnavailable,
    Unsupported,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorktreeOutcome {
    Available {
        project_id: WorkbenchProjectId,
        #[serde(deserialize_with = "deserialize_attempt")]
        attempt: u32,
        status: Box<WorktreeOperationStatus>,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: WorktreeAdmissionFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub operation_id: WorkbenchRequestId,
    pub outcome: WorkbenchWorktreeOutcome,
}
impl WorkbenchWorktreeResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkbenchWorktreeOutcome::Available {
            attempt, status, ..
        } = &self.outcome
        {
            validate_attempt(*attempt)?;
            match status.as_ref() {
                WorktreeOperationStatus::Pending { progress, .. } => validate_progress(progress)?,
                WorktreeOperationStatus::Failed {
                    reason, progress, ..
                } => {
                    validate_progress(progress)?;
                    if let WorktreeOperationFailure::Git {
                        error,
                        status,
                        diagnostics,
                        ..
                    } = reason
                        && (diagnostics.len() > 2048
                            || (*error != WorktreeGitError::GitFailed && status.is_some()))
                    {
                        return Err(ContractError::InvalidDocument);
                    }
                }
                WorktreeOperationStatus::Opened {
                    reused,
                    terminal_outcome,
                    ..
                } => {
                    if *reused != terminal_outcome.is_none() {
                        return Err(ContractError::InvalidDocument);
                    }
                }
                WorktreeOperationStatus::Removed { .. } => {}
            }
        }
        Ok(())
    }
}
fn validate_attempt(attempt: u32) -> Result<(), ContractError> {
    if !(1..=MAXIMUM_WORKTREE_OPERATION_ATTEMPTS).contains(&attempt) {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

// The shared contract governs decoded numbers, including equivalent decimal
// and exponent spellings. Bounds below are exactly representable in binary64.
fn integral(value: f64, minimum: f64, maximum: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value >= minimum && value <= maximum
}
fn deserialize_attempt<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if integral(value, 1.0, f64::from(MAXIMUM_WORKTREE_OPERATION_ATTEMPTS)) {
        Ok(value as u32)
    } else {
        Err(de::Error::custom("invalid Worktree attempt"))
    }
}
fn deserialize_status<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i32>, D::Error> {
    Option::<f64>::deserialize(deserializer)?
        .map(|value| {
            if integral(value, f64::from(i32::MIN), f64::from(i32::MAX)) {
                Ok(value as i32)
            } else {
                Err(de::Error::custom("invalid Git process status"))
            }
        })
        .transpose()
}
fn deserialize_diagnostics<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<u8>, D::Error> {
    let values = Vec::<f64>::deserialize(deserializer)?;
    if values.len() > 2048 || values.iter().any(|value| !integral(*value, 0.0, 255.0)) {
        return Err(de::Error::custom("invalid Git diagnostic bytes"));
    }
    Ok(values.into_iter().map(|value| value as u8).collect())
}
fn validate_progress(progress: &WorktreeProgress) -> Result<(), ContractError> {
    if let Some(checkout) = &progress.checkout
        && progress.path.as_ref() != Some(&checkout.path)
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

fn deserialize_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}
fn deserialize_present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
