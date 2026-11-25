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
        assert_eq!(owner.content_y, 5);
        // Headings and Workspaces share the edge marker cell and fill the whole band.
        for tracks in [group, owner] {
            assert_eq!(tracks.navigation, Rect::new(band.x, 4, 1, 3));
            assert!(tracks.state.is_empty() && tracks.guide.is_empty());
            assert_eq!(tracks.surface(), band);
        }
        // A Workspace's disclosure stands in its Agents' state column and its name in their title
        // column; the guide stays on chrome, before the padded Agent surface.
        assert_eq!(owner.disclosure, Rect::new(agent.state.x, 5, 1, 1));
        assert_eq!(owner.primary.x, agent.primary.x);
        assert!(agent.guide.right() <= agent.surface().x);
        assert!(agent.surface().x < agent.state.x);
        assert_eq!(agent.surface().right(), band.right());
        assert_eq!(group.navigation.right(), group.primary.x);
        assert_eq!(agent.navigation.right(), agent.primary.x);
        assert!(group.disclosure.is_empty() && agent.disclosure.is_empty());
        for tracks in [group, owner, agent] {
            assert_eq!(
                tracks.unread,
                Rect::new(tracks.primary.right() - 1, 4, 1, 3)
            );
            assert_eq!(tracks.primary.right() + AUXILIARY_GAP, tracks.auxiliary.x);
        }
    }
}

#[test]
fn heading_disclosure_follows_displayed_identity_and_only_owns_its_first_line_cell() {
    for width in [24, 30, 60] {
        let tracks = Tracks::resolve(
            Rect::new(3, 4, width, 4),
            1,
            AuxiliaryActions::new(true),
            RowKind::Group,
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

#[test]
fn insufficient_identity_width_omits_disclosure_instead_of_publishing_a_phantom_cell() {
    for width in [2, 3] {
        let tracks = Tracks::resolve(
            Rect::new(0, 0, width, 1),
            0,
            AuxiliaryActions::default(),
            RowKind::Group,
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
        assert_eq!(none.primary.right(), band.right());
        assert_eq!(
            none.primary.width,
            create.primary.width + AUXILIARY_GAP + create.auxiliary.width
        );
        assert_eq!(create.create, Some(create.auxiliary));
        assert_eq!(create.auxiliary.right(), band.right());
        assert_eq!(create.auxiliary.width, if width >= 52 { 11 } else { 3 });
    }
}
