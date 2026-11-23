//! Retires bindings whose terminal has ended for good and queues the Worker retirement that lets
//! the Worker forget its Disposed record. It never reuses a terminal ID and never decides that a
//! Close ended a terminal or that nothing names its binding; callers prove both.

use std::{collections::BTreeSet, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleStepOrdinal,
    OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalId, TerminalLifecycleAction,
    TerminalLifecycleAuthorization, TerminalLifecycleRequest, TerminalOperationAuthorization,
    TerminalOperationLease, WorkerId,
};
use orchbee_client_server_protocol::CreateOutcome;

use super::{
    MAXIMUM_TERMINAL_BINDINGS, TerminalCoordinator, TerminalPhase, WorkerPlacement,
    matches_placement,
};

/// A binding the Server retired whose Worker still holds the terminal's Disposed record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerRetirement {
    authorization: TerminalOperationAuthorization,
    identity: HostTerminalIdentity,
}

impl WorkerRetirement {
    pub(super) const fn new(
        authorization: TerminalOperationAuthorization,
        identity: HostTerminalIdentity,
    ) -> Self {
        Self {
            authorization,
            identity,
        }
    }
}

impl TerminalCoordinator {
    /// The terminal and Worker IDs of every retained Workbench binding, which a Close can retire.
    pub(crate) fn workbench_terminal_ids(&self) -> BTreeSet<([u8; 16], [u8; 16])> {
        self.bindings
            .values()
            .filter(|binding| !binding.popup_owned)
            .map(|binding| {
                (
                    binding.authorization.terminal_id().as_bytes(),
                    binding.authorization.worker_id().as_bytes(),
                )
            })
            .collect()
    }

    /// Retires the one Workbench binding of `terminal_id` on `worker_id` that a Close ended: with
    /// `identity`, the exact Host terminal the Close disposed; without one, a create that
    /// definitively made no terminal. The caller proves that Close and that no committed
    /// Presentation names the binding. A request naming it afterwards finds it unknown, and the
    /// monotonic allocator never issues its IDs again.
    pub(crate) fn retire_closed(
        &mut self,
        terminal_id: TerminalId,
        worker_id: WorkerId,
        identity: Option<HostTerminalIdentity>,
    ) -> bool {
        let mut matches = self.bindings.iter().filter(|(_, binding)| {
            binding.authorization.terminal_id() == terminal_id
                && binding.authorization.worker_id() == worker_id
        });
        let Some((reference, binding)) = matches.next() else {
            return false;
        };
        if matches.next().is_some() || binding.popup_owned {
            return false;
        }
        let retirement = match (identity, binding.phase) {
            (
                Some(expected),
                TerminalPhase::AwaitingStart(actual)
                | TerminalPhase::Started {
                    identity: actual, ..
                },
            ) if names(expected, actual) => {
                Some(WorkerRetirement::new(binding.authorization, actual))
            }
            (None, TerminalPhase::Unavailable(None))
                if matches!(
                    binding.create_outcome,
                    Some(CreateOutcome::Failed { .. } | CreateOutcome::Unavailable { .. })
                ) =>
            {
                None
            }
            _ => return false,
        };
        let reference = reference.clone();
        self.bindings.remove(&reference);
        if let Some(retirement) = retirement {
            self.queue_worker_retirement(retirement);
        }
        true
    }

    pub(super) fn queue_worker_retirement(&mut self, retirement: WorkerRetirement) {
        // Past the bound the oldest record stays with the Worker, which no longer counts it.
        if self.worker_retirements.len() >= MAXIMUM_TERMINAL_BINDINGS {
            self.worker_retirements.pop_front();
        }
        self.worker_retirements.push_back(retirement);
    }

    /// The next queued Worker retirement for `worker`, as a fresh lifecycle request. Retirements
    /// for another Worker generation are dropped: that Worker and its Disposed records are gone.
    pub(crate) fn next_worker_retirement(
        &mut self,
        worker: WorkerPlacement,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Option<TerminalLifecycleRequest> {
        while let Some(retirement) = self.worker_retirements.pop_front() {
            if !matches_placement(retirement.authorization, worker) {
                continue;
            }
            let request = self.worker_retirement_request(retirement, expiry);
            if request.is_none() {
                self.worker_retirements.push_front(retirement);
            }
            return request;
        }
        None
    }

    fn worker_retirement_request(
        &mut self,
        retirement: WorkerRetirement,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Option<TerminalLifecycleRequest> {
        let operation = OperationId::from_bytes(self.allocate_identity()?).ok()?;
        let request_id = OperationId::from_bytes(self.allocate_identity()?).ok()?;
        let lease = LeaseId::from_bytes(self.allocate_identity()?).ok()?;
        let authorization = retirement.authorization;
        Some(TerminalLifecycleRequest {
            authorization: TerminalLifecycleAuthorization {
                capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
                terminal_id: authorization.terminal_id(),
                worker_id: authorization.worker_id(),
                worker_generation: authorization.worker_generation(),
                binding_fence: authorization.fence(),
                lease: TerminalOperationLease::new(lease, expiry),
            },
            close_operation_id: operation,
            request_id,
            attempt_generation: NonZeroU64::MIN,
            step_ordinal: LifecycleStepOrdinal::new(1).ok()?,
            target: retirement.identity,
            action: TerminalLifecycleAction::Retire,
        })
    }
}

/// Whether a Close that disposed `expected` ended the binding that runs as `actual`: the same Host
/// terminal, or the one it moved from, since a move keeps the creation and a Close the Server began
/// before it recorded the move still names the old identity.
fn names(expected: HostTerminalIdentity, actual: HostTerminalIdentity) -> bool {
    expected == actual
        || (expected.creation_id() == actual.creation_id()
            && expected.host_epoch() != actual.host_epoch())
}
