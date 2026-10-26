//! Physical rectangles are clipped before lazy sampling of immutable source pixels.
use crate::{terminal_graphics::placement, workbench_client::PaneGraphicsCanvas};
use orchbee_workbench_protocol::{PaneGraphicsCellPixels, SessionId};
use ratatui::layout::Rect;

pub(super) const CANVAS_BOUND: usize = 64 * 1024 * 1024;
const IMAGE_BOUND: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct Scene {
    pub(super) session: SessionId,
    pub(super) canvas: PaneGraphicsCanvas,
    body: Rect,
    cells: PaneGraphicsCellPixels,
    pub(super) images: Vec<ImagePlan>,
    pub(super) visible_bytes: usize,
}
impl Scene {
    pub(super) fn new(
        session: &SessionId,
        canvas: &PaneGraphicsCanvas,
        body: Rect,
        cells: PaneGraphicsCellPixels,
    ) -> Option<Self> {
        if body.is_empty() || cells.width == 0 || cells.height == 0 {
            return None;
        }
        let mut images = Vec::new();
        let mut visible_bytes: usize = 0;
        for (index, layer) in canvas.layers().iter().enumerate() {
            let metadata = layer.metadata();
            let grid = placement::grid((body.width, body.height), metadata, Some(&cells));
            let left = i64::from(body.x) + i64::from(metadata.placement.viewport_column);
            let top = i64::from(body.y) + i64::from(metadata.placement.viewport_row);
            let x0 = left.max(i64::from(body.x));
            let y0 = top.max(i64::from(body.y));
            let x1 = (left + grid.0).min(i64::from(body.right()));
            let y1 = (top + grid.1).min(i64::from(body.bottom()));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            let visible = Rect::new(
                u16::try_from(x0).ok()?,
                u16::try_from(y0).ok()?,
                u16::try_from(x1 - x0).ok()?,
                u16::try_from(y1 - y0).ok()?,
            );
            let width = u32::from(visible.width) * u32::from(cells.width);
            let height = u32::from(visible.height) * u32::from(cells.height);
            let bytes = usize::try_from(width)
                .ok()?
                .checked_mul(usize::try_from(height).ok()?)?
                .checked_mul(4)?;
            if bytes > IMAGE_BOUND {
                return None;
            }
            visible_bytes = visible_bytes.checked_add(bytes)?;
            if visible_bytes > CANVAS_BOUND {
                return None;
            }
            images.push(ImagePlan {
                index,
                visible,
                width,
                height,
                bytes,
                left,
                top,
                grid,
                cells,
            });
        }
        Some(Self {
            session: session.clone(),
            canvas: canvas.clone(),
            body,
            cells,
            images,
            visible_bytes,
        })
    }
    pub(super) fn scope_matches(&self, other: &Self) -> bool {
        self.session == other.session
            && self.body == other.body
            && self.cells == other.cells
            && self.canvas.same_source_and_layers(&other.canvas)
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.scope_matches(other) && self.canvas.same_content(&other.canvas)
    }
}
#[derive(Clone, Debug)]
pub(super) struct ImagePlan {
    pub(super) index: usize,
    pub(super) visible: Rect,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) bytes: usize,
    left: i64,
    top: i64,
    grid: (i64, i64),
    cells: PaneGraphicsCellPixels,
}
impl ImagePlan {
    pub(super) fn fill(&self, canvas: &PaneGraphicsCanvas, offset: usize, bytes: &mut [u8]) {
        let layer = &canvas.layers()[self.index];
        let metadata = layer.metadata();
        let virtual_width = i128::from(self.grid.0) * i128::from(self.cells.width);
        let virtual_height = i128::from(self.grid.1) * i128::from(self.cells.height);
        let dx =
            (i128::from(self.visible.x) - i128::from(self.left)) * i128::from(self.cells.width);
        let dy =
            (i128::from(self.visible.y) - i128::from(self.top)) * i128::from(self.cells.height);
        for (index, pixel) in bytes.chunks_exact_mut(4).enumerate() {
            let position = (offset / 4 + index) as u64;
            let x = dx + i128::from(position % u64::from(self.width));
            let y = dy + i128::from(position / u64::from(self.width));
            let sx = (x * i128::from(metadata.width) / virtual_width) as usize;
            let sy = (y * i128::from(metadata.height) / virtual_height) as usize;
            let source = (sy * metadata.width as usize + sx) * 4;
            pixel.copy_from_slice(&layer.pixels()[source..source + 4]);
        }
    }
}
