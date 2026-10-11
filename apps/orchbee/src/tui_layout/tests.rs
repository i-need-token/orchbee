use ratatui::layout::Rect;

use orchbee_workbench_protocol::{
    ClientSplitRatio, PanelOrientation, WorkbenchPanelSlotId, WorkbenchSplitId,
};

use crate::{client_view::SidebarSurface, terminal_session::NativePanelTree};

use super::{
    LeafArea, SidebarLayer, SidebarWidth, ViewportTier, centered_overlay, divide_panes,
    divider_glyph, overlay_area, pane_content, place_overlay, project_panel_tree,
    project_shell_areas, terminal_area,
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
fn docks_the_sidebar_beside_full_height_content_and_an_overlaid_status() {
    let viewport = Rect::new(0, 0, 120, 40);
    let areas = project_shell_areas(viewport, SidebarSurface::Docked(SidebarWidth::DEFAULT));

    assert_eq!(areas.tier, ViewportTier::Full);
    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Docked)
    );
    assert_eq!(
        areas.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(0, 0, SidebarWidth::DEFAULT.columns(), 40))
    );
    assert_eq!(areas.workbench, Rect::new(30, 0, 90, 40));
    assert_eq!(areas.tabs, Rect::new(30, 0, 90, 1));
    assert_eq!(areas.panel, Rect::new(30, 1, 90, 39));
    assert_eq!(areas.status, Rect::new(30, 39, 90, 1));
    assert_eq!(
        terminal_area(areas.panel, areas.tier.shows_panel_frame()),
        Rect::new(31, 2, 88, 37)
    );
}

#[test]
fn overlays_the_left_edge_without_reflowing_the_compact_workbench() {
    let viewport = Rect::new(4, 2, 70, 20);
    let hidden = project_shell_areas(viewport, SidebarSurface::Hidden);
    let overlay = project_shell_areas(viewport, SidebarSurface::Overlay(SidebarWidth::DEFAULT));
    let docked_intent =
        project_shell_areas(viewport, SidebarSurface::Docked(SidebarWidth::DEFAULT));

    assert_eq!(hidden.tier, ViewportTier::Compact);
    assert_eq!(hidden.workbench, overlay.workbench);
    assert_eq!(overlay.workbench, Rect::new(4, 2, 70, 20));
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Overlay)
    );
    assert_eq!(
        overlay.sidebar.map(|sidebar| sidebar.area),
        Some(Rect::new(4, 2, SidebarWidth::DEFAULT.columns(), 20))
    );
    // A docked intent still presents as an overlay at Compact width.
    assert_eq!(
        docked_intent.sidebar.map(|sidebar| sidebar.layer),
        Some(SidebarLayer::Overlay)
    );
}

#[test]
fn drops_the_tab_strip_and_sidebar_at_minimal_and_too_small() {
    let minimal = project_shell_areas(
        Rect::new(0, 0, 50, 10),
        SidebarSurface::Overlay(SidebarWidth::DEFAULT),
    );
    assert_eq!(minimal.tier, ViewportTier::Minimal);
    assert!(minimal.tabs.is_empty());
    assert_eq!(minimal.panel, Rect::new(0, 0, 50, 10));
    assert_eq!(minimal.sidebar, None);
    assert_eq!(minimal.status, Rect::new(0, 9, 50, 1));

    let tiny = project_shell_areas(
        Rect::new(0, 0, 15, 4),
        SidebarSurface::Docked(SidebarWidth::DEFAULT),
    );
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
    let areas = project_shell_areas(viewport, SidebarSurface::Docked(SidebarWidth::DEFAULT));
    let content = overlay_area(&areas);

    assert_eq!(content, Rect::new(30, 1, 90, 38));
    assert!(areas.panel.intersects(areas.status));
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
    let division =
        divide_panes(area, PanelOrientation::Horizontal, ClientSplitRatio::HALF).unwrap();

    assert_eq!(division.divider, Rect::new(12, 1, 1, 9));
    assert_eq!(division.first, Rect::new(2, 1, 10, 9));
    assert_eq!(division.second, Rect::new(13, 1, 10, 9));
    assert!(!division.first.intersects(division.divider));
    assert!(!division.second.intersects(division.divider));

    let vertical = divide_panes(area, PanelOrientation::Vertical, ClientSplitRatio::HALF).unwrap();
    assert_eq!(vertical.divider, Rect::new(2, 5, 21, 1));
    assert_eq!(vertical.first, Rect::new(2, 1, 21, 4));
    assert_eq!(vertical.second, Rect::new(2, 6, 21, 4));
}

#[test]
fn refuses_pane_division_below_the_leaf_minimum() {
    assert_eq!(
        divide_panes(
            Rect::new(0, 0, 8, 9),
            PanelOrientation::Horizontal,
            ClientSplitRatio::HALF
        ),
        None
    );
    assert_eq!(
        divide_panes(
            Rect::new(0, 0, 20, 6),
            PanelOrientation::Vertical,
            ClientSplitRatio::HALF
        ),
        None
    );
    assert_eq!(
        divide_panes(
            Rect::new(0, 0, 9, 2),
            PanelOrientation::Horizontal,
            ClientSplitRatio::HALF
        ),
        None
    );
}

#[test]
fn reserves_a_title_row_and_scrollbar_column_inside_each_leaf() {
    let leaf = Rect::new(3, 2, 12, 7);
    assert_eq!(pane_content(leaf), Rect::new(3, 3, 11, 6));
    assert_eq!(pane_content(Rect::new(0, 0, 1, 2)), Rect::new(0, 1, 1, 1));
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

fn tree_id(value: u128) -> String {
    format!("{value:032x}")
}

fn panel(index: usize) -> NativePanelTree {
    NativePanelTree::Panel { panel_index: index }
}

fn split(
    first: NativePanelTree,
    second: NativePanelTree,
    orientation: PanelOrientation,
    id: u128,
) -> NativePanelTree {
    NativePanelTree::Split {
        ratio: Default::default(),
        first: Box::new(first),
        second: Box::new(second),
        first_slot_id: WorkbenchPanelSlotId::parse(&tree_id(id)).unwrap(),
        second_slot_id: WorkbenchPanelSlotId::parse(&tree_id(id + 1)).unwrap(),
        orientation,
        split_id: WorkbenchSplitId::parse(&tree_id(id + 2)).unwrap(),
    }
}

#[test]
fn projects_a_single_panel_to_one_leaf() {
    let tree = panel(2);
    let projection = project_panel_tree(Rect::new(1, 1, 90, 38), &tree, Some(0), &|_, _, _| {
        ClientSplitRatio::HALF
    });

    assert_eq!(
        projection.leaves,
        vec![LeafArea {
            panel_index: 2,
            leaf: Rect::new(1, 1, 90, 38),
            content: Rect::new(1, 2, 89, 37),
        }]
    );
    assert!(projection.dividers.is_empty());
    assert_eq!(
        projection.active_content(Some(2)),
        Some(Rect::new(1, 2, 89, 37))
    );
}

#[test]
fn projects_a_split_into_leaves_sharing_one_divider() {
    let tree = split(panel(0), panel(1), PanelOrientation::Horizontal, 10);
    let projection = project_panel_tree(Rect::new(0, 0, 21, 9), &tree, Some(1), &|_, _, _| {
        ClientSplitRatio::HALF
    });

    assert_eq!(projection.dividers, vec![Rect::new(10, 0, 1, 9)]);
    assert_eq!(
        projection.leaves,
        vec![
            LeafArea {
                panel_index: 0,
                leaf: Rect::new(0, 0, 10, 9),
                content: Rect::new(0, 1, 9, 8),
            },
            LeafArea {
                panel_index: 1,
                leaf: Rect::new(11, 0, 10, 9),
                content: Rect::new(11, 1, 9, 8),
            },
        ]
    );
    // The selected leaf — not the first — is the Session resize target.
    assert_eq!(
        projection.active_content(Some(1)),
        Some(Rect::new(11, 1, 9, 8))
    );
    assert_eq!(
        projection.active_content(Some(0)),
        Some(Rect::new(0, 1, 9, 8))
    );
    // Falls back to the first projected leaf when nothing matches.
    assert_eq!(projection.active_content(None), Some(Rect::new(0, 1, 9, 8)));
    assert_eq!(
        projection.active_content(Some(99)),
        Some(Rect::new(0, 1, 9, 8))
    );
}

#[test]
fn projects_nested_splits_with_one_divider_per_split() {
    let tree = split(
        panel(0),
        split(panel(1), panel(2), PanelOrientation::Vertical, 20),
        PanelOrientation::Horizontal,
        10,
    );
    let projection = project_panel_tree(Rect::new(0, 0, 41, 19), &tree, Some(2), &|_, _, _| {
        ClientSplitRatio::HALF
    });

    assert_eq!(
        projection.dividers,
        vec![Rect::new(20, 0, 1, 19), Rect::new(21, 9, 20, 1)]
    );
    assert_eq!(
        projection.leaves,
        vec![
            LeafArea {
                panel_index: 0,
                leaf: Rect::new(0, 0, 20, 19),
                content: Rect::new(0, 1, 19, 18),
            },
            LeafArea {
                panel_index: 1,
                leaf: Rect::new(21, 0, 20, 9),
                content: Rect::new(21, 1, 19, 8),
            },
            LeafArea {
                panel_index: 2,
                leaf: Rect::new(21, 10, 20, 9),
                content: Rect::new(21, 11, 19, 8),
            },
        ]
    );
    assert_eq!(
        projection.active_content(Some(2)),
        Some(Rect::new(21, 11, 19, 8))
    );
}

#[test]
fn collapses_an_unfittable_split_to_the_selected_leaf() {
    let tree = split(panel(0), panel(1), PanelOrientation::Horizontal, 10);
    // 8 columns cannot fit two >=4-column panes plus a divider.
    let projection = project_panel_tree(Rect::new(0, 0, 8, 9), &tree, Some(1), &|_, _, _| {
        ClientSplitRatio::HALF
    });

    assert_eq!(projection.leaves.len(), 1);
    assert_eq!(projection.leaves[0].panel_index, 1);
    assert_eq!(projection.leaves[0].leaf, Rect::new(0, 0, 8, 9));
    assert!(projection.dividers.is_empty());
    assert_eq!(
        projection.active_content(Some(1)),
        Some(Rect::new(0, 1, 7, 8))
    );

    // With no matching selection the first leaf wins.
    let fallback = project_panel_tree(Rect::new(0, 0, 8, 9), &tree, Some(9), &|_, _, _| {
        ClientSplitRatio::HALF
    });
    assert_eq!(fallback.leaves[0].panel_index, 0);
    assert_eq!(
        fallback.active_content(Some(9)),
        Some(Rect::new(0, 1, 7, 8))
    );
}

mod generated;

#[test]
fn weighted_cells_round_once_and_clamp_to_both_child_minimums() {
    for (width, ratio, expected) in [
        (8, 0.1, None),
        (9, 0.1, Some(4)),
        (10, 0.9, Some(5)),
        (22, 0.5, Some(11)),
        (22, 0.25, Some(5)),
        (22, 0.9, Some(17)),
    ] {
        let result = divide_panes(
            Rect::new(7, 9, width, 20),
            PanelOrientation::Horizontal,
            ClientSplitRatio::new(ratio).unwrap(),
        );
        assert_eq!(
            result.map(|division| division.first.width),
            expected,
            "{width} {ratio}"
        );
        if let Some(division) = result {
            assert_eq!(division.first.width + division.second.width + 1, width);
            assert_eq!(division.second.x, division.divider.x + 1);
        }
    }
    let tree = split(
        panel(0),
        split(panel(1), panel(2), PanelOrientation::Vertical, 200),
        PanelOrientation::Horizontal,
        100,
    );
    let ratio = ClientSplitRatio::new(0.25).unwrap();
    let projection = project_panel_tree(Rect::new(0, 0, 41, 21), &tree, Some(2), &|_, _, _| ratio);
    assert_eq!(projection.leaves[0].leaf, Rect::new(0, 0, 10, 21));
    assert_eq!(projection.leaves[1].leaf, Rect::new(11, 0, 30, 5));
    assert_eq!(projection.leaves[2].leaf, Rect::new(11, 6, 30, 15));
    assert!(projection.splits.iter().all(|split| split.ratio == ratio));
    let hidden = project_panel_tree(Rect::new(0, 0, 8, 6), &tree, Some(2), &|_, _, _| ratio);
    assert_eq!(hidden.leaves.len(), 1);
    assert!(hidden.splits.iter().all(|split| split.ratio == ratio));
    assert!(hidden.splits[1].area.is_none());
}
