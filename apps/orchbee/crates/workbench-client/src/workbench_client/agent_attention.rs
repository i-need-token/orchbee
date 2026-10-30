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

mod physical;
mod physical_observer;
pub(super) use physical::PhysicalRound;
use physical_observer::PhysicalObserver;
mod state;
use state::{AttentionPaint, AttentionState};

const READ_BUDGET: Duration = Duration::from_secs(5);
const READ_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAttentionDraw {
    paint: Option<AttentionPaint>,
    physical: Option<physical::PhysicalPaint>,
    pub(super) panes: Vec<PaintedPane>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PaintedPane {
    pub(super) pane: PaneRef,
    pub(super) presentation: String,
    pub(super) route: orchbee_terminal_protocol::TerminalFrameRoute,
    pub(super) host_terminal_id: Option<String>,
}
impl PaintedPane {
    fn matches(&self, binding: &orchbee_agent_observation_protocol::AgentLaunchBinding) -> bool {
        self.presentation == binding.presentation_id.as_str()
            && self.route.host_epoch.to_text() == binding.host_epoch.as_str()
            && self.host_terminal_id.as_deref() == Some(binding.host_terminal_id.as_str())
    }
}

pub(super) struct AgentAttention {
    state: AttentionState,
    physical: PhysicalObserver,
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    changes: Option<crate::local_client::subscription::ChangeSubscription>,
    dirty: bool,
    lane: Option<AgentReadLane>,
    cursor: Option<AgentChangesCursor>,
    revision: Option<u64>,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next_read: Option<Instant>,
    /// An Agent change arrived while a read was in flight.
    expedited: bool,
    stopping: bool,
    redraw: bool,
}

impl AgentAttention {
    pub(super) fn terminal_edited(&mut self, pane: &PaintedPane) {
        self.redraw |= self.state.edit_completed(&pane.presentation);
        self.redraw |= self
            .physical
            .state
            .edit_completed(|binding| pane.matches(binding));
    }

    pub(super) fn session_completion_edited(
        &self,
        session: &orchbee_agent_observation_protocol::AgentSessionId,
        generation: orchbee_agent_observation_protocol::AgentSafeInteger,
        sequence: orchbee_agent_observation_protocol::AgentSafeInteger,
    ) -> bool {
        self.state.completion_edited(session, generation, sequence)
    }

    pub(super) fn edited_completions(
        &self,
    ) -> Vec<(
        orchbee_agent_observation_protocol::AgentLaunchBinding,
        orchbee_workbench_protocol::WorkbenchRevision,
    )> {
        self.physical.state.edited_completions()
    }

    pub(super) fn completion_edited(&self, pane: &PaintedPane) -> bool {
        self.physical
            .state
            .completion_edited(|binding| pane.matches(binding))
    }

    pub(super) fn session_unread(
        &self,
        session: &orchbee_agent_observation_protocol::AgentSessionId,
    ) -> bool {
        self.state.session_unread(session)
    }
    pub(super) fn agent_in_terminal(
        &self,
        presentation: &str,
        epoch: &str,
        terminal: &str,
    ) -> bool {
        self.physical
            .state
            .agent_in_terminal(presentation, epoch, terminal)
    }
    pub(super) fn physical_view_version(&self) -> Option<(u64, usize)> {
        self.physical.state.view_version()
    }

    pub(super) fn focus_matches(
        &self,
        target: &orchbee_agent_observation_protocol::AgentNameTarget,
        presentation: &str,
    ) -> Option<bool> {
        match target {
            orchbee_agent_observation_protocol::AgentNameTarget::Session { session_id } => {
                self.state.focus_matches(session_id, presentation)
            }
            orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding } => self
                .physical
                .state
                .focus_matches(binding)
                .map(|matches| matches && binding.presentation_id.as_str() == presentation),
        }
    }

    /// Explicit focus marks the selected Tab's current known versions seen, even unfocused.
    /// This consumes only the current snapshots; no deferred acknowledgement survives this call.
    pub(super) fn acknowledge_focused_tab(&mut self, presentations: &BTreeSet<&str>) {
        if let Some(paint) = self.state.paint(presentations) {
            self.redraw |= self
                .state
                .acknowledge(&paint, None, |id| presentations.contains(id));
        }
        if let Some(paint) = self
            .physical
            .state
            .paint(|binding| presentations.contains(binding.presentation_id.as_str()))
        {
            self.redraw |= self.physical.acknowledge(&paint, None, |binding| {
                presentations.contains(binding.presentation_id.as_str())
            });
        }
    }

    pub(super) fn configure_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_runtime::NotificationSettings,
    ) -> bool {
        self.physical.configure_notifications(revision, settings)
    }

    pub(super) fn notification_requires_card(&self) -> bool {
        self.physical.notification_requires_card()
    }

    pub(super) fn take_notification(
        &mut self,
        sink_available: bool,
    ) -> Option<(
        orchbee_local_runtime::NotificationDelivery,
        crate::workbench_client::agent_notifications::AgentNotificationCommand,
    )> {
        self.physical.take_notification(sink_available)
    }

    pub(super) fn after_draw(&mut self) -> bool {
        self.physical.after_draw()
    }
    pub(super) fn read_physical(
        &self,
        bindings: &[orchbee_agent_observation_protocol::AgentLaunchBinding],
        focused: Option<bool>,
    ) -> Option<orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot> {
        self.physical.state.snapshot(bindings, focused)
    }
    #[cfg(test)]
    pub(in crate::workbench_client) fn publish_physical_for_test(&mut self, round: &PhysicalRound) {
        self.physical.publish(round).unwrap();
    }
    pub(super) fn read(
        &self,
        sessions: &[orchbee_agent_observation_protocol::AgentSessionId],
        focused: Option<bool>,
    ) -> Option<orchbee_workbench_protocol::ClientAgentAttentionSnapshot> {
        self.state.snapshot(self.cursor.clone()?, focused, sessions)
    }
    #[cfg(test)]
    pub(in crate::workbench_client) fn publish_for_test(
        &mut self,
        round: &super::agent_read_lane::AttentionRound,
    ) {
        self.state.publish(round).unwrap();
        self.cursor = Some(round.cursor.clone());
        self.revision = Some(round.revision);
    }
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            state: AttentionState::default(),
            physical: PhysicalObserver::new(clock.clone()),
            clock,
            origin: None,
            changes: None,
            dirty: true,
            lane: None,
            cursor: None,
            revision: None,
            epoch: 0,
            inflight: None,
            next_read: None,
            expedited: false,
            stopping: false,
            redraw: false,
        }
    }

    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.physical.set_origin(origin.clone());
        self.changes = origin.watch_changes().ok();
        self.dirty = true;
        self.origin = Some(origin);
        self.next_read = None;
    }

    pub(super) fn close(&mut self) {
        self.physical.close();
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        self.origin = None;
        self.changes = None;
        self.inflight = None;
        self.stopping = false;
        self.redraw |= self.state.unavailable();
    }

    /// Process exit: asks both lane threads to stop without waiting.
    pub(super) fn request_stop(&mut self) {
        self.physical.request_stop();
        if let Some(lane) = &mut self.lane {
            lane.interrupt();
        }
    }

    /// Process exit: observes both lane threads' ends against the shared deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        let physical = self.physical.stop_before(deadline);
        let lane = self
            .lane
            .as_mut()
            .map_or(Ok(()), |lane| lane.stop_before(deadline));
        physical.and(lane)
    }

    /// Reads now, or right after the in-flight read, instead of awaiting the interval.
    pub(super) fn expedite(&mut self) {
        self.dirty = true;
        if self.inflight.is_some() {
            self.expedited = true;
        } else {
            self.next_read = None;
        }
    }

    pub(super) fn focus_changed(&mut self) {
        self.redraw = true;
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
        let mut changed = std::mem::take(&mut self.redraw) | self.physical.tick();
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
            self.next_read = if std::mem::take(&mut self.expedited) {
                None
            } else {
                now.checked_add(READ_INTERVAL)
            };
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
                    self.dirty = true;
                    changed |= self.state.unavailable();
                }
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.inflight = None;
            self.stopping = false;
            self.next_read = now.checked_add(READ_INTERVAL);
            self.dirty = true;
            changed |= self.state.unavailable();
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) && !self.stopping {
            self.stopping = true;
            if let Some(lane) = &mut self.lane {
                lane.interrupt();
            }
            changed |= self.state.unavailable();
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
            self.dirty = false;
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
        self.state
            .unseen_in(presentation)
            .max(self.physical.state.unseen_in(presentation))
    }

    pub(super) fn paint(&self, panes: Vec<PaintedPane>) -> Option<AgentAttentionDraw> {
        let presentations: BTreeSet<_> = panes
            .iter()
            .map(|pane| pane.presentation.as_str())
            .collect();
        let paint = self.state.paint(&presentations);
        let physical = self
            .physical
            .state
            .paint(|binding| panes.iter().any(|pane| pane.matches(binding)));
        (paint.is_some() || physical.is_some()).then_some(AgentAttentionDraw {
            paint,
            physical,
            panes,
        })
    }

    pub(super) fn acknowledge(
        &mut self,
        draw: &AgentAttentionDraw,
        focused: Option<bool>,
        visible: &[PaintedPane],
    ) {
        if let Some(paint) = &draw.paint {
            self.redraw |= self.state.acknowledge(paint, focused, |presentation| {
                visible.iter().any(|pane| pane.presentation == presentation)
            });
        }
        if let Some(paint) = &draw.physical {
            self.redraw |= self.physical.acknowledge(paint, focused, |binding| {
                visible.iter().any(|pane| pane.matches(binding))
            });
        }
    }
}

impl Drop for AgentAttention {
    fn drop(&mut self) {
        self.close();
    }
}
