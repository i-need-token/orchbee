//! Lifecycle of the isolated physical observation reader; the state owner stays pure.
use super::physical::PhysicalAttentionState;
use super::{
    AgentReadLane, Instant, LaneCommand, LaneOutcome, LocalClientOrigin, LocalOperationClock,
    READ_BUDGET, READ_INTERVAL,
};

pub(super) struct PhysicalObserver {
    pub(super) state: PhysicalAttentionState,
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<AgentReadLane>,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next_read: Option<Instant>,
    stopping: bool,
}

impl PhysicalObserver {
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            state: PhysicalAttentionState::default(),
            clock,
            origin: None,
            lane: None,
            epoch: 0,
            inflight: None,
            next_read: None,
            stopping: false,
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
        self.state.unavailable();
    }

    pub(super) fn tick(&mut self) -> bool {
        let now = self.clock.now();
        let mut changed = false;
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::PhysicalAttention { epoch, result } = outcome else {
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
            let late = self.inflight.is_some_and(|(_, deadline)| now >= deadline);
            self.inflight = None;
            self.next_read = now.checked_add(READ_INTERVAL);
            if self.stopping || late {
                changed |= self.state.unavailable();
                continue;
            }
            match result {
                Ok(round) => match self.state.publish(&round) {
                    Ok(updated) => {
                        changed |= updated;
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
        if self
            .lane
            .as_ref()
            .is_some_and(|lane| lane.send(LaneCommand::PhysicalAttention { epoch, deadline }))
        {
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
}
impl Drop for PhysicalObserver {
    fn drop(&mut self) {
        self.close();
    }
}
