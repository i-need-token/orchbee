//! Lifecycle of the isolated physical observation reader; the state owner stays pure.
use super::physical::PhysicalAttentionState;
use super::{
    AgentReadLane, Instant, LaneCommand, LaneOutcome, LocalClientOrigin, LocalOperationClock,
    PhysicalRound, READ_BUDGET, READ_INTERVAL,
};
use crate::workbench_client::agent_notifications::{
    AgentNotificationCommand, AgentNotificationIdentity, AgentNotificationPolicy,
    AgentNotificationScheduler, AgentSoundPolicy,
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_local_runtime::{AgentSoundSetting, NotificationDelivery, NotificationSettings};

pub(super) struct PhysicalObserver {
    pub(super) state: PhysicalAttentionState,
    scheduler: AgentNotificationScheduler,
    notification_delivery: NotificationDelivery,
    clock: LocalOperationClock,
    time_origin: Instant,
    origin: Option<LocalClientOrigin>,
    changes: Option<crate::local_client::subscription::ChangeSubscription>,
    dirty: bool,
    lane: Option<AgentReadLane>,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next_read: Option<Instant>,
    stopping: bool,
}

impl PhysicalObserver {
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        let time_origin = clock.now();
        let mut scheduler = AgentNotificationScheduler::default();
        scheduler.configure(
            0,
            AgentNotificationPolicy {
                popup_enabled: false,
                ..AgentNotificationPolicy::default()
            },
        );
        Self {
            state: PhysicalAttentionState::default(),
            scheduler,
            notification_delivery: NotificationDelivery::Off,
            clock,
            time_origin,
            origin: None,
            changes: None,
            dirty: true,
            lane: None,
            epoch: 0,
            inflight: None,
            next_read: None,
            stopping: false,
        }
    }

    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.changes = origin.watch_changes().ok();
        self.dirty = true;
        self.origin = Some(origin);
        self.next_read = None;
    }

    pub(super) fn close(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        self.origin = None;
        self.changes = None;
        self.inflight = None;
        self.stopping = false;
        self.unavailable();
    }
    /// Process exit: asks the lane thread to stop without waiting.
    pub(super) fn request_stop(&mut self) {
        if let Some(lane) = &mut self.lane {
            lane.interrupt();
        }
    }

    /// Process exit: observes the lane thread's end against the shared deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.lane
            .as_mut()
            .map_or(Ok(()), |lane| lane.stop_before(deadline))
    }

    pub(super) fn configure_notifications(
        &mut self,
        revision: u64,
        settings: &NotificationSettings,
    ) -> bool {
        let delivery_changed = self.notification_delivery != settings.agent_delivery;
        self.notification_delivery = settings.agent_delivery;
        let policy = AgentNotificationPolicy {
            delay_millis: settings.delay_seconds.saturating_mul(1000),
            popup_enabled: settings.agent_delivery != NotificationDelivery::Off,
            sound_enabled: settings.sound_enabled,
            agent_sounds: settings
                .agent_sounds()
                .iter()
                .map(|(agent, setting)| {
                    (
                        agent.clone(),
                        match setting {
                            AgentSoundSetting::Default => AgentSoundPolicy::Default,
                            AgentSoundSetting::On => AgentSoundPolicy::On,
                            AgentSoundSetting::Off => AgentSoundPolicy::Off,
                        },
                    )
                })
                .collect(),
        };
        self.scheduler.configure(revision, policy) || delivery_changed
    }

    pub(super) fn notification_requires_card(&self) -> bool {
        self.notification_delivery == NotificationDelivery::Tui
    }

    pub(super) fn take_notification(
        &mut self,
        sink_available: bool,
    ) -> Option<(NotificationDelivery, AgentNotificationCommand)> {
        let now = self.now_millis()?;
        self.scheduler
            .take_ready(now, sink_available)
            .map(|command| (self.notification_delivery, command))
    }

    pub(super) fn after_draw(&mut self) -> bool {
        self.scheduler.after_draw()
    }

    pub(super) fn acknowledge(
        &mut self,
        paint: &super::physical::PhysicalPaint,
        focused: Option<bool>,
        visible: impl FnMut(&AgentLaunchBinding) -> bool,
    ) -> bool {
        let versions = self.state.painted_versions(paint, visible);
        let acknowledged = self.state.acknowledge_painted_versions(&versions, focused);
        let mut changed = !acknowledged.is_empty();
        for (binding, sequence) in versions {
            let identity = AgentNotificationIdentity {
                presentation_id: binding.presentation_id.as_str().to_owned(),
                generation: binding.generation.as_str().to_owned(),
                host_epoch: binding.host_epoch.as_str().to_owned(),
                host_terminal_id: binding.host_terminal_id.as_str().to_owned(),
            };
            changed |= if focused == Some(false) {
                self.scheduler.suppress_painted(&identity, sequence.value())
            } else {
                self.scheduler.acknowledge(&identity, sequence.value())
            };
        }
        changed
    }

    pub(super) fn publish(&mut self, round: &PhysicalRound) -> Result<bool, ()> {
        let now = self.now_millis().ok_or(())?;
        let mut state = self.state.clone();
        let state_changed = state.publish(round)?;
        let observations = state.notification_observations().ok_or(())?;
        let mut scheduler = self.scheduler.clone();
        let notification_changed = scheduler.observe(now, observations)?;
        self.state = state;
        self.scheduler = scheduler;
        Ok(state_changed || notification_changed)
    }

    fn unavailable(&mut self) -> bool {
        self.state.unavailable() | self.scheduler.unavailable()
    }

    fn now_millis(&self) -> Option<u64> {
        self.clock
            .now()
            .checked_duration_since(self.time_origin)?
            .as_millis()
            .try_into()
            .ok()
    }

    pub(super) fn tick(&mut self) -> bool {
        let now = self.clock.now();
        if self
            .changes
            .as_ref()
            .is_some_and(|changes| changes.changed().unwrap_or(true))
        {
            self.dirty = true;
        }
        let mut changed = false;
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::PhysicalAttention { epoch, result } = outcome else {
                self.stopping = true;
                if let Some(lane) = &mut self.lane {
                    lane.interrupt();
                }
                changed |= self.unavailable();
                continue;
            };
            if self.inflight.map(|pending| pending.0) != Some(epoch) {
                continue;
            }
            let late = self.inflight.is_some_and(|(_, deadline)| now >= deadline);
            self.inflight = None;
            self.next_read = now.checked_add(READ_INTERVAL);
            if self.stopping || late {
                changed |= self.unavailable();
                continue;
            }
            match result {
                Ok(round) => match self.publish(&round) {
                    Ok(updated) => {
                        changed |= updated;
                    }
                    Err(()) => {
                        changed |= self.unavailable();
                    }
                },
                Err(_) => {
                    self.dirty = true;
                    changed |= self.unavailable();
                }
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.inflight = None;
            self.stopping = false;
            self.next_read = now.checked_add(READ_INTERVAL);
            self.dirty = true;
            changed |= self.unavailable();
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) && !self.stopping {
            self.stopping = true;
            if let Some(lane) = &mut self.lane {
                lane.interrupt();
            }
            changed |= self.unavailable();
        }
        if !self.dirty
            || self.inflight.is_some()
            || self.stopping
            || self.next_read.is_some_and(|next| now < next)
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
                    return changed | self.unavailable();
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
            self.dirty = false;
            self.inflight = Some((epoch, deadline));
        } else {
            self.stopping = true;
            if let Some(lane) = &mut self.lane {
                lane.interrupt();
            }
            changed |= self.unavailable();
        }
        changed
    }
}
impl Drop for PhysicalObserver {
    fn drop(&mut self) {
        self.close();
    }
}
