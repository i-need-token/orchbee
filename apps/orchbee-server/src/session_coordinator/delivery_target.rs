//! Where a Presentation stands when the Server delivers a line into it, a restart's resume or a
//! creation's Agent start: gone, bound without an answering terminal, or the exact started terminal
//! a delivery may reach; and whether that terminal's shell has echoed a command. It reads the
//! Workbench and the terminal registry under the coordinator lock its caller holds and decides
//! nothing about the delivery.
use super::{ProcessObservationTarget, SessionCoordinator, TargetLifecycle};
use crate::terminal_coordinator::WorkerPlacement;
use orchbee_workbench_protocol::{PresentationBinding, TerminalPresentationId};

/// Where the Presentation of a delivery stands when it is attempted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResumeTarget {
    /// The Presentation is no longer a bound member of the Workbench.
    Gone,
    /// Bound as this, but no started, unfenced physical terminal of the current Worker answers.
    Unavailable(PresentationBinding),
    /// The started physical terminal its current binding names.
    Started(ProcessObservationTarget),
}

impl SessionCoordinator {
    /// The Presentation's current binding and, when the current Worker has started it and no
    /// lifecycle fence covers it, the exact physical terminal a delivery may reach.
    pub(crate) fn agent_resume_target(
        &self,
        presentation: &TerminalPresentationId,
        current_worker: Option<WorkerPlacement>,
    ) -> ResumeTarget {
        let Some(workbench) = self.workbench.as_ref() else {
            return ResumeTarget::Gone;
        };
        let binding = workbench
            .snapshot()
            .terminal_contexts
            .iter()
            .filter_map(|context| {
                workbench
                    .presentation_membership(&context.terminal_context_id)
                    .ok()
            })
            .find_map(|membership| {
                membership
                    .members()
                    .iter()
                    .find(|member| member.presentation_id() == presentation)
                    .map(|member| member.binding().clone())
            });
        let Some(binding) = binding else {
            return ResumeTarget::Gone;
        };
        let parse = |text: &str| u128::from_str_radix(text, 16).ok().map(u128::to_be_bytes);
        let target = parse(binding.terminal_id.as_str())
            .zip(parse(binding.worker_id.as_str()))
            .and_then(|(terminal, worker)| {
                Some((
                    orchbee_capability_node_protocol::TerminalId::from_bytes(terminal).ok()?,
                    orchbee_capability_node_protocol::WorkerId::from_bytes(worker).ok()?,
                ))
            })
            .and_then(|(terminal, worker)| {
                self.terminals
                    .binding_send_target(terminal, worker, current_worker)
                    .ok()
            })
            .filter(|target| self.target_lifecycle(*target) == TargetLifecycle::Unfenced);
        match target {
            Some(target) => ResumeTarget::Started(ProcessObservationTarget {
                session_id: workbench.session_id().clone(),
                binding,
                target,
            }),
            None => ResumeTarget::Unavailable(binding),
        }
    }

    /// Whether the shell of this exact Terminal generation has echoed any command.
    pub(crate) fn agent_resume_command_observed(&self, target: &ProcessObservationTarget) -> bool {
        self.latest_command(&orchbee_server::TerminalAgentBinding {
            binding: target.binding.clone(),
            worker_generation: target.target.worker.generation,
            identity: target.target.identity,
        })
        .is_some()
    }
}
