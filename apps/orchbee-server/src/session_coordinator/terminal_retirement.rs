//! Retires the terminal bindings that retained Closes ended once no committed Presentation names
//! them. It reads Close evidence and Workbench membership; it never closes, disposes or publishes.

use orchbee_capability_node_protocol::{
    LeaseExpiresAtUnixMilliseconds, TerminalId, TerminalLifecycleRequest, WorkerId,
};
use orchbee_workbench_protocol::PresentationBinding;

use super::SessionCoordinator;
use crate::terminal_coordinator::WorkerPlacement;

impl SessionCoordinator {
    /// Retires every Workbench binding whose terminal a retained Close disposed, or whose create a
    /// published Close ended without a terminal, once no committed Presentation names it. A
    /// rebound or published-away member therefore stops counting against the binding bound.
    pub(crate) fn retire_released_terminals(&mut self) {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let Some(authority) = workbench.as_ref() else {
            return;
        };
        let retained = terminals.workbench_terminal_ids();
        if retained.is_empty() {
            return;
        }
        let candidates: Vec<_> = authority
            .ended_terminal_tab_close_targets()
            .filter_map(|(binding, runtime)| {
                let (terminal_id, worker_id, identity) = match runtime {
                    Some(runtime) => (
                        runtime.terminal_id(),
                        runtime.worker_id(),
                        Some(runtime.host_identity()),
                    ),
                    None => {
                        let (terminal_id, worker_id) = physical_ids(binding)?;
                        (terminal_id, worker_id, None)
                    }
                };
                retained
                    .contains(&(terminal_id.as_bytes(), worker_id.as_bytes()))
                    .then(|| (binding.clone(), terminal_id, worker_id, identity))
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        let named = authority.terminal_restart_bindings();
        for (binding, terminal_id, worker_id, identity) in candidates {
            if !named.contains(&binding) {
                terminals.retire_closed(terminal_id, worker_id, identity);
            }
        }
    }

    /// The next retired binding whose Disposed record the current Worker still holds.
    pub(crate) fn next_worker_retirement(
        &mut self,
        serving: &[WorkerPlacement],
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Option<TerminalLifecycleRequest> {
        self.terminals.next_worker_retirement(serving, expiry)
    }
}

/// The physical terminal and Worker a Workbench binding names.
pub(super) fn physical_ids(binding: &PresentationBinding) -> Option<(TerminalId, WorkerId)> {
    let terminal = u128::from_str_radix(binding.terminal_id.as_str(), 16).ok()?;
    let worker = u128::from_str_radix(binding.worker_id.as_str(), 16).ok()?;
    Some((
        TerminalId::from_bytes(terminal.to_be_bytes()).ok()?,
        WorkerId::from_bytes(worker.to_be_bytes()).ok()?,
    ))
}
