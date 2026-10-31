//! Copy-local cropping keeps served glyphs, cursor, selection and pointer coordinates together.
use orchbee_terminal_protocol::TerminalTextRow;
use ratatui::{Frame, layout::Rect, widgets::Clear};

use crate::{
    terminal_copy_mode::{CopyCrop, CopyModeCursor, CopyModeView, Surface},
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget, ListScrollTarget},
    tui_theme::{Role, Theme},
    workbench_client::{ViewportSurface, WorkbenchClientView},
};

/// Returns the crop this frame painted, for the shell's draw receipt.
pub(in crate::tui_render) fn paint_surface(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    native: &WorkbenchClientView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<CopyCrop> {
    let cursor = mode.cursor?;
    let panel = native.focus_panel_id()?;
    let source = native.copy_content_source(panel)?;
    let pointer = mode.pointer?;
    let surface = pointer.content_surface()?;
    if Surface::checked(
        source.screen,
        source.rows.generation,
        source.rows.revision,
        source.rows.columns,
        source.rows.total_rows,
        source.rows.start_row,
        source.rows.row_count,
    ) != Some(surface)
    {
        return None;
    }
    let (rows, terminal_theme) = match native.pane_viewport(panel) {
        Some(ViewportSurface::Window { window, .. }) => (
            window.rows.as_slice(),
            theme.with_host_terminal_colors(window.foreground, window.background),
        ),
        None => {
            let terminal = native.pane_projection(panel)?.frame()?;
            (
                terminal.rows.as_slice(),
                theme.with_host_terminal_colors(terminal.foreground, terminal.background),
            )
        }
        Some(_) => return None,
    };
    let content = content_rect(frame.area(), area, surface);
    let glyph_width = cursor_width(rows, cursor);
    if content.is_empty() || glyph_width > content.width {
        return None;
    }
    // The crop moves from the origin the episode last painted only as far as the cursor glyph
    // needs. A cursor moved inside the painted surface, as a pointer press or drag moves it, keeps
    // the rectangle and origin that the gesture's later reports resolve against.
    let previous = native
        .copy_crop()
        .unwrap_or(CopyModeCursor { column: 0, row: 0 });
    let origin = CopyModeCursor {
        column: previous
            .column
            .min(cursor.column)
            .max(
                cursor
                    .column
                    .saturating_add(glyph_width)
                    .saturating_sub(content.width),
            )
            .min(surface.columns.saturating_sub(content.width)),
        row: previous
            .row
            .min(cursor.row)
            .max(cursor.row.saturating_sub(content.height - 1))
            .min(surface.row_count.saturating_sub(content.height)),
    };
    frame.render_widget(Clear, area);
    super::super::terminal_cells::render_rows_at(
        frame,
        content,
        rows,
        origin.column,
        origin.row,
        &terminal_theme,
    );
    super::super::selection::paint_cropped(
        frame,
        content,
        surface.start_row + u64::from(origin.row),
        origin.column,
        native.pane_selection(panel),
        theme.style(Role::SurfaceSelection),
    );
    super::paint_cursor(
        frame,
        content,
        CopyModeView {
            cursor: Some(CopyModeCursor {
                column: cursor.column - origin.column,
                row: cursor.row - origin.row,
            }),
            ..mode
        },
        theme,
    );
    if let Some(token) = pointer.surface() {
        hits.record(
            HitLayer::ChromeSurface,
            content,
            HitTarget::CopySurface {
                token,
                area: content,
                origin,
            },
        );
        hits.record(
            HitLayer::ChromeSurface,
            content,
            HitTarget::ListScroll(ListScrollTarget::Copy(pointer)),
        );
    }
    Some(pointer.crop(origin))
}

fn content_rect(viewport: Rect, mut area: Rect, surface: Surface) -> Rect {
    // The shell owns these rows even when terminal geometry includes them.
    // Only this local projection yields; the Host window and input coordinates stay intact.
    area = area.intersection(viewport);
    if area.y == viewport.y && area.height != 0 {
        area.y += 1; // Minimal layout's copy toolbar.
        area.height -= 1;
    }
    area.height = area
        .height
        .min(viewport.bottom().saturating_sub(1).saturating_sub(area.y));
    area.width = area.width.min(surface.columns);
    area.height = area.height.min(surface.row_count);
    area
}

fn cursor_width(rows: &[TerminalTextRow], cursor: CopyModeCursor) -> u16 {
    rows.iter()
        .find(|row| row.index == cursor.row)
        .and_then(|row| row.cells.get(usize::from(cursor.column)))
        .map_or(1, |cell| if cell.width == 1 { 2 } else { 1 })
}
