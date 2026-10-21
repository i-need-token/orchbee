use super::*;

#[test]
fn fixed_tracks_keep_each_depth_stable_across_width_modes() {
    for (width, density, auxiliary) in [
        (60, Density::Comfortable, 11),
        (30, Density::Compact, 3),
        (24, Density::Narrow, 3),
    ] {
        let actions = AuxiliaryActions::new(true);
        let root =
            Tracks::resolve(Rect::new(3, 4, width, 3), 1, 0, actions, RowKind::Owner).unwrap();
        let child =
            Tracks::resolve(Rect::new(3, 4, width, 3), 1, 1, actions, RowKind::Owner).unwrap();
        let descendant =
            Tracks::resolve(Rect::new(3, 4, width, 3), 1, 2, actions, RowKind::Owner).unwrap();
        let agent =
            Tracks::resolve(Rect::new(3, 4, width, 3), 1, 2, actions, RowKind::Agent).unwrap();
        assert_eq!(root.density(), density);
        assert_eq!(root.auxiliary.width, auxiliary);
        assert_eq!(child.primary.x - root.primary.x, INDENT_WIDTH);
        assert_eq!(descendant.primary.x - child.primary.x, INDENT_WIDTH);
        assert_eq!(root.primary.x, root.band.x + 4);
        assert!(root.ordinal.is_empty());
        assert_eq!(agent.primary.x - descendant.primary.x, ORDINAL_WIDTH);
        assert_eq!(agent.ordinal.width, ORDINAL_WIDTH);
        assert_eq!(agent.state.x, agent.ordinal.right());
        assert_eq!(root.content_y, 5);
        assert_eq!(root.navigation, Rect::new(4, 4, 1, 3));
        assert!(root.disclosure.is_empty());
        assert!(child.disclosure.is_empty());
        assert_eq!(root.state, Rect::new(root.band.x + 2, 4, 1, 3));
        assert_eq!(root.state.right() + STATE_GAP, root.primary.x);
        assert_eq!(
            root.current,
            Rect::new(root.primary.right() + METADATA_GAP, 4, 1, 3)
        );
        assert_eq!(root.unread, Rect::new(root.current.right(), 4, 1, 3));
        assert_eq!(root.unread.right(), root.auxiliary.x);
    }
}

#[test]
fn disclosure_follows_displayed_identity_and_only_owns_its_first_line_cell() {
    for width in [24, 30, 60] {
        for depth in [0, 1, 2] {
            let tracks = Tracks::resolve(
                Rect::new(3, 4, width, 4),
                1,
                depth,
                AuxiliaryActions::new(true),
                RowKind::Owner,
            )
            .unwrap();
            for displayed_width in [0, 3, 8, usize::MAX] {
                let inline = tracks.with_disclosure_after(displayed_width);
                assert_eq!(inline.primary, tracks.primary);
                assert_eq!(inline.auxiliary, tracks.auxiliary);
                assert_eq!(inline.disclosure.y, 5);
                assert_eq!(inline.disclosure.height, 1);
                assert_eq!(inline.disclosure.width, 1);
                assert_eq!(
                    inline.disclosure.x,
                    tracks.primary.x
                        + displayed_width.min(usize::from(tracks.text_width(true))) as u16
                        + 1
                );
                assert!(inline.disclosure.right() <= tracks.primary.right());
            }
        }
    }
}

#[test]
fn insufficient_identity_width_omits_disclosure_instead_of_publishing_a_phantom_cell() {
    for width in [8, 9] {
        let tracks = Tracks::resolve(
            Rect::new(0, 0, width, 1),
            0,
            0,
            AuxiliaryActions::default(),
            RowKind::Owner,
        )
        .unwrap();
        assert_eq!(tracks.text_width(true), tracks.primary.width);
        assert!(
            tracks
                .with_disclosure_after(usize::MAX)
                .disclosure
                .is_empty()
        );
    }
}

#[test]
fn density_uses_the_complete_sidebar_surface_not_the_padded_tree_band() {
    let tracks = Tracks::resolve_for_width(
        Rect::new(1, 2, 50, 1),
        0,
        0,
        52,
        AuxiliaryActions::new(true),
        RowKind::Owner,
    )
    .unwrap();
    assert_eq!(tracks.density(), Density::Comfortable);
    assert_eq!(tracks.auxiliary.width, 11);
}

#[test]
fn only_visible_create_controls_reserve_a_trailing_extent() {
    for width in [24, 30, 60] {
        let band = Rect::new(0, 0, width, 1);
        let none =
            Tracks::resolve(band, 0, 0, AuxiliaryActions::new(false), RowKind::Owner).unwrap();
        let create =
            Tracks::resolve(band, 0, 0, AuxiliaryActions::new(true), RowKind::Owner).unwrap();
        assert!(none.auxiliary.is_empty());
        assert_eq!(none.primary.x, create.primary.x);
        assert_eq!(
            none.primary.width,
            create.primary.width + create.auxiliary.width
        );
        assert_eq!(create.create, Some(create.auxiliary));
        assert_eq!(create.auxiliary.right(), band.right());
        assert_eq!(create.auxiliary.width, if width >= 52 { 11 } else { 3 });
        for tracks in [none, create] {
            assert_eq!(tracks.state.right() + STATE_GAP, tracks.primary.x);
            assert_eq!(tracks.primary.right() + METADATA_GAP, tracks.current.x);
            assert_eq!(tracks.current.right(), tracks.unread.x);
            assert_eq!(tracks.unread.right(), tracks.auxiliary.x);
        }
    }
}
