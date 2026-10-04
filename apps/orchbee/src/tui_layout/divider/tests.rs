//! Geometry vectors and generated cell partitions independent of the terminal adapter.
use super::*;

fn target(orientation: PanelOrientation, area: Rect, ratio: f64) -> DrawnDivider {
    let ratio = ClientSplitRatio::new(ratio).unwrap();
    let source = DividerSource {
        site: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        context: WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap(),
        tab: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
        focus: WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap(),
        revision: WorkbenchRevision::new(1),
    };
    let split = WorkbenchSplitId::parse(&format!("{:032x}", 5)).unwrap();
    let first = WorkbenchPanelSlotId::parse(&format!("{:032x}", 6)).unwrap();
    let second = WorkbenchPanelSlotId::parse(&format!("{:032x}", 7)).unwrap();
    DrawnDivider::new(
        source,
        Rect::new(0, 0, 300, 300),
        &SplitArea {
            split_id: &split,
            first_slot_id: &first,
            second_slot_id: &second,
            orientation,
            ratio,
            area: Some(area),
            divider: Some(divide_panes(area, orientation, ratio).unwrap().divider),
        },
    )
    .unwrap()
}

#[test]
fn divider_preview_uses_original_axis_displacement_and_exact_return_to_origin() {
    for orientation in [PanelOrientation::Horizontal, PanelOrientation::Vertical] {
        let target = target(
            orientation,
            Rect::new(11, 13, 101, 101),
            0.49000000000000005,
        );
        let source = target.clone();
        let (x, y) = (target.divider.x, target.divider.y);
        let mut drag = DividerDrag::begin(target, x, y).unwrap();
        let (next_x, next_y) = match orientation {
            PanelOrientation::Horizontal => (x + 10, y),
            PanelOrientation::Vertical => (x, y + 10),
        };
        drag.update(next_x, next_y);
        assert_eq!(drag.ratio.value(), source.ratio.value() + 0.1);
        assert_eq!(
            drag.preview,
            divide_panes(source.area, orientation, drag.ratio)
                .unwrap()
                .divider
        );
        drag.update(x, y);
        assert_eq!(drag.ratio, source.ratio);
        assert_eq!(drag.preview, source.divider);
        drag.update(
            if orientation == PanelOrientation::Vertical {
                0
            } else {
                x
            },
            if orientation == PanelOrientation::Horizontal {
                0
            } else {
                y
            },
        );
        assert_eq!(drag.ratio, source.ratio);
        assert_eq!(drag.target, source);
    }
}

#[test]
fn divider_preview_clamps_generated_extents_and_never_moves_its_source() {
    for orientation in [PanelOrientation::Horizontal, PanelOrientation::Vertical] {
        for extent in [9, 10, 11, 19, 60, 101, 257] {
            for ratio in [0.1, 0.12000000000000001, 0.49, 0.5, 0.7, 0.9] {
                let source = target(orientation, Rect::new(7, 9, extent, extent), ratio);
                let (x, y) = (source.divider.x, source.divider.y);
                let mut drag = DividerDrag::begin(source.clone(), x, y).unwrap();
                let mut previous = 0.1;
                for position in 0..=300 {
                    drag.update(
                        if orientation == PanelOrientation::Horizontal {
                            position
                        } else {
                            x
                        },
                        if orientation == PanelOrientation::Vertical {
                            position
                        } else {
                            y
                        },
                    );
                    assert!((0.1..=0.9).contains(&drag.ratio.value()));
                    assert!(drag.ratio.value() >= previous);
                    assert_eq!(drag.target, source);
                    let geometry = divide_panes(source.area, orientation, drag.ratio).unwrap();
                    assert_eq!(geometry.divider, drag.preview);
                    assert!(geometry.first.intersection(geometry.second).is_empty());
                    assert!(
                        source
                            .area
                            .contains((drag.preview.x, drag.preview.y).into())
                    );
                    previous = drag.ratio.value();
                }
                drag.update(u16::MAX, u16::MAX);
                assert_eq!(drag.ratio.value(), 0.9);
                drag.update(0, 0);
                assert_eq!(drag.ratio.value(), 0.1);
                assert!(DividerDrag::begin(source.clone(), source.area.x, source.area.y).is_none());
            }
        }
    }
}
