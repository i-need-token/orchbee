//! Complete immutable pane graphics canvases pinned to one Workbench revision, and the draw facts
//! that name them; validation only, no reads or presentation.
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_LAYERS_PER_PANE, MAXIMUM_GRAPHICS_RESIDENT_BYTES, PaneGraphicsLayer,
    PaneGraphicsTarget, PresentationBinding, Snapshot, WorkbenchRevision, graphics_rgba_bytes,
};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Source {
    pub(crate) target: PaneGraphicsTarget,
    pub(crate) binding: Option<PresentationBinding>,
}
impl Source {
    pub(crate) fn matches(&self, snapshot: &Snapshot) -> bool {
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
    pub(crate) fn new(metadata: PaneGraphicsLayer, pixels: Vec<u8>) -> Option<Self> {
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
    pub(crate) source: Source,
    revision: WorkbenchRevision,
    pub(crate) layers: Vec<Layer>,
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
    pub(crate) fn new(
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
    pub(crate) source: Source,
    pub(crate) revision: Option<WorkbenchRevision>,
    pub(crate) transport: orchbee_workbench_protocol::PaneGraphicsTransport,
}
impl Draw {
    pub(crate) fn new(source: Source, revision: Option<WorkbenchRevision>) -> Self {
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
