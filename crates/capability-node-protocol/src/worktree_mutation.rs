//! Defines checked Git mutation operands and retained-job receipts with explicit effect and
//! failure distinctions. It performs no repository mutation and does not turn an unknown
//! outcome into permission to retry.

use crate::{AbsoluteCwd, OperationId, ProtocolError, WorkerGeneration, WorkerId};

pub const MAXIMUM_WORKTREE_MUTATION_FIELDS_BYTES: usize = 7936;
pub const MAXIMUM_GIT_REFERENCE_BYTES: usize = 1024;
pub const MAXIMUM_WORKTREE_DIAGNOSTIC_BYTES: usize = 2048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitReference(String);

impl GitReference {
    /// Lexical argv safety only. Git owns branch and revision syntax validation.
    pub fn new(value: &str) -> Result<Self, ProtocolError> {
        if value.is_empty()
            || value.len() > MAXIMUM_GIT_REFERENCE_BYTES
            || value.starts_with('-')
            || value.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeCheckoutSelector {
    Path(AbsoluteCwd),
    Branch(GitReference),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutation {
    /// Read the actual non-bare source checkout, including a source subdirectory.
    InspectSource,
    Create {
        path: AbsoluteCwd,
        branch: GitReference,
        base: GitReference,
    },
    Open(WorktreeCheckoutSelector),
    InspectRemove {
        path: AbsoluteCwd,
    },
    Remove {
        path: AbsoluteCwd,
        expected_common_directory: AbsoluteCwd,
        force: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeMutationIntent {
    source: AbsoluteCwd,
    trust_repository: bool,
    mutation: WorktreeMutation,
}

impl WorktreeMutationIntent {
    pub fn new(
        source: AbsoluteCwd,
        trust_repository: bool,
        mutation: WorktreeMutation,
    ) -> Result<Self, ProtocolError> {
        let fields = source.as_str().len()
            + match &mutation {
                WorktreeMutation::InspectSource => 0,
                WorktreeMutation::Create { path, branch, base } => {
                    path.as_str().len() + branch.as_str().len() + base.as_str().len()
                }
                WorktreeMutation::Open(WorktreeCheckoutSelector::Path(path))
                | WorktreeMutation::InspectRemove { path } => path.as_str().len(),
                WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(branch)) => {
                    branch.as_str().len()
                }
                WorktreeMutation::Remove {
                    path,
                    expected_common_directory,
                    ..
                } => path.as_str().len() + expected_common_directory.as_str().len(),
            };
        check_fields(fields)?;
        Ok(Self {
            source,
            trust_repository,
            mutation,
        })
    }

    #[must_use]
    pub const fn source(&self) -> &AbsoluteCwd {
        &self.source
    }

    #[must_use]
    pub const fn trust_repository(&self) -> bool {
        self.trust_repository
    }

    #[must_use]
    pub const fn mutation(&self) -> &WorktreeMutation {
        &self.mutation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeCheckout {
    repository_root: AbsoluteCwd,
    common_directory: AbsoluteCwd,
    path: AbsoluteCwd,
    branch: Option<GitReference>,
    dirty: bool,
}

impl WorktreeCheckout {
    pub fn new(
        repository_root: AbsoluteCwd,
        common_directory: AbsoluteCwd,
        path: AbsoluteCwd,
        branch: Option<GitReference>,
        dirty: bool,
    ) -> Result<Self, ProtocolError> {
        check_fields(
            repository_root.as_str().len()
                + common_directory.as_str().len()
                + path.as_str().len()
                + branch.as_ref().map_or(0, |value| value.as_str().len()),
        )?;
        Ok(Self {
            repository_root,
            common_directory,
            path,
            branch,
            dirty,
        })
    }

    #[must_use]
    pub const fn repository_root(&self) -> &AbsoluteCwd {
        &self.repository_root
    }

    #[must_use]
    pub const fn common_directory(&self) -> &AbsoluteCwd {
        &self.common_directory
    }

    #[must_use]
    pub const fn path(&self) -> &AbsoluteCwd {
        &self.path
    }

    #[must_use]
    pub const fn branch(&self) -> Option<&GitReference> {
        self.branch.as_ref()
    }

    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.dirty
    }
}

fn check_fields(actual: usize) -> Result<(), ProtocolError> {
    if actual > MAXIMUM_WORKTREE_MUTATION_FIELDS_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual,
            maximum: MAXIMUM_WORKTREE_MUTATION_FIELDS_BYTES,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeMutationStage {
    Probe,
    Select,
    CheckBranch,
    DirtyCheck,
    Mutate,
    Verify,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeMutationEffect {
    None,
    MayHaveChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeMutationError {
    GitFailed(Option<i32>),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeMutationFailure {
    pub stage: WorktreeMutationStage,
    pub effect: WorktreeMutationEffect,
    pub error: WorktreeMutationError,
    diagnostics: Vec<u8>,
}

impl WorktreeMutationFailure {
    pub fn new(
        stage: WorktreeMutationStage,
        effect: WorktreeMutationEffect,
        error: WorktreeMutationError,
        diagnostics: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        if diagnostics.len() > MAXIMUM_WORKTREE_DIAGNOSTIC_BYTES {
            return Err(ProtocolError::PayloadTooLarge {
                actual: diagnostics.len(),
                maximum: MAXIMUM_WORKTREE_DIAGNOSTIC_BYTES,
            });
        }
        Ok(Self {
            stage,
            effect,
            error,
            diagnostics,
        })
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[u8] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutationCompletion {
    Checkout(WorktreeCheckout),
    Removed { path: AbsoluteCwd },
    Failed(WorktreeMutationFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutationJobAction {
    Start(WorktreeMutationIntent),
    Poll { job_id: OperationId },
    Cancel { job_id: OperationId },
}
impl WorktreeMutationJobAction {
    /// Admits this mutation action's response class; callers validate operation and job identities.
    #[must_use]
    pub fn admits(&self, outcome: &WorktreeMutationOutcome) -> bool {
        match self {
            Self::Start(_) => matches!(outcome, WorktreeMutationOutcome::Start(_)),
            Self::Poll { .. } => matches!(outcome, WorktreeMutationOutcome::Poll(_)),
            Self::Cancel { .. } => matches!(outcome, WorktreeMutationOutcome::Cancel(_)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeMutationRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: WorktreeMutationJobAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeMutationStartOutcome {
    Accepted,
    Existing,
    Busy,
    Conflict,
    Full,
    SpawnFailed,
    PhysicalUnknown,
    Unsupported,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutationPollOutcome {
    Pending,
    Completed(WorktreeMutationCompletion),
    Unknown,
    PhysicalUnknown,
    RejectedFencing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeMutationCancelOutcome {
    Registered,
    AlreadyTerminal,
    Unknown,
    PhysicalUnknown,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutationOutcome {
    Start(WorktreeMutationStartOutcome),
    Poll(WorktreeMutationPollOutcome),
    Cancel(WorktreeMutationCancelOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeMutationResult {
    pub operation_id: OperationId,
    pub job_id: OperationId,
    pub outcome: WorktreeMutationOutcome,
}
