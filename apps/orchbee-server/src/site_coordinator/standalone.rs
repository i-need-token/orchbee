use super::{
    PostcommitCoordinationError, SiteCoordinator, StandaloneTerminal, composition_matches,
    open_create_outcome, open_outcome, presentation_binding, tree_composition_matches, unavailable,
};
use crate::terminal_coordinator::{PendingOperation, TerminalCoordinator, WorkerPlacement};
use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, TerminalId as CapabilityTerminalId,
    WorkerId as CapabilityWorkerId,
};
use orchbee_client_server_protocol::{
    CreateOutcome, CreateRejectedReason, CreateUnavailableReason, OpenOutcome, RequestId,
    ServerMessage, StartMode, TerminalCreateRequest, TerminalOpenRequest,
};
use orchbee_server::{WorkbenchTerminalMembership, WorkbenchTerminalTab};
use orchbee_workbench_protocol::PresentationBinding;

#[derive(Debug)]
pub(crate) enum PreparedOpen {
    Ready(OpenOutcome),
    Wait,
    Dispatch(Box<PendingOperation>),
}

pub(super) struct PreparedStandaloneRetarget<'site> {
    slot: &'site mut StandaloneTerminal,
    next: Option<StandaloneTerminal>,
}

impl PreparedStandaloneRetarget<'_> {
    pub(super) fn commit(self) -> bool {
        if let Some(next) = self.next {
            *self.slot = next;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
pub(super) fn prepare_standalone_close_settlement<'site>(
    slot: &'site mut StandaloneTerminal,
    terminals: &TerminalCoordinator,
    before: &WorkbenchTerminalMembership,
    after: &WorkbenchTerminalMembership,
    removed: &WorkbenchTerminalTab,
) -> Result<PreparedStandaloneRetarget<'site>, PostcommitCoordinationError> {
    prepare_standalone_close_settlement_members(
        slot,
        terminals,
        before,
        after,
        std::slice::from_ref(removed),
    )
}

pub(super) fn prepare_standalone_close_settlement_members<'site>(
    slot: &'site mut StandaloneTerminal,
    terminals: &TerminalCoordinator,
    before: &WorkbenchTerminalMembership,
    after: &WorkbenchTerminalMembership,
    removed: &[WorkbenchTerminalTab],
) -> Result<PreparedStandaloneRetarget<'site>, PostcommitCoordinationError> {
    if !before.is_standalone()
        || !after.is_standalone()
        || before.terminal_context_id() != after.terminal_context_id()
        || removed.iter().any(|removed_member| {
            before
                .members()
                .iter()
                .filter(|member| *member == removed_member)
                .count()
                != 1
        })
        || !before
            .members()
            .iter()
            .filter(|member| {
                !removed
                    .iter()
                    .any(|removed_member| removed_member == *member)
            })
            .eq(after.members().iter())
        || !tree_composition_matches(before, slot, terminals)
    {
        return Err(PostcommitCoordinationError);
    }
    let current_reference = match &*slot {
        StandaloneTerminal::Empty => {
            return Ok(PreparedStandaloneRetarget { slot, next: None });
        }
        StandaloneTerminal::Creating(reference) | StandaloneTerminal::Settled { reference, .. } => {
            reference
        }
    };
    let current_binding = terminals
        .retained_authorization(current_reference)
        .and_then(presentation_binding)
        .ok_or(PostcommitCoordinationError)?;
    let next = if after
        .members()
        .iter()
        .any(|member| member.binding() == &current_binding)
    {
        let retained = standalone_for_binding(terminals, &current_binding)?;
        if &retained != slot {
            return Err(PostcommitCoordinationError);
        }
        None
    } else {
        Some(match after.members().first() {
            Some(member) => standalone_for_binding(terminals, member.binding())?,
            None => StandaloneTerminal::Empty,
        })
    };
    Ok(PreparedStandaloneRetarget { slot, next })
}

/// Prepare Open against an admitted relocation candidate. Workbench proves the
/// transfer; Site proves that every resulting member still has a retained runtime.
pub(super) fn prepare_standalone_relocation<'site>(
    slot: &'site mut StandaloneTerminal,
    terminals: &TerminalCoordinator,
    before: &WorkbenchTerminalMembership,
    after: &WorkbenchTerminalMembership,
) -> Result<PreparedStandaloneRetarget<'site>, PostcommitCoordinationError> {
    if !before.is_standalone()
        || !after.is_standalone()
        || before.terminal_context_id() != after.terminal_context_id()
        || !composition_matches(before, slot, terminals)
        || before
            .members()
            .iter()
            .chain(after.members())
            .any(|member| !terminals.has_retained_binding(member.binding()))
    {
        return Err(PostcommitCoordinationError);
    }
    let current = match &*slot {
        StandaloneTerminal::Empty => None,
        StandaloneTerminal::Creating(reference) | StandaloneTerminal::Settled { reference, .. } => {
            terminals
                .retained_authorization(reference)
                .and_then(presentation_binding)
        }
    };
    let binding = current
        .as_ref()
        .filter(|binding| {
            after
                .members()
                .iter()
                .any(|member| member.binding() == *binding)
        })
        .or_else(|| after.members().first().map(WorkbenchTerminalTab::binding));
    let target = match binding {
        Some(binding) => standalone_for_binding(terminals, binding)?,
        None => StandaloneTerminal::Empty,
    };
    let next = (&target != slot).then_some(target);
    Ok(PreparedStandaloneRetarget { slot, next })
}

fn standalone_for_binding(
    terminals: &TerminalCoordinator,
    binding: &PresentationBinding,
) -> Result<StandaloneTerminal, PostcommitCoordinationError> {
    let terminal_id = u128::from_str_radix(binding.terminal_id.as_str(), 16)
        .ok()
        .and_then(|identity| CapabilityTerminalId::from_bytes(identity.to_be_bytes()).ok())
        .ok_or(PostcommitCoordinationError)?;
    let worker_id = u128::from_str_radix(binding.worker_id.as_str(), 16)
        .ok()
        .and_then(|identity| CapabilityWorkerId::from_bytes(identity.to_be_bytes()).ok())
        .ok_or(PostcommitCoordinationError)?;
    let retained = terminals
        .retained_create_for_binding(terminal_id, worker_id)
        .ok_or(PostcommitCoordinationError)?;
    Ok(match retained.outcome {
        Some(outcome) => StandaloneTerminal::Settled {
            reference: retained.reference.clone(),
            outcome: open_create_outcome(retained.reference, outcome),
        },
        None => StandaloneTerminal::Creating(retained.reference.clone()),
    })
}

impl SiteCoordinator {
    pub(crate) fn prepare_open(
        &mut self,
        request: &TerminalOpenRequest,
        candidate: Option<RequestId>,
        worker: Option<WorkerPlacement>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    ) -> PreparedOpen {
        let Self {
            terminals,
            workbench,
            standalone,
            ..
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
        };
        let membership = match workbench.standalone_presentation_membership() {
            Ok(membership) => membership,
            Err(_invariant) => {
                workbench.retire_standalone_composition();
                return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
            }
        };
        if !composition_matches(&membership, standalone, terminals) {
            workbench.retire_standalone_composition();
            return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
        }
        match &*standalone {
            StandaloneTerminal::Creating(_) => return PreparedOpen::Wait,
            StandaloneTerminal::Settled {
                reference: _,
                outcome,
            } => {
                if let OpenOutcome::Opened {
                    terminal_create_request_id,
                } = outcome
                    && !worker.is_some_and(|worker| {
                        terminals
                            .viewer_target(terminal_create_request_id, worker)
                            .is_some()
                    })
                {
                    return PreparedOpen::Ready(unavailable(
                        CreateUnavailableReason::TerminalRuntimeUnavailable,
                    ));
                }
                return PreparedOpen::Ready(outcome.clone());
            }
            StandaloneTerminal::Empty => {}
        }
        if workbench
            .snapshot()
            .terminal_contexts
            .iter()
            .any(|context| {
                &context.terminal_context_id == membership.terminal_context_id()
                    && !context.tabs.is_empty()
            })
        {
            return PreparedOpen::Ready(unavailable(
                CreateUnavailableReason::TerminalRuntimeUnavailable,
            ));
        }
        if worker.is_none() {
            return PreparedOpen::Ready(unavailable(CreateUnavailableReason::NoCompatibleWorker));
        }
        let Some(reference) = candidate.filter(|candidate| candidate != request.request_id())
        else {
            return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
        };
        let plan = match terminals.plan_create(
            TerminalCreateRequest::new(
                reference.clone(),
                None,
                None,
                StartMode::Immediate {
                    geometry: request.geometry(),
                },
                None,
            ),
            worker,
            expiry,
        ) {
            Ok(plan) => plan,
            Err(result) => {
                if matches!(&result, ServerMessage::TerminalCreateResult(result)
                    if result.outcome() == CreateOutcome::Rejected { reason: CreateRejectedReason::NotAuthorized })
                {
                    return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
                }
                return PreparedOpen::Ready(open_outcome(&reference, &result));
            }
        };
        let Some(binding) = presentation_binding(plan.authorization()) else {
            return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
        };
        let membership = match workbench.prepare_standalone_intent(binding) {
            Ok(membership) => membership,
            Err(_refusal) => {
                return PreparedOpen::Ready(unavailable(CreateUnavailableReason::ServerBusy));
            }
        };
        // Both plans were validated under the same Site gate. No fallible work or external
        // effects may separate these ownership transfers from the Workbench commit.
        let pending = plan.commit();
        let _metadata = membership.commit();
        *standalone = StandaloneTerminal::Creating(reference);
        PreparedOpen::Dispatch(pending)
    }
}
