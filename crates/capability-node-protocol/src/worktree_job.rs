use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Byte bound on the Start action's directory operand: one absolute local-platform
/// path, nonempty and NUL-free.
pub const MAXIMUM_WORKTREE_JOB_DIRECTORY_BYTES: usize = 4096;

/// Conservative policy bound on the summed variable byte fields of one
/// kind-16 result. The tightest arm's arithmetic ceiling is 8149; 7936 keeps
/// every arm below the 8192-byte frame payload with declared headroom.
pub const MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES: usize = 7936;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeJobStage {
    Probe,
    List,
}

impl WorktreeJobStage {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Probe => 1,
            Self::List => 2,
        }
    }

    pub(crate) fn from_code(code: u8) -> Result<Self, ProtocolError> {
        match code {
            1 => Ok(Self::Probe),
            2 => Ok(Self::List),
            _ => Err(ProtocolError::InvalidPayload),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeJobAction {
    Start {
        directory: Vec<u8>,
    },
    /// Windows source discovery; legacy Start bytes and behavior remain unchanged.
    StartSource {
        directory: Vec<u8>,
        trust_repository: bool,
    },
    /// Windows read-only branch/upstream observation; no repository trust expansion.
    StartStatus {
        directory: Vec<u8>,
    },
    Poll {
        job_id: OperationId,
    },
    Cancel {
        job_id: OperationId,
    },
}

impl WorktreeJobAction {
    pub fn start_status(directory: Vec<u8>) -> Result<Self, ProtocolError> {
        Self::start(directory.clone())?;
        Ok(Self::StartStatus { directory })
    }

    pub fn start_source(directory: Vec<u8>, trust_repository: bool) -> Result<Self, ProtocolError> {
        Self::start(directory.clone())?;
        Ok(Self::StartSource {
            directory,
            trust_repository,
        })
    }

    /// The directory must be an absolute, nonempty, NUL-free byte path before
    /// any request is constructed.
    pub fn start(directory: Vec<u8>) -> Result<Self, ProtocolError> {
        if directory.is_empty()
            || directory.len() > MAXIMUM_WORKTREE_JOB_DIRECTORY_BYTES
            || !crate::terminal_launch::is_absolute_path(&directory)
            || directory.contains(&0)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Start { directory })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeJobActionKind {
    Start,
    Poll,
    Cancel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeJobRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: WorktreeJobAction,
}

/// Per-action outcome vocabularies are closed; a tag only ever appears inside
/// its own action's result, never ambiguously across actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeJobStartOutcome {
    Accepted,
    Busy,
    InvalidDirectory,
    /// The owner thread could not be spawned; no child ever existed.
    SpawnThreadFailed,
    Unsupported,
    /// The slot is permanently occupied by an uncertain cleanup tombstone.
    Dead,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeJobPollOutcome {
    Pending,
    StatusCompleted {
        status: Vec<u8>,
        stderr: Vec<u8>,
    },
    SourceCompleted {
        common_dir: Vec<u8>,
        source_checkout: Vec<u8>,
        listing: Vec<u8>,
        stderr: Vec<u8>,
    },
    Completed {
        common_dir: Vec<u8>,
        listing: Vec<u8>,
        stderr: Vec<u8>,
    },
    GitFailed {
        stage: WorktreeJobStage,
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    SpawnUnavailable {
        stage: WorktreeJobStage,
    },
    SpawnFailed {
        stage: WorktreeJobStage,
    },
    /// `None` when the wire serialization budget itself overflowed; the
    /// backend's own output bounds carry `Some(stage)`.
    TooLarge {
        stage: Option<WorktreeJobStage>,
    },
    Timeout,
    Cancelled,
    OwnerIo {
        stage: Option<WorktreeJobStage>,
    },
    Unknown,
    PhysicalUnknown,
    RejectedFencing,
}

impl WorktreeJobPollOutcome {
    pub fn status_completed(status: Vec<u8>, stderr: Vec<u8>) -> Result<Self, ProtocolError> {
        check_fields(status.len() + stderr.len())?;
        Ok(Self::StatusCompleted { status, stderr })
    }

    pub fn source_completed(
        common_dir: Vec<u8>,
        source_checkout: Vec<u8>,
        listing: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        check_fields(common_dir.len() + source_checkout.len() + listing.len() + stderr.len())?;
        Ok(Self::SourceCompleted {
            common_dir,
            source_checkout,
            listing,
            stderr,
        })
    }

    /// Construct a completed outcome only when the summed byte fields fit the
    /// wire budget; the caller emits `TooLarge { stage: None }` on overflow.
    pub fn completed(
        common_dir: Vec<u8>,
        listing: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        check_fields(common_dir.len() + listing.len() + stderr.len())?;
        Ok(Self::Completed {
            common_dir,
            listing,
            stderr,
        })
    }

    pub fn git_failed(
        stage: WorktreeJobStage,
        status: Option<i32>,
        stderr: Vec<u8>,
    ) -> Result<Self, ProtocolError> {
        check_fields(stderr.len())?;
        Ok(Self::GitFailed {
            stage,
            status,
            stderr,
        })
    }
}

fn check_fields(bytes: usize) -> Result<(), ProtocolError> {
    if bytes > MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: bytes,
            maximum: MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES,
        });
    }
    Ok(())
}

/// `AlreadyTerminal` is summary-only: the terminal outcome stays re-readable
/// through Poll until the receipt expires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorktreeJobCancelOutcome {
    Registered,
    AlreadyTerminal,
    Unknown,
    PhysicalUnknown,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeJobResult {
    pub operation_id: OperationId,
    pub job_id: OperationId,
    pub outcome: WorktreeJobOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeJobOutcome {
    Start(WorktreeJobStartOutcome),
    Poll(WorktreeJobPollOutcome),
    Cancel(WorktreeJobCancelOutcome),
}

impl WorktreeJobOutcome {
    pub(crate) const fn action_kind(&self) -> WorktreeJobActionKind {
        match self {
            Self::Start(_) => WorktreeJobActionKind::Start,
            Self::Poll(_) => WorktreeJobActionKind::Poll,
            Self::Cancel(_) => WorktreeJobActionKind::Cancel,
        }
    }
}
