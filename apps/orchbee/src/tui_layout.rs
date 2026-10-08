//! Deterministic shell and pane geometry; no terminal or shared-layout mutations.
mod divider;
mod sidebar;
pub(crate) use divider::{DividerDrag, DividerSource, DrawnDivider};
use ratatui::layout::{Constraint, Layout, Rect};
pub(crate) use sidebar::{DrawnSidebarWidth, SidebarWidth, SidebarWidthChange, SidebarWidthDrag};

use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, WorkbenchPanelSlotId, WorkbenchSplitId,
};

use crate::{client_view::SidebarSurface, terminal_session::NativePanelTree};

pub(crate) const FULL_MINIMUM_COLUMNS: u16 = 100;
pub(crate) const FULL_MINIMUM_ROWS: u16 = 24;
pub(crate) const COMPACT_MINIMUM_COLUMNS: u16 = 60;
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
pub(crate) const PANE_TITLE_ROWS: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ViewportTier {
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

    pub(crate) const fn shows_panel_frame(self) -> bool {
        matches!(self, Self::Compact | Self::Full)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarLayer {
    Docked,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SidebarArea {
    pub(crate) area: Rect,
    pub(crate) layer: SidebarLayer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShellAreas {
    pub(crate) body: Rect,
    pub(crate) panel: Rect,
    pub(crate) sidebar: Option<SidebarArea>,
    pub(crate) status: Rect,
    pub(crate) tabs: Rect,
    pub(crate) tier: ViewportTier,
    pub(crate) workbench: Rect,
}

pub(crate) fn project_shell_areas(area: Rect, sidebar_surface: SidebarSurface) -> ShellAreas {
    let tier = ViewportTier::from_area(area);
    let [body, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(STATUS_ROWS)]).areas(area);
    let surface = effective_sidebar_surface(tier, sidebar_surface);
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

fn effective_sidebar_surface(tier: ViewportTier, surface: SidebarSurface) -> SidebarSurface {
    let width = match surface {
        SidebarSurface::Docked(width) | SidebarSurface::Overlay(width) => Some(width),
        SidebarSurface::Hidden => None,
    };
    match tier {
        ViewportTier::TooSmall | ViewportTier::Minimal => SidebarSurface::Hidden,
        ViewportTier::Compact => width.map_or(SidebarSurface::Hidden, SidebarSurface::Overlay),
        ViewportTier::Full => width.map_or(SidebarSurface::Hidden, SidebarSurface::Docked),
    }
}

pub(crate) const fn overlay_area(areas: &ShellAreas) -> Rect {
    areas.panel
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

pub(crate) fn centered_overlay(area: Rect, maximum_width: u16, maximum_height: u16) -> Rect {
    let width = area
        .width
        .saturating_sub(OVERLAY_HORIZONTAL_MARGIN)
        .min(maximum_width);
    let height = area
        .height
        .saturating_sub(OVERLAY_VERTICAL_MARGIN)
        .min(maximum_height);
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
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

pub(crate) const fn pane_content(leaf: Rect) -> Rect {
    Rect::new(
        leaf.x,
        leaf.y.saturating_add(PANE_TITLE_ROWS),
        leaf.width,
        leaf.height.saturating_sub(PANE_TITLE_ROWS),
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

/// One visible leaf's geometry: the outer rect (including its one-row title)
/// and the terminal content rect the session may resize to.
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
    tree: &'a NativePanelTree,
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
    tree: &'a NativePanelTree,
    active_panel: Option<usize>,
    ratios: &impl Fn(
        &WorkbenchSplitId,
        &WorkbenchPanelSlotId,
        &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
    projection: &mut PanelTreeProjection<'a>,
) {
    match tree {
        NativePanelTree::Panel { panel_index } => projection.leaves.push(LeafArea {
            panel_index: *panel_index,
            leaf: area,
            content: pane_content(area),
        }),
        NativePanelTree::Split {
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
                    if let Some(NativePanelTree::Panel { panel_index }) =
                        active_leaf(tree, active_panel).or_else(|| first_leaf(tree))
                    {
                        projection.leaves.push(LeafArea {
                            panel_index: *panel_index,
                            leaf: area,
                            content: pane_content(area),
                        });
                    }
                }
            }
        }
    }
}

fn collect_hidden_splits<'a>(
    tree: &'a NativePanelTree,
    ratios: &impl Fn(
        &WorkbenchSplitId,
        &WorkbenchPanelSlotId,
        &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
    output: &mut Vec<SplitArea<'a>>,
) {
    if let NativePanelTree::Split {
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

fn active_leaf(tree: &NativePanelTree, active: Option<usize>) -> Option<&NativePanelTree> {
    match tree {
        NativePanelTree::Panel { panel_index } => (Some(*panel_index) == active).then_some(tree),
        NativePanelTree::Split { first, second, .. } => {
            active_leaf(first, active).or_else(|| active_leaf(second, active))
        }
    }
}

fn first_leaf(tree: &NativePanelTree) -> Option<&NativePanelTree> {
    match tree {
        NativePanelTree::Panel { .. } => Some(tree),
        NativePanelTree::Split { first, .. } => first_leaf(first),
    }
}

/// The content rect of the leaf the live terminal attaches to: the active
/// panel when it is projected, otherwise the first projected leaf (the same
/// fallback the renderer uses when the active leaf cannot be placed).
impl PanelTreeProjection<'_> {
    pub(crate) fn active_content(&self, active_panel: Option<usize>) -> Option<Rect> {
        self.leaves
            .iter()
            .find(|leaf| Some(leaf.panel_index) == active_panel)
            .or_else(|| self.leaves.first())
            .map(|leaf| leaf.content)
    }
}
