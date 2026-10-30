//! Complete immutable canvases, visible source scope and one cancellable read owner.
use super::{
    ClientPanel, ClientPresentation,
    agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome},
    state::WorkbenchClientState,
};
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_LAYERS, MAXIMUM_GRAPHICS_RESIDENT_BYTES, WorkbenchPanelId,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
const RETRY_DELAY: Duration = Duration::from_millis(100);
const DEADLINE: Duration = Duration::from_secs(5);
#[cfg(any(test, feature = "test-support"))]
pub(super) use super::agent_read_lane::pane_graphics::Layer;
pub(super) use super::agent_read_lane::pane_graphics::{BATCH, Job, Scope, Source};
pub use super::agent_read_lane::pane_graphics::{Canvas, Draw};
pub(super) struct Driver {
    host: crate::terminal_graphics::Facts,
    clock: LocalOperationClock,
    origin: Option<LocalClientOrigin>,
    changes: Option<crate::local_client::subscription::ChangeSubscription>,
    pending_sources: usize,
    lane: Option<AgentReadLane>,
    scope: Option<Scope>,
    rows: BTreeMap<WorkbenchPanelId, Canvas>,
    draws: Vec<Draw>,
    epoch: u64,
    inflight: Option<(u64, Instant)>,
    next: Option<Instant>,
    cursor: usize,
    stopping: bool,
}
impl Driver {
    pub(super) fn observe_host(&mut self, facts: crate::terminal_graphics::Facts) {
        self.host = facts;
    }
    pub(super) fn cell_pixels(&self) -> Option<orchbee_workbench_protocol::PaneGraphicsCellPixels> {
        self.host.cell_capabilities().cell_pixels
    }
    #[cfg(any(test, feature = "test-support"))]
    pub(in crate::workbench_client) fn install_for_test(&mut self, canvas: Canvas) {
        assert!(
            self.scope
                .as_ref()
                .is_some_and(|scope| scope.sources.contains(&canvas.source))
        );
        self.rows
            .insert(canvas.source.target.panel_id.clone(), canvas);
    }
    pub(super) fn observation(
        &self,
        view: &orchbee_workbench_protocol::ClientViewObservation,
    ) -> Option<orchbee_workbench_protocol::PaneGraphicsClientObservation> {
        let scope = self
            .scope
            .as_ref()
            .filter(|scope| scope.revision == view.workbench_revision)?;
        let mut draws = self
            .draws
            .iter()
            .filter(|draw| scope.sources.contains(&draw.source))
            .collect::<Vec<_>>();
        let complete = draws.len() <= BATCH;
        let mut capabilities = self.host.cell_capabilities();
        if draws
            .iter()
            .any(|draw| draw.transport == orchbee_workbench_protocol::PaneGraphicsTransport::Kitty)
        {
            capabilities.transport = orchbee_workbench_protocol::PaneGraphicsTransport::Kitty;
        }
        draws.sort_by_key(|draw| draw.revision.is_none());
        Some(orchbee_workbench_protocol::PaneGraphicsClientObservation {
            workbench_revision: scope.revision,
            capabilities,
            complete,
            panes: draws
                .into_iter()
                .take(BATCH)
                .map(|draw| orchbee_workbench_protocol::PaneGraphicsClientPane {
                    target: draw.source.target.clone(),
                    binding: draw.source.binding.clone(),
                    drawn_revision: draw.revision,
                })
                .collect(),
        })
    }
    pub(super) fn new(clock: LocalOperationClock) -> Self {
        Self {
            host: crate::terminal_graphics::Facts::default(),
            clock,
            origin: None,
            changes: None,
            pending_sources: usize::MAX,
            lane: None,
            scope: None,
            rows: BTreeMap::new(),
            draws: Vec::new(),
            epoch: 0,
            inflight: None,
            next: None,
            cursor: 0,
            stopping: false,
        }
    }
    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        self.close();
        self.changes = origin.watch_changes().ok();
        self.pending_sources = usize::MAX;
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) {
        if let Some(lane) = self.lane.take() {
            lane.close();
        }
        self.origin = None;
        self.changes = None;
        self.pending_sources = 0;
        self.scope = None;
        self.rows.clear();
        self.draws.clear();
        self.inflight = None;
        self.next = None;
        self.stopping = false;
    }
    fn interrupt(&mut self) {
        if let Some(lane) = &mut self.lane {
            lane.interrupt();
            self.stopping = true;
        }
        self.inflight = None;
    }
    pub(super) fn observe(&mut self, mut scope: Scope, draws: Vec<Draw>) {
        scope
            .sources
            .sort_by(|a, b| a.target.panel_id.cmp(&b.target.panel_id));
        scope
            .sources
            .dedup_by(|a, b| a.target.panel_id == b.target.panel_id);
        if self
            .scope
            .as_ref()
            .is_none_or(|old| old.session != scope.session || old.sources != scope.sources)
        {
            self.interrupt();
            self.next = None;
            self.cursor = 0;
            self.pending_sources = usize::MAX;
            self.rows.retain(|_, row| {
                self.scope
                    .as_ref()
                    .is_some_and(|old| old.session == scope.session)
                    && scope.sources.contains(&row.source)
            });
        }
        self.scope = Some(scope);
        self.draws = draws;
    }
    pub(super) fn view(
        &self,
        panel: &ClientPanel,
        presentation: &ClientPresentation,
    ) -> Option<&Canvas> {
        let source = Source::capture(panel, presentation);
        self.scope
            .as_ref()
            .filter(|scope| scope.sources.contains(&source))?;
        self.rows
            .get(panel.panel_id())
            .filter(|row| row.source == source)
    }
    pub(super) fn tick(&mut self, state: Option<&WorkbenchClientState>) -> bool {
        let now = self.clock.now();
        if self
            .changes
            .as_ref()
            .is_some_and(|changes| changes.changed().unwrap_or(true))
        {
            self.pending_sources = usize::MAX;
            self.next = None;
        }
        let mut changed = false;
        if self
            .scope
            .as_ref()
            .is_some_and(|scope| state.is_none_or(|state| !scope.matches(state)))
        {
            self.interrupt();
            self.scope = None;
            self.rows.clear();
            self.draws.clear();
            self.next = None;
            changed = true;
        }
        while let Some(outcome) = self.lane.as_mut().and_then(AgentReadLane::take_outcome) {
            let LaneOutcome::Graphics { epoch, result } = outcome else {
                self.interrupt();
                continue;
            };
            let Some((expected, deadline)) = self.inflight else {
                continue;
            };
            if expected != epoch || self.stopping {
                continue;
            }
            self.inflight = None;
            self.next = None;
            if let Ok(round) = result
                && now < deadline
                && self.scope.as_ref().is_some_and(|scope| {
                    scope.session == round.scope.session
                        && round
                            .scope
                            .sources
                            .iter()
                            .all(|s| scope.sources.contains(s))
                })
            {
                let mut candidate = self.rows.clone();
                for (id, row) in round.rows {
                    candidate.insert(id, row);
                }
                if candidate.values().map(Canvas::bytes).sum::<usize>()
                    > MAXIMUM_GRAPHICS_RESIDENT_BYTES
                    || candidate.values().map(|r| r.layers.len()).sum::<usize>()
                        > MAXIMUM_GRAPHICS_LAYERS
                {
                    candidate.retain(|id, _| {
                        round.scope.sources.iter().any(|s| &s.target.panel_id == id)
                    });
                }
                if candidate.values().map(Canvas::bytes).sum::<usize>()
                    <= MAXIMUM_GRAPHICS_RESIDENT_BYTES
                    && candidate.values().map(|r| r.layers.len()).sum::<usize>()
                        <= MAXIMUM_GRAPHICS_LAYERS
                {
                    changed |= candidate.len() != self.rows.len()
                        || candidate
                            .iter()
                            .any(|(id, row)| self.rows.get(id).is_none_or(|old| !old.same(row)));
                    self.rows = candidate;
                }
            } else {
                // A failed final batch still owes its snapshot; wait for the error backoff,
                // then retry the visible scope without requiring another semantic commit.
                self.pending_sources = usize::MAX;
                self.next = now.checked_add(RETRY_DELAY);
            }
        }
        if self.lane.as_ref().is_some_and(AgentReadLane::finished) {
            self.lane.take();
            self.stopping = false;
            if self.inflight.take().is_some() {
                self.pending_sources = usize::MAX;
                self.next = now.checked_add(RETRY_DELAY);
            }
        }
        if self.inflight.is_some_and(|(_, deadline)| now >= deadline) {
            self.interrupt();
            self.pending_sources = usize::MAX;
            self.next = now.checked_add(RETRY_DELAY);
        }
        if self.pending_sources == 0
            || self.stopping
            || self.inflight.is_some()
            || self.next.is_some_and(|next| now < next)
        {
            return changed;
        }
        let (Some(scope), Some(origin)) = (self.scope.as_ref(), self.origin.clone()) else {
            return changed;
        };
        if scope.sources.is_empty() {
            return changed;
        }
        self.cursor %= scope.sources.len();
        self.pending_sources = self.pending_sources.min(scope.sources.len());
        let sources = scope
            .sources
            .iter()
            .cycle()
            .skip(self.cursor)
            .take(BATCH.min(self.pending_sources))
            .cloned()
            .collect::<Vec<_>>();
        self.cursor = (self.cursor + sources.len()) % scope.sources.len();
        self.pending_sources -= sources.len();
        let previous = self
            .rows
            .iter()
            .filter(|(_, row)| sources.contains(&row.source))
            .map(|(id, row)| (id.clone(), row.clone()))
            .collect();
        let job = Job {
            scope: Scope {
                session: scope.session.clone(),
                revision: scope.revision,
                sources,
            },
            previous,
        };
        if self.lane.is_none() {
            match AgentReadLane::spawn(origin, self.clock.clone()) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    self.pending_sources = usize::MAX;
                    self.next = now.checked_add(RETRY_DELAY);
                    return changed;
                }
            }
        }
        let (Some(epoch), Some(deadline)) = (self.epoch.checked_add(1), now.checked_add(DEADLINE))
        else {
            self.close();
            return true;
        };
        self.epoch = epoch;
        if self.lane.as_ref().is_some_and(|lane| {
            lane.send(LaneCommand::Graphics {
                epoch,
                job,
                deadline,
            })
        }) {
            self.inflight = Some((epoch, deadline));
        } else {
            self.interrupt();
            self.pending_sources = usize::MAX;
            self.next = now.checked_add(RETRY_DELAY);
        }
        changed
    }
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.close();
    }
}
