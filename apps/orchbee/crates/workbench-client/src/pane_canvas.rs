//! Complete immutable pane graphics canvases pinned to one revision, the draw facts that name
//! them, and the shared pixel sampling every renderer uses; validation only, no reads or
//! presentation.
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_LAYERS_PER_PANE, MAXIMUM_GRAPHICS_RESIDENT_BYTES, PaneGraphicsLayer,
    PaneGraphicsTarget, PresentationBinding, Snapshot, WorkbenchRevision, graphics_rgba_bytes,
};
use std::{collections::BTreeSet, sync::Arc};
pub(crate) mod terminal;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Source {
    pub(crate) target: PaneGraphicsTarget,
    pub(crate) binding: Option<PresentationBinding>,
    pub(crate) lane: Lane,
}

/// The two image sources a Panel shows; only Session layers have Workbench draw receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Lane {
    /// The Session's Panel image layers, written through the graphics API.
    Session,
    /// Images the program in the Panel printed with the Kitty graphics protocol.
    Terminal,
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
    pixels: Arc<Vec<u8>>,
    channels: u8,
    window: Option<Window>,
}

/// Where a source crop lands inside a layer's whole-cell grid, in Host pixels. A Kitty placement
/// rarely fills whole cells; the rest of its grid stays transparent rather than stretched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Window {
    /// The grid rectangle's size.
    pub(crate) frame: (u32, u32),
    /// Destination origin and size inside the grid rectangle.
    pub(crate) destination: (u32, u32, u32, u32),
    /// Source crop origin and size inside the image.
    pub(crate) source: (u32, u32, u32, u32),
}

impl Layer {
    pub(crate) fn new(metadata: PaneGraphicsLayer, pixels: Vec<u8>) -> Option<Self> {
        (graphics_rgba_bytes(metadata.width, metadata.height).ok()? == pixels.len()).then(|| Self {
            metadata,
            pixels: Arc::new(pixels),
            channels: 4,
            window: None,
        })
    }
    /// Shares a decoded image of one to four channels without converting it to RGBA.
    pub(crate) fn windowed(
        metadata: PaneGraphicsLayer,
        pixels: Arc<Vec<u8>>,
        channels: u8,
        window: Window,
    ) -> Option<Self> {
        let (x, y, width, height) = window.source;
        let (_, _, destination_width, destination_height) = window.destination;
        ((1..=4).contains(&channels)
            && u64::from(metadata.width) * u64::from(metadata.height) * u64::from(channels)
                == pixels.len() as u64
            && width > 0
            && height > 0
            && u64::from(x) + u64::from(width) <= u64::from(metadata.width)
            && u64::from(y) + u64::from(height) <= u64::from(metadata.height)
            && destination_width > 0
            && destination_height > 0
            && window.frame.0 > 0
            && window.frame.1 > 0)
            .then_some(Self {
                metadata,
                pixels,
                channels,
                window: Some(window),
            })
    }
    pub(crate) fn metadata(&self) -> &PaneGraphicsLayer {
        &self.metadata
    }
    #[cfg(test)]
    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    /// The straight-alpha RGBA sample at `x.0 / x.1` across and `y.0 / y.1` down the layer's
    /// grid, both fractions in `0..1`; `None` where the layer shows nothing.
    pub(crate) fn sample(&self, x: (i128, i128), y: (i128, i128)) -> Option<[u8; 4]> {
        let width = i128::from(self.metadata.width);
        let height = i128::from(self.metadata.height);
        let (sx, sy) = match self.window {
            None => (x.0 * width / x.1, y.0 * height / y.1),
            Some(window) => (
                window_axis(
                    x,
                    window.frame.0,
                    (window.destination.0, window.destination.2),
                    (window.source.0, window.source.2),
                )?,
                window_axis(
                    y,
                    window.frame.1,
                    (window.destination.1, window.destination.3),
                    (window.source.1, window.source.3),
                )?,
            ),
        };
        if !(0..width).contains(&sx) || !(0..height).contains(&sy) {
            return None;
        }
        let channels = usize::from(self.channels);
        let index = usize::try_from(sy * width + sx).ok()? * channels;
        let pixel = self.pixels.get(index..index + channels)?;
        Some(match *pixel {
            [gray] => [gray, gray, gray, 255],
            [gray, alpha] => [gray, gray, gray, alpha],
            [red, green, blue] => [red, green, blue, 255],
            [red, green, blue, alpha] => [red, green, blue, alpha],
            _ => return None,
        })
    }
    fn same(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.channels == other.channels
            && self.window == other.window
            && Arc::ptr_eq(&self.pixels, &other.pixels)
    }
}

/// Maps a grid fraction to a source coordinate, or `None` outside the destination span.
fn window_axis(
    fraction: (i128, i128),
    frame: u32,
    (start, length): (u32, u32),
    (source_start, source_length): (u32, u32),
) -> Option<i128> {
    let position = fraction.0 * i128::from(frame);
    let start = i128::from(start) * fraction.1;
    let length = i128::from(length) * fraction.1;
    (start..start + length)
        .contains(&position)
        .then(|| i128::from(source_start) + (position - start) * i128::from(source_length) / length)
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
                lane: Lane::Session,
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
            || layers.iter().map(|l| l.pixels.len()).sum::<usize>()
                > MAXIMUM_GRAPHICS_RESIDENT_BYTES
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
        self.layers.iter().map(|l| l.pixels.len()).sum()
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
