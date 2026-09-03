use ratatui::layout::Rect;

use crate::client_view::SidebarSurface;

use super::{DOCKED_SIDEBAR_COLUMNS, SidebarLayer, centered_overlay, project_shell_areas};

#[test]
fn docks_a_fixed_business_sidebar_without_reducing_the_status_width() {
    let viewport = Rect::new(0, 0, 120, 40);
    let areas = project_shell_areas(viewport, SidebarSurface::Docked);

    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Docked)
    );
    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(0, 0, DOCKED_SIDEBAR_COLUMNS, 38))
    );
    assert_eq!(areas.workbench, Rect::new(30, 0, 90, 38));
    assert_eq!(areas.tabs, Rect::new(30, 0, 90, 3));
    assert_eq!(areas.panel, Rect::new(30, 3, 90, 35));
    assert_eq!(areas.status, Rect::new(0, 38, 120, 2));
}

#[test]
fn overlays_the_left_edge_without_reflowing_the_narrow_workbench() {
    let viewport = Rect::new(4, 2, 70, 24);
    let hidden = project_shell_areas(viewport, SidebarSurface::Hidden);
    let overlay = project_shell_areas(viewport, SidebarSurface::Overlay);

    assert_eq!(hidden.workbench, overlay.workbench);
    assert_eq!(overlay.workbench, Rect::new(4, 2, 70, 22));
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Overlay)
    );
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(4, 2, DOCKED_SIDEBAR_COLUMNS, 22))
    );
}

#[test]
fn clips_sidebar_and_centered_overlays_to_tiny_viewports() {
    let viewport = Rect::new(7, 9, 12, 4);
    let shell = project_shell_areas(viewport, SidebarSurface::Overlay);
    let centered = centered_overlay(viewport, 72, 28);

    assert_eq!(
        shell.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(7, 9, 12, 2))
    );
    assert_eq!(centered, Rect::new(9, 10, 8, 2));
}
