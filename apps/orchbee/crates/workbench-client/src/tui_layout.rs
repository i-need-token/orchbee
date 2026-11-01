//! Deterministic shell and pane geometry; no terminal or shared-layout mutations.
mod divider;
mod pane_geometry;
mod resize;
mod sidebar;
pub use crate::sidebar_width::SidebarWidth;
pub(crate) use divider::DrawnResize;
pub use divider::{DividerDrag, DividerSource, DrawnDivider};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use pane_geometry::arranged_pane_area;
#[cfg(test)]
pub(crate) use pane_geometry::arranged_terminal_geometry;
pub(crate) use pane_geometry::{PaneArrangement, TabPanes, pane_geometries};
pub use pane_geometry::{active_pane_area, terminal_geometry};
use ratatui::layout::{Constraint, Layout, Rect};
pub use resize::ResizeProjection;
pub use sidebar::{DrawnSidebarWidth, SidebarWidthDrag};

use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, WorkbenchPanelSlotId, WorkbenchSplitId,
};

use crate::client_view::SidebarSurface;
use crate::workbench_projection::ClientPanelTree;

pub(crate) const FULL_MINIMUM_COLUMNS: u16 = 100;
pub(crate) const FULL_MINIMUM_ROWS: u16 = 24;
pub(crate) use crate::sidebar_width::COMPACT_MINIMUM_COLUMNS;
pub(crate) const COMPACT_MINIMUM_ROWS: u16 = 12;
pub(crate) const TOO_SMALL_MINIMUM_COLUMNS: u16 = 20;
pub(crate) const TOO_SMALL_MINIMUM_ROWS: u16 = 5;
pub(crate) const STATUS_ROWS: u16 = 1;
pub(crate) const TAB_STRIP_ROWS: u16 = 1;
pub(crate) const OVERLAY_HORIZONTAL_MARGIN: u16 = 4;
pub(crate) const OVERLAY_VERTICAL_MARGIN: u16 = 2;
pub(crate) const MINIMUM_OVERLAY_INNER_COLUMNS: u16 = 4;
pub(crate) const MINIMUM_OVERLAY_INNER_ROWS: u16 = 3;
pub(crate) const MINIMUM_PANE_COLUMNS: u16 = 4;
pub(crate) const MINIMUM_PANE_ROWS: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ViewportTier {
    TooSmall,
    Minimal,
    Compact,
    Full,
}

impl ViewportTier {
    pub(crate) const fn from_size(columns: u16, rows: u16) -> Self {
        let width = Self::from_columns(columns);
        let height = Self::from_rows(rows);
        if width.rank() <= height.rank() {
            width
        } else {
            height
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::TooSmall => 0,
            Self::Minimal => 1,
            Self::Compact => 2,
            Self::Full => 3,
        }
    }

    pub(crate) const fn from_area(area: Rect) -> Self {
        Self::from_size(area.width, area.height)
    }

    const fn from_columns(columns: u16) -> Self {
        if columns < TOO_SMALL_MINIMUM_COLUMNS {
            Self::TooSmall
        } else if columns < COMPACT_MINIMUM_COLUMNS {
            Self::Minimal
        } else if columns < FULL_MINIMUM_COLUMNS {
            Self::Compact
        } else {
            Self::Full
        }
    }

    const fn from_rows(rows: u16) -> Self {
        if rows < TOO_SMALL_MINIMUM_ROWS {
            Self::TooSmall
        } else if rows < COMPACT_MINIMUM_ROWS {
            Self::Minimal
        } else if rows < FULL_MINIMUM_ROWS {
            Self::Compact
        } else {
            Self::Full
        }
    }

    pub(crate) const fn shows_tab_strip(self) -> bool {
        matches!(self, Self::Compact | Self::Full)
    }

    pub const fn shows_panel_frame(self) -> bool {
        matches!(self, Self::Compact | Self::Full)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarLayer {
    Docked,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarArea {
    pub(crate) area: Rect,
    pub(crate) layer: SidebarLayer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShellAreas {
    pub(crate) body: Rect,
    pub panel: Rect,
    pub sidebar: Option<SidebarArea>,
    pub(crate) status: Rect,
    pub(crate) tabs: Rect,
    pub tier: ViewportTier,
    pub(crate) workbench: Rect,
}

/// Authored guidance yields rows occupied by a global chrome action without
/// changing terminal geometry.
pub(crate) fn guidance_area(content: Rect, global_action: Option<Rect>) -> Rect {
    let Some(action) = global_action.filter(|action| !content.intersection(*action).is_empty())
    else {
        return content;
    };
    let occupied = action
        .bottom()
        .saturating_sub(content.y)
        .min(content.height);
    Rect::new(
        content.x,
        content.y + occupied,
        content.width,
        content.height - occupied,
    )
}

pub fn project_shell_areas(area: Rect, sidebar_surface: SidebarSurface) -> ShellAreas {
    let tier = ViewportTier::from_area(area);
    // Content reaches the last row; the status floats over it and reserves no terminal height.
    let body = area;
    let surface = effective_sidebar_surface(tier, sidebar_surface, area.width);
    let (workbench, sidebar) = match surface {
        SidebarSurface::Docked(width) => {
            let [sidebar, workbench] = Layout::horizontal([
                Constraint::Length(width.effective_columns(area.width)),
                Constraint::Min(0),
            ])
            .areas(body);
            (
                workbench,
                Some(SidebarArea {
                    area: sidebar,
                    layer: SidebarLayer::Docked,
                }),
            )
        }
        SidebarSurface::Hidden => (body, None),
        SidebarSurface::Overlay(width) => (
            body,
            Some(SidebarArea {
                area: Rect::new(
                    body.x,
                    body.y,
                    width.effective_columns(area.width),
                    body.height,
                ),
                layer: SidebarLayer::Overlay,
            }),
        ),
    };
    let (tabs, panel) = if tier.shows_tab_strip() {
        let [tabs, panel] =
            Layout::vertical([Constraint::Length(TAB_STRIP_ROWS), Constraint::Min(0)])
                .areas(workbench);
        (tabs, panel)
    } else {
        (Rect::new(workbench.x, workbench.y, 0, 0), workbench)
    };

    let status_left = sidebar.map_or(workbench.x, |sidebar| sidebar.area.right());
    let status = Rect::new(
        status_left,
        area.bottom().saturating_sub(STATUS_ROWS),
        area.right().saturating_sub(status_left),
        STATUS_ROWS.min(area.height),
    );
    ShellAreas {
        body,
        panel,
        sidebar,
        status,
        tabs,
        tier,
        workbench,
    }
}

fn effective_sidebar_surface(
    tier: ViewportTier,
    surface: SidebarSurface,
    columns: u16,
) -> SidebarSurface {
    let width = match surface {
        SidebarSurface::Docked(width) | SidebarSurface::Overlay(width) => Some(width),
        SidebarSurface::Hidden => None,
    };
    match tier {
        ViewportTier::TooSmall | ViewportTier::Minimal => SidebarSurface::Hidden,
        // A docked sidebar stays beside the content while the content keeps the Compact minimum;
        // only a narrower host, or an explicit overlay request, covers the content instead.
        ViewportTier::Compact => match surface {
            SidebarSurface::Docked(width)
                if columns.saturating_sub(width.effective_columns(columns))
                    >= COMPACT_MINIMUM_COLUMNS =>
            {
                surface
            }
            _ => width.map_or(SidebarSurface::Hidden, SidebarSurface::Overlay),
        },
        ViewportTier::Full => width.map_or(SidebarSurface::Hidden, SidebarSurface::Docked),
    }
}

/// The floating status chip's bounds: the last row beside a visible sidebar, one cell in from each
/// end. TooSmall keeps the whole row for its full-width exit hint.
pub(crate) const fn status_chip_area(areas: &ShellAreas) -> Rect {
    let status = areas.status;
    if matches!(areas.tier, ViewportTier::TooSmall) || status.width < 2 {
        return status;
    }
    Rect::new(status.x + 1, status.y, status.width - 2, status.height)
}

pub(crate) fn place_overlay(
    area: Rect,
    preferred_width: u16,
    preferred_height: u16,
) -> Option<Rect> {
    let width = preferred_width.min(area.width.saturating_sub(OVERLAY_HORIZONTAL_MARGIN));
    let height = preferred_height.min(area.height.saturating_sub(OVERLAY_VERTICAL_MARGIN));
    if width < MINIMUM_OVERLAY_INNER_COLUMNS.saturating_add(2)
        || height < MINIMUM_OVERLAY_INNER_ROWS.saturating_add(2)
    {
        return None;
    }
    Some(Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaneDivision {
    pub(crate) first: Rect,
    pub(crate) divider: Rect,
    pub(crate) second: Rect,
}

pub(crate) fn divide_panes(
    area: Rect,
    orientation: PanelOrientation,
    ratio: ClientSplitRatio,
) -> Option<PaneDivision> {
    match orientation {
        PanelOrientation::Horizontal => {
            let share = area.width.saturating_sub(1);
            let first_width = split_extent(share, MINIMUM_PANE_COLUMNS, ratio)?;
            let second_width = share.saturating_sub(first_width);
            if first_width < MINIMUM_PANE_COLUMNS
                || second_width < MINIMUM_PANE_COLUMNS
                || area.height < MINIMUM_PANE_ROWS
            {
                return None;
            }
            Some(PaneDivision {
                first: Rect::new(area.x, area.y, first_width, area.height),
                divider: Rect::new(area.x.saturating_add(first_width), area.y, 1, area.height),
                second: Rect::new(
                    area.x.saturating_add(first_width).saturating_add(1),
                    area.y,
                    second_width,
                    area.height,
                ),
            })
        }
        PanelOrientation::Vertical => {
            let share = area.height.saturating_sub(1);
            let first_height = split_extent(share, MINIMUM_PANE_ROWS, ratio)?;
            let second_height = share.saturating_sub(first_height);
            if area.width < MINIMUM_PANE_COLUMNS
                || first_height < MINIMUM_PANE_ROWS
                || second_height < MINIMUM_PANE_ROWS
            {
                return None;
            }
            Some(PaneDivision {
                first: Rect::new(area.x, area.y, area.width, first_height),
                divider: Rect::new(area.x, area.y.saturating_add(first_height), area.width, 1),
                second: Rect::new(
                    area.x,
                    area.y.saturating_add(first_height).saturating_add(1),
                    area.width,
                    second_height,
                ),
            })
        }
    }
}

// The admitted ratio is finite and positive and share is u16, so rounding
// cannot escape 0..=65535 before the minimum-size clamp.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn split_extent(share: u16, minimum: u16, ratio: ClientSplitRatio) -> Option<u16> {
    if share < minimum * 2 {
        return None;
    }
    Some(((f64::from(share) * ratio.value()).round() as u16).clamp(minimum, share - minimum))
}

/// Reserve a stable chrome column without reducing a one-column terminal to zero.
const fn scrollbar_content(area: Rect) -> Rect {
    Rect::new(
        area.x,
        area.y,
        area.width.saturating_sub((area.width > 1) as u16),
        area.height,
    )
}

pub(crate) fn divider_glyph(up: bool, down: bool, left: bool, right: bool) -> char {
    match (up, down, left, right) {
        (true, true, false, false) | (true, false, false, false) | (false, true, false, false) => {
            '│'
        }
        (false, false, true, true) | (false, false, true, false) | (false, false, false, true) => {
            '─'
        }
        (true, true, true, false) => '┤',
        (true, true, false, true) => '├',
        (false, true, true, true) => '┬',
        (true, false, true, true) => '┴',
        (true, false, true, false) => '┘',
        (true, false, false, true) => '└',
        (false, true, true, false) => '┐',
        (false, true, false, true) => '┌',
        (true, true, true, true) => '┼',
        (false, false, false, false) => '│',
    }
}

#[cfg(test)]
mod tests;

/// The honest terminal content area inside `panel`: inset by one cell only
/// when the panel frame is actually drawn (`tier.shows_panel_frame()`). At
/// Minimal and TooSmall no frame exists, so no cells are deducted — the rect
/// equals what is painted and what the session is resized to. A degenerate
/// panel yields an empty rect; callers must treat that as "cannot present",
/// never as a resize target.
pub(crate) const fn terminal_area(panel: Rect, framed: bool) -> Rect {
    if framed {
        Rect::new(
            panel.x.saturating_add(1),
            panel.y.saturating_add(1),
            panel.width.saturating_sub(2),
            panel.height.saturating_sub(2),
        )
    } else {
        panel
    }
}

/// How a Tab's visible panes occupy the panel area. Titles sit on frame and divider lines, so a
/// pane never gives up a terminal row for its title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneFrame {
    /// One pane fills the panel without a frame or title.
    Bare,
    /// The zoomed pane fills the outer frame and titles itself on the frame's top line.
    Zoomed,
    /// The split tree fills the outer frame; each leaf titles itself on the line above it.
    Split,
}

impl PaneFrame {
    /// Frames appear only where the tier draws them and more than one pane needs telling apart.
    pub const fn of(framed: bool, panel_count: usize, zoomed: bool) -> Self {
        if !framed || panel_count < 2 {
            Self::Bare
        } else if zoomed {
            Self::Zoomed
        } else {
            Self::Split
        }
    }
}

/// A split returns the tree root; a bare or zoomed pane returns its terminal cells.
/// Paint, PTY geometry and layout queries all consume this projection.
pub const fn native_terminal_area(panel: Rect, frame: PaneFrame) -> Rect {
    match frame {
        PaneFrame::Bare => scrollbar_content(panel),
        PaneFrame::Zoomed => scrollbar_content(terminal_area(panel, true)),
        PaneFrame::Split => terminal_area(panel, true),
    }
}

/// One visible leaf's geometry: the outer rect, whose title sits on the line above it, and the
/// terminal content rect the session may resize to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LeafArea {
    pub(crate) panel_index: usize,
    pub(crate) leaf: Rect,
    pub(crate) content: Rect,
}

/// The single source of truth for a panel tree's screen geometry: every
/// visible leaf and shared divider, projected from the same walk the renderer
/// paints. Session geometry must consume `leaves` — never a recomputed path —
/// so a drawn pane and a resized pane cannot diverge.
#[derive(Clone, Debug, Default)]
pub(crate) struct PanelTreeProjection<'a> {
    pub(crate) leaves: Vec<LeafArea>,
    pub(crate) dividers: Vec<Rect>,
    pub(crate) splits: Vec<SplitArea<'a>>,
}

/// Canonical split identity and its resolved geometry; collapsed descendants have no area.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SplitArea<'a> {
    pub(crate) split_id: &'a WorkbenchSplitId,
    pub(crate) first_slot_id: &'a WorkbenchPanelSlotId,
    pub(crate) second_slot_id: &'a WorkbenchPanelSlotId,
    pub(crate) orientation: PanelOrientation,
    pub(crate) ratio: ClientSplitRatio,
    pub(crate) area: Option<Rect>,
    pub(crate) divider: Option<Rect>,
}

/// Project `tree` onto `area` exactly the way [`render_panel_tree`] paints it:
/// recursive Client-weighted splits separated by one shared divider cell, falling back
/// to the active leaf (or the first leaf) when a split cannot fit its minimum
/// pane size. `active_panel` selects the fallback leaf and identifies which
/// leaf the live terminal attaches to.
pub(crate) fn project_panel_tree<'a>(
    area: Rect,
    tree: &'a ClientPanelTree,
    active_panel: Option<usize>,
    ratios: &impl Fn(
        &WorkbenchSplitId,
        &WorkbenchPanelSlotId,
        &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
) -> PanelTreeProjection<'a> {
    let mut projection = PanelTreeProjection::default();
    collect_panel_tree(area, tree, active_panel, ratios, &mut projection);
    projection
}

fn collect_panel_tree<'a>(
    area: Rect,
    tree: &'a ClientPanelTree,
    active_panel: Option<usize>,
    ratios: &impl Fn(
        &WorkbenchSplitId,
        &WorkbenchPanelSlotId,
        &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
    projection: &mut PanelTreeProjection<'a>,
) {
    match tree {
        ClientPanelTree::Panel { panel_index } => projection.leaves.push(LeafArea {
            panel_index: *panel_index,
            leaf: area,
            content: scrollbar_content(area),
        }),
        ClientPanelTree::Split {
            first,
            second,
            orientation,
            split_id,
            first_slot_id,
            second_slot_id,
            ..
        } => {
            let ratio = ratios(split_id, first_slot_id, second_slot_id);
            let division = divide_panes(area, *orientation, ratio);
            projection.splits.push(SplitArea {
                split_id,
                first_slot_id,
                second_slot_id,
                orientation: *orientation,
                ratio,
                area: (!area.is_empty()).then_some(area),
                divider: division.map(|division| division.divider),
            });
            match division {
                Some(division) => {
                    projection.dividers.push(division.divider);
                    collect_panel_tree(division.first, first, active_panel, ratios, projection);
                    collect_panel_tree(division.second, second, active_panel, ratios, projection);
                }
                None => {
                    collect_hidden_splits(first, ratios, &mut projection.splits);
                    collect_hidden_splits(second, ratios, &mut projection.splits);
                    if let Some(ClientPanelTree::Panel { panel_index }) =
                        active_leaf(tree, active_panel).or_else(|| first_leaf(tree))
                    {
                        projection.leaves.push(LeafArea {
                            panel_index: *panel_index,
                            leaf: area,
                            content: scrollbar_content(area),
                        });
                    }
                }
            }
        }
    }
}

fn collect_hidden_splits<'a>(
    tree: &'a ClientPanelTree,
    ratios: &impl Fn(
        &WorkbenchSplitId,
        &WorkbenchPanelSlotId,
        &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
    output: &mut Vec<SplitArea<'a>>,
) {
    if let ClientPanelTree::Split {
        first,
        second,
        split_id,
        first_slot_id,
        second_slot_id,
        orientation,
        ..
    } = tree
    {
        output.push(SplitArea {
            split_id,
            first_slot_id,
            second_slot_id,
            orientation: *orientation,
            ratio: ratios(split_id, first_slot_id, second_slot_id),
            area: None,
            divider: None,
        });
        collect_hidden_splits(first, ratios, output);
        collect_hidden_splits(second, ratios, output);
    }
}

fn active_leaf(tree: &ClientPanelTree, active: Option<usize>) -> Option<&ClientPanelTree> {
    match tree {
        ClientPanelTree::Panel { panel_index } => (Some(*panel_index) == active).then_some(tree),
        ClientPanelTree::Split { first, second, .. } => {
            active_leaf(first, active).or_else(|| active_leaf(second, active))
        }
    }
}

fn first_leaf(tree: &ClientPanelTree) -> Option<&ClientPanelTree> {
    match tree {
        ClientPanelTree::Panel { .. } => Some(tree),
        ClientPanelTree::Split { first, .. } => first_leaf(first),
    }
}

/// The content rect of the leaf the live terminal attaches to: the active
/// panel when it is projected, otherwise the first projected leaf (the same
/// fallback the renderer uses when the active leaf cannot be placed).
#[cfg(any(test, feature = "test-support"))]
impl PanelTreeProjection<'_> {
    pub(crate) fn active_content(&self, active_panel: Option<usize>) -> Option<Rect> {
        self.leaves
            .iter()
            .find(|leaf| Some(leaf.panel_index) == active_panel)
            .or_else(|| self.leaves.first())
            .map(|leaf| leaf.content)
    }
}

/// Crate-internal `DividerDrag` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait DividerDragTestAccess: Sized {
    fn target(&self) -> &DrawnDivider;
    fn ratio(&self) -> ClientSplitRatio;
    fn begin(target: DrawnDivider, column: u16, row: u16) -> Option<Self>;
}

#[cfg(any(test, feature = "test-support"))]
impl DividerDragTestAccess for DividerDrag {
    fn target(&self) -> &DrawnDivider {
        &self.target
    }
    fn ratio(&self) -> ClientSplitRatio {
        self.ratio
    }
    fn begin(target: DrawnDivider, column: u16, row: u16) -> Option<Self> {
        DividerDrag::begin(target, column, row)
    }
}

/// Crate-internal `SidebarWidth` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait SidebarWidthTestAccess: Sized {
    const DEFAULT: Self;
}

#[cfg(any(test, feature = "test-support"))]
impl SidebarWidthTestAccess for SidebarWidth {
    const DEFAULT: Self = SidebarWidth::DEFAULT;
}

/// Crate-internal `DrawnSidebarWidth` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait DrawnSidebarWidthTestAccess {
    fn edge(self) -> Rect;
}

#[cfg(any(test, feature = "test-support"))]
impl DrawnSidebarWidthTestAccess for DrawnSidebarWidth {
    fn edge(self) -> Rect {
        DrawnSidebarWidth::edge(self)
    }
}

/// Crate-internal `ShellAreas` regions the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait ShellAreasTestAccess {
    fn status(&self) -> Rect;
    fn tabs(&self) -> Rect;
}

#[cfg(any(test, feature = "test-support"))]
impl ShellAreasTestAccess for ShellAreas {
    fn status(&self) -> Rect {
        self.status
    }
    fn tabs(&self) -> Rect {
        self.tabs
    }
}

/// Crate-internal `SidebarArea` regions the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait SidebarAreaTestAccess {
    fn area(&self) -> Rect;
}

#[cfg(any(test, feature = "test-support"))]
impl SidebarAreaTestAccess for SidebarArea {
    fn area(&self) -> Rect {
        self.area
    }
}

/// Crate-internal `DrawnDivider` geometry the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait DrawnDividerTestAccess {
    fn divider(&self) -> Rect;
}

#[cfg(any(test, feature = "test-support"))]
impl DrawnDividerTestAccess for DrawnDivider {
    fn divider(&self) -> Rect {
        self.divider
    }
}

/// Field-for-field `DividerSource` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct DividerSourceFixture {
    pub session: orchbee_workbench_protocol::SessionId,
    pub context: orchbee_workbench_protocol::WorkbenchTerminalContextId,
    pub tab: orchbee_workbench_protocol::WorkbenchTabId,
    pub focus: orchbee_workbench_protocol::WorkbenchPanelId,
    pub revision: orchbee_workbench_protocol::WorkbenchRevision,
}

#[cfg(any(test, feature = "test-support"))]
impl From<DividerSourceFixture> for DividerSource {
    fn from(fixture: DividerSourceFixture) -> Self {
        let DividerSourceFixture {
            session,
            context,
            tab,
            focus,
            revision,
        } = fixture;
        Self {
            session,
            context,
            tab,
            focus,
            revision,
        }
    }
}

/// Field-for-field `DrawnDivider` fixture the application's tests build values from.
#[cfg(any(test, feature = "test-support"))]
pub struct DrawnDividerFixture {
    pub source: DividerSource,
    pub viewport: Rect,
    pub split: orchbee_workbench_protocol::WorkbenchSplitId,
    pub first: orchbee_workbench_protocol::WorkbenchPanelSlotId,
    pub second: orchbee_workbench_protocol::WorkbenchPanelSlotId,
    pub orientation: orchbee_workbench_protocol::PanelOrientation,
    pub ratio: orchbee_workbench_protocol::ClientSplitRatio,
    pub area: Rect,
    pub divider: Rect,
}

#[cfg(any(test, feature = "test-support"))]
impl From<DrawnDividerFixture> for DrawnDivider {
    fn from(fixture: DrawnDividerFixture) -> Self {
        let DrawnDividerFixture {
            source,
            viewport,
            split,
            first,
            second,
            orientation,
            ratio,
            area,
            divider,
        } = fixture;
        Self {
            source,
            viewport,
            split,
            first,
            second,
            orientation,
            ratio,
            area,
            divider,
        }
    }
}
