//! What a confirmation decision reads from committed state for one sampled terminal, and the
//! admission of the key or hold the Server then sends to that exact terminal. It reads under the
//! coordinator lock its caller holds and decides nothing about a confirmation.
use super::process_observation::Selected;
use super::{SessionCoordinator, TargetLifecycle};
use crate::terminal_coordinator::{BindingViewerTarget, SendOperationIdentities, WorkerPlacement};
use orchbee_server::TerminalAgentBinding;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

fn hex(bytes: [u8; 16]) -> String {
    format!("{:032x}", u128::from_be_bytes(bytes))
}

impl SessionCoordinator {
    /// The registered fact of the exact physical terminal `target` names, if it is still bound.
    fn confirmation_physical(&self, target: BindingViewerTarget) -> Option<TerminalAgentBinding> {
        let worker = hex(target.worker.worker_id.as_bytes());
        let terminal = hex(target.terminal_id.as_bytes());
        self.terminal_agent_bindings()
            .ok()?
            .into_iter()
            .find(|fact| {
                fact.binding.worker_id.as_str() == worker
                    && fact.binding.terminal_id.as_str() == terminal
                    && fact.worker_generation == target.worker.generation
                    && fact.identity == target.identity
            })
    }

    /// The directory of the Workspace that shows `target`'s terminal, when the run the Session
    /// publishes for that terminal is `agent`. The outer `None` means another run or none is
    /// published there; the inner one, that the Workspace has no Directory.
    pub(crate) fn confirmation_root(
        &self,
        target: BindingViewerTarget,
        agent: Selected,
    ) -> Option<Option<WorkbenchDirectoryPath>> {
        let physical = self.confirmation_physical(target)?;
        if self.published_run(&physical) != Some(agent) {
            return None;
        }
        let workbench = self.workbench.as_ref()?;
        let snapshot = workbench.snapshot();
        let context = snapshot.terminal_contexts.iter().find(|context| {
            workbench
                .presentation_membership(&context.terminal_context_id)
                .is_ok_and(|membership| {
                    membership
                        .members()
                        .iter()
                        .any(|member| *member.binding() == physical.binding)
                })
        });
        Some(context.and_then(|context| {
            let workspace =
                snapshot.navigation.workspaces.iter().find(|workspace| {
                    workspace.terminal_context_id == context.terminal_context_id
                })?;
            snapshot
                .navigation
                .workspace_directory(&workspace.workspace_id)
                .map(|directory| directory.path.clone())
        }))
    }

    /// Admits one Server-decided send to the exact physical terminal a sampler read: the binding is
    /// still exposed, started on the current Worker, unfenced and not closing.
    pub(crate) fn prepare_confirmation_send(
        &mut self,
        target: BindingViewerTarget,
        current_worker: Option<WorkerPlacement>,
    ) -> Option<SendOperationIdentities> {
        let physical = self.confirmation_physical(target)?;
        let workbench = self.workbench.as_ref()?;
        if workbench.terminal_binding_exposure(workbench.session_id(), &physical.binding)
            != orchbee_server::WorkbenchBindingExposure::Exposed
            || self.terminals.binding_send_target(
                target.terminal_id,
                target.worker.worker_id,
                current_worker,
            ) != Ok(target)
            || self.binding_has_close(&physical.binding)
            || self.target_lifecycle(target) != TargetLifecycle::Unfenced
        {
            return None;
        }
        self.terminals.allocate_send_identities()
    }
}
