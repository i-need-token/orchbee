//! Closed, versioned lifecycle observations and exact binding authorization.
//! Read-only inspection never grants a retained Close or mutation authority.
use std::num::{NonZeroU8, NonZeroU16, NonZeroU64};

use crate::{
    HostTerminalIdentity, OperationId, ProtocolError, TerminalFence, TerminalId,
    TerminalOperationLease, WorkerGeneration, WorkerId,
};

pub const TERMINAL_LIFECYCLE_CAPABILITY_VERSION: TerminalLifecycleCapabilityVersion =
    TerminalLifecycleCapabilityVersion::new(NonZeroU16::MIN);
pub const LIFECYCLE_INVENTORY_PAGE_TERMINALS: u16 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleCapabilityVersion(NonZeroU16);

impl TerminalLifecycleCapabilityVersion {
    #[must_use]
    pub const fn new(value: NonZeroU16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleStepOrdinal(NonZeroU8);

impl LifecycleStepOrdinal {
    pub fn new(value: u8) -> Result<Self, ProtocolError> {
        if value > 32 {
            return Err(ProtocolError::InvalidPayload);
        }
        NonZeroU8::new(value)
            .map(Self)
            .ok_or(ProtocolError::InvalidPayload)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleAuthorization {
    pub capability_version: TerminalLifecycleCapabilityVersion,
    pub terminal_id: TerminalId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub binding_fence: TerminalFence,
    pub lease: TerminalOperationLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleInventoryCursor {
    pub inventory_generation: NonZeroU64,
    pub offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLifecycleAction {
    Terminate,
    Force,
    Dispose,
    InventoryPage {
        cursor: Option<LifecycleInventoryCursor>,
    },
    /// Read-only preflight; does not claim or advance a Close lifecycle owner.
    InspectInventoryPage {
        cursor: Option<LifecycleInventoryCursor>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleRequest {
    pub authorization: TerminalLifecycleAuthorization,
    pub close_operation_id: OperationId,
    pub request_id: OperationId,
    pub attempt_generation: NonZeroU64,
    pub step_ordinal: LifecycleStepOrdinal,
    pub target: HostTerminalIdentity,
    pub action: TerminalLifecycleAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleTerminalPhase {
    AwaitingStart,
    Running { terminate_requested: bool },
    Finishing { terminate_requested: bool },
    ContainingFailure,
    Exited { terminate_requested: bool },
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleTargetObservation {
    pub revision: u64,
    pub phase: LifecycleTerminalPhase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleInventoryPage {
    pub inventory_generation: NonZeroU64,
    pub next_cursor: Option<LifecycleInventoryCursor>,
    pub returned_count: u16,
    pub target: Option<LifecycleTargetObservation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRejection {
    WrongWorker,
    StaleWorkerGeneration,
    UnsupportedCapability,
    LeaseExpired,
    BindingUnavailable,
    BindingFenceMismatch,
    HostIdentityMismatch,
    LifecycleConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleHostFailure {
    HostRefused,
    CleanupIncomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleInventoryFailure {
    StaleCursor,
    PageRejected,
    InvalidPage,
    IdentityConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleUnknownReason {
    TransportLost,
    Deadline,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLifecycleOutcome {
    HostUnavailableBeforeHandoff,
    TerminateAccepted,
    ForceAccepted,
    Disposed,
    InventoryPage(LifecycleInventoryPage),
    Rejected(LifecycleRejection),
    HostFailure(LifecycleHostFailure),
    InventoryFailure(LifecycleInventoryFailure),
    OutcomeUnknown(LifecycleUnknownReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleResult {
    request: TerminalLifecycleRequest,
    outcome: TerminalLifecycleOutcome,
}

impl TerminalLifecycleResult {
    pub fn new(
        request: TerminalLifecycleRequest,
        outcome: TerminalLifecycleOutcome,
    ) -> Result<Self, ProtocolError> {
        use TerminalLifecycleAction as Action;
        use TerminalLifecycleOutcome as Outcome;
        if request.authorization.capability_version != TERMINAL_LIFECYCLE_CAPABILITY_VERSION
            && matches!(request.action, Action::InspectInventoryPage { .. })
        {
            return Err(ProtocolError::InvalidPayload);
        }
        let failed = matches!(
            outcome,
            Outcome::Rejected(_)
                | Outcome::HostFailure(LifecycleHostFailure::HostRefused)
                | Outcome::OutcomeUnknown(_)
                | Outcome::HostUnavailableBeforeHandoff
        );
        let compatible = match request.action {
            Action::Terminate => matches!(outcome, Outcome::TerminateAccepted) || failed,
            Action::Force => matches!(outcome, Outcome::ForceAccepted) || failed,
            Action::Dispose => {
                matches!(
                    outcome,
                    Outcome::Disposed
                        | Outcome::HostFailure(LifecycleHostFailure::CleanupIncomplete)
                ) || failed
            }
            Action::InventoryPage { cursor } | Action::InspectInventoryPage { cursor } => {
                if let Outcome::InventoryPage(page) = outcome {
                    validate_page(cursor, page)?;
                }
                matches!(
                    outcome,
                    Outcome::InventoryPage(_) | Outcome::InventoryFailure(_)
                ) || failed
            }
        };
        if !compatible {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { request, outcome })
    }

    #[must_use]
    pub const fn request(self) -> TerminalLifecycleRequest {
        self.request
    }

    #[must_use]
    pub const fn outcome(self) -> TerminalLifecycleOutcome {
        self.outcome
    }
}

fn validate_page(
    cursor: Option<LifecycleInventoryCursor>,
    page: LifecycleInventoryPage,
) -> Result<(), ProtocolError> {
    if page.returned_count > LIFECYCLE_INVENTORY_PAGE_TERMINALS
        || (page.target.is_some() && page.returned_count == 0)
        || cursor.is_some_and(|cursor| cursor.inventory_generation != page.inventory_generation)
    {
        return Err(ProtocolError::InvalidPayload);
    }
    let offset = cursor.map_or(0, |cursor| cursor.offset);
    let end = offset
        .checked_add(u64::from(page.returned_count))
        .ok_or(ProtocolError::InvalidPayload)?;
    if page.next_cursor.is_some_and(|next| {
        next.inventory_generation != page.inventory_generation
            || page.returned_count == 0
            || next.offset != end
    }) {
        return Err(ProtocolError::InvalidPayload);
    }
    Ok(())
}
