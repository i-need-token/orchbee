//! Live pane geometry projected from the same shell layout the renderer paints. The shell
//! projection serves startup and every shell without a Workbench Client; a Workbench Client's
//! panes reach it through the [`PaneArrangement`] port.

use std::io;

use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::{
    ClientSplitRatio, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchSplitId,
};
use ratatui::layout::Rect;

use super::{
    PaneFrame, ShellAreas, ViewportTier, native_terminal_area, project_panel_tree,
    project_shell_areas, terminal_area,
};
use crate::{
    client_view::{ClientView, SidebarSurface},
    terminal_device::WindowDimensions,
    workbench_client::ClientPanelTree,
};

/// What pane geometry reads from a Workbench Client. The Client view implements it in the
/// composition, so layout projects the Client's panes without naming the composition.
pub(crate) trait PaneArrangement {
    /// The active Tab's Panels.
    type Tab: TabPanes;

    /// The sidebar surface the Client shows over the Client view's own `surface`.
    fn sidebar_surface(&self, surface: SidebarSurface) -> SidebarSurface;

    /// The active Tab, or `None` without one.
    fn active_tab(&self) -> Option<Self::Tab>;

    /// Whether the active Tab draws its focused Panel across the whole content.
    fn zoom_active(&self) -> bool;

    /// The focused Panel: the leaf that takes input, resize, and the focused-frame chrome.
    fn focus_panel_id(&self) -> Option<&WorkbenchPanelId>;
}

/// One Tab's Panels as a Workbench Client arranges them.
pub(crate) trait TabPanes {
    /// How many Panels the Tab holds.
    fn panel_count(&self) -> usize;

    /// The canonical split tree over the Tab's Panel indexes.
    fn panel_tree(&self) -> &ClientPanelTree;

    /// The index of the Tab's active Panel.
    fn active_panel(&self) -> Option<usize>;

    /// This Client's split weight for exact ordered slot identities.
    fn split_ratio(
        &self,
        split: &WorkbenchSplitId,
        first: &WorkbenchPanelSlotId,
        second: &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio;

    /// The identity of the Panel at `index`.
    fn panel_id(&self, index: usize) -> Option<&WorkbenchPanelId>;
}

/// The pane geometry of a shell without a Workbench Client, computed from the same pure layout
/// the renderer paints. `Ok(None)` means the viewport cannot present a pane right now (too small,
/// or the pane is empty): the caller keeps the last valid geometry instead of resizing — the
/// session is never told a fabricated size. `Err` means `Geometry` validation rejected the built
/// value (for example an out-of-range or oversized geometry); a zero cell pixel pair is legal
/// "pixels unknown" and stays `(0, 0)` — never repaired with fake cells.
pub fn terminal_geometry(
    dimensions: WindowDimensions,
    view: &ClientView,
) -> io::Result<Option<Geometry>> {
    let areas = project_shell_areas(to_area(dimensions), view.sidebar_surface());
    area_geometry(dimensions, active_pane_area(&areas))
}

/// The cell rect the terminal of a shell without a Workbench Client attaches to: the whole panel
/// content. `None` when the viewport cannot present a pane at all — explicit, not inferred from a
/// returned rect.
pub fn active_pane_area(areas: &ShellAreas) -> Option<Rect> {
    if areas.tier == ViewportTier::TooSmall {
        return None;
    }
    let content = terminal_area(areas.panel, areas.tier.shows_panel_frame());
    (!content.is_empty()).then_some(content)
}

/// The cell rect a Workbench Client's live terminal attaches to, identical to what
/// [`crate::tui_render::render_terminal_shell`] paints: the whole panel content for a single
/// pane or a Minimal viewport (active pane only, no tree chrome), otherwise the active leaf's
/// content inside the projected split tree. `None` when the viewport cannot present a pane at
/// all — explicit, not inferred from a returned rect.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn arranged_pane_area(
    areas: &ShellAreas,
    arrangement: &impl PaneArrangement,
) -> Option<Rect> {
    if areas.tier == ViewportTier::TooSmall {
        return None;
    }
    let framed = areas.tier.shows_panel_frame();
    let tab = arrangement.active_tab()?;
    let content = native_terminal_area(
        areas.panel,
        PaneFrame::of(framed, tab.panel_count(), arrangement.zoom_active()),
    );
    let area = if framed && tab.panel_count() > 1 && !arrangement.zoom_active() {
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

/// The geometry of [`arranged_pane_area`], with the same withholding and validation as
/// [`terminal_geometry`].
#[cfg(test)]
pub(crate) fn arranged_terminal_geometry(
    dimensions: WindowDimensions,
    view: &ClientView,
    arrangement: &impl PaneArrangement,
) -> io::Result<Option<Geometry>> {
    let areas = project_shell_areas(
        to_area(dimensions),
        arrangement.sidebar_surface(view.sidebar_surface()),
    );
    area_geometry(dimensions, arranged_pane_area(&areas, arrangement))
}

/// Every visible leaf's `(panel_id, panel_index, geometry)` for the native
/// session — the membership observation that gives each pane its own viewer.
/// The paned path consumes the same `project_panel_tree` leaves the renderer
/// paints; the single-pane path observes the focus pane on the whole content
/// rect, matching what the renderer draws there.
pub(crate) fn pane_geometries(
    dimensions: WindowDimensions,
    view: &ClientView,
    arrangement: &impl PaneArrangement,
) -> io::Result<Vec<(WorkbenchPanelId, usize, Geometry)>> {
    let areas = project_shell_areas(
        to_area(dimensions),
        arrangement.sidebar_surface(view.sidebar_surface()),
    );
    if areas.tier == ViewportTier::TooSmall {
        return Ok(Vec::new());
    }
    let framed = areas.tier.shows_panel_frame();
    let Some(tab) = arrangement.active_tab() else {
        return Ok(Vec::new());
    };
    let content =
        native_terminal_area(areas.panel, PaneFrame::of(framed, tab.panel_count(), false));
    let zoomed_content =
        native_terminal_area(areas.panel, PaneFrame::of(framed, tab.panel_count(), true));
    let (cell_width, cell_height) = cell_pixels(dimensions);
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
            let Some(panel_id) = tab.panel_id(leaf.panel_index) else {
                continue;
            };
            // Under zoom the observed set stays the full canonical leaf
            // list: the focused leaf reports the whole content area so its
            // terminal converges to the drawn size, while hidden leaves
            // report their unzoomed canonical rect — attached, alive, and
            // never resized.
            let rect = if arrangement.zoom_active() && tab.active_panel() == Some(leaf.panel_index)
            {
                zoomed_content
            } else {
                leaf.content
            };
            panes.push((panel_id.clone(), leaf.panel_index, leaf_geometry(rect)?));
        }
        return Ok(panes);
    }
    let Some(panel_id) = arrangement.focus_panel_id() else {
        return Ok(Vec::new());
    };
    let index = tab.active_panel().unwrap_or(0);
    Ok(vec![(panel_id.clone(), index, leaf_geometry(content)?)])
}

/// The terminal geometry of a presentable pane `area`.
fn area_geometry(dimensions: WindowDimensions, area: Option<Rect>) -> io::Result<Option<Geometry>> {
    let Some(area) = area else {
        return Ok(None);
    };
    let (width, height) = cell_pixels(dimensions);
    Geometry::new(area.width, area.height, width, height)
        .map(Some)
        .map_err(io::Error::other)
}

/// Each cell's pixel size, or `(0, 0)` — pixels unknown — when the host reports none.
fn cell_pixels(dimensions: WindowDimensions) -> (u16, u16) {
    match (
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
    }
}

const fn to_area(dimensions: WindowDimensions) -> Rect {
    Rect::new(0, 0, dimensions.columns, dimensions.rows)
}
