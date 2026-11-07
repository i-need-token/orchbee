//! Images a program printed with the Kitty graphics protocol become one immutable single-layer
//! canvas per resolved placement, so the Panel image renderers draw them as they draw Session
//! layers. Nothing here decodes, retains, scales, or paints pixels.

use super::{Canvas, Lane, Layer, Source, Window};
use orchbee_terminal_protocol::TerminalGraphics;
use orchbee_workbench_protocol::{PaneGraphicsLayer, PaneGraphicsPlacement, WorkbenchRevision};

/// One canvas per placement, in the Host's paint order. The layer covers the whole cells its
/// destination touches; `cell_pixels` is the Host cell size the placement was resolved with.
pub(crate) fn canvases(
    source: &Source,
    graphics: &TerminalGraphics,
    cell_pixels: (u16, u16),
) -> Vec<Canvas> {
    let (cell_width, cell_height) = (u32::from(cell_pixels.0), u32::from(cell_pixels.1));
    if cell_width == 0 || cell_height == 0 {
        // The Host carries no placement without pixel metrics.
        return Vec::new();
    }
    let source = Source {
        lane: Lane::Terminal,
        ..source.clone()
    };
    // Terminal canvases carry the Host resource revision; it never reaches a Workbench receipt.
    let revision = WorkbenchRevision::new(graphics.resource_revision);
    graphics
        .placements
        .iter()
        .filter_map(|placement| {
            let image = graphics.image(placement.image_id)?;
            let cells = |offset: u32, length: u32, cell: u32| {
                let columns = (u64::from(offset) + u64::from(length)).div_ceil(u64::from(cell));
                Some((
                    u32::try_from(columns).ok()?,
                    u32::try_from(columns * u64::from(cell)).ok()?,
                ))
            };
            let (grid_columns, frame_width) =
                cells(placement.offset_x, placement.destination_width, cell_width)?;
            let (grid_rows, frame_height) = cells(
                placement.offset_y,
                placement.destination_height,
                cell_height,
            )?;
            let metadata = PaneGraphicsLayer {
                // Every geometric input names the layer, so a moved image is a new physical one.
                layer_id: format!(
                    "kitty:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}x{}",
                    image.id,
                    image.generation,
                    placement.placement_id,
                    u8::from(placement.internal_id),
                    placement.column,
                    placement.row,
                    placement.offset_x,
                    placement.offset_y,
                    placement.source_x,
                    placement.source_y,
                    placement.source_width,
                    placement.source_height,
                    placement.destination_width,
                    placement.destination_height,
                    placement.z,
                    cell_width,
                    cell_height,
                ),
                z_index: placement.z,
                width: image.width,
                height: image.height,
                placement: PaneGraphicsPlacement {
                    viewport_column: placement.column,
                    viewport_row: placement.row,
                    grid_columns,
                    grid_rows,
                },
                resource_revision: revision,
                stream_id: None,
            };
            let layer = Layer::windowed(
                metadata,
                image.pixels.clone(),
                image.format.channels(),
                Window {
                    frame: (frame_width, frame_height),
                    destination: (
                        placement.offset_x,
                        placement.offset_y,
                        placement.destination_width,
                        placement.destination_height,
                    ),
                    source: (
                        placement.source_x,
                        placement.source_y,
                        placement.source_width,
                        placement.source_height,
                    ),
                },
            )?;
            Canvas::new(source.clone(), revision, vec![layer])
        })
        .collect()
}
