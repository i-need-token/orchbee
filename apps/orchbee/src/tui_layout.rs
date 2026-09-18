use ratatui::layout::{Constraint, Layout, Rect};

use orchbee_workbench_protocol::PanelOrientation;

use crate::client_view::SidebarSurface;

pub(crate) const DOCKED_SIDEBAR_COLUMNS: u16 = 30;
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
        SidebarSurface::Docked => {
            let [sidebar, workbench] = Layout::horizontal([
                Constraint::Length(DOCKED_SIDEBAR_COLUMNS),
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
        SidebarSurface::Overlay => (
            body,
            Some(SidebarArea {
                area: Rect::new(
                    body.x,
                    body.y,
                    body.width.min(DOCKED_SIDEBAR_COLUMNS),
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
    let visible = !matches!(surface, SidebarSurface::Hidden);
    match tier {
        ViewportTier::TooSmall | ViewportTier::Minimal => SidebarSurface::Hidden,
        ViewportTier::Compact => {
            if visible {
                SidebarSurface::Overlay
            } else {
                SidebarSurface::Hidden
            }
        }
        ViewportTier::Full => {
            if visible {
                SidebarSurface::Docked
            } else {
                SidebarSurface::Hidden
            }
        }
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

pub(crate) fn divide_panes(area: Rect, orientation: PanelOrientation) -> Option<PaneDivision> {
    match orientation {
        PanelOrientation::Horizontal => {
            let share = area.width.saturating_sub(1);
            let first_width = share.div_ceil(2);
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
            let first_height = share.div_ceil(2);
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

pub(crate) const fn terminal_area(panel: Rect) -> Rect {
    Rect::new(
        panel.x.saturating_add(1),
        panel.y.saturating_add(1),
        panel.width.saturating_sub(2),
        panel.height.saturating_sub(2),
    )
}
