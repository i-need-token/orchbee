//! Cell raster images are retained content, clipped to one real Panel body.
use crate::{terminal_session::PaneGraphicsCanvas, terminal_text::display_width};
use orchbee_workbench_protocol::{PaneGraphicsCellPixels, PaneGraphicsLayer};
use ratatui::{
    Frame,
    buffer::{Buffer, CellDiffOption},
    layout::Rect,
    style::{Color, Modifier},
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    canvas: &PaneGraphicsCanvas,
    cell_pixels: Option<&PaneGraphicsCellPixels>,
) {
    let mut painted = vec![false; usize::from(area.width) * usize::from(area.height)];
    for layer in canvas.layers() {
        paint_layers(
            frame.buffer_mut(),
            area,
            layer.metadata(),
            layer.pixels(),
            cell_pixels,
            &mut painted,
        );
    }
}

fn grid(
    area: Rect,
    layer: &PaneGraphicsLayer,
    cell_pixels: Option<&PaneGraphicsCellPixels>,
) -> (i64, i64) {
    crate::terminal_graphics::placement::grid((area.width, area.height), layer, cell_pixels)
}

#[cfg(test)]
fn paint(buffer: &mut Buffer, area: Rect, layer: &PaneGraphicsLayer, rgba: &[u8]) {
    paint_layers(
        buffer,
        area,
        layer,
        rgba,
        None,
        &mut vec![false; usize::from(area.width) * usize::from(area.height)],
    );
}

fn paint_layers(
    buffer: &mut Buffer,
    area: Rect,
    layer: &PaneGraphicsLayer,
    rgba: &[u8],
    cell_pixels: Option<&PaneGraphicsCellPixels>,
    painted: &mut [bool],
) {
    if area.is_empty() {
        return;
    }
    let (columns, rows) = grid(area, layer, cell_pixels);
    let left = i64::from(area.x) + i64::from(layer.placement.viewport_column);
    let top = i64::from(area.y) + i64::from(layer.placement.viewport_row);
    let x0 = left.max(i64::from(area.x));
    let y0 = top.max(i64::from(area.y));
    let x1 = (left + columns).min(i64::from(area.right()));
    let y1 = (top + rows).min(i64::from(area.bottom()));
    for y in y0..y1 {
        for x in x0..x1 {
            let sx = (x - left) * i64::from(layer.width) / columns;
            let sample = |half: i64| {
                let sy = (2 * (y - top) + half) * i64::from(layer.height) / (2 * rows);
                let index = ((sy * i64::from(layer.width) + sx) * 4) as usize;
                rgba.get(index..index + 4)
                    .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
            };
            let (Some(upper), Some(lower)) = (sample(0), sample(1)) else {
                continue;
            };
            if upper[3] == 0 && lower[3] == 0 {
                continue;
            }
            let Some(cell) = buffer.cell_mut((x as u16, y as u16)) else {
                continue;
            };
            // A wide glyph and its spacer are one terminal-owned unit. Preserve
            // both rather than producing a dangling head or skipped image cell.
            if cell.diff_option == CellDiffOption::Skip || display_width(cell.symbol()) > 1 {
                continue;
            }
            let index = (y - i64::from(area.y)) as usize * usize::from(area.width)
                + (x - i64::from(area.x)) as usize;
            if layer.z_index < 0 && cell.symbol() != " " && !painted[index] {
                cell.bg = blend(average(upper, lower), cell.bg);
                continue;
            }
            let upper_base = if cell.symbol() == "▀" {
                cell.fg
            } else {
                cell.bg
            };
            let upper = blend(upper, upper_base);
            let lower = blend(lower, cell.bg);
            cell.set_symbol("▀").set_fg(upper).set_bg(lower);
            cell.modifier = Modifier::empty();
            painted[index] = true;
        }
    }
}

fn average(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(a[3]) + u32::from(b[3]);
    if alpha == 0 {
        return [0; 4];
    }
    let component = |index| {
        ((u32::from(a[index]) * u32::from(a[3]) + u32::from(b[index]) * u32::from(b[3])) / alpha)
            as u8
    };
    [component(0), component(1), component(2), (alpha / 2) as u8]
}

fn blend(pixel: [u8; 4], base: Color) -> Color {
    if pixel[3] == 0 {
        return base;
    }
    let alpha = u32::from(pixel[3]);
    if let Color::Rgb(r, g, b) = base {
        let component = |index, background| {
            ((u32::from(pixel[index]) * alpha + u32::from(background) * (255 - alpha) + 127) / 255)
                as u8
        };
        return Color::Rgb(component(0, r), component(1, g), component(2, b));
    }
    // Default/palette colors have no observed RGB value. Keep their identity
    // below half opacity; do not invent a host background for alpha blending.
    if alpha < 128 {
        base
    } else {
        Color::Rgb(pixel[0], pixel[1], pixel[2])
    }
}

#[cfg(test)]
mod tests;
