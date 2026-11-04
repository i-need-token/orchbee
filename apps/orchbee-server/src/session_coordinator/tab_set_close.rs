//! Joins a Tab set's frozen members to current runtimes under the Session gate. An if-idle Start
//! is decided by the Server's own evidence, read outside the gate; each child is an ordinary Tab
//! Close admitted under the set's token.
use std::num::NonZeroU64;

use orchbee_server::{
    TabSetCloseBegin, WorkbenchGrant, WorkbenchTabCloseMemberRuntime,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTerminalTab,
};
use orchbee_workbench_protocol::{
    TabSetCloseCondition, WorkbenchTabSetCloseRequest, WorkbenchTabSetCloseResult, WorkerId,
};

use super::{IdleCloseMember, SessionCoordinator, tab_close::apply_idle_proof};
use crate::terminal_coordinator::WorkerPlacement;

/// What a set request becomes under the Session gate.
pub(crate) enum TabSetCloseStart {
    Answered(WorkbenchTabSetCloseResult),
    /// An if-idle Start that must first read these terminals, member by member in set order.
    /// Nothing is retained yet.
    Read(Vec<IdleCloseMember>),
}

impl SessionCoordinator {
    /// Answers a Read or a known Start, retains a confirmed Start, and captures an if-idle Start's
    /// runtimes without retaining it. A runtime this owner cannot resolve asks first.
    pub(crate) fn start_tab_set_close(
        &mut self,
        request: WorkbenchTabSetCloseRequest,
        worker: Option<WorkerPlacement>,
        incarnation: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
    ) -> Option<TabSetCloseStart> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let session_id = workbench.session_id().clone();
        let grant = WorkbenchGrant::read_only(session_id.clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_tab_set_close(
            Some(&grant),
            request,
            incarnation,
            owner_available,
        ) {
            TabSetCloseBegin::Ready(result) => return Some(TabSetCloseStart::Answered(result)),
            TabSetCloseBegin::Admitted(admission) => admission,
        };
        if admission.condition() == TabSetCloseCondition::Confirmed {
            return Some(TabSetCloseStart::Answered(admission.admit()));
        }
        let members = admission
            .presentations()
            .map(|member| Self::idle_close_member(terminals, &session_id, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        Some(match members {
            Ok(members) => TabSetCloseStart::Read(members),
            Err(_) => TabSetCloseStart::Answered(admission.confirmation_required()),
        })
    }

    /// Final admission of an if-idle Start. Every guard applies again, and each member's runtime
    /// must be never created or proven quiet by evidence read from that exact runtime.
    pub(crate) fn finish_tab_set_close_if_idle(
        &mut self,
        request: WorkbenchTabSetCloseRequest,
        worker: Option<WorkerPlacement>,
        incarnation: Option<(WorkerId, NonZeroU64)>,
        owner_available: bool,
        proof: &[WorkbenchTabCloseMemberRuntime],
    ) -> Option<WorkbenchTabSetCloseResult> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let workbench = workbench.as_mut()?;
        let grant =
            WorkbenchGrant::read_only(workbench.session_id().clone()).with_terminal_lifecycle();
        let admission = match workbench.begin_tab_set_close(
            Some(&grant),
            request,
            incarnation,
            owner_available,
        ) {
            TabSetCloseBegin::Ready(result) => return Some(result),
            TabSetCloseBegin::Admitted(admission) => admission,
        };
        let runtimes = admission
            .presentations()
            .map(|member| Self::terminal_tab_close_runtime(terminals, member.binding(), worker))
            .collect::<Result<Vec<_>, _>>();
        let Ok(mut runtimes) = runtimes else {
            return Some(admission.confirmation_required());
        };
        apply_idle_proof(&mut runtimes, Some(proof));
        Some(
            if runtimes.iter().all(|runtime| {
                matches!(
                    runtime,
                    WorkbenchTabCloseMemberRuntime::Uncreated { .. }
                        | WorkbenchTabCloseMemberRuntime::QuietRuntime { .. }
                )
            }) {
                admission.admit()
            } else {
                admission.confirmation_required()
            },
        )
    }

    /// The runtimes behind one set member's frozen Presentations, which its conditional child reads
    /// before it is submitted.
    pub(crate) fn tab_set_member_idle_close(
        &self,
        presentations: &[WorkbenchTerminalTab],
        worker: Option<WorkerPlacement>,
    ) -> Result<Vec<IdleCloseMember>, WorkbenchTabCloseRuntimeUnavailableReason> {
        let session_id = self
            .workbench
            .as_ref()
            .ok_or(WorkbenchTabCloseRuntimeUnavailableReason::ServerBusy)?
            .session_id()
            .clone();
        presentations
            .iter()
            .map(|member| {
                Self::idle_close_member(&self.terminals, &session_id, member.binding(), worker)
            })
            .collect()
    }
}
