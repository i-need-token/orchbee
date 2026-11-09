//! The Client's one scrollbar: a pane paints it only from verified rows in its reserved chrome
//! column, and the sidebar list in its trailing padding column while the list overflows.

use ratatui::{Frame, layout::Rect, widgets::Clear};

use crate::{
    terminal_scrollbar::ScrollbarRange,
    tui_hit_map::{HitLayer, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
    workbench_client::WorkbenchClientView,
};
use orchbee_workbench_protocol::WorkbenchPanelId;

/// Paints `track` on `surface` and, when `range` is known, the thumb it places there: `┃` in
/// `accent-focus` while its region owns the keyboard, otherwise `text-secondary`, over a `│`
/// track in `frame-unfocused`. Returns the thumb, or `None` when there is none to grab.
pub(super) fn paint(
    frame: &mut Frame<'_>,
    track: Rect,
    surface: Role,
    range: Option<ScrollbarRange>,
    focused: bool,
    theme: &Theme,
) -> Option<Rect> {
    frame.render_widget(Clear, track);
    frame.buffer_mut().set_style(track, theme.style(surface));
    let thumb = range?.thumb(track)?;
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
    Some(thumb)
}

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
    // Paint and terminal resize share geometry; never scale a stale frame.
    let source = native
        .pane_scrollbar(panel)
        .filter(|source| (source.columns, source.rows) == (content.width, content.height));
    let focused = pane_focused && native.focus_panel_id() == Some(panel);
    let (Some(source), Some(pane)) = (source, native.pane_ref_for_panel(panel)) else {
        let _ = paint(frame, track, Role::SurfaceCanvas, None, focused, theme);
        return;
    };
    if paint(
        frame,
        track,
        Role::SurfaceCanvas,
        Some(source.range()),
        focused,
        theme,
    )
    .is_none()
    {
        return;
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
