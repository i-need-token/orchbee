//! Copy-local cropping keeps served glyphs, cursor, selection and pointer coordinates together.
use orchbee_terminal_protocol::TerminalTextRow;
use ratatui::{Frame, layout::Rect, widgets::Clear};

use crate::{
    terminal_copy_mode::{CopyModeCursor, CopyModeToken, CopyModeView, Surface},
    terminal_session::{NativeSessionView, NativeViewportSurface},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget, ListScrollTarget},
    tui_theme::{Role, Theme},
};

pub(in crate::tui_render) fn paint_surface(
    frame: &mut Frame<'_>,
    area: Rect,
    mode: CopyModeView<'_>,
    native: &NativeSessionView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    let Some(cursor) = mode.cursor else { return };
    let Some(panel) = native.focus_panel_id() else {
        return;
    };
    let Some(source) = native.copy_content_source(panel) else {
        return;
    };
    let Some(surface) = mode.pointer.and_then(CopyModeToken::content_surface) else {
        return;
    };
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
        return;
    }
    let (rows, terminal_theme) = match native.pane_viewport(panel) {
        Some(NativeViewportSurface::Window { window, .. }) => (
            window.rows.as_slice(),
            theme.with_host_terminal_colors(window.foreground, window.background),
        ),
        None => {
            let Some(terminal) = native
                .pane_projection(panel)
                .and_then(|value| value.frame())
            else {
                return;
            };
            (
                terminal.rows.as_slice(),
                theme.with_host_terminal_colors(terminal.foreground, terminal.background),
            )
        }
        Some(_) => return,
    };
    let content = content_rect(frame.area(), area, surface);
    let glyph_width = cursor_width(rows, cursor);
    if content.is_empty() || glyph_width > content.width {
        return;
    }
    let origin = CopyModeCursor {
        column: cursor
            .column
            .saturating_add(glyph_width)
            .saturating_sub(content.width)
            .min(surface.columns.saturating_sub(content.width)),
        row: cursor
            .row
            .saturating_sub(content.height - 1)
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
    if let Some(token) = mode.pointer
        && let Some(surface) = token.surface()
    {
        hits.record(
            HitLayer::ChromeSurface,
            content,
            HitTarget::CopySurface {
                token: surface,
                area: content,
                origin,
            },
        );
        hits.record(
            HitLayer::ChromeSurface,
            content,
            HitTarget::ListScroll(ListScrollTarget::Copy(token)),
        );
    }
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
