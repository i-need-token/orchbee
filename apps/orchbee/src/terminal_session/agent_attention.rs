//! Windows Client attention: one bounded observation lane and local paint acknowledgements.
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use super::{
    agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome},
    state::PaneRef,
};
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_agent_observation_protocol::AgentChangesCursor;

mod state;
use state::{AttentionPaint, AttentionState};

const READ_BUDGET: Duration = Duration::from_secs(5);
const READ_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeAgentAttentionDraw {
    paint: AttentionPaint,
    pub(super) panes: Vec<(PaneRef, String)>,
}

pub(super) struct AgentAttention {
    state: AttentionState,
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<AgentReadLane>,
    cursor: Option<AgentChangesCursor>,
    revision: Option<u64>,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next_read: Option<Instant>,
    stopping: bool,
    redraw: bool,
}

impl AgentAttention {
    #[cfg(test)]
    pub(in crate::terminal_session) fn publish_for_test(
        &mut self,
        round: &super::agent_read_lane::AttentionRound,
    ) {
        self.state.publish(round).unwrap();
    }
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            state: AttentionState::default(),
            clock,
            origin: None,
            lane: None,
            cursor: None,
            revision: None,
            epoch: 0,
            inflight: None,
            next_read: None,
            stopping: false,
            redraw: false,
        }
    }

    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.origin = Some(origin);
        self.next_read = None;
    }

    pub(super) fn close(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        self.origin = None;
        self.inflight = None;
        self.stopping = false;
        self.redraw |= self.state.unavailable();
    }

    pub(super) fn focus_changed(&mut self) {
        self.redraw = true;
    }

    pub(super) fn tick(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = std::mem::take(&mut self.redraw);
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::Attention { epoch, result } = outcome else {
                self.stopping = true;
                if let Some(lane) = &mut self.lane {
                    lane.interrupt();
                }
                changed |= self.state.unavailable();
                continue;
            };
            if self.inflight.map(|pending| pending.0) != Some(epoch) {
                continue;
            }
            self.inflight = None;
            self.next_read = now.checked_add(READ_INTERVAL);
            if self.stopping {
                continue;
            }
            match result {
                Ok(round) => match self.state.publish(&round) {
                    Ok(updated) => {
                        changed |= updated;
                        self.cursor = Some(round.cursor);
                        self.revision = Some(round.revision);
                    }
                    Err(()) => {
                        changed |= self.state.unavailable();
                    }
                },
                Err(_) => {
                    changed |= self.state.unavailable();
                }
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.inflight = None;
            self.stopping = false;
            self.next_read = now.checked_add(READ_INTERVAL);
            changed |= self.state.unavailable();
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) && !self.stopping {
            self.stopping = true;
            if let Some(lane) = &mut self.lane {
                lane.interrupt();
            }
            changed |= self.state.unavailable();
        }
        if self.inflight.is_some() || self.stopping || self.next_read.is_some_and(|next| now < next)
        {
            return changed;
        }
        let Some(origin) = self.origin.clone() else {
            return changed;
        };
        if self.lane.is_none() {
            match AgentReadLane::spawn(origin, self.clock.clone()) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    self.next_read = now.checked_add(READ_INTERVAL);
                    return changed | self.state.unavailable();
                }
            }
        }
        let (Some(epoch), Some(deadline)) =
            (self.epoch.checked_add(1), now.checked_add(READ_BUDGET))
        else {
            self.close();
            return true;
        };
        self.epoch = epoch;
        if self.lane.as_ref().is_some_and(|lane| {
            lane.send(LaneCommand::Attention {
                epoch,
                cursor: self.cursor.clone(),
                revision: self.revision,
                deadline,
            })
        }) {
            self.inflight = Some((epoch, deadline));
        } else {
            self.stopping = true;
            if let Some(lane) = &mut self.lane {
                lane.interrupt();
            }
            changed |= self.state.unavailable();
        }
        changed
    }

    pub(super) fn unseen_in(&self, presentation: &str) -> usize {
        self.state.unseen_in(presentation)
    }

    pub(super) fn paint(&self, panes: Vec<(PaneRef, String)>) -> Option<NativeAgentAttentionDraw> {
        let presentations: BTreeSet<_> = panes
            .iter()
            .map(|(_, presentation)| presentation.as_str())
            .collect();
        let paint = self.state.paint(&presentations)?;
        Some(NativeAgentAttentionDraw { paint, panes })
    }

    pub(super) fn acknowledge(
        &mut self,
        draw: &NativeAgentAttentionDraw,
        focused: Option<bool>,
        visible: &BTreeSet<&str>,
    ) {
        self.redraw |= self
            .state
            .acknowledge(&draw.paint, focused, |presentation| {
                visible.contains(presentation)
            });
    }
}

impl Drop for AgentAttention {
    fn drop(&mut self) {
        self.close();
    }
}
