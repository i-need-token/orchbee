use super::*;

#[test]
fn forty_column_bar_resolves_every_control_before_paint() {
    let theme = Theme::rgb_test_dark();
    let area = Rect::new(0, 0, 40, 1);

    let geometry = resolve_geometry(area, area, Locale::EnUs.resize_done_label(), false, &theme)
        .expect("the established compact resize bar fits at forty columns");

    assert_eq!(geometry.exit, ExitGeometry::Action(Rect::new(31, 0, 9, 1)));
    assert_eq!(
        geometry.controls,
        vec![
            CompactGeometry {
                control: CompactControl::Adjust {
                    direction: PanelDirection::Left,
                    fine: false,
                },
                rect: Rect::new(0, 0, 3, 1),
            },
            CompactGeometry {
                control: CompactControl::Adjust {
                    direction: PanelDirection::Down,
                    fine: false,
                },
                rect: Rect::new(4, 0, 3, 1),
            },
            CompactGeometry {
                control: CompactControl::Adjust {
                    direction: PanelDirection::Up,
                    fine: false,
                },
                rect: Rect::new(8, 0, 3, 1),
            },
            CompactGeometry {
                control: CompactControl::Adjust {
                    direction: PanelDirection::Right,
                    fine: false,
                },
                rect: Rect::new(12, 0, 3, 1),
            },
            CompactGeometry {
                control: CompactControl::Step { fine: false },
                rect: Rect::new(16, 0, 4, 1),
            },
            CompactGeometry {
                control: CompactControl::Help,
                rect: Rect::new(21, 0, 3, 1),
            },
        ]
    );
    assert_eq!(
        geometry
            .controls
            .iter()
            .map(|control| control.control.icon())
            .collect::<Vec<_>>(),
        [
            Some(IconActionGlyph::Previous),
            Some(IconActionGlyph::Down),
            Some(IconActionGlyph::Up),
            Some(IconActionGlyph::Next),
            None,
            Some(IconActionGlyph::Help),
        ]
    );
    assert!(
        geometry
            .controls
            .windows(2)
            .all(|pair| pair[0].rect.right() < pair[1].rect.x)
    );
    assert!(
        geometry
            .controls
            .last()
            .is_some_and(|control| control.rect.right() < geometry.exit.rect().x)
    );
}

#[test]
fn localization_and_step_state_keep_fixed_control_extents() {
    let theme = Theme::rgb_test_dark();
    let area = Rect::new(0, 0, 40, 1);
    let coarse = resolve_geometry(area, area, Locale::EnUs.resize_done_label(), false, &theme)
        .expect("English coarse geometry fits");
    let fine = resolve_geometry(area, area, Locale::ZhCn.resize_done_label(), true, &theme)
        .expect("Chinese fine geometry fits");

    assert_eq!(coarse.exit.rect(), fine.exit.rect());
    assert_eq!(
        coarse
            .controls
            .iter()
            .map(|control| control.rect)
            .collect::<Vec<_>>(),
        fine.controls
            .iter()
            .map(|control| control.rect)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        coarse.controls[4].control,
        CompactControl::Step { fine: false }
    );
    assert_eq!(
        fine.controls[4].control,
        CompactControl::Step { fine: true }
    );
    assert_eq!(step_label(false), "[5%]");
    assert_eq!(step_label(true), "[1%]");
}

#[test]
fn narrow_bars_keep_a_whole_exit_and_never_publish_partial_controls() {
    let theme = Theme::rgb_test_dark();
    let narrow = Rect::new(0, 0, 8, 1);
    let tiny = Rect::new(0, 0, 2, 1);

    let narrow = resolve_geometry(
        narrow,
        narrow,
        Locale::EnUs.resize_done_label(),
        false,
        &theme,
    )
    .expect("the close fallback fits");

    assert_eq!(narrow.exit, ExitGeometry::Close(Rect::new(4, 0, 3, 1)));
    assert_eq!(narrow.controls.len(), 1);
    assert_eq!(narrow.controls[0].rect, Rect::new(0, 0, 3, 1));
    assert!(
        resolve_geometry(tiny, tiny, Locale::EnUs.resize_done_label(), false, &theme).is_none()
    );
    assert!(
        resolve_geometry(
            Rect::new(0, 0, 4, 1),
            Rect::new(3, 0, 4, 1),
            Locale::EnUs.resize_done_label(),
            false,
            &theme,
        )
        .is_none()
    );
}

#[test]
fn three_cell_exit_is_complete_and_narrower_refusal_preserves_buffer_and_hits() {
    use crate::client_view::{ClientAction, ClientActionDisposition};
    use ratatui::{Terminal, backend::TestBackend, style::Style};

    let resize_view = |width| {
        let mut view = ClientView::new(width);
        let transition = view.decide_action(ClientAction::EnterResizeMode);
        view.commit(transition, ClientActionDisposition::KeepNavigator);
        assert!(view.resize_mode());
        view
    };
    let theme = Theme::rgb_test_dark();

    let mut accepted = Terminal::new(TestBackend::new(3, 2)).unwrap();
    let mut accepted_hits = HitMapBuilder::new();
    let accepted_view = resize_view(3);
    accepted
        .draw(|frame| {
            render(
                frame,
                &accepted_view,
                None,
                Locale::EnUs,
                &theme,
                &mut accepted_hits,
            );
        })
        .unwrap();
    assert_eq!(
        accepted
            .backend()
            .buffer()
            .content()
            .iter()
            .take(3)
            .map(|cell| cell.symbol())
            .collect::<String>(),
        "[x]"
    );
    let accepted_map = accepted_hits.finish();
    for x in 0..3 {
        assert!(matches!(
            accepted_map.hit_target_at(x, 0),
            Some(HitTarget::Resize(ResizeControl::Exit))
        ));
    }

    for width in 1..3 {
        let mut refused = Terminal::new(TestBackend::new(width, 1)).unwrap();
        let mut refused_hits = HitMapBuilder::new();
        refused_hits.record(
            HitLayer::Foreground,
            Rect::new(0, 0, width, 1),
            HitTarget::OverlaySurface,
        );
        let refused_view = resize_view(width);
        let underlay = "z".repeat(usize::from(width));
        refused
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .set_string(0, 0, &underlay, Style::default());
                render(
                    frame,
                    &refused_view,
                    None,
                    Locale::EnUs,
                    &theme,
                    &mut refused_hits,
                );
            })
            .unwrap();
        assert_eq!(
            refused
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>(),
            underlay
        );
        assert!(refused_view.resize_mode());
        let refused_map = refused_hits.finish();
        for x in 0..width {
            assert!(matches!(
                refused_map.hit_target_at(x, 0),
                Some(HitTarget::OverlaySurface)
            ));
        }
    }
}
