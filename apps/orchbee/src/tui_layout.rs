use ratatui::layout::{Constraint, Layout, Rect};

use crate::client_view::SidebarSurface;

pub(crate) const DOCKED_SIDEBAR_COLUMNS: u16 = 30;

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
    pub(crate) panel: Rect,
    pub(crate) sidebar: Option<SidebarArea>,
    pub(crate) status: Rect,
    pub(crate) tabs: Rect,
    pub(crate) workbench: Rect,
}

pub(crate) fn project_shell_areas(area: Rect, sidebar_surface: SidebarSurface) -> ShellAreas {
    let [body, status] = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);
    let (workbench, sidebar) = match sidebar_surface {
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
    let [tabs, panel] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(workbench);

    ShellAreas {
        panel,
        sidebar,
        status,
        tabs,
        workbench,
    }
}

pub(crate) fn centered_overlay(area: Rect, maximum_width: u16, maximum_height: u16) -> Rect {
    let available_width = area.width.saturating_sub(4);
    let available_height = area.height.saturating_sub(2);
    let width = available_width.min(maximum_width);
    let height = available_height.min(maximum_height);
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
}

#[cfg(test)]
mod tests;
