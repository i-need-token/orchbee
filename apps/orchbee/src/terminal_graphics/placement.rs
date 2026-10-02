//! Shared measured-aspect placement; virtual axes stay wider than host coordinates.
use orchbee_workbench_protocol::{PaneGraphicsCellPixels, PaneGraphicsLayer};

pub(crate) fn grid(
    body: (u16, u16),
    layer: &PaneGraphicsLayer,
    cell_pixels: Option<&PaneGraphicsCellPixels>,
) -> (i64, i64) {
    let w = i64::from(layer.width);
    let h = i64::from(layer.height);
    let columns = i64::from(layer.placement.grid_columns);
    let rows = i64::from(layer.placement.grid_rows);
    let (cell_width, cell_height) = cell_pixels
        .filter(|cell| cell.width > 0 && cell.height > 0)
        .map_or((1, 2), |cell| {
            (i64::from(cell.width), i64::from(cell.height))
        });
    let pixel_width = w * cell_height;
    let pixel_height = h * cell_width;
    match (columns, rows) {
        (0, 0) => {
            let columns =
                i64::from(body.0).min((i64::from(body.1) * pixel_width / pixel_height).max(1));
            (
                columns,
                ((pixel_height * columns + pixel_width - 1) / pixel_width)
                    .max(1)
                    .min(i64::from(body.1)),
            )
        }
        (0, rows) => (
            ((pixel_width * rows + pixel_height - 1) / pixel_height).max(1),
            rows,
        ),
        (columns, 0) => (
            columns,
            ((pixel_height * columns + pixel_width - 1) / pixel_width).max(1),
        ),
        dimensions => dimensions,
    }
}
