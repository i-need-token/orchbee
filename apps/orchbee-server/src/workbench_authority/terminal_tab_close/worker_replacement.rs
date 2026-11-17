//! Carries the terminal Closes a lost Worker left in flight over to the replacement Worker that
//! adopted their Host: what each Close issued, so the replacement refuses a resend of a step the
//! lost Worker may have run, and the retained runtime targets moved into the replacement's
//! generation, so a Close continues on it. It changes no Close's progress or attempt.

use std::num::NonZeroU64;

use orchbee_capability_node_protocol::{
    LifecycleStepOrdinal, OperationId, TerminalId, WorkerGeneration, WorkerId,
};

use super::super::{RetainedRequest, WorkbenchAuthority};

/// What a Close issued for one terminal in its current attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseInFlight {
    pub close_operation_id: OperationId,
    pub attempt_generation: NonZeroU64,
    /// The highest step ordinal the attempt issued.
    pub step_ordinal: LifecycleStepOrdinal,
    /// Whether a step may have reached the Host without a known result.
    pub step_effect_unknown: bool,
}

impl WorkbenchAuthority {
    /// The Close that issued a step to terminal `terminal_id` of Worker `worker_id` in
    /// `generation`, in its current attempt.
    #[must_use]
    pub fn terminal_tab_close_in_flight(
        &self,
        worker_id: WorkerId,
        generation: WorkerGeneration,
        terminal_id: TerminalId,
    ) -> Option<WorkbenchTabCloseInFlight> {
        self.ledger.values().find_map(|entry| {
            let RetainedRequest::TabClose(record) = entry else {
                return None;
            };
            if record.is_finished() {
                return None;
            }
            let index = record.targets.iter().position(|target| {
                target.runtime.is_some_and(|runtime| {
                    runtime.worker_id == worker_id
                        && runtime.worker_generation == generation
                        && runtime.terminal_id == terminal_id
                })
            })?;
            let (step_ordinal, step_effect_unknown) = record.continuation(index)?.issued()?;
            let operation = u128::from_str_radix(record.command.request_id.as_str(), 16).ok()?;
            Some(WorkbenchTabCloseInFlight {
                close_operation_id: OperationId::from_bytes(operation.to_be_bytes()).ok()?,
                attempt_generation: NonZeroU64::new(record.generation.value())?,
                step_ordinal,
                step_effect_unknown,
            })
        })
    }

    /// Moves each retained runtime target of Worker `worker_id` in generation `lost` whose terminal
    /// `imported` names into generation `replacement`, once the replacement adopted that terminal.
    pub fn move_terminal_tab_close_targets(
        &mut self,
        worker_id: WorkerId,
        lost: WorkerGeneration,
        replacement: WorkerGeneration,
        imported: &[TerminalId],
    ) {
        for entry in self.ledger.values_mut() {
            let RetainedRequest::TabClose(record) = entry else {
                continue;
            };
            for target in &mut record.targets {
                if let Some(runtime) = target.runtime.as_mut()
                    && runtime.worker_id == worker_id
                    && runtime.worker_generation == lost
                    && imported.contains(&runtime.terminal_id)
                {
                    runtime.worker_generation = replacement;
                }
            }
        }
    }
}
