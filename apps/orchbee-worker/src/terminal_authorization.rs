use std::{cmp::Ordering, collections::BTreeMap, fmt, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostEpoch, HostLaunchAccepted,
    HostLaunchState, HostStartAccepted, HostTerminalIdentity, ProtocolError,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalId, TerminalLaunchFailure, TerminalLaunchMode,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalLaunchResult,
    TerminalOperationAuthorization, TerminalStartFailure, TerminalStartOutcome,
    TerminalStartRejection, TerminalStartResult, WorkerGeneration, WorkerId,
};
use orchbee_local_worker_protocol::WorkerId as LocalWorkerId;

pub(crate) const MAXIMUM_RETAINED_TERMINAL_BINDINGS: usize = 1_024;

mod lifecycle;
pub(crate) use lifecycle::{LifecycleAdmission, LifecycleDispatch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthorizedHostOperation {
    Launch(AuthorizedTerminalLaunch),
    Start(AuthorizedTerminalStart),
}

impl AuthorizedHostOperation {
    fn authorization(&self) -> TerminalOperationAuthorization {
        match self {
            Self::Launch(launch) => launch.authorization(),
            Self::Start(start) => start.authorization(),
        }
    }

    fn lease_is_expired(&self, now_unix_milliseconds: u64) -> bool {
        now_unix_milliseconds >= self.authorization().lease().expires_at().get()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorizedHostDispatch {
    operation: Box<AuthorizedHostOperation>,
}

impl AuthorizedHostDispatch {
    #[must_use]
    pub(crate) const fn operation(&self) -> &AuthorizedHostOperation {
        &self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostOperationAdmission {
    Dispatch(AuthorizedHostDispatch),
    ResultReady,
    SlotOccupied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostHandoffAdmission {
    HandedOff,
    ResultReady,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AuthorizedHostOperationResult {
    Launch(TerminalLaunchResult),
    Start(TerminalStartResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LaunchHostCompletion {
    Accepted {
        identity: HostTerminalIdentity,
    },
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved until Host supplies an explicit rejection discriminator"
        )
    )]
    Rejected {
        host_epoch: HostEpoch,
    },
    Failed {
        host_epoch: HostEpoch,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartHostCompletion {
    Accepted(HostStartAccepted),
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved until Host supplies an explicit rejection discriminator"
        )
    )]
    Rejected {
        host_epoch: HostEpoch,
    },
    Failed {
        host_epoch: HostEpoch,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthorizationKernelError {
    DispatchMismatch,
    InvalidDispatchPhase,
    OperationKindMismatch,
    InconsistentBinding,
    Protocol(ProtocolError),
}

impl fmt::Display for AuthorizationKernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DispatchMismatch => formatter.write_str(
                "Host-operation dispatch does not match the current Worker authorization slot",
            ),
            Self::InvalidDispatchPhase => formatter.write_str(
                "Host-operation dispatch is not in the phase required by this observation",
            ),
            Self::OperationKindMismatch => {
                formatter.write_str("Host-operation completion has the wrong operation kind")
            }
            Self::InconsistentBinding => formatter
                .write_str("Worker terminal authorization binding is internally inconsistent"),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AuthorizationKernelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::DispatchMismatch
            | Self::InvalidDispatchPhase
            | Self::OperationKindMismatch
            | Self::InconsistentBinding => None,
        }
    }
}

impl From<ProtocolError> for AuthorizationKernelError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalAuthorizationBinding {
    cached_result: Option<AuthorizedHostOperationResult>,
    operation: AuthorizedHostOperation,
    lifecycle: Option<lifecycle::LifecycleReceipts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostOperationSlot {
    Lifecycle(lifecycle::LifecycleSlot),
    AwaitingHandoff(AuthorizedHostOperation),
    HandedOff {
        host_epoch: HostEpoch,
        operation: AuthorizedHostOperation,
    },
    ResultReady(AuthorizedHostOperationResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionRejection {
    LifecycleActive,
    WrongWorker,
    StaleWorkerGeneration,
    LeaseExpired,
    StaleFence,
    ConflictingReplay,
    UnsupportedCapability,
    CapacityExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AdmissionAction {
    Dispatch,
    Reject(AdmissionRejection),
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorizedHostOperationKernel {
    // Global OperationId freshness is an authenticated Server-producer
    // precondition. This per-terminal map is intentionally not an unbounded
    // collision ledger; issuer violations lie outside this refinement.
    bindings: BTreeMap<TerminalId, TerminalAuthorizationBinding>,
    current_worker_generation: WorkerGeneration,
    current_worker_id: WorkerId,
    maximum_observed_unix_milliseconds: u64,
    slot: Option<HostOperationSlot>,
}

impl AuthorizedHostOperationKernel {
    pub(crate) fn new(
        current_worker_id: LocalWorkerId,
        current_worker_generation: NonZeroU64,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            bindings: BTreeMap::new(),
            current_worker_generation: WorkerGeneration::new(current_worker_generation),
            current_worker_id: WorkerId::from_bytes(current_worker_id.as_bytes())?,
            maximum_observed_unix_milliseconds: 0,
            slot: None,
        })
    }

    pub(crate) fn admit_launch(
        &mut self,
        launch: AuthorizedTerminalLaunch,
        now_unix_milliseconds: u64,
    ) -> Result<HostOperationAdmission, AuthorizationKernelError> {
        self.admit_operation(
            AuthorizedHostOperation::Launch(launch),
            now_unix_milliseconds,
            None,
        )
    }

    pub(crate) fn admit_start(
        &mut self,
        start: AuthorizedTerminalStart,
        now_unix_milliseconds: u64,
        observed_host_epoch: HostEpoch,
    ) -> Result<HostOperationAdmission, AuthorizationKernelError> {
        let identity_changed_result = if start.identity().host_epoch() == observed_host_epoch {
            None
        } else {
            Some(AuthorizedHostOperationResult::Start(
                TerminalStartResult::new(
                    start.authorization(),
                    start.identity(),
                    TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
                        observed_host_epoch,
                    }),
                )?,
            ))
        };
        self.admit_operation(
            AuthorizedHostOperation::Start(start),
            now_unix_milliseconds,
            identity_changed_result,
        )
    }

    fn admit_operation(
        &mut self,
        operation: AuthorizedHostOperation,
        now_unix_milliseconds: u64,
        pre_dispatch_result: Option<AuthorizedHostOperationResult>,
    ) -> Result<HostOperationAdmission, AuthorizationKernelError> {
        let now_unix_milliseconds = self.observe_time(now_unix_milliseconds);
        if self.slot.is_some() {
            return Ok(HostOperationAdmission::SlotOccupied);
        }

        match self.admission_action(&operation, now_unix_milliseconds)? {
            AdmissionAction::Reject(reason) => {
                let result = rejected_result(&operation, reason)?;
                self.slot = Some(HostOperationSlot::ResultReady(result));
                Ok(HostOperationAdmission::ResultReady)
            }
            AdmissionAction::Replay => {
                let result = self.cached_result_for(&operation)?;
                self.slot = Some(HostOperationSlot::ResultReady(result));
                Ok(HostOperationAdmission::ResultReady)
            }
            AdmissionAction::Dispatch => {
                if let Some(result) = pre_dispatch_result {
                    self.slot = Some(HostOperationSlot::ResultReady(result));
                    return Ok(HostOperationAdmission::ResultReady);
                }
                let terminal_id = operation.authorization().terminal_id();
                self.bindings.insert(
                    terminal_id,
                    TerminalAuthorizationBinding {
                        cached_result: None,
                        operation: operation.clone(),
                        lifecycle: None,
                    },
                );
                self.slot = Some(HostOperationSlot::AwaitingHandoff(operation.clone()));
                Ok(HostOperationAdmission::Dispatch(AuthorizedHostDispatch {
                    operation: Box::new(operation),
                }))
            }
        }
    }

    fn admission_action(
        &self,
        operation: &AuthorizedHostOperation,
        now_unix_milliseconds: u64,
    ) -> Result<AdmissionAction, AuthorizationKernelError> {
        let authorization = operation.authorization();
        if authorization.worker_id() != self.current_worker_id {
            return Ok(AdmissionAction::Reject(AdmissionRejection::WrongWorker));
        }
        if authorization.worker_generation() != self.current_worker_generation {
            return Ok(AdmissionAction::Reject(
                AdmissionRejection::StaleWorkerGeneration,
            ));
        }

        let existing_binding = self.bindings.get(&authorization.terminal_id());
        if let Some(binding) = existing_binding
            && authorization.fence() == binding.operation.authorization().fence()
            && operation == &binding.operation
        {
            if binding.cached_result.is_none() {
                return Err(AuthorizationKernelError::InconsistentBinding);
            }
            return Ok(AdmissionAction::Replay);
        }

        if existing_binding.is_some_and(|binding| binding.lifecycle.is_some()) {
            return Ok(AdmissionAction::Reject(AdmissionRejection::LifecycleActive));
        }

        if authorization.capability_version() != TERMINAL_LAUNCH_CAPABILITY_VERSION {
            return Ok(AdmissionAction::Reject(
                AdmissionRejection::UnsupportedCapability,
            ));
        }
        if operation.lease_is_expired(now_unix_milliseconds) {
            return Ok(AdmissionAction::Reject(AdmissionRejection::LeaseExpired));
        }

        if let Some(binding) = existing_binding {
            match authorization
                .fence()
                .get()
                .cmp(&binding.operation.authorization().fence().get())
            {
                Ordering::Less => {
                    return Ok(AdmissionAction::Reject(AdmissionRejection::StaleFence));
                }
                Ordering::Equal => {
                    return Ok(AdmissionAction::Reject(
                        AdmissionRejection::ConflictingReplay,
                    ));
                }
                Ordering::Greater => {}
            }
        }
        if existing_binding.is_none() && self.bindings.len() >= MAXIMUM_RETAINED_TERMINAL_BINDINGS {
            return Ok(AdmissionAction::Reject(
                AdmissionRejection::CapacityExceeded,
            ));
        }
        Ok(AdmissionAction::Dispatch)
    }

    fn cached_result_for(
        &self,
        operation: &AuthorizedHostOperation,
    ) -> Result<AuthorizedHostOperationResult, AuthorizationKernelError> {
        self.bindings
            .get(&operation.authorization().terminal_id())
            .filter(|binding| binding.operation == *operation)
            .and_then(|binding| binding.cached_result.clone())
            .ok_or(AuthorizationKernelError::InconsistentBinding)
    }

    pub(crate) fn begin_host_handoff(
        &mut self,
        dispatch: &AuthorizedHostDispatch,
        now_unix_milliseconds: u64,
        host_epoch: HostEpoch,
    ) -> Result<HostHandoffAdmission, AuthorizationKernelError> {
        let operation = self.require_awaiting_handoff(dispatch)?;
        let now_unix_milliseconds = self.observe_time(now_unix_milliseconds);
        if operation.lease_is_expired(now_unix_milliseconds) {
            let result = rejected_result(&operation, AdmissionRejection::LeaseExpired)?;
            self.cache_and_queue_result(operation, result)?;
            return Ok(HostHandoffAdmission::ResultReady);
        }
        if let AuthorizedHostOperation::Start(start) = &operation
            && start.identity().host_epoch() != host_epoch
        {
            let result = AuthorizedHostOperationResult::Start(TerminalStartResult::new(
                start.authorization(),
                start.identity(),
                TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
                    observed_host_epoch: host_epoch,
                }),
            )?);
            self.cache_and_queue_result(operation, result)?;
            return Ok(HostHandoffAdmission::ResultReady);
        }

        self.slot = Some(HostOperationSlot::HandedOff {
            host_epoch,
            operation,
        });
        Ok(HostHandoffAdmission::HandedOff)
    }

    pub(crate) fn record_pre_handoff_failure(
        &mut self,
        dispatch: &AuthorizedHostDispatch,
        now_unix_milliseconds: u64,
    ) -> Result<(), AuthorizationKernelError> {
        let operation = self.require_awaiting_handoff(dispatch)?;
        let now_unix_milliseconds = self.observe_time(now_unix_milliseconds);
        let result = if operation.lease_is_expired(now_unix_milliseconds) {
            rejected_result(&operation, AdmissionRejection::LeaseExpired)?
        } else {
            pre_handoff_failure_result(&operation)?
        };
        self.cache_and_queue_result(operation, result)
    }

    pub(crate) fn record_outcome_unknown(
        &mut self,
        dispatch: &AuthorizedHostDispatch,
    ) -> Result<(), AuthorizationKernelError> {
        let (operation, host_epoch) = self.require_handed_off(dispatch)?;
        let result = match &operation {
            AuthorizedHostOperation::Launch(launch) => {
                AuthorizedHostOperationResult::Launch(TerminalLaunchResult::new(
                    launch.authorization(),
                    TerminalLaunchOutcome::OutcomeUnknown { host_epoch },
                )?)
            }
            AuthorizedHostOperation::Start(start) => {
                AuthorizedHostOperationResult::Start(TerminalStartResult::new(
                    start.authorization(),
                    start.identity(),
                    TerminalStartOutcome::OutcomeUnknown,
                )?)
            }
        };
        self.cache_and_queue_result(operation, result)
    }

    pub(crate) fn record_launch_completion(
        &mut self,
        dispatch: &AuthorizedHostDispatch,
        completion: LaunchHostCompletion,
    ) -> Result<(), AuthorizationKernelError> {
        let (operation, handed_off_epoch) = self.require_handed_off(dispatch)?;
        let AuthorizedHostOperation::Launch(launch) = &operation else {
            return Err(AuthorizationKernelError::OperationKindMismatch);
        };
        let outcome = match completion {
            LaunchHostCompletion::Accepted { identity } => {
                require_same_host_epoch(identity.host_epoch(), handed_off_epoch)?;
                let state = match launch.mode() {
                    TerminalLaunchMode::Immediate(geometry) => HostLaunchState::Started(geometry),
                    TerminalLaunchMode::Deferred => HostLaunchState::AwaitingStart,
                };
                TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(identity, state))
            }
            LaunchHostCompletion::Rejected { host_epoch } => {
                require_same_host_epoch(host_epoch, handed_off_epoch)?;
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                    host_epoch,
                })
            }
            LaunchHostCompletion::Failed { host_epoch } => {
                require_same_host_epoch(host_epoch, handed_off_epoch)?;
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                    host_epoch,
                })
            }
        };
        let result = AuthorizedHostOperationResult::Launch(TerminalLaunchResult::new(
            launch.authorization(),
            outcome,
        )?);
        self.cache_and_queue_result(operation, result)
    }

    pub(crate) fn record_start_completion(
        &mut self,
        dispatch: &AuthorizedHostDispatch,
        completion: StartHostCompletion,
    ) -> Result<(), AuthorizationKernelError> {
        let (operation, handed_off_epoch) = self.require_handed_off(dispatch)?;
        let AuthorizedHostOperation::Start(start) = &operation else {
            return Err(AuthorizationKernelError::OperationKindMismatch);
        };
        let outcome = match completion {
            StartHostCompletion::Accepted(accepted) => {
                require_same_host_epoch(accepted.identity().host_epoch(), handed_off_epoch)?;
                if accepted.geometry() != start.geometry() {
                    return Err(ProtocolError::ContradictoryHostObservation.into());
                }
                TerminalStartOutcome::HostAccepted(accepted)
            }
            StartHostCompletion::Rejected { host_epoch } => {
                require_same_host_epoch(host_epoch, handed_off_epoch)?;
                TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected { host_epoch })
            }
            StartHostCompletion::Failed { host_epoch } => {
                require_same_host_epoch(host_epoch, handed_off_epoch)?;
                TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure {
                    host_epoch,
                })
            }
        };
        let result = AuthorizedHostOperationResult::Start(TerminalStartResult::new(
            start.authorization(),
            start.identity(),
            outcome,
        )?);
        self.cache_and_queue_result(operation, result)
    }

    #[must_use]
    pub(crate) fn take_result(&mut self) -> Option<AuthorizedHostOperationResult> {
        match self.slot.take() {
            Some(HostOperationSlot::ResultReady(result)) => Some(result),
            slot => {
                self.slot = slot;
                None
            }
        }
    }

    fn observe_time(&mut self, now_unix_milliseconds: u64) -> u64 {
        self.maximum_observed_unix_milliseconds = self
            .maximum_observed_unix_milliseconds
            .max(now_unix_milliseconds);
        self.maximum_observed_unix_milliseconds
    }

    fn require_awaiting_handoff(
        &self,
        dispatch: &AuthorizedHostDispatch,
    ) -> Result<AuthorizedHostOperation, AuthorizationKernelError> {
        match self.slot.as_ref() {
            Some(HostOperationSlot::AwaitingHandoff(operation)) => {
                if operation == dispatch.operation() {
                    Ok(operation.clone())
                } else {
                    Err(AuthorizationKernelError::DispatchMismatch)
                }
            }
            Some(HostOperationSlot::HandedOff { operation, .. }) => {
                if operation == dispatch.operation() {
                    Err(AuthorizationKernelError::InvalidDispatchPhase)
                } else {
                    Err(AuthorizationKernelError::DispatchMismatch)
                }
            }
            Some(HostOperationSlot::ResultReady(_) | HostOperationSlot::Lifecycle(_)) => {
                Err(AuthorizationKernelError::InvalidDispatchPhase)
            }
            None => Err(AuthorizationKernelError::DispatchMismatch),
        }
    }

    fn require_handed_off(
        &self,
        dispatch: &AuthorizedHostDispatch,
    ) -> Result<(AuthorizedHostOperation, HostEpoch), AuthorizationKernelError> {
        match self.slot.as_ref() {
            Some(HostOperationSlot::HandedOff {
                host_epoch,
                operation,
            }) => {
                if operation == dispatch.operation() {
                    Ok((operation.clone(), *host_epoch))
                } else {
                    Err(AuthorizationKernelError::DispatchMismatch)
                }
            }
            Some(HostOperationSlot::AwaitingHandoff(operation)) => {
                if operation == dispatch.operation() {
                    Err(AuthorizationKernelError::InvalidDispatchPhase)
                } else {
                    Err(AuthorizationKernelError::DispatchMismatch)
                }
            }
            Some(HostOperationSlot::ResultReady(_) | HostOperationSlot::Lifecycle(_)) => {
                Err(AuthorizationKernelError::InvalidDispatchPhase)
            }
            None => Err(AuthorizationKernelError::DispatchMismatch),
        }
    }

    fn cache_and_queue_result(
        &mut self,
        operation: AuthorizedHostOperation,
        result: AuthorizedHostOperationResult,
    ) -> Result<(), AuthorizationKernelError> {
        let terminal_id = operation.authorization().terminal_id();
        let binding = self
            .bindings
            .get_mut(&terminal_id)
            .ok_or(AuthorizationKernelError::InconsistentBinding)?;
        if binding.operation != operation || binding.cached_result.is_some() {
            return Err(AuthorizationKernelError::InconsistentBinding);
        }
        binding.cached_result = Some(result.clone());
        self.slot = Some(HostOperationSlot::ResultReady(result));
        Ok(())
    }
}

fn rejected_result(
    operation: &AuthorizedHostOperation,
    reason: AdmissionRejection,
) -> Result<AuthorizedHostOperationResult, ProtocolError> {
    match operation {
        AuthorizedHostOperation::Launch(launch) => Ok(AuthorizedHostOperationResult::Launch(
            TerminalLaunchResult::new(
                launch.authorization(),
                TerminalLaunchOutcome::Rejected(match reason {
                    AdmissionRejection::LifecycleActive => TerminalLaunchRejection::LifecycleActive,
                    AdmissionRejection::WrongWorker => TerminalLaunchRejection::WrongWorker,
                    AdmissionRejection::StaleWorkerGeneration => {
                        TerminalLaunchRejection::StaleWorkerGeneration
                    }
                    AdmissionRejection::LeaseExpired => TerminalLaunchRejection::LeaseExpired,
                    AdmissionRejection::StaleFence => TerminalLaunchRejection::StaleFence,
                    AdmissionRejection::ConflictingReplay => {
                        TerminalLaunchRejection::ConflictingReplay
                    }
                    AdmissionRejection::UnsupportedCapability => {
                        TerminalLaunchRejection::UnsupportedCapability
                    }
                    AdmissionRejection::CapacityExceeded => {
                        TerminalLaunchRejection::CapacityExceeded
                    }
                }),
            )?,
        )),
        AuthorizedHostOperation::Start(start) => Ok(AuthorizedHostOperationResult::Start(
            TerminalStartResult::new(
                start.authorization(),
                start.identity(),
                TerminalStartOutcome::Rejected(match reason {
                    AdmissionRejection::LifecycleActive => TerminalStartRejection::LifecycleActive,
                    AdmissionRejection::WrongWorker => TerminalStartRejection::WrongWorker,
                    AdmissionRejection::StaleWorkerGeneration => {
                        TerminalStartRejection::StaleWorkerGeneration
                    }
                    AdmissionRejection::LeaseExpired => TerminalStartRejection::LeaseExpired,
                    AdmissionRejection::StaleFence => TerminalStartRejection::StaleFence,
                    AdmissionRejection::ConflictingReplay => {
                        TerminalStartRejection::ConflictingReplay
                    }
                    AdmissionRejection::UnsupportedCapability => {
                        TerminalStartRejection::UnsupportedCapability
                    }
                    AdmissionRejection::CapacityExceeded => {
                        TerminalStartRejection::CapacityExceeded
                    }
                }),
            )?,
        )),
    }
}

fn pre_handoff_failure_result(
    operation: &AuthorizedHostOperation,
) -> Result<AuthorizedHostOperationResult, ProtocolError> {
    match operation {
        AuthorizedHostOperation::Launch(launch) => Ok(AuthorizedHostOperationResult::Launch(
            TerminalLaunchResult::new(
                launch.authorization(),
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
            )?,
        )),
        AuthorizedHostOperation::Start(start) => Ok(AuthorizedHostOperationResult::Start(
            TerminalStartResult::new(
                start.authorization(),
                start.identity(),
                TerminalStartOutcome::Failed(TerminalStartFailure::HostUnavailableBeforeHandoff),
            )?,
        )),
    }
}

fn require_same_host_epoch(
    observed: HostEpoch,
    handed_off: HostEpoch,
) -> Result<(), AuthorizationKernelError> {
    if observed == handed_off {
        Ok(())
    } else {
        Err(ProtocolError::ContradictoryHostObservation.into())
    }
}

#[cfg(test)]
mod tests;
