//! Hands the bindings of a lost Worker to the replacement Worker that adopts its Host, each with
//! the Close in flight on it, and moves them and those Closes into the replacement's generation
//! once it adopted. It sends nothing to a Worker.

use orchbee_capability_node_protocol::{AdoptBinding, AdoptCloseInFlight, TerminalId};

use super::SessionCoordinator;
use crate::terminal_coordinator::{AdoptionImport, WorkerPlacement};

impl SessionCoordinator {
    /// What a replacement of `lost` is asked to adopt: each settled binding of `lost`, with the
    /// Close that issued a step to it in its current attempt.
    pub(crate) fn adoption_bindings(&self, lost: WorkerPlacement) -> Vec<AdoptBinding> {
        let mut bindings = self.terminals.adoption_bindings(lost);
        if let Some(authority) = self.workbench.as_ref() {
            for binding in &mut bindings {
                binding.close = authority
                    .terminal_tab_close_in_flight(
                        lost.worker_id,
                        lost.generation,
                        binding.terminal_id,
                    )
                    .map(|close| AdoptCloseInFlight {
                        close_operation_id: close.close_operation_id,
                        attempt_generation: close.attempt_generation,
                        step_ordinal: close.step_ordinal,
                        step_effect_unknown: close.step_effect_unknown,
                    });
            }
        }
        bindings
    }

    /// Moves the bindings of `lost` that `sent` names into `replacement`'s generation once
    /// `replacement` adopted them, and the Closes retained on them; a binding `missing` names
    /// becomes unavailable instead.
    pub(crate) fn import_adopted(
        &mut self,
        lost: WorkerPlacement,
        replacement: WorkerPlacement,
        sent: &[TerminalId],
        missing: &[TerminalId],
    ) -> AdoptionImport {
        let import = self
            .terminals
            .import_adopted(lost, replacement, sent, missing);
        if let Some(authority) = self.workbench.as_mut() {
            let imported: Vec<TerminalId> = sent
                .iter()
                .filter(|terminal| !missing.contains(terminal))
                .copied()
                .collect();
            authority.move_terminal_tab_close_targets(
                lost.worker_id,
                lost.generation,
                replacement.generation,
                &imported,
            );
        }
        import
    }
}
