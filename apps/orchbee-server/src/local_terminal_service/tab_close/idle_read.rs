//! The read-only `workbench.workspace-idle` check: whether closing one Workspace would ask first,
//! by the evidence and the rule of a conditional Workspace Close, and which terminals keep it from
//! closing unasked. Members come from the current Presentation membership; nothing is admitted,
//! reserved or retained, and each inventory walk is the read-only inspection a conditional Close
//! makes. Unlike that Close, the check reads every member within the budget, so it can name them
//! all. A Close admitted later reads its own evidence again.
use super::super::{LocalTerminalService, target_handle};
use super::evidence::CloseFacts;
use super::idle::WORKSPACE_EVIDENCE_BUDGET;
use crate::session_coordinator::ProcessObservationTarget;
use crate::terminal_coordinator::WorkerPlacement;
use crate::worker_registry::WorkerRegistry;
use orchbee_capability_node_protocol::LifecycleTerminalPhase;
use orchbee_server::WorkbenchTabCloseMemberRuntime;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkspaceIdleFailure as Failure, WorkspaceIdleMember,
    WorkspaceIdleOutcome as Outcome, WorkspaceIdleReason as Reason, WorkspaceIdleRequest,
    WorkspaceIdleResult,
};
use std::time::Instant;

impl LocalTerminalService {
    pub(crate) fn read_workspace_idle(&self, request: WorkspaceIdleRequest) -> WorkspaceIdleResult {
        self.read_workspace_idle_with(request, |targets, deadline| {
            self.request_every_close_fact(targets, deadline)
        })
    }

    /// `read` returns one entry per live target, in order, holding what was read by `deadline`.
    pub(super) fn read_workspace_idle_with(
        &self,
        request: WorkspaceIdleRequest,
        read: impl FnOnce(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts>,
    ) -> WorkspaceIdleResult {
        let outcome = self
            .workspace_idle(&request, read)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        WorkspaceIdleResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            workspace_id: request.workspace_id,
            outcome,
        }
    }

    fn workspace_idle(
        &self,
        request: &WorkspaceIdleRequest,
        read: impl FnOnce(Vec<ProcessObservationTarget>, Instant) -> Vec<CloseFacts>,
    ) -> Result<Outcome, Failure> {
        match self.workbench_session_matches(&request.session_id) {
            Some(true) => {}
            Some(false) => return Err(Failure::Denied),
            None => return Err(Failure::Unavailable),
        }
        let deadline = Instant::now()
            .checked_add(WORKSPACE_EVIDENCE_BUDGET)
            .ok_or(Failure::Unavailable)?;
        let (members, handles) = {
            let workers = self.workers.lock().ok();
            let handles = workers
                .as_deref()
                .map(WorkerRegistry::serving_handles)
                .unwrap_or_default();
            let serving: Vec<_> = handles.iter().filter_map(WorkerPlacement::of).collect();
            let coordinator = self.coordinator.lock().map_err(|_| Failure::Unavailable)?;
            let members = coordinator.capture_workspace_idle(&request.workspace_id, &serving)?;
            (members, handles)
        };
        // One verdict per member, `None` once it is proven idle: ended, never started, or showing
        // nothing beyond an idle shell. A runtime this owner cannot resolve stays unknown.
        let mut verdicts = Vec::with_capacity(members.len());
        let mut live = Vec::new();
        for member in &members {
            let verdict = match &member.runtime {
                None => Some(Reason::Unknown),
                Some((
                    WorkbenchTabCloseMemberRuntime::Uncreated { .. }
                    | WorkbenchTabCloseMemberRuntime::QuietRuntime { .. },
                    _,
                )) => None,
                Some((WorkbenchTabCloseMemberRuntime::Runtime { target, .. }, observation)) => {
                    let phase =
                        target_handle(&handles, target.worker_id(), target.worker_generation())
                            .and_then(|handle| {
                                self.inspect_close_target(
                                    handle,
                                    &request.request_id,
                                    *target,
                                    deadline,
                                )
                            })
                            .map(|observed| observed.phase);
                    match (phase, observation) {
                        (
                            Some(
                                LifecycleTerminalPhase::Exited { .. }
                                | LifecycleTerminalPhase::Failed,
                            ),
                            _,
                        ) => None,
                        (Some(_), Some(observation)) => {
                            live.push((verdicts.len(), observation.clone()));
                            Some(Reason::Unknown)
                        }
                        (Some(_), None) | (None, _) => Some(Reason::Unknown),
                    }
                }
            };
            verdicts.push(verdict);
        }
        if !live.is_empty() {
            let targets = live.iter().map(|(_, target)| target.clone()).collect();
            let facts = read(targets, deadline);
            for ((index, _), facts) in live.into_iter().zip(facts) {
                verdicts[index] = if facts.runs_nothing() {
                    None
                } else if facts.shows_running() {
                    Some(Reason::Running)
                } else {
                    Some(Reason::Unknown)
                };
            }
        }
        let busy: Vec<_> = members
            .iter()
            .zip(verdicts)
            .filter_map(|(member, verdict)| {
                verdict.map(|reason| WorkspaceIdleMember {
                    tab_id: member.tab.tab_id().clone(),
                    panel_id: member.tab.panel_id().clone(),
                    presentation_id: member.tab.presentation_id().clone(),
                    reason,
                })
            })
            .collect();
        Ok(if busy.is_empty() {
            Outcome::Idle {}
        } else {
            Outcome::Busy { members: busy }
        })
    }
}
