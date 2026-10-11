use super::*;
use crate::tui_layout::{SidebarLayer, ViewportTier, terminal_area};

fn width(columns: u16) -> SidebarWidth {
    SidebarWidth::new(columns).unwrap()
}

#[test]
fn sidebar_width_bounds_and_tiers_share_one_offset_geometry_projection() {
    for columns in [0, 23, 61, u16::MAX] {
        assert_eq!(SidebarWidth::new(columns), None);
    }
    assert_eq!(SidebarWidth::default().columns(), 30);
    for preferred in [24, 30, 60] {
        for (columns, rows) in [
            (120, 40),
            (100, 24),
            (99, 24),
            (60, 12),
            (120, 23),
            (120, 11),
            (59, 40),
            (15, 4),
        ] {
            let viewport = Rect::new(7, 3, columns, rows);
            for surface in [
                SidebarSurface::Docked(width(preferred)),
                SidebarSurface::Overlay(width(preferred)),
                SidebarSurface::Hidden,
            ] {
                let areas = project_shell_areas(viewport, surface);
                let target = DrawnSidebarWidth::new(viewport, surface);
                assert_eq!(areas.status.bottom(), viewport.bottom());
                assert!(areas.panel.intersects(areas.status));
                assert!(
                    areas
                        .sidebar
                        .is_none_or(|sidebar| !sidebar.area.intersects(areas.status))
                );
                if surface == SidebarSurface::Hidden || areas.tier < ViewportTier::Compact {
                    assert_eq!(target, None);
                    assert_eq!(areas.sidebar, None);
                    continue;
                }
                let target = target.unwrap();
                assert_eq!(target.sidebar, areas.sidebar.unwrap());
                assert_eq!(target.preferred(), width(preferred));
                let effective = preferred.min(columns / 2);
                assert_eq!(target.sidebar.area, Rect::new(7, 3, effective, rows));
                assert_eq!(target.edge(), Rect::new(7 + effective - 1, 3, 1, rows));
                match target.sidebar.layer {
                    SidebarLayer::Docked => {
                        assert_eq!(areas.workbench.x, target.sidebar.area.right());
                        assert_eq!(areas.workbench.width + effective, columns);
                    }
                    SidebarLayer::Overlay => {
                        assert_eq!(areas.workbench, areas.body);
                    }
                }
                assert!(!terminal_area(areas.panel, true).is_empty());
            }
        }
    }
}

#[test]
fn constrained_width_restores_preference_and_never_confuses_equal_rectangles() {
    let surface = SidebarSurface::Docked(width(60));
    let projected: Vec<_> = [120, 60, 50, 120]
        .into_iter()
        .map(|columns| {
            DrawnSidebarWidth::new(Rect::new(4, 2, columns, 40), surface)
                .map(|target| (target.sidebar.area.width, target.preferred()))
        })
        .collect();
    assert_eq!(
        projected,
        vec![
            Some((60, width(60))),
            Some((30, width(60))),
            None,
            Some((60, width(60)))
        ]
    );
    let viewport = Rect::new(4, 2, 60, 12);
    let old = DrawnSidebarWidth::new(viewport, surface).unwrap();
    let newer = DrawnSidebarWidth::new(viewport, SidebarSurface::Docked(width(40))).unwrap();
    assert_eq!(old.sidebar, newer.sidebar);
    assert_ne!(old, newer, "painted identity includes the preferred width");
    assert_eq!(old.adjusted(SidebarWidthChange::Wider), width(60));
    assert_eq!(old.adjusted(SidebarWidthChange::Narrower), width(28));
    assert_eq!(old.adjusted(SidebarWidthChange::Reset), width(30));
    let minimum = DrawnSidebarWidth::new(viewport, SidebarSurface::Overlay(width(24))).unwrap();
    assert_eq!(minimum.adjusted(SidebarWidthChange::Narrower), width(24));
}

#[test]
fn sidebar_drag_requires_the_painted_edge_and_uses_absolute_bounded_displacement() {
    for preferred in [24, 30, 60] {
        for columns in [60, 99, 100, 120] {
            let target = DrawnSidebarWidth::new(
                Rect::new(7, 3, columns, 40),
                SidebarSurface::Docked(width(preferred)),
            )
            .unwrap();
            let edge = target.edge();
            assert!(SidebarWidthDrag::begin(target, edge.x - 1, edge.y).is_none());
            assert!(SidebarWidthDrag::begin(target, edge.x, edge.bottom()).is_none());
            let mut drag = SidebarWidthDrag::begin(target, edge.x, edge.y + 1).unwrap();
            for column in [0, 24, 41, 150, u16::MAX, edge.x] {
                drag.update(column);
                let mut direct = SidebarWidthDrag::begin(target, edge.x, edge.y + 1).unwrap();
                direct.update(column);
                assert_eq!(
                    drag, direct,
                    "motion never accumulates rounding or clamping"
                );
                assert!((24..=60).contains(&drag.width.columns()));
                assert_eq!(drag.preview.y, edge.y);
                assert_eq!(drag.preview.height, edge.height);
                assert_eq!(drag.preview.width, 1);
                assert!(drag.preview.x >= target.sidebar.area.x + 23);
                assert!(drag.preview.x < target.viewport.x + columns / 2);
                assert_eq!(drag.target, target, "preview leaves the source immutable");
            }
            assert_eq!(
                drag.width,
                width(preferred),
                "returning to the press preserves preference"
            );
            assert_eq!(drag.preview, edge);
            if target.sidebar.area.width == columns / 2 {
                drag.update(u16::MAX);
                assert_eq!(
                    drag.width,
                    width(preferred),
                    "a clamped no-op keeps hidden preference"
                );
                assert_eq!(drag.preview, edge);
            }
        }
    }
}
