//! Joins what the coordinator settled with the Presentation that shows each terminal and the Close
//! in flight on it, as the binding record keeps them, and gives the cheap observation the
//! checkpoint saver compares to decide when to write that record again. It writes nothing.

use std::collections::BTreeMap;

use orchbee_capability_node_protocol::{
    AdoptCloseInFlight, TerminalId, WorkerGeneration, WorkerId,
};
use orchbee_server::{WorkbenchAuthority, WorkbenchTabCloseInFlight};
use orchbee_workbench_protocol::WorkbenchRevision;

use super::SessionCoordinator;
use crate::{
    binding_record::{Capture, RecordedTerminal},
    terminal_coordinator::RecordedState,
};

/// Everything the binding record says that can change while the Session runs, short of what a
/// terminal fixed at creation: the Workbench revision covers the Presentations, and each settled
/// binding and Close in flight covers the rest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BindingObservation {
    revision: Option<WorkbenchRevision>,
    states: Vec<RecordedState>,
    closes: Vec<(
        WorkerId,
        WorkerGeneration,
        TerminalId,
        WorkbenchTabCloseInFlight,
    )>,
}

impl SessionCoordinator {
    pub(crate) fn binding_observation(&self) -> BindingObservation {
        BindingObservation {
            revision: self
                .workbench
                .as_ref()
                .map(|workbench| workbench.snapshot().revision),
            states: self.terminals.recorded_states(),
            closes: self
                .workbench
                .as_ref()
                .map(WorkbenchAuthority::terminal_tab_closes_in_flight)
                .unwrap_or_default(),
        }
    }

    /// Each settled binding of the Session Worker `worker_id` that a Presentation shows, with the
    /// Close in flight on it, and the identity the coordinator allocates next. A binding no
    /// Presentation shows is left out: its terminal was disposed and it stays only for replay.
    pub(crate) fn binding_capture(&self, worker_id: WorkerId) -> Capture {
        let (presentations, closes) = match self.workbench.as_ref() {
            Some(workbench) => (
                workbench.bound_presentations(),
                workbench.terminal_tab_closes_in_flight(),
            ),
            None => (Vec::new(), Vec::new()),
        };
        let mut shown = BTreeMap::new();
        for (binding, presentation_id, launch_program) in presentations {
            let (Ok(terminal), Ok(worker)) = (
                u128::from_str_radix(binding.terminal_id.as_str(), 16),
                u128::from_str_radix(binding.worker_id.as_str(), 16),
            ) else {
                continue;
            };
            shown
                .entry((terminal, worker))
                .or_insert((presentation_id, launch_program));
        }
        let worker = u128::from_be_bytes(worker_id.as_bytes());
        let terminals = self
            .terminals
            .recorded_bindings()
            .into_iter()
            .filter(|recorded| recorded.worker.worker_id == worker_id)
            .filter_map(|recorded| {
                let terminal = recorded.binding.terminal_id;
                let (presentation_id, launch_program) = shown
                    .get(&(u128::from_be_bytes(terminal.as_bytes()), worker))?
                    .clone();
                let close = closes
                    .iter()
                    .find(|(worker_id, generation, closing, _)| {
                        *worker_id == recorded.worker.worker_id
                            && *generation == recorded.worker.generation
                            && *closing == terminal
                    })
                    .map(|(_, _, _, close)| AdoptCloseInFlight {
                        close_operation_id: close.close_operation_id,
                        attempt_generation: close.attempt_generation,
                        step_ordinal: close.step_ordinal,
                        step_effect_unknown: close.step_effect_unknown,
                    });
                Some(RecordedTerminal {
                    binding: orchbee_capability_node_protocol::AdoptBinding {
                        close,
                        ..recorded.binding
                    },
                    geometry: recorded.geometry,
                    create_request: recorded.reference,
                    presentation_id,
                    launch_program,
                    deferred: recorded.deferred,
                    interactive_shell: recorded.interactive_shell,
                    profile: recorded.profile_provenance,
                    launch_selection: recorded.launch_selection,
                })
            })
            .collect();
        Capture {
            worker_id,
            next_identity: self.terminals.next_identity(),
            terminals,
        }
    }
}
