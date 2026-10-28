//! Live pane geometry projected from the same shell layout the renderer paints.

use std::io;

use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::WorkbenchPanelId;
use ratatui::layout::Rect;

use super::{
    PaneFrame, ShellAreas, ViewportTier, native_terminal_area, project_panel_tree,
    project_shell_areas, terminal_area,
};
use crate::{
    client_view::ClientView, terminal_device::WindowDimensions,
    workbench_client::WorkbenchClientView,
};

/// The session's real pane geometry, computed from the same pure layout the
/// renderer paints. `Ok(None)` means the viewport cannot present a pane right
/// now (too small, or the active leaf is empty): the caller keeps the last
/// valid geometry instead of resizing — the session is never told a
/// fabricated size. `Err` means `Geometry` validation rejected the built
/// value (for example an out-of-range or oversized geometry); a zero cell
/// pixel pair is legal "pixels unknown" and stays `(0, 0)` — never repaired
/// with fake cells.
pub fn terminal_geometry(
    dimensions: WindowDimensions,
    view: &ClientView,
    workbench_client: Option<WorkbenchClientView<'_>>,
) -> io::Result<Option<Geometry>> {
    let sidebar = match workbench_client.as_ref() {
        Some(native) => native.sidebar_surface(view.sidebar_surface()),
        _ => view.sidebar_surface(),
    };
    let areas = project_shell_areas(to_area(dimensions), sidebar);
    let Some(area) = active_pane_area(&areas, workbench_client) else {
        return Ok(None);
    };
    let (width, height) = match (
        dimensions
            .pixel_width
            .checked_div(dimensions.columns)
            .unwrap_or(0),
        dimensions
            .pixel_height
            .checked_div(dimensions.rows)
            .unwrap_or(0),
    ) {
        (0, _) | (_, 0) => (0, 0),
        cell_pixels => cell_pixels,
    };
    Geometry::new(area.width, area.height, width, height)
        .map(Some)
        .map_err(io::Error::other)
}

/// The cell rect the live terminal attaches to, identical to what
/// [`crate::tui_render::render_terminal_shell`] paints: the whole panel content for a single
/// pane or a Minimal viewport (active pane only, no tree chrome), otherwise
/// the active leaf's content inside the projected split tree. `None` when the
/// viewport cannot present a pane at all — explicit, not inferred from a
/// returned rect.
pub fn active_pane_area(
    areas: &ShellAreas,
    workbench_client: Option<WorkbenchClientView<'_>>,
) -> Option<Rect> {
    if areas.tier == ViewportTier::TooSmall {
        return None;
    }
    let framed = areas.tier.shows_panel_frame();
    let content = terminal_area(areas.panel, framed);
    let native = match workbench_client {
        Some(native) => native,
        None => return (!content.is_empty()).then_some(content),
    };
    let tab = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))?;
    let content = native_terminal_area(
        areas.panel,
        PaneFrame::of(framed, tab.panel_count(), native.zoom_active()),
    );
    let area = if framed && tab.panel_count() > 1 && !native.zoom_active() {
        project_panel_tree(
            content,
            tab.panel_tree(),
            tab.active_panel(),
            &|split, first, second| tab.split_ratio(split, first, second),
        )
        .active_content(tab.active_panel())
    } else {
        Some(content)
    }?;
    (!area.is_empty()).then_some(area)
}

/// Every visible leaf's `(panel_id, panel_index, geometry)` for the native
/// session — the membership observation that gives each pane its own viewer.
/// The paned path consumes the same `project_panel_tree` leaves the renderer
/// paints; the single-pane path observes the focus pane on the whole content
/// rect, matching what the renderer draws there.
pub fn pane_geometries(
    dimensions: WindowDimensions,
    view: &ClientView,
    native: &WorkbenchClientView<'_>,
) -> io::Result<Vec<(WorkbenchPanelId, usize, Geometry)>> {
    let areas = project_shell_areas(
        to_area(dimensions),
        native.sidebar_surface(view.sidebar_surface()),
    );
    if areas.tier == ViewportTier::TooSmall {
        return Ok(Vec::new());
    }
    let framed = areas.tier.shows_panel_frame();
    let Some(tab) = native
        .active_position()
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| native.tab(index))
    else {
        return Ok(Vec::new());
    };
    let content =
        native_terminal_area(areas.panel, PaneFrame::of(framed, tab.panel_count(), false));
    let zoomed_content =
        native_terminal_area(areas.panel, PaneFrame::of(framed, tab.panel_count(), true));
    let (cell_width, cell_height) = match (
        dimensions
            .pixel_width
            .checked_div(dimensions.columns)
            .unwrap_or(0),
        dimensions
            .pixel_height
            .checked_div(dimensions.rows)
            .unwrap_or(0),
    ) {
        (0, _) | (_, 0) => (0, 0),
        cell_pixels => cell_pixels,
    };
    let leaf_geometry = |rect: Rect| {
        Geometry::new(rect.width, rect.height, cell_width, cell_height).map_err(io::Error::other)
    };
    if framed && tab.panel_count() > 1 {
        let projection = project_panel_tree(
            content,
            tab.panel_tree(),
            tab.active_panel(),
            &|split, first, second| tab.split_ratio(split, first, second),
        );
        let mut panes = Vec::with_capacity(projection.leaves.len());
        for leaf in &projection.leaves {
            let Some(panel) = tab.panel(leaf.panel_index) else {
                continue;
            };
            // Under zoom the observed set stays the full canonical leaf
            // list: the focused leaf reports the whole content area so its
            // terminal converges to the drawn size, while hidden leaves
            // report their unzoomed canonical rect — attached, alive, and
            // never resized.
            let rect = if native.zoom_active() && tab.active_panel() == Some(leaf.panel_index) {
                zoomed_content
            } else {
                leaf.content
            };
            panes.push((
                panel.panel_id().clone(),
                leaf.panel_index,
                leaf_geometry(rect)?,
            ));
        }
        return Ok(panes);
    }
    let Some(panel_id) = native.focus_panel_id() else {
        return Ok(Vec::new());
    };
    let index = tab.active_panel().unwrap_or(0);
    Ok(vec![(panel_id.clone(), index, leaf_geometry(content)?)])
}

const fn to_area(dimensions: WindowDimensions) -> Rect {
    Rect::new(0, 0, dimensions.columns, dimensions.rows)
}
