//! Joins retained close membership to current physical bindings and reserves exact lifecycle
//! steps and publication transitions. It does not issue Host requests or treat removed
//! topology as evidence that a physical terminal was disposed.

use std::time::Instant;

use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
    TerminalLifecycleAuthorization, TerminalLifecycleRequest, TerminalOperationLease,
};
#[cfg(test)]
use orchbee_server::CloseContinuationNext;
use orchbee_server::{
    CloseStepToken, WorkbenchGrant, WorkbenchProjectDissolveBegin, WorkbenchTabCloseBegin,
    WorkbenchTabCloseMemberRuntime, WorkbenchTabClosePreparation, WorkbenchTabCloseResolution,
    WorkbenchTabCloseResumeBegin, WorkbenchTabCloseResumePreparation,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTabCloseTarget,
    WorkbenchWorkspaceCloseBegin, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation,
};
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchTabCloseRead,
    WorkbenchTabCloseReadResult, WorkbenchTabCloseResume, WorkbenchTabCloseResumeResult,
    WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadResult, WorkbenchWorkspaceCloseResume,
    WorkbenchWorkspaceCloseResumeResult,
};

use super::{PostcommitCoordinationError, ProcessObservationTarget, SessionCoordinator};
use crate::terminal_coordinator::{BindingViewerTarget, TerminalCoordinator, WorkerPlacement};

/// One captured member of a conditional Workspace, Tab or Panel Close, with the viewer target the
/// Server reads a live runtime through.
pub(crate) type IdleCloseMember = (
    WorkbenchTabCloseMemberRuntime,
    Option<ProcessObservationTarget>,
);

impl SessionCoordinator {
    /// Captures every primary/child binding of a conditional Workspace Close without reserving
    /// it, exactly as a conditional Tab or Panel Close captures its own members. Ordinary
    /// admission refusals remain retained by their existing ledger owner; a runtime this owner
    /// cannot resolve asks first. The caller drops the Session gate before reading anything, then
    /// final admission repeats these guards and compares exact runtime incarnations.
    pub(crate) fn prepare_workspace_idle_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
    ) -> Option<Result<Vec<IdleCloseMember>, WorkbenchCommandResult>> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let session_id = workbench.session_id().clone();
        let grant = WorkbenchGrant::read_only(session_id.clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_workspace_close(Some(&grant), command) {
            WorkbenchWorkspaceCloseBegin::Ready(result) => return Some(Err(result)),
            WorkbenchWorkspaceCloseBegin::Admitted(admission) => admission,
        };
        let admission = match admission.resolve_after_composition() {
            WorkbenchTabCloseResolution::Ready(result) => return Some(Err(result)),
            WorkbenchTabCloseResolution::Primary(admission) => admission,
        };
        let members = admission
            .members()
            .iter()
            .map(|member| Self::idle_close_member(terminals, &session_id, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        Some(match members {
            Ok(members) => Ok(members),
            Err(_) => Err(admission.confirmation_required()),
        })
    }

    /// Captures every member of a conditional Tab or Panel Close without reserving it. Ordinary
    /// refusals and joins of an open Close stay with the ledger owner; a runtime this owner cannot
    /// resolve asks first. The caller drops the Session gate before reading anything, then final
    /// admission repeats these guards and compares exact runtime incarnations.
    pub(crate) fn prepare_terminal_tab_idle_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
    ) -> Option<Result<Vec<IdleCloseMember>, WorkbenchCommandResult>> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let session_id = workbench.session_id().clone();
        let grant = WorkbenchGrant::read_only(session_id.clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_terminal_tab_close(Some(&grant), command) {
            WorkbenchTabCloseBegin::Ready(result) => return Some(Err(result)),
            WorkbenchTabCloseBegin::Admitted(admission) => admission,
        };
        let admission = match admission.resolve_after_composition() {
            WorkbenchTabCloseResolution::Ready(result) => return Some(Err(result)),
            WorkbenchTabCloseResolution::Primary(admission) => admission,
        };
        let members = admission
            .members()
            .iter()
            .map(|member| Self::idle_close_member(terminals, &session_id, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        Some(match members {
            Ok(members) => Ok(members),
            Err(_) => Err(admission.confirmation_required()),
        })
    }

    /// One member's current runtime and, while it runs, the viewer target its evidence is read
    /// through.
    pub(super) fn idle_close_member(
        terminals: &TerminalCoordinator,
        session_id: &orchbee_workbench_protocol::SessionId,
        binding: &orchbee_workbench_protocol::PresentationBinding,
        worker: Option<WorkerPlacement>,
    ) -> Result<IdleCloseMember, WorkbenchTabCloseRuntimeUnavailableReason> {
        let runtime = Self::terminal_tab_close_runtime(terminals, binding, worker)?;
        let observation = match &runtime {
            WorkbenchTabCloseMemberRuntime::Runtime { target, .. } => {
                Some(ProcessObservationTarget {
                    session_id: session_id.clone(),
                    binding: binding.clone(),
                    target: BindingViewerTarget {
                        terminal_id: target.terminal_id(),
                        identity: target.host_identity(),
                        worker: WorkerPlacement {
                            worker_id: target.worker_id(),
                            generation: target.worker_generation(),
                        },
                    },
                })
            }
            _ => None,
        };
        Ok((runtime, observation))
    }

    pub(crate) fn prepare_close_inspection(
        &mut self,
        target: orchbee_server::WorkbenchTabCloseRuntimeTarget,
        close_id: &WorkbenchRequestId,
        cursor: Option<orchbee_capability_node_protocol::LifecycleInventoryCursor>,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Option<TerminalLifecycleRequest> {
        let request_id = self.terminals.allocate_identity()?;
        let lease_id = self.terminals.allocate_identity()?;
        Some(TerminalLifecycleRequest {
            authorization: TerminalLifecycleAuthorization {
                capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
                terminal_id: target.terminal_id(),
                worker_id: target.worker_id(),
                worker_generation: target.worker_generation(),
                binding_fence: target.fence(),
                lease: TerminalOperationLease::new(LeaseId::from_bytes(lease_id).ok()?, expiry),
            },
            close_operation_id: OperationId::from_bytes(
                u128::from_str_radix(close_id.as_str(), 16)
                    .ok()?
                    .to_be_bytes(),
            )
            .ok()?,
            request_id: OperationId::from_bytes(request_id).ok()?,
            attempt_generation: std::num::NonZeroU64::MIN,
            step_ordinal: orchbee_capability_node_protocol::LifecycleStepOrdinal::new(1).ok()?,
            target: target.host_identity(),
            action:
                orchbee_capability_node_protocol::TerminalLifecycleAction::InspectInventoryPage {
                    cursor,
                },
        })
    }
    pub(crate) fn execute_project_dissolve_if_idle(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
    ) -> Option<WorkbenchCommandResult> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant = WorkbenchGrant::manage_navigation(workbench.session_id().clone());
        let admission = match workbench.begin_project_dissolve_if_idle(Some(&grant), command) {
            WorkbenchProjectDissolveBegin::Ready(result) => return Some(result),
            WorkbenchProjectDissolveBegin::Admitted(admission) => admission,
        };
        let runtimes = admission
            .members()
            .iter()
            .map(|member| Self::terminal_tab_close_runtime(terminals, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        Some(match runtimes {
            Ok(runtimes) => admission.finish_resolved_members(runtimes),
            Err(reason) => admission.runtime_unavailable(reason),
        })
    }

    pub(crate) fn execute_terminal_tab_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
    ) -> Option<WorkbenchCommandResult> {
        self.execute_terminal_tab_close_with_proof(
            command,
            worker,
            owner_available,
            now,
            None,
            None,
        )
    }

    /// Admission of one Tab set member's own Close under the set's claimed `token` of operation
    /// and member index. A conditional child counts `proof` only for the exact runtime it was
    /// read from.
    pub(crate) fn execute_tab_set_member_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        token: (&WorkbenchRequestId, usize),
        proof: Option<&[WorkbenchTabCloseMemberRuntime]>,
    ) -> Option<WorkbenchCommandResult> {
        self.execute_terminal_tab_close_with_proof(
            command,
            worker,
            owner_available,
            now,
            proof,
            Some(token),
        )
    }

    /// Final admission of a conditional Tab or Panel Close. Each member's proof counts only for
    /// the exact runtime it was gathered from; anything else asks first.
    pub(crate) fn execute_terminal_tab_close_with_idle_proof(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        proof: &[WorkbenchTabCloseMemberRuntime],
    ) -> Option<WorkbenchCommandResult> {
        self.execute_terminal_tab_close_with_proof(
            command,
            worker,
            owner_available,
            now,
            Some(proof),
            None,
        )
    }

    fn execute_terminal_tab_close_with_proof(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        idle_proof: Option<&[WorkbenchTabCloseMemberRuntime]>,
        set: Option<(&WorkbenchRequestId, usize)>,
    ) -> Option<WorkbenchCommandResult> {
        let conditional = matches!(
            command.action,
            orchbee_workbench_protocol::WorkbenchAction::TabCloseIfIdle { .. }
                | orchbee_workbench_protocol::WorkbenchAction::PanelCloseIfIdle { .. }
        );
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.session_id().clone()).with_terminal_lifecycle();
        let begin = match set {
            Some((operation_id, index)) => {
                workbench.begin_tab_set_member_close(Some(&grant), command, operation_id, index)
            }
            None => workbench.begin_terminal_tab_close(Some(&grant), command),
        };
        let admission = match begin {
            WorkbenchTabCloseBegin::Ready(result) => return Some(result),
            WorkbenchTabCloseBegin::Admitted(admission) => admission,
        };
        let resolution = if conditional {
            admission.resolve_after_composition()
        } else {
            admission.resolve_after_confirmation(now, owner_available)
        };
        let admission = match resolution {
            WorkbenchTabCloseResolution::Ready(result) => return Some(result),
            WorkbenchTabCloseResolution::Primary(admission) => admission,
        };
        if !owner_available && !admission.members().is_empty() {
            return Some(
                admission
                    .runtime_unavailable(WorkbenchTabCloseRuntimeUnavailableReason::ServerBusy),
            );
        }
        let runtimes = admission
            .members()
            .iter()
            .map(|member| Self::terminal_tab_close_runtime(terminals, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        let mut runtimes = match runtimes {
            Ok(runtimes) => runtimes,
            Err(_) if conditional => return Some(admission.confirmation_required()),
            Err(reason) => return Some(admission.runtime_unavailable(reason)),
        };
        apply_idle_proof(&mut runtimes, idle_proof);
        Some(match admission.finish_resolved_members(runtimes, now) {
            WorkbenchTabClosePreparation::Ready(result) => result,
            WorkbenchTabClosePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    /// `workspace.close` execution: the Workspace Close admits a Context
    /// scope, then shares the identical runtime
    /// resolution, admission finish, and commit as the leaf scopes. The
    /// contract is all-or-nothing at admission: an empty membership asks for
    /// no runtime; a populated one fails fast on the first unresolvable
    /// member with zero retained effect.
    pub(crate) fn execute_workspace_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
    ) -> Option<WorkbenchCommandResult> {
        self.execute_workspace_close_with_group(command, worker, owner_available, now, None, None)
    }

    pub(crate) fn execute_workspace_close_with_idle_proof(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        proof: &[WorkbenchTabCloseMemberRuntime],
    ) -> Option<WorkbenchCommandResult> {
        self.execute_workspace_close_with_group(
            command,
            worker,
            owner_available,
            now,
            None,
            Some(proof),
        )
    }

    pub(crate) fn execute_workspace_group_member_close(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        token: (&orchbee_workbench_protocol::WorkbenchRequestId, u32, usize),
    ) -> Option<WorkbenchCommandResult> {
        self.execute_workspace_close_with_group(
            command,
            worker,
            owner_available,
            now,
            Some(token),
            None,
        )
    }

    fn execute_workspace_close_with_group(
        &mut self,
        command: WorkbenchCommand,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
        group: Option<(&orchbee_workbench_protocol::WorkbenchRequestId, u32, usize)>,
        idle_proof: Option<&[WorkbenchTabCloseMemberRuntime]>,
    ) -> Option<WorkbenchCommandResult> {
        let conditional = matches!(
            command.action,
            orchbee_workbench_protocol::WorkbenchAction::WorkspaceCloseIfIdle { .. }
        );
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.session_id().clone()).with_terminal_lifecycle();
        let begin = if let Some((operation_id, attempt, index)) = group {
            workbench.begin_workspace_group_member_close(
                Some(&grant),
                command,
                operation_id,
                attempt,
                index,
            )
        } else {
            workbench.begin_workspace_close(Some(&grant), command)
        };
        let admission = match begin {
            WorkbenchWorkspaceCloseBegin::Ready(result) => return Some(result),
            WorkbenchWorkspaceCloseBegin::Admitted(admission) => admission,
        };
        let admission = match if conditional {
            admission.resolve_after_composition()
        } else {
            admission.resolve_after_confirmation(now, owner_available)
        } {
            WorkbenchTabCloseResolution::Ready(result) => return Some(result),
            WorkbenchTabCloseResolution::Primary(admission) => admission,
        };
        if !owner_available && !admission.members().is_empty() {
            return Some(
                admission
                    .runtime_unavailable(WorkbenchTabCloseRuntimeUnavailableReason::ServerBusy),
            );
        }
        let runtimes = admission
            .members()
            .iter()
            .map(|member| Self::terminal_tab_close_runtime(terminals, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        let mut runtimes = match runtimes {
            Ok(runtimes) => runtimes,
            Err(_) if conditional => return Some(admission.confirmation_required()),
            Err(reason) => return Some(admission.runtime_unavailable(reason)),
        };
        apply_idle_proof(&mut runtimes, idle_proof);
        Some(match admission.finish_resolved_members(runtimes, now) {
            WorkbenchTabClosePreparation::Ready(result) => result,
            WorkbenchTabClosePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn read_close_operation(
        &self,
        request: orchbee_workbench_protocol::CloseOperationRead,
    ) -> Option<orchbee_workbench_protocol::CloseOperationResult> {
        let workbench = self.workbench.as_ref()?;
        let grant = WorkbenchGrant::read_only(workbench.session_id().clone());
        Some(workbench.read_close_operation(Some(&grant), request))
    }

    pub(crate) fn read_workspace_close(
        &self,
        request: WorkbenchWorkspaceCloseRead,
    ) -> Option<WorkbenchWorkspaceCloseReadResult> {
        let workbench = self.workbench.as_ref()?;
        let grant = WorkbenchGrant::read_only(workbench.session_id().clone());
        Some(workbench.read_workspace_close(Some(&grant), request))
    }

    pub(crate) fn resume_workspace_close(
        &mut self,
        request: WorkbenchWorkspaceCloseResume,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        local_cleanup_available: bool,
        now: Instant,
    ) -> Option<WorkbenchWorkspaceCloseResumeResult> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.session_id().clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_workspace_close_resume(Some(&grant), request) {
            WorkbenchWorkspaceCloseResumeBegin::Ready(result) => return Some(result),
            WorkbenchWorkspaceCloseResumeBegin::Admitted(admission) => admission,
        };
        let runtime_targets = admission.runtime_targets();
        if !runtime_targets.is_empty() && !owner_available {
            return Some(admission.runtime_unavailable());
        }
        if admission.needs_local_cleanup() && !local_cleanup_available {
            return Some(admission.runtime_unavailable());
        }
        if runtime_targets.iter().any(|target| {
            !Self::terminal_tab_close_runtime_matches(
                terminals,
                target.member().binding(),
                target.runtime(),
                worker,
            )
        }) {
            return Some(admission.runtime_unavailable());
        }
        Some(match admission.finish(now) {
            WorkbenchWorkspaceCloseResumePreparation::Ready(result) => result,
            WorkbenchWorkspaceCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn read_terminal_tab_close(
        &self,
        request: WorkbenchTabCloseRead,
    ) -> Option<WorkbenchTabCloseReadResult> {
        let workbench = self.workbench.as_ref()?;
        let grant = WorkbenchGrant::read_only(workbench.session_id().clone());
        Some(workbench.read_terminal_tab_close(Some(&grant), request))
    }

    pub(crate) fn resume_terminal_tab_close(
        &mut self,
        request: WorkbenchTabCloseResume,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        local_cleanup_available: bool,
        now: Instant,
    ) -> Option<WorkbenchTabCloseResumeResult> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.session_id().clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_terminal_tab_close_resume(Some(&grant), request) {
            WorkbenchTabCloseResumeBegin::Ready(result) => return Some(result),
            WorkbenchTabCloseResumeBegin::Admitted(admission) => admission,
        };
        let runtime_targets = admission.runtime_targets();
        if !runtime_targets.is_empty() && !owner_available {
            return Some(admission.runtime_unavailable());
        }
        if admission.needs_local_cleanup() && !local_cleanup_available {
            return Some(admission.runtime_unavailable());
        }
        if runtime_targets.iter().any(|target| {
            !Self::terminal_tab_close_runtime_matches(
                terminals,
                target.member().binding(),
                target.runtime(),
                worker,
            )
        }) {
            return Some(admission.runtime_unavailable());
        }
        Some(match admission.finish(now) {
            WorkbenchTabCloseResumePreparation::Ready(result) => result,
            WorkbenchTabCloseResumePreparation::Prepared(prepared) => prepared.commit(),
        })
    }

    pub(crate) fn close_authority(
        &mut self,
    ) -> Result<&mut orchbee_server::WorkbenchAuthority, PostcommitCoordinationError> {
        self.workbench.as_mut().ok_or(PostcommitCoordinationError)
    }

    pub(crate) fn close_runtime_matches_for_member(
        &self,
        close_id: &WorkbenchRequestId,
        binding: &orchbee_workbench_protocol::PresentationBinding,
        worker: Option<WorkerPlacement>,
    ) -> bool {
        self.workbench
            .as_ref()
            .and_then(|authority| {
                authority.terminal_tab_close_target_for_binding(close_id, binding)
            })
            .is_some_and(|target| {
                Self::terminal_tab_close_runtime_matches(
                    &self.terminals,
                    target.member().binding(),
                    target.runtime(),
                    worker,
                )
            })
    }

    #[cfg(test)]
    pub(crate) fn reserve_close_lifecycle_step(
        &mut self,
        close_id: &WorkbenchRequestId,
        now: Instant,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Result<Option<CloseStepToken>, PostcommitCoordinationError> {
        let (target, action, step_ordinal) = {
            let workbench = self.workbench.as_mut().ok_or(PostcommitCoordinationError)?;
            let step = workbench
                .next_terminal_tab_close_step_target(close_id, now)
                .map_err(|_| PostcommitCoordinationError)?;
            let CloseContinuationNext::Lifecycle {
                action,
                step_ordinal,
            } = step.next()
            else {
                return Ok(None);
            };
            let target = step.target().cloned().ok_or(PostcommitCoordinationError)?;
            (target, action, step_ordinal)
        };
        self.reserve_close_lifecycle_step_for_target(
            close_id,
            target,
            action,
            step_ordinal,
            now,
            expiry,
        )
    }

    pub(crate) fn reserve_close_lifecycle_step_for_target(
        &mut self,
        close_id: &WorkbenchRequestId,
        target: WorkbenchTabCloseTarget,
        action: orchbee_capability_node_protocol::TerminalLifecycleAction,
        step_ordinal: orchbee_capability_node_protocol::LifecycleStepOrdinal,
        now: Instant,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Result<Option<CloseStepToken>, PostcommitCoordinationError> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut().ok_or(PostcommitCoordinationError)?;
        let runtime = target.runtime().ok_or(PostcommitCoordinationError)?;
        let generation = workbench
            .terminal_tab_close_generation(close_id)
            .ok_or(PostcommitCoordinationError)?;
        let (Some(request_id), Some(lease_id)) =
            (terminals.allocate_identity(), terminals.allocate_identity())
        else {
            return Ok(None);
        };
        let request = TerminalLifecycleRequest {
            authorization: TerminalLifecycleAuthorization {
                capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
                terminal_id: runtime.terminal_id(),
                worker_id: runtime.worker_id(),
                worker_generation: runtime.worker_generation(),
                binding_fence: runtime.fence(),
                lease: TerminalOperationLease::new(
                    LeaseId::from_bytes(lease_id).map_err(|_| PostcommitCoordinationError)?,
                    expiry,
                ),
            },
            close_operation_id: OperationId::from_bytes(
                u128::from_str_radix(close_id.as_str(), 16)
                    .map_err(|_| PostcommitCoordinationError)?
                    .to_be_bytes(),
            )
            .map_err(|_| PostcommitCoordinationError)?,
            request_id: OperationId::from_bytes(request_id)
                .map_err(|_| PostcommitCoordinationError)?,
            attempt_generation: std::num::NonZeroU64::new(generation.value())
                .ok_or(PostcommitCoordinationError)?,
            step_ordinal,
            target: runtime.host_identity(),
            action,
        };
        workbench
            .reserve_terminal_tab_close_step(close_id, now, request)
            .map(Some)
            .map_err(|_| PostcommitCoordinationError)
    }

    pub(crate) fn publish_terminal_tab_close(
        &mut self,
        close_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<(), PostcommitCoordinationError> {
        let workbench = self.workbench.as_mut().ok_or(PostcommitCoordinationError)?;
        if let Ok(prepared) = workbench.prepare_terminal_tab_close_publication(close_id, now) {
            let _closed = prepared.commit();
        }
        Ok(())
    }
}

/// Replaces each currently resolved runtime with the proof gathered from that exact runtime. A
/// member whose runtime changed since keeps its plain resolution, and the conditional gate asks.
pub(super) fn apply_idle_proof(
    runtimes: &mut [WorkbenchTabCloseMemberRuntime],
    proof: Option<&[WorkbenchTabCloseMemberRuntime]>,
) {
    let Some(proof) = proof.filter(|proof| proof.len() == runtimes.len()) else {
        return;
    };
    for (runtime, proof) in runtimes.iter_mut().zip(proof) {
        if matches!((&*runtime, proof), (
            WorkbenchTabCloseMemberRuntime::Runtime { target: current, .. },
            WorkbenchTabCloseMemberRuntime::QuietRuntime { target: observed, .. }
        ) if current == observed)
        {
            *runtime = proof.clone();
        }
    }
}
