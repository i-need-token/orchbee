//! Resolves the Presentations a published-title read names to their committed Workbench binding
//! and retained physical terminal. It reads committed state only: it never maintains Agent
//! reports, commits, or advances any revision.
use super::SessionCoordinator;
use orchbee_capability_node_protocol::{HostTerminalIdentity, WorkerGeneration};
use orchbee_workbench_protocol::{
    PanelAgentFailure, PresentationBinding, SessionId, TerminalPresentationId,
};

/// One requested Presentation: its Workbench binding, null when unknown or unbound, and the
/// physical terminal retained for that binding, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TitleTarget {
    pub(crate) binding: Option<PresentationBinding>,
    pub(crate) physical: Option<(WorkerGeneration, HostTerminalIdentity)>,
}

impl SessionCoordinator {
    /// One target per id, in request order.
    pub(crate) fn presentation_title_targets(
        &self,
        session: &SessionId,
        presentation_ids: &[TerminalPresentationId],
    ) -> Result<Vec<TitleTarget>, PanelAgentFailure> {
        let workbench = self
            .workbench
            .as_ref()
            .ok_or(PanelAgentFailure::Unavailable)?;
        if workbench.session_id() != session {
            return Err(PanelAgentFailure::SessionMismatch);
        }
        let mut targets = vec![
            TitleTarget {
                binding: None,
                physical: None,
            };
            presentation_ids.len()
        ];
        for context in &workbench.snapshot().terminal_contexts {
            let membership = workbench
                .presentation_membership(&context.terminal_context_id)
                .map_err(|_| PanelAgentFailure::Unavailable)?;
            for member in membership.members() {
                if let Some(index) = presentation_ids
                    .iter()
                    .position(|id| id == member.presentation_id())
                    && let Some(target) = targets.get_mut(index)
                {
                    target.binding = Some(member.binding().clone());
                }
            }
        }
        let bindings: Vec<PresentationBinding> = targets
            .iter()
            .filter_map(|target| target.binding.clone())
            .collect();
        let facts = self
            .agent_registration_for_bindings(&bindings)
            .map_err(|_| PanelAgentFailure::Unavailable)?;
        for fact in facts {
            for target in &mut targets {
                if target.binding.as_ref() == Some(fact.binding()) {
                    target.physical = Some((fact.worker_generation(), fact.identity()));
                }
            }
        }
        Ok(targets)
    }
}
