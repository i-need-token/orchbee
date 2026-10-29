//! Admits the canonical Close owed to an interactive Shell that exited with code 0. It decides
//! eligibility from retained creation facts and committed Presentation membership, then reuses
//! the ordinary Panel Close admission. It does not observe Host lifecycle, dispose terminals, or
//! publish removal; the existing Close continuation owns every effect after admission.

use std::time::Instant;

use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchPanelId, WorkbenchRequestId, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

use super::{SessionCoordinator, TargetLifecycle, target_lifecycle::target_binding};
use crate::terminal_coordinator::{BindingViewerTarget, TerminalPhase, WorkerPlacement};

impl SessionCoordinator {
    /// `target` names the exact physical terminal whose Host exit record with exit code 0 Server
    /// has just validated. Returns whether this call admitted a new Close. A target that is already
    /// fenced, launched as a non-interactive program, not the only Presentation of its Panel, or no
    /// longer the retained Started incarnation keeps its settled output.
    pub(crate) fn close_after_normal_exit(
        &mut self,
        target: BindingViewerTarget,
        request_id: impl FnOnce() -> Option<WorkbenchRequestId>,
        worker: Option<WorkerPlacement>,
        owner_available: bool,
        now: Instant,
    ) -> bool {
        if self.target_lifecycle(target) != TargetLifecycle::Unfenced {
            return false;
        }
        let Some(retained) = self
            .terminals
            .retained_create_for_binding(target.terminal_id, target.worker.worker_id)
        else {
            return false;
        };
        let started = matches!(
            retained.phase,
            TerminalPhase::Started { identity, .. } if identity == target.identity
        );
        if !started
            || retained.worker_generation != target.worker.generation
            || !retained.interactive_shell
        {
            return false;
        }
        let Some(snapshot) = self
            .workbench
            .as_ref()
            .map(|authority| authority.snapshot())
        else {
            return false;
        };
        let Some((terminal_context_id, tab_id, panel_id)) =
            target_binding(target).and_then(|binding| sole_primary(snapshot, &binding))
        else {
            return false;
        };
        let (session_id, expected_revision) = (snapshot.session_id.clone(), snapshot.revision);
        let Some(request_id) = request_id() else {
            return false;
        };
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id,
            expected_revision,
            action: WorkbenchAction::PanelClose {
                terminal_context_id,
                tab_id,
                panel_id,
            },
        };
        self.execute_terminal_tab_close(command, worker, owner_available, now)
            .is_some_and(|result| {
                matches!(
                    result.outcome,
                    WorkbenchCommandOutcome::PanelCloseAccepted { .. }
                        | WorkbenchCommandOutcome::TabCloseAccepted { .. }
                )
            })
    }
}

/// The Panel whose primary Presentation is the only Presentation and is bound to `binding`.
fn sole_primary(
    snapshot: &Snapshot,
    binding: &PresentationBinding,
) -> Option<(WorkbenchTerminalContextId, WorkbenchTabId, WorkbenchPanelId)> {
    fn visit<'snapshot>(
        node: &'snapshot PanelNode,
        binding: &PresentationBinding,
    ) -> Option<&'snapshot WorkbenchPanelId> {
        match node {
            PanelNode::Panel {
                child_presentations,
                panel_id,
                primary_presentation,
                ..
            } => (child_presentations.is_empty()
                && primary_presentation.binding.as_ref() == Some(binding))
            .then_some(panel_id),
            PanelNode::Split { children, .. } => {
                children.iter().find_map(|slot| visit(&slot.node, binding))
            }
        }
    }
    snapshot.terminal_contexts.iter().find_map(|context| {
        context.tabs.iter().find_map(|tab| {
            visit(&tab.root, binding).map(|panel_id| {
                (
                    context.terminal_context_id.clone(),
                    tab.tab_id.clone(),
                    panel_id.clone(),
                )
            })
        })
    })
}
