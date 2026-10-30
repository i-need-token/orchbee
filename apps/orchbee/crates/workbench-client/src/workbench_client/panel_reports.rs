//! Windows selected-Presentation observations on an isolated, cancellable read lane.
use super::{
    ClientPanel, ClientPresentation,
    agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome},
    state::WorkbenchClientState,
};
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use std::time::{Duration, Instant};

// Selected presentations include Host process/output detection with no event source. Keep this
// bounded physical sample; canonical metadata is also refreshed on normal view changes.
const INTERVAL: Duration = Duration::from_secs(1);
const DEADLINE: Duration = Duration::from_secs(5);
pub(crate) use super::agent_read_lane::panel_reports::PanelReport;
pub(super) use super::agent_read_lane::panel_reports::{Round, Scope, SelectedPanel};
#[derive(Clone, Copy)]
pub(crate) enum PanelReportView<'a> {
    Pending,
    Unavailable,
    Ready(&'a PanelReport),
}

pub(super) struct PanelReports {
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    lane: Option<AgentReadLane>,
    scope: Option<Scope>,
    round: Option<Round>,
    unavailable: bool,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next_read: Option<Instant>,
    /// An Agent change arrived while a read was in flight.
    expedited: bool,
    stopping: bool,
}
impl PanelReports {
    #[cfg(test)]
    pub(in crate::workbench_client) fn publish_for_test(&mut self, round: Round) {
        self.scope = Some(round.scope.clone());
        self.round = Some(round);
        self.unavailable = false;
    }
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            lane: None,
            scope: None,
            round: None,
            unavailable: false,
            epoch: 0,
            inflight: None,
            next_read: None,
            expedited: false,
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
        self.scope = None;
        self.round = None;
        self.inflight = None;
        self.stopping = false;
        self.unavailable = true;
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
    pub(super) fn matches(&self, state: &WorkbenchClientState) -> bool {
        self.scope.is_some() && self.scope == Scope::capture(state)
    }
    pub(super) fn view(
        &self,
        panel: &ClientPanel,
        presentation: &ClientPresentation,
    ) -> PanelReportView<'_> {
        let Some(selected) = SelectedPanel::capture(panel, presentation) else {
            return PanelReportView::Unavailable;
        };
        if self
            .scope
            .as_ref()
            .is_none_or(|s| !s.panels.contains(&selected))
        {
            return PanelReportView::Pending;
        }
        if self.unavailable {
            return PanelReportView::Unavailable;
        }
        self.round
            .as_ref()
            .and_then(|r| r.rows.get(panel.panel_id()))
            .map_or(PanelReportView::Pending, PanelReportView::Ready)
    }
    fn fail(&mut self) -> bool {
        let changed = !self.unavailable || self.round.is_some();
        self.unavailable = true;
        self.round = None;
        changed
    }
    fn interrupt(&mut self) {
        self.stopping = self.lane.is_some();
        if let Some(lane) = &mut self.lane {
            lane.interrupt();
        }
    }
    /// Reads now, or right after the in-flight read, instead of awaiting the interval.
    pub(super) fn expedite(&mut self) {
        if self.inflight.is_some() {
            self.expedited = true;
        } else {
            self.next_read = None;
        }
    }
    pub(super) fn tick(&mut self, scope: Option<Scope>) -> bool {
        let now = self.clock.now();
        let mut changed = false;
        if self.scope != scope {
            self.interrupt();
            self.inflight = None;
            self.scope = scope;
            self.round = None;
            self.unavailable = false;
            self.next_read = None;
            changed = true;
        }
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::PanelReports { epoch, result } = outcome else {
                self.interrupt();
                changed |= self.fail();
                continue;
            };
            let Some((expected, deadline)) = self.inflight else {
                continue;
            };
            if expected != epoch || self.stopping {
                continue;
            }
            self.inflight = None;
            self.next_read = if std::mem::take(&mut self.expedited) {
                None
            } else {
                now.checked_add(INTERVAL)
            };
            match result {
                Ok(round) if self.scope.as_ref() == Some(&round.scope) && now < deadline => {
                    changed |= self.unavailable || self.round.as_ref() != Some(&round);
                    self.unavailable = false;
                    self.round = Some(round);
                }
                _ => changed |= self.fail(),
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.stopping = false;
            if self.inflight.take().is_some() {
                changed |= self.fail();
                self.next_read = now.checked_add(INTERVAL);
            }
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) && !self.stopping {
            self.interrupt();
            changed |= self.fail();
        }
        if self.inflight.is_some() || self.stopping || self.next_read.is_some_and(|next| now < next)
        {
            return changed;
        }
        let (Some(scope), Some(origin)) = (self.scope.clone(), self.origin.clone()) else {
            return changed;
        };
        if scope.panels.is_empty() {
            return changed;
        }
        if self.lane.is_none() {
            match AgentReadLane::spawn(origin, self.clock.clone()) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    self.next_read = now.checked_add(INTERVAL);
                    return changed | self.fail();
                }
            }
        }
        let (Some(epoch), Some(deadline)) = (self.epoch.checked_add(1), now.checked_add(DEADLINE))
        else {
            self.close();
            return true;
        };
        self.epoch = epoch;
        if self.lane.as_ref().is_some_and(|l| {
            l.send(LaneCommand::PanelReports {
                epoch,
                scope,
                deadline,
            })
        }) {
            self.inflight = Some((epoch, deadline));
        } else {
            self.interrupt();
            changed |= self.fail();
        }
        changed
    }
}
impl Drop for PanelReports {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
