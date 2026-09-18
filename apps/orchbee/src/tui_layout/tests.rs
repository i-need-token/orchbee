use ratatui::layout::Rect;

use orchbee_workbench_protocol::PanelOrientation;

use crate::client_view::SidebarSurface;

use super::{
    DOCKED_SIDEBAR_COLUMNS, SidebarLayer, ViewportTier, centered_overlay, divide_panes,
    divider_glyph, overlay_area, pane_content, place_overlay, project_shell_areas, terminal_area,
};

#[test]
fn classifies_tiers_by_the_lower_of_the_width_and_height_tiers() {
    let cases = [
        (120, 40, ViewportTier::Full),
        (100, 24, ViewportTier::Full),
        (99, 24, ViewportTier::Compact),
        (100, 23, ViewportTier::Compact),
        (80, 24, ViewportTier::Compact),
        (60, 12, ViewportTier::Compact),
        (120, 10, ViewportTier::Minimal),
        (59, 40, ViewportTier::Minimal),
        (50, 10, ViewportTier::Minimal),
        (20, 5, ViewportTier::Minimal),
        (19, 40, ViewportTier::TooSmall),
        (120, 4, ViewportTier::TooSmall),
        (15, 4, ViewportTier::TooSmall),
    ];
    for (columns, rows, expected) in cases {
        assert_eq!(
            ViewportTier::from_size(columns, rows),
            expected,
            "{columns}x{rows}"
        );
    }
}

#[test]
fn docks_a_fixed_business_sidebar_without_reducing_the_status_width() {
    let viewport = Rect::new(0, 0, 120, 40);
    let areas = project_shell_areas(viewport, SidebarSurface::Docked);

    assert_eq!(areas.tier, ViewportTier::Full);
    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Docked)
    );
    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(0, 0, DOCKED_SIDEBAR_COLUMNS, 39))
    );
    assert_eq!(areas.workbench, Rect::new(30, 0, 90, 39));
    assert_eq!(areas.tabs, Rect::new(30, 0, 90, 1));
    assert_eq!(areas.panel, Rect::new(30, 1, 90, 38));
    assert_eq!(areas.status, Rect::new(0, 39, 120, 1));
    assert_eq!(
        terminal_area(areas.panel, areas.tier.shows_panel_frame()),
        Rect::new(31, 2, 88, 36)
    );
}

#[test]
fn overlays_the_left_edge_without_reflowing_the_compact_workbench() {
    let viewport = Rect::new(4, 2, 70, 20);
    let hidden = project_shell_areas(viewport, SidebarSurface::Hidden);
    let overlay = project_shell_areas(viewport, SidebarSurface::Overlay);
    let docked_intent = project_shell_areas(viewport, SidebarSurface::Docked);

    assert_eq!(hidden.tier, ViewportTier::Compact);
    assert_eq!(hidden.workbench, overlay.workbench);
    assert_eq!(overlay.workbench, Rect::new(4, 2, 70, 19));
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Overlay)
    );
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(4, 2, DOCKED_SIDEBAR_COLUMNS, 19))
    );
    // A docked intent still presents as an overlay at Compact width.
    assert_eq!(
        docked_intent.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Overlay)
    );
}

#[test]
fn drops_the_tab_strip_and_sidebar_at_minimal_and_too_small() {
    let minimal = project_shell_areas(Rect::new(0, 0, 50, 10), SidebarSurface::Overlay);
    assert_eq!(minimal.tier, ViewportTier::Minimal);
    assert!(minimal.tabs.is_empty());
    assert_eq!(minimal.panel, Rect::new(0, 0, 50, 9));
    assert_eq!(minimal.sidebar, None);
    assert_eq!(minimal.status, Rect::new(0, 9, 50, 1));

    let tiny = project_shell_areas(Rect::new(0, 0, 15, 4), SidebarSurface::Docked);
    assert_eq!(tiny.tier, ViewportTier::TooSmall);
    assert!(tiny.tabs.is_empty());
    assert_eq!(tiny.sidebar, None);
    assert_eq!(tiny.status, Rect::new(0, 3, 15, 1));
    assert_eq!(tiny.panel, tiny.body);
    // TooSmall draws no panel frame, so the honest content area is the panel
    // itself; `active_pane_area` still withholds it (Ok(None)), so the session
    // keeps its last valid geometry instead of receiving a fabricated resize.
    assert_eq!(
        terminal_area(tiny.panel, tiny.tier.shows_panel_frame()),
        tiny.panel
    );
}

#[test]
fn terminal_area_reports_degenerate_panels_as_empty_never_fabricated() {
    // A framed degenerate panel yields an empty rect; callers must treat that
    // as "cannot present" (Ok(None)) — never floor it to a fake 1x1 resize.
    for panel in [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 15, 1),
        Rect::new(0, 0, 2, 30),
        Rect::new(4, 2, 30, 2),
    ] {
        assert!(
            terminal_area(panel, true).is_empty(),
            "framed {panel:?} must not fabricate a resize target"
        );
    }
    // Without a drawn frame no cells are deducted: the honest rect is the
    // panel itself.
    let unframed = Rect::new(0, 0, 15, 3);
    assert_eq!(terminal_area(unframed, false), unframed);
}

#[test]
fn keeps_overlay_geometry_inside_the_content_area_and_never_the_status() {
    let viewport = Rect::new(0, 0, 120, 40);
    let areas = project_shell_areas(viewport, SidebarSurface::Docked);
    let content = overlay_area(&areas);

    assert_eq!(content, areas.panel);
    assert!(!content.intersects(areas.status));

    let placed = place_overlay(content, u16::MAX, u16::MAX).unwrap();
    assert_eq!(placed, Rect::new(32, 2, 86, 36));
    assert!(!placed.intersects(areas.status));
}

#[test]
fn refuses_overlays_below_the_minimum_inner_area() {
    let areas = project_shell_areas(Rect::new(0, 0, 50, 10), SidebarSurface::Hidden);
    let content = overlay_area(&areas);

    assert!(place_overlay(content, 60, 8).is_some());
    // Inner height would be 2 rows once margins and the border are removed.
    assert_eq!(place_overlay(Rect::new(0, 0, 20, 6), 10, 4), None);
    assert_eq!(place_overlay(Rect::new(0, 0, 9, 20), 7, 10), None);
    assert_eq!(place_overlay(Rect::new(0, 0, 4, 4), 2, 2), None);
}

#[test]
fn centers_overlays_toward_the_top_left_cell() {
    let area = Rect::new(0, 0, 21, 11);
    let placed = place_overlay(area, 10, 5).unwrap();

    // Leftover margins are 11 columns and 6 rows; the extra cell goes bottom-right.
    assert_eq!(placed, Rect::new(5, 3, 10, 5));
    assert!(placed.right() <= area.right() && placed.bottom() <= area.bottom());
}

#[test]
fn divides_panes_with_one_shared_divider_cell() {
    let area = Rect::new(2, 1, 21, 9);
    let division = divide_panes(area, PanelOrientation::Horizontal).unwrap();

    assert_eq!(division.divider, Rect::new(12, 1, 1, 9));
    assert_eq!(division.first, Rect::new(2, 1, 10, 9));
    assert_eq!(division.second, Rect::new(13, 1, 10, 9));
    assert!(!division.first.intersects(division.divider));
    assert!(!division.second.intersects(division.divider));

    let vertical = divide_panes(area, PanelOrientation::Vertical).unwrap();
    assert_eq!(vertical.divider, Rect::new(2, 5, 21, 1));
    assert_eq!(vertical.first, Rect::new(2, 1, 21, 4));
    assert_eq!(vertical.second, Rect::new(2, 6, 21, 4));
}

#[test]
fn refuses_pane_division_below_the_leaf_minimum() {
    assert_eq!(
        divide_panes(Rect::new(0, 0, 8, 9), PanelOrientation::Horizontal),
        None
    );
    assert_eq!(
        divide_panes(Rect::new(0, 0, 20, 6), PanelOrientation::Vertical),
        None
    );
    assert_eq!(
        divide_panes(Rect::new(0, 0, 9, 2), PanelOrientation::Horizontal),
        None
    );
}

#[test]
fn reserves_a_title_row_inside_each_leaf() {
    let leaf = Rect::new(3, 2, 12, 7);
    assert_eq!(pane_content(leaf), Rect::new(3, 3, 12, 6));
}

#[test]
fn resolves_divider_junction_glyphs_from_neighbouring_cells() {
    assert_eq!(divider_glyph(true, true, false, false), '│');
    assert_eq!(divider_glyph(false, false, true, true), '─');
    assert_eq!(divider_glyph(true, true, true, false), '┤');
    assert_eq!(divider_glyph(true, true, false, true), '├');
    assert_eq!(divider_glyph(false, true, true, true), '┬');
    assert_eq!(divider_glyph(true, false, true, true), '┴');
    assert_eq!(divider_glyph(true, true, true, true), '┼');
}

#[test]
fn clips_legacy_centered_overlays_to_tiny_viewports() {
    let viewport = Rect::new(7, 9, 12, 4);
    let centered = centered_overlay(viewport, 72, 28);

    assert_eq!(centered, Rect::new(9, 10, 8, 2));
}
