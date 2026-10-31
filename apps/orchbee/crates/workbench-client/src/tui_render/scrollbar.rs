//! Paints a scrollbar only from verified rows, in the pane's reserved chrome column.

use ratatui::{Frame, layout::Rect, widgets::Clear};

use crate::{
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
    workbench_client::WorkbenchClientView,
};
use orchbee_workbench_protocol::WorkbenchPanelId;

#[expect(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    outer: Rect,
    content: Rect,
    panel: &WorkbenchPanelId,
    pane_focused: bool,
    native: &WorkbenchClientView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    if content.is_empty() || content.right() >= outer.right() {
        return;
    }
    let track = Rect::new(content.right(), content.y, 1, content.height);
    frame.render_widget(Clear, track);
    frame
        .buffer_mut()
        .set_style(track, theme.style(Role::SurfaceCanvas));
    let Some(source) = native.pane_scrollbar(panel) else {
        return;
    };
    // Paint and terminal resize share geometry; never scale a stale frame.
    if (source.columns, source.rows) != (content.width, content.height) {
        return;
    }
    let Some(thumb) = source.thumb(track) else {
        return;
    };
    let Some(pane) = native.pane_ref_for_panel(panel) else {
        return;
    };
    let focused = pane_focused && native.focus_panel_id() == Some(panel);
    for y in track.y..track.bottom() {
        let (symbol, role) = if y >= thumb.y && y < thumb.bottom() {
            (
                theme.glyphs.scrollbar_thumb,
                if focused {
                    Role::AccentFocus
                } else {
                    Role::TextSecondary
                },
            )
        } else {
            (theme.glyphs.scrollbar_track, Role::FrameUnfocused)
        };
        frame
            .buffer_mut()
            .set_string(track.x, y, symbol, theme.style(role));
    }
    hits.record(
        HitLayer::Foreground,
        track,
        HitTarget::Scrollbar {
            pane,
            content,
            source,
        },
    );
}
