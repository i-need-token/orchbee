//! Windows selected-Presentation observations on an isolated, cancellable read lane.
use super::agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome, ROUND_FRESH_FOR};
use super::{ClientPanel, ClientPresentation};
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use crate::workbench_state::WorkbenchClientState;
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
    round_at: Option<Instant>,
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
        self.round_at = Some(self.clock.now());
        self.unavailable = false;
    }
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            clock,
            origin: None,
            lane: None,
            scope: None,
            round: None,
            round_at: None,
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
        self.round_at = None;
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
    /// Rows answer for exact Presentations, so only another Session hides them.
    pub(super) fn matches(&self, state: &WorkbenchClientState) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| &scope.session == state.cache().session_id())
    }
    pub(super) fn view(
        &self,
        panel: &ClientPanel,
        presentation: &ClientPresentation,
    ) -> PanelReportView<'_> {
        SelectedPanel::capture(panel, presentation)
            .map_or(PanelReportView::Unavailable, |selected| {
                self.selected_view(&selected)
            })
    }
    fn selected_view(&self, selected: &SelectedPanel) -> PanelReportView<'_> {
        // A round answers for the Presentations it read at whatever revision the Client has since
        // reached, so a commit cannot blank a state glyph until the next read completes.
        if let Some(row) = self
            .round
            .as_ref()
            .filter(|round| round.scope.panels.contains(selected))
            .and_then(|round| round.rows.get(&selected.panel_id))
        {
            return PanelReportView::Ready(row);
        }
        let current = self
            .scope
            .as_ref()
            .is_some_and(|scope| scope.panels.contains(selected));
        if current && self.unavailable {
            PanelReportView::Unavailable
        } else {
            PanelReportView::Pending
        }
    }
    fn fail(&mut self, now: Instant) -> bool {
        if self
            .round_at
            .is_some_and(|at| now.saturating_duration_since(at) <= ROUND_FRESH_FOR)
        {
            return false;
        }
        let changed = !self.unavailable || self.round.is_some();
        self.unavailable = true;
        self.round = None;
        self.round_at = None;
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
            // A commit keeps the round for the Presentations it read until it ages out; another
            // Session, or a scope sharing none of them such as another Tab, has nothing to keep.
            let fresh = self
                .round_at
                .is_some_and(|at| now.saturating_duration_since(at) <= ROUND_FRESH_FOR);
            let kept = fresh
                && self.round.as_ref().is_some_and(|round| {
                    scope.as_ref().is_some_and(|scope| {
                        round.scope.session == scope.session
                            && scope
                                .panels
                                .iter()
                                .any(|panel| round.scope.panels.contains(panel))
                    })
                });
            if !kept {
                self.round = None;
                self.round_at = None;
            }
            self.scope = scope;
            self.unavailable = false;
            self.next_read = None;
            changed = true;
        }
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::PanelReports { epoch, result } = outcome else {
                self.interrupt();
                changed |= self.fail(now);
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
                    self.round_at = Some(now);
                }
                _ => changed |= self.fail(now),
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.stopping = false;
            if self.inflight.take().is_some() {
                changed |= self.fail(now);
                self.next_read = now.checked_add(INTERVAL);
            }
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) && !self.stopping {
            self.interrupt();
            changed |= self.fail(now);
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
                    return changed | self.fail(now);
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
            changed |= self.fail(now);
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
