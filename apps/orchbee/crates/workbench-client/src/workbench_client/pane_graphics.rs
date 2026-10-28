//! Complete immutable canvases, visible source scope and one cancellable read owner.
use super::{
    ClientPanel, ClientPresentation,
    agent_read_lane::{AgentReadLane, LaneCommand, LaneOutcome},
    state::WorkbenchClientState,
};
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_LAYERS, MAXIMUM_GRAPHICS_LAYERS_PER_PANE, MAXIMUM_GRAPHICS_RESIDENT_BYTES,
    PaneGraphicsLayer, PaneGraphicsTarget, PresentationBinding, SessionId, Snapshot,
    WorkbenchPanelId, WorkbenchRevision, graphics_rgba_bytes,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};
const RETRY_DELAY: Duration = Duration::from_millis(100);
const DEADLINE: Duration = Duration::from_secs(5);
pub(super) const BATCH: usize = 64;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Source {
    pub(super) target: PaneGraphicsTarget,
    pub(super) binding: Option<PresentationBinding>,
}
impl Source {
    pub(super) fn capture(panel: &ClientPanel, presentation: &ClientPresentation) -> Self {
        Self {
            target: PaneGraphicsTarget {
                panel_id: panel.panel_id().clone(),
                presentation_id: Some(presentation.presentation_id().clone()),
            },
            binding: presentation.binding().map(|b| PresentationBinding {
                worker_id: b.worker_id().clone(),
                terminal_id: b.terminal_id().clone(),
            }),
        }
    }
    pub(super) fn matches(&self, snapshot: &Snapshot) -> bool {
        fn find<'a>(
            node: &'a orchbee_workbench_protocol::PanelNode,
            source: &Source,
        ) -> Option<&'a orchbee_workbench_protocol::Presentation> {
            use orchbee_workbench_protocol::PanelNode;
            match node {
                PanelNode::Panel {
                    panel_id,
                    primary_presentation,
                    child_presentations,
                    ..
                } if panel_id == &source.target.panel_id => std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|p| Some(&p.presentation_id) == source.target.presentation_id.as_ref()),
                PanelNode::Panel { .. } => None,
                PanelNode::Split { children, .. } => {
                    children.iter().find_map(|slot| find(&slot.node, source))
                }
            }
        }
        snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find_map(|tab| find(&tab.root, self))
            .is_some_and(|p| p.binding == self.binding)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Layer {
    metadata: PaneGraphicsLayer,
    rgba: Arc<Vec<u8>>,
}
impl Layer {
    pub(in crate::workbench_client) fn new(
        metadata: PaneGraphicsLayer,
        pixels: Vec<u8>,
    ) -> Option<Self> {
        (graphics_rgba_bytes(metadata.width, metadata.height).ok()? == pixels.len()).then(|| Self {
            metadata,
            rgba: Arc::new(pixels),
        })
    }
    pub(crate) fn metadata(&self) -> &PaneGraphicsLayer {
        &self.metadata
    }
    pub(crate) fn pixels(&self) -> &[u8] {
        &self.rgba
    }
    fn same(&self, other: &Self) -> bool {
        self.metadata == other.metadata && Arc::ptr_eq(&self.rgba, &other.rgba)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Canvas {
    pub(super) source: Source,
    revision: WorkbenchRevision,
    layers: Vec<Layer>,
}
impl Canvas {
    #[cfg(test)]
    pub(crate) fn fixture(
        target: PaneGraphicsTarget,
        revision: WorkbenchRevision,
        layers: Vec<(PaneGraphicsLayer, Vec<u8>)>,
    ) -> Self {
        Self::new(
            Source {
                target,
                binding: None,
            },
            revision,
            layers
                .into_iter()
                .map(|(metadata, pixels)| {
                    Layer::new(metadata, pixels).expect("valid graphics fixture")
                })
                .collect(),
        )
        .expect("valid graphics fixture")
    }
    #[cfg(test)]
    pub(crate) fn draw_for_test(&self) -> Draw {
        Draw::new(self.source.clone(), Some(self.revision))
    }
    pub(in crate::workbench_client) fn new(
        source: Source,
        revision: WorkbenchRevision,
        layers: Vec<Layer>,
    ) -> Option<Self> {
        let names = layers
            .iter()
            .map(|l| &l.metadata.layer_id)
            .collect::<BTreeSet<_>>();
        if layers.len() > MAXIMUM_GRAPHICS_LAYERS_PER_PANE
            || names.len() != layers.len()
            || layers
                .iter()
                .any(|l| l.metadata.resource_revision > revision)
            || layers.iter().map(|l| l.rgba.len()).sum::<usize>() > MAXIMUM_GRAPHICS_RESIDENT_BYTES
            || layers.windows(2).any(|p| {
                (p[0].metadata.z_index, &p[0].metadata.layer_id)
                    > (p[1].metadata.z_index, &p[1].metadata.layer_id)
            })
        {
            return None;
        }
        Some(Self {
            source,
            revision,
            layers,
        })
    }
    pub(crate) fn revision(&self) -> WorkbenchRevision {
        self.revision
    }
    pub(crate) fn layers(&self) -> &[Layer] {
        &self.layers
    }
    pub(crate) fn bytes(&self) -> usize {
        self.layers.iter().map(|l| l.rgba.len()).sum()
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.revision == other.revision && self.same_content(other)
    }
    pub(crate) fn same_content(&self, other: &Self) -> bool {
        self.source == other.source
            && self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .all(|(a, b)| a.same(b))
    }
    #[cfg(test)]
    pub(crate) fn revised_for_test(&self, revision: WorkbenchRevision) -> Self {
        Self::new(self.source.clone(), revision, self.layers.clone())
            .expect("valid revised fixture")
    }
    pub(crate) fn same_source_and_layers(&self, other: &Self) -> bool {
        self.source == other.source
            && self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .all(|(a, b)| a.metadata.layer_id == b.metadata.layer_id)
    }
    pub(crate) fn matches_draw(&self, draw: &Draw) -> bool {
        self.source == draw.source
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draw {
    pub(super) source: Source,
    pub(super) revision: Option<WorkbenchRevision>,
    transport: orchbee_workbench_protocol::PaneGraphicsTransport,
}
impl Draw {
    pub(in crate::workbench_client) fn new(
        source: Source,
        revision: Option<WorkbenchRevision>,
    ) -> Self {
        Self {
            source,
            revision,
            transport: orchbee_workbench_protocol::PaneGraphicsTransport::CellRaster,
        }
    }
    pub(crate) fn physical(mut self, revision: Option<WorkbenchRevision>) -> Self {
        self.revision = revision;
        self.transport = orchbee_workbench_protocol::PaneGraphicsTransport::Kitty;
        self
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Scope {
    pub(super) session: SessionId,
    pub(super) revision: WorkbenchRevision,
    pub(super) sources: Vec<Source>,
}
impl Scope {
    fn matches(&self, state: &WorkbenchClientState) -> bool {
        let cache = state.cache();
        if &self.session != cache.session_id() {
            return false;
        }
        let Some(active) = state.active() else {
            return self.sources.is_empty();
        };
        let Some(tab) = cache
            .tabs()
            .iter()
            .find(|tab| tab.tab_id() == active.key().anchor().tab_id())
        else {
            return false;
        };
        self.sources.iter().all(|source| {
            (0..tab.panel_count())
                .filter_map(|i| tab.panel(i))
                .find(|p| p.panel_id() == &source.target.panel_id)
                .is_some_and(|p| Source::capture(p, state.selected_presentation(p)) == *source)
        })
    }
}
pub(super) struct Job {
    pub(super) scope: Scope,
    pub(super) previous: BTreeMap<WorkbenchPanelId, Canvas>,
}
pub(super) struct Round {
    pub(super) scope: Scope,
    pub(super) rows: BTreeMap<WorkbenchPanelId, Canvas>,
}
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
    #[cfg(test)]
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
