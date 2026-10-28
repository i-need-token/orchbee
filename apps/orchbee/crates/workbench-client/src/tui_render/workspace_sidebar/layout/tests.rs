use super::*;

#[test]
fn group_owner_and_agent_tracks_align_the_tree_across_width_modes() {
    for (width, density, auxiliary) in [
        (60, Density::Comfortable, 11),
        (30, Density::Compact, 3),
        (24, Density::Narrow, 3),
    ] {
        let actions = AuxiliaryActions::new(true);
        let band = Rect::new(3, 4, width, 3);
        let group = Tracks::resolve(band, 1, actions, RowKind::Group).unwrap();
        let owner = Tracks::resolve(band, 1, actions, RowKind::Owner).unwrap();
        let agent = Tracks::resolve(band, 1, actions, RowKind::Agent).unwrap();
        assert_eq!(owner.density(), density);
        assert_eq!(owner.auxiliary.width, auxiliary);
        assert_eq!(owner.navigation, Rect::new(5, 4, 1, 3));
        assert_eq!(owner.content_y, 5);
        // A heading starts after the keyboard marker; a Workspace state follows one padding cell.
        assert_eq!(group.primary.x, band.x + NAVIGATION_WIDTH);
        assert!(group.state.is_empty() && group.guide.is_empty());
        assert_eq!(owner.state, Rect::new(band.x + 1, 4, 1, 3));
        assert!(owner.guide.is_empty());
        // The guide sits under the Workspace state; the padded surface aligns with the parent name.
        assert_eq!(agent.guide, Rect::new(owner.state.x, 4, GUIDE_WIDTH, 3));
        assert_eq!(agent.surface().x, agent.guide.right() + GUIDE_GAP);
        assert_eq!(agent.surface().x, owner.primary.x);
        assert_eq!(agent.state.x, agent.surface().x + 1);
        assert_eq!(agent.surface().right(), band.right());
        assert_eq!(owner.surface(), band);
        assert_eq!(group.surface(), band);
        for tracks in [owner, agent] {
            assert_eq!(tracks.state.right() + STATE_GAP, tracks.primary.x);
            assert_eq!(tracks.navigation.x, tracks.state.right());
            assert_eq!(tracks.navigation.right(), tracks.primary.x);
            assert!(tracks.disclosure.is_empty());
        }
        assert_eq!(
            owner.current,
            Rect::new(owner.primary.right() + METADATA_GAP, 4, 1, 3)
        );
        assert_eq!(owner.unread, Rect::new(owner.current.right(), 4, 1, 3));
        assert_eq!(owner.unread.right(), owner.auxiliary.x);
    }
}

#[test]
fn disclosure_follows_displayed_identity_and_only_owns_its_first_line_cell() {
    for width in [24, 30, 60] {
        for kind in [RowKind::Group, RowKind::Owner] {
            let tracks = Tracks::resolve(
                Rect::new(3, 4, width, 4),
                1,
                AuxiliaryActions::new(true),
                kind,
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
    for width in [7, 8] {
        let tracks = Tracks::resolve(
            Rect::new(0, 0, width, 1),
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
        let none = Tracks::resolve(band, 0, AuxiliaryActions::new(false), RowKind::Owner).unwrap();
        let create = Tracks::resolve(band, 0, AuxiliaryActions::new(true), RowKind::Owner).unwrap();
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
