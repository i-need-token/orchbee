//! Whole revision-pinned resources are staged off the UI/control connections.
#[cfg(test)]
mod tests;
use super::{
    Arc, AtomicBool, Instant, LaneFailure, LocalClient, LocalClientOrigin, LocalOperationClock,
    Ordering, mint_request_id,
};
use crate::workbench_client::{ClientPanel, ClientPresentation, state::WorkbenchClientState};
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_CHUNK_BYTES, MAXIMUM_GRAPHICS_LAYERS, MAXIMUM_GRAPHICS_RESIDENT_BYTES,
    PaneGraphicsAction as Action, PaneGraphicsLayer, PaneGraphicsOutcome as Outcome,
    PaneGraphicsRequest, ProtocolVersion, Snapshot, SnapshotReadIntent, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchSnapshotOutcome, graphics_rgba_bytes,
};
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_LAYERS_PER_PANE, PaneGraphicsTarget, PresentationBinding, SessionId,
    WorkbenchPanelId,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
pub(super) fn read_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client_slot: &mut Option<LocalClient>,
    job: Job,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<Round, LaneFailure> {
    if client_slot.is_none() {
        *client_slot = Some(
            origin
                .lane_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = client_slot.as_mut().ok_or(LaneFailure::Transport)?;
    let result = Reader {
        client,
        clock,
        deadline,
        wake,
        shutdown,
        job: &job,
    }
    .read();
    if result.is_err() {
        *client_slot = None;
    }
    result.map(|rows| Round {
        scope: job.scope,
        rows,
    })
}
struct Reader<'a> {
    client: &'a mut LocalClient,
    clock: &'a LocalOperationClock,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'a>,
    shutdown: &'a Arc<AtomicBool>,
    job: &'a Job,
}
impl Reader<'_> {
    fn id(&self) -> Result<WorkbenchRequestId, LaneFailure> {
        if self.shutdown.load(Ordering::SeqCst) || self.clock.now() >= self.deadline {
            return Err(LaneFailure::Transport);
        }
        WorkbenchRequestId::parse(mint_request_id()?.as_str()).map_err(|_| LaneFailure::Transport)
    }
    fn snapshot(&mut self) -> Result<Snapshot, LaneFailure> {
        let request = SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
        };
        let reply = self
            .client
            .submit_workbench_snapshot_read_before_cancellable(
                request,
                self.deadline,
                Some(self.wake),
            )
            .map_err(|_| LaneFailure::Transport)?;
        let WorkbenchSnapshotOutcome::Available { snapshot } = reply.outcome else {
            return Err(LaneFailure::Unavailable);
        };
        if snapshot.session_id != self.job.scope.session
            || snapshot.revision < self.job.scope.revision
            || !self.job.scope.sources.iter().all(|s| s.matches(&snapshot))
        {
            return Err(LaneFailure::Unavailable);
        }
        Ok(snapshot)
    }
    fn exchange(&mut self, action: Action) -> Result<Outcome, LaneFailure> {
        let request = PaneGraphicsRequest {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
            session_id: self.job.scope.session.clone(),
            action,
        };
        let reply = self
            .client
            .pane_graphics(&request, self.deadline, Some(self.wake))
            .map_err(|_| LaneFailure::Transport)?;
        match reply.outcome {
            Outcome::Failed {
                reason: orchbee_workbench_protocol::PaneGraphicsFailure::Denied,
            } => Err(LaneFailure::Denied),
            Outcome::Failed { .. } => Err(LaneFailure::Unavailable),
            outcome => Ok(outcome),
        }
    }
    fn catalog(
        &mut self,
        source: &Source,
    ) -> Result<(WorkbenchRevision, Vec<PaneGraphicsLayer>), LaneFailure> {
        let Outcome::Catalog {
            target,
            revision,
            layers,
        } = self.exchange(Action::Catalog {
            target: source.target.clone(),
        })?
        else {
            return Err(LaneFailure::Transport);
        };
        if target != source.target {
            return Err(LaneFailure::Transport);
        }
        Ok((revision, layers))
    }
    fn resource(
        &mut self,
        source: &Source,
        metadata: &PaneGraphicsLayer,
    ) -> Result<Layer, LaneFailure> {
        if let Some(layer) = self
            .job
            .previous
            .get(&source.target.panel_id)
            .filter(|canvas| canvas.source == *source)
            .and_then(|canvas| canvas.layers().iter().find(|l| l.metadata() == metadata))
        {
            return Ok(layer.clone());
        }
        let total = graphics_rgba_bytes(metadata.width, metadata.height)
            .map_err(|_| LaneFailure::Transport)?;
        let mut pixels = Vec::with_capacity(total);
        while pixels.len() < total {
            let offset = u32::try_from(pixels.len()).map_err(|_| LaneFailure::Transport)?;
            let Outcome::Pixels {
                target,
                layer_id,
                resource_revision,
                width,
                height,
                offset: returned,
                total_bytes,
                data_base64,
                complete,
            } = self.exchange(Action::Pixels {
                target: source.target.clone(),
                layer_id: metadata.layer_id.clone(),
                resource_revision: metadata.resource_revision,
                offset,
                maximum_bytes: MAXIMUM_GRAPHICS_CHUNK_BYTES as u32,
            })?
            else {
                return Err(LaneFailure::Transport);
            };
            if target != source.target
                || layer_id != metadata.layer_id
                || resource_revision != metadata.resource_revision
                || width != metadata.width
                || height != metadata.height
                || returned != offset
                || total_bytes as usize != total
            {
                return Err(LaneFailure::Transport);
            }
            let chunk = base64::engine::general_purpose::STANDARD
                .decode(data_base64)
                .map_err(|_| LaneFailure::Transport)?;
            if chunk.is_empty()
                || chunk.len() > MAXIMUM_GRAPHICS_CHUNK_BYTES
                || pixels
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|end| end > total)
                || complete != (pixels.len() + chunk.len() == total)
            {
                return Err(LaneFailure::Transport);
            }
            pixels.extend(chunk);
        }
        Layer::new(metadata.clone(), pixels).ok_or(LaneFailure::Transport)
    }
    fn read(
        &mut self,
    ) -> Result<BTreeMap<orchbee_workbench_protocol::WorkbenchPanelId, Canvas>, LaneFailure> {
        if self.job.scope.sources.len() > BATCH {
            return Err(LaneFailure::Unavailable);
        }
        self.snapshot()?;
        let mut rows = BTreeMap::new();
        let mut bytes = 0;
        let mut resources = 0;
        for source in &self.job.scope.sources {
            let (_, metadata) = self.catalog(source)?;
            let mut layers = Vec::new();
            for layer in &metadata {
                bytes += graphics_rgba_bytes(layer.width, layer.height)
                    .map_err(|_| LaneFailure::Transport)?;
                resources += 1;
                if bytes > MAXIMUM_GRAPHICS_RESIDENT_BYTES || resources > MAXIMUM_GRAPHICS_LAYERS {
                    return Err(LaneFailure::Unavailable);
                }
                layers.push(self.resource(source, layer)?);
            }
            let (after, after_layers) = self.catalog(source)?;
            if metadata != after_layers {
                return Err(LaneFailure::Unavailable);
            }
            let canvas =
                Canvas::new(source.clone(), after, layers).ok_or(LaneFailure::Transport)?;
            rows.insert(source.target.panel_id.clone(), canvas);
        }
        self.snapshot()?;
        Ok(rows)
    }
}

pub(in crate::workbench_client) const BATCH: usize = 64;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct Source {
    pub(in crate::workbench_client) target: PaneGraphicsTarget,
    pub(in crate::workbench_client) binding: Option<PresentationBinding>,
}
impl Source {
    pub(in crate::workbench_client) fn capture(
        panel: &ClientPanel,
        presentation: &ClientPresentation,
    ) -> Self {
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
    pub(in crate::workbench_client) fn matches(&self, snapshot: &Snapshot) -> bool {
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
pub struct Canvas {
    pub(in crate::workbench_client) source: Source,
    revision: WorkbenchRevision,
    pub(in crate::workbench_client) layers: Vec<Layer>,
}
impl Canvas {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fixture(
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
    #[cfg(any(test, feature = "test-support"))]
    pub fn draw_for_test(&self) -> Draw {
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
    pub(in crate::workbench_client) source: Source,
    pub(in crate::workbench_client) revision: Option<WorkbenchRevision>,
    pub(in crate::workbench_client) transport: orchbee_workbench_protocol::PaneGraphicsTransport,
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
pub(in crate::workbench_client) struct Scope {
    pub(in crate::workbench_client) session: SessionId,
    pub(in crate::workbench_client) revision: WorkbenchRevision,
    pub(in crate::workbench_client) sources: Vec<Source>,
}
impl Scope {
    pub(in crate::workbench_client) fn matches(&self, state: &WorkbenchClientState) -> bool {
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
pub(in crate::workbench_client) struct Job {
    pub(in crate::workbench_client) scope: Scope,
    pub(in crate::workbench_client) previous: BTreeMap<WorkbenchPanelId, Canvas>,
}
pub(in crate::workbench_client) struct Round {
    pub(in crate::workbench_client) scope: Scope,
    pub(in crate::workbench_client) rows: BTreeMap<WorkbenchPanelId, Canvas>,
}
