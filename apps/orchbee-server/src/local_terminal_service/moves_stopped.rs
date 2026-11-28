//! The failed move that stopped a Worker generation's moves, which the handshake `query` answer
//! reports while that generation serves: a replacement generation moves again, so its turns speak
//! for themselves.

use super::LocalTerminalService;
use crate::terminal_coordinator::WorkerPlacement;
use orchbee_capability_node_protocol::TerminalMoveToken;
use std::sync::Mutex;

/// The Worker generation a failed move stopped, with the step and reason its last turn answered.
#[derive(Default)]
pub(super) struct MovesStopped(
    Mutex<Option<(WorkerPlacement, TerminalMoveToken, TerminalMoveToken)>>,
);

impl LocalTerminalService {
    /// Records that a move failed at `step` for `reason`, which stopped the moves of `worker`.
    pub(crate) fn record_moves_stopped(
        &self,
        worker: WorkerPlacement,
        step: TerminalMoveToken,
        reason: TerminalMoveToken,
    ) {
        if let Ok(mut stopped) = self.moves_stopped.0.lock() {
            *stopped = Some((worker, step, reason));
        }
    }

    /// The step and reason of the failed move that stopped the moves of the Worker generation that
    /// serves now, if one did.
    pub(crate) fn moves_stopped(&self) -> Option<(TerminalMoveToken, TerminalMoveToken)> {
        let serving = WorkerPlacement::of(&self.workers.lock().ok()?.request_handle()?)?;
        let (worker, step, reason) = (*self.moves_stopped.0.lock().ok()?)?;
        (worker == serving).then_some((step, reason))
    }
}
