//! Lifetime of the Agent board's read lane. The composition owns the lane thread: it starts one
//! on demand for the open Board episode, carries the Board's typed requests to it, returns its
//! answers in delivery order, and retires it when the episode ends, at connection release, and at
//! process exit. The Board itself never holds a lane.

use std::{io, time::Instant};

use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use crate::workbench_client::agent_board::{
    AgentBoard, AgentBoardEpisodeToken, AgentBoardReadEvent, AgentBoardReadRefusal,
    AgentBoardReadRequest,
};
use crate::workbench_client::agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome};

#[cfg(test)]
mod tests;

/// The Board's read lane and the retired lanes whose threads are still exiting.
///
/// Invariants:
/// - At most one lane thread is alive. A lane starts only after every retired predecessor's
///   thread has ended, and nothing on the UI path joins a running thread.
/// - The live lane serves exactly one open Board episode. Once that episode ends, `follow`
///   interrupts the lane and keeps its handle in `retired` until the thread ends, so the thread is
///   neither joined on the UI path nor detached.
/// - `next_event` returns every answer the live lane queued before it reports the lane's end.
/// - `request_stop` and `stop_before` cover the live and retired lanes under the shared Client
///   exit budget; `release` joins them all before the connection's mailbox retires.
pub(in crate::workbench_client::ui) struct BoardReads {
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    live: Option<(AgentBoardEpisodeToken, AgentReadLane)>,
    retired: Vec<AgentReadLane>,
}

impl BoardReads {
    pub(in crate::workbench_client::ui) const fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            live: None,
            retired: Vec::new(),
        }
    }

    /// The origin that later lanes connect through; a live lane keeps its own.
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.origin = Some(origin);
    }

    /// Closes `board` and retires the lane that served its episode in the same step.
    pub(in crate::workbench_client::ui) fn close_board(&mut self, board: &mut AgentBoard) {
        board.close();
        self.follow(board.open_episode());
    }

    /// Retires the live lane unless it serves `open`, the Board's current open episode.
    pub(super) fn follow(&mut self, open: Option<AgentBoardEpisodeToken>) {
        if self
            .live
            .as_ref()
            .is_some_and(|(episode, _)| Some(*episode) != open)
            && let Some((_, mut lane)) = self.live.take()
        {
            // Cancel order: the episode change already invalidated the lane's answers and stopped
            // scheduling; now the in-flight exchange is interrupted and the thread asked to exit.
            lane.interrupt();
            self.retired.push(lane);
        }
    }

    /// Hands one request to the lane serving its episode, starting that lane on demand.
    pub(super) fn issue(
        &mut self,
        request: AgentBoardReadRequest,
    ) -> Result<(), AgentBoardReadRefusal> {
        let episode = request.episode();
        self.follow(Some(episode));
        if self.live.is_none() {
            // Release ended predecessors first, then hold the start while any predecessor thread
            // is still exiting.
            self.retired.retain(|lane| !lane.finished());
            if !self.retired.is_empty() {
                return Err(AgentBoardReadRefusal::LaneUnavailable);
            }
            let origin = self
                .origin
                .clone()
                .ok_or(AgentBoardReadRefusal::LaneUnavailable)?;
            let lane = AgentReadLane::spawn(origin, self.clock.clone())
                .map_err(|_| AgentBoardReadRefusal::LaneUnavailable)?;
            self.live = Some((episode, lane));
        }
        let sent = self
            .live
            .as_ref()
            .is_some_and(|(_, lane)| lane.send(LaneCommand::Board(request)));
        if sent {
            return Ok(());
        }
        // The thread is gone. Keep the handle until it has ended; the next request starts a
        // fresh lane after that.
        self.retired.extend(self.live.take().map(|(_, lane)| lane));
        Err(AgentBoardReadRefusal::Rejected)
    }

    /// The next read event in delivery order: each answer the live lane queued, then the lane's
    /// end once its thread has exited. The ended lane is joined and released here.
    pub(super) fn next_event(&mut self) -> Option<AgentBoardReadEvent> {
        let (_, lane) = self.live.as_mut()?;
        // Observe the end before the drain: an ended thread queues nothing more, so an empty
        // drain after it proves the last answer was already returned.
        let ended = lane.finished();
        if let Some(outcome) = lane.take_outcome() {
            return Some(match outcome {
                LaneOutcome::Board(answer) => AgentBoardReadEvent::Answer(answer),
                _ => AgentBoardReadEvent::Foreign,
            });
        }
        if !ended {
            return None;
        }
        if let Some((_, lane)) = self.live.take() {
            lane.close();
        }
        Some(AgentBoardReadEvent::LaneEnded)
    }

    /// Process exit: asks the live and retired lane threads to stop without waiting.
    pub(in crate::workbench_client::ui) fn request_stop(&mut self) {
        self.live
            .iter_mut()
            .map(|(_, lane)| lane)
            .chain(&mut self.retired)
            .for_each(AgentReadLane::interrupt);
    }

    /// Process exit: observes every lane thread's end against the shared deadline. Every lane is
    /// observed even after a failure; the first failure is reported.
    pub(in crate::workbench_client::ui) fn stop_before(
        &mut self,
        deadline: Instant,
    ) -> io::Result<()> {
        self.live
            .iter_mut()
            .map(|(_, lane)| lane)
            .chain(&mut self.retired)
            .map(|lane| lane.stop_before(deadline))
            .fold(Ok(()), io::Result::and)
    }

    /// Connection release: interrupts and joins the live and retired lanes. The wake write ends
    /// an in-flight exchange, so each join waits only for the thread to leave its loop.
    pub(super) fn release(&mut self) {
        if let Some((_, lane)) = self.live.take() {
            lane.close();
        }
        for lane in self.retired.drain(..) {
            lane.close();
        }
    }
}
