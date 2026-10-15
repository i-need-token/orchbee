//! Pure Sidebar tree tracks derived from the actual painted Sidebar width.

use crate::machine_picker::SidebarTarget;
use ratatui::layout::Rect;

const NAVIGATION_WIDTH: u16 = 1;
const INDENT_WIDTH: u16 = 2;
const DISCLOSURE_WIDTH: u16 = 2;
const ORDINAL_WIDTH: u16 = 2;
const CURRENT_WIDTH: u16 = 1;
const STATE_WIDTH: u16 = 1;
const UNREAD_WIDTH: u16 = 1;
const METADATA_GAP: u16 = 1;
const COMFORTABLE_MINIMUM: u16 = 52;
const COMPACT_MINIMUM: u16 = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RowKind {
    Owner,
    Agent,
}

impl RowKind {
    pub(super) const fn for_target(target: &SidebarTarget) -> Self {
        match target {
            SidebarTarget::Agent { .. } => Self::Agent,
            SidebarTarget::Project { .. } | SidebarTarget::Context { .. } => Self::Owner,
        }
    }

    const fn ordinal_width(self) -> u16 {
        match self {
            Self::Owner => 0,
            Self::Agent => ORDINAL_WIDTH,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Density {
    Comfortable,
    Compact,
    Narrow,
}

impl Density {
    pub(super) const fn for_width(width: u16) -> Self {
        if width >= COMFORTABLE_MINIMUM {
            Self::Comfortable
        } else if width >= COMPACT_MINIMUM {
            Self::Compact
        } else {
            Self::Narrow
        }
    }

    pub(super) const fn direct_create(self) -> bool {
        !matches!(self, Self::Narrow)
    }

    const fn menu_width(self) -> u16 {
        match self {
            Self::Comfortable => 17,
            Self::Compact | Self::Narrow => 6,
        }
    }

    const fn create_width(self) -> u16 {
        match self {
            Self::Comfortable => 10,
            Self::Compact => 5,
            Self::Narrow => 0,
        }
    }
}

/// Auxiliary controls owned by the exact Sidebar source for one row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct AuxiliaryActions {
    menu: bool,
    create: bool,
}

impl AuxiliaryActions {
    pub(super) const fn new(menu: bool, create: bool) -> Self {
        Self {
            menu,
            create: menu && create,
        }
    }

    fn width(self, density: Density) -> u16 {
        if !self.menu {
            return 0;
        }
        density.menu_width()
            + if self.create && density.create_width() > 0 {
                density.create_width() + 1
            } else {
                0
            }
    }

    fn create(self, density: Density, auxiliary: Rect) -> Option<Rect> {
        let width = density.create_width();
        if !self.create || width == 0 {
            return None;
        }
        Some(Rect::new(auxiliary.x, auxiliary.y, width, auxiliary.height))
    }

    fn menu(self, density: Density, auxiliary: Rect) -> Rect {
        if !self.menu {
            return Rect::new(auxiliary.right(), auxiliary.y, 0, auxiliary.height);
        }
        Rect::new(
            auxiliary.right() - density.menu_width(),
            auxiliary.y,
            density.menu_width(),
            auxiliary.height,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Tracks {
    density: Density,
    pub(super) band: Rect,
    pub(super) content_y: u16,
    pub(super) navigation: Rect,
    pub(super) disclosure: Rect,
    pub(super) ordinal: Rect,
    pub(super) current: Rect,
    pub(super) state: Rect,
    pub(super) unread: Rect,
    pub(super) primary: Rect,
    pub(super) auxiliary: Rect,
    pub(super) create: Option<Rect>,
    pub(super) menu: Rect,
}

impl Tracks {
    #[cfg(test)]
    pub(super) fn resolve(
        band: Rect,
        leading_gap: u16,
        depth: u16,
        actions: AuxiliaryActions,
        kind: RowKind,
    ) -> Option<Self> {
        Self::resolve_for_width(band, leading_gap, depth, band.width, actions, kind)
    }

    pub(super) fn resolve_for_width(
        band: Rect,
        leading_gap: u16,
        depth: u16,
        sidebar_width: u16,
        actions: AuxiliaryActions,
        kind: RowKind,
    ) -> Option<Self> {
        if band.is_empty() || leading_gap >= band.height {
            return None;
        }
        let density = Density::for_width(sidebar_width);
        let auxiliary_width = actions.width(density);
        if auxiliary_width > band.width {
            return None;
        }
        let auxiliary = Rect::new(
            band.right().saturating_sub(auxiliary_width),
            band.y,
            auxiliary_width,
            band.height,
        );
        let content_y = band.y.saturating_add(leading_gap);
        let navigation = Rect::new(
            band.x,
            band.y,
            NAVIGATION_WIDTH.min(band.width),
            band.height,
        );
        let indent = depth.saturating_mul(INDENT_WIDTH);
        let disclosure_x = band
            .x
            .saturating_add(NAVIGATION_WIDTH)
            .saturating_add(indent);
        let disclosure = clipped_track(band, disclosure_x, DISCLOSURE_WIDTH);
        let ordinal = clipped_track(band, disclosure.right(), kind.ordinal_width());
        let state = clipped_track(band, ordinal.right(), STATE_WIDTH);
        let metadata_width = METADATA_GAP + CURRENT_WIDTH + UNREAD_WIDTH;
        let primary_right = auxiliary.x.checked_sub(metadata_width)?;
        let primary_x = state.right();
        if primary_x >= primary_right {
            return None;
        }
        let current = clipped_track(band, primary_right + METADATA_GAP, CURRENT_WIDTH);
        let unread = clipped_track(band, current.right(), UNREAD_WIDTH);
        let primary = Rect::new(primary_x, band.y, primary_right - primary_x, band.height);
        let create = actions.create(density, auxiliary);
        let menu = actions.menu(density, auxiliary);
        Some(Self {
            density,
            band,
            content_y,
            navigation,
            disclosure,
            ordinal,
            current,
            state,
            unread,
            primary,
            auxiliary,
            create,
            menu,
        })
    }

    pub(super) const fn density(self) -> Density {
        self.density
    }

    pub(super) const fn first_line(self, track: Rect) -> Rect {
        Rect::new(track.x, self.content_y, track.width, 1)
    }
}

fn clipped_track(band: Rect, x: u16, width: u16) -> Rect {
    let x = x.min(band.right());
    Rect::new(
        x,
        band.y,
        band.right().saturating_sub(x).min(width),
        band.height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_tracks_keep_each_depth_stable_across_width_modes() {
        for (width, density, auxiliary) in [
            (60, Density::Comfortable, 28),
            (30, Density::Compact, 12),
            (24, Density::Narrow, 6),
        ] {
            let actions = AuxiliaryActions::new(true, true);
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
            assert_eq!(root.content_y, 5);
            assert_eq!(root.navigation, Rect::new(3, 4, 1, 3));
            assert_eq!(root.state, Rect::new(root.disclosure.right(), 4, 1, 3));
            assert_eq!(
                root.current,
                Rect::new(root.primary.right() + METADATA_GAP, 4, 1, 3)
            );
            assert_eq!(root.unread, Rect::new(root.current.right(), 4, 1, 3));
            assert_eq!(root.unread.right(), root.auxiliary.x);
        }
    }

    #[test]
    fn density_uses_the_complete_sidebar_surface_not_the_padded_tree_band() {
        let tracks = Tracks::resolve_for_width(
            Rect::new(1, 2, 50, 1),
            0,
            0,
            52,
            AuxiliaryActions::new(true, true),
            RowKind::Owner,
        )
        .unwrap();
        assert_eq!(tracks.density(), Density::Comfortable);
        assert_eq!(tracks.auxiliary.width, 28);
    }

    #[test]
    fn primary_uses_cells_not_owned_by_semantically_absent_controls() {
        let band = Rect::new(0, 0, 30, 1);
        let none = Tracks::resolve(
            band,
            0,
            2,
            AuxiliaryActions::new(false, false),
            RowKind::Owner,
        )
        .unwrap();
        let menu = Tracks::resolve(
            band,
            0,
            2,
            AuxiliaryActions::new(true, false),
            RowKind::Owner,
        )
        .unwrap();
        let both = Tracks::resolve(
            band,
            0,
            2,
            AuxiliaryActions::new(true, true),
            RowKind::Owner,
        )
        .unwrap();

        assert_eq!(none.auxiliary.width, 0);
        assert_eq!(menu.auxiliary.width, 6);
        assert_eq!(both.auxiliary.width, 12);
        assert_eq!(none.primary.width, menu.primary.width + 6);
        assert_eq!(menu.primary.width, both.primary.width + 6);
        assert_eq!(none.primary.x, menu.primary.x);
        assert_eq!(menu.primary.x, both.primary.x);
        for tracks in [none, menu, both] {
            assert_eq!(tracks.primary.right() + METADATA_GAP, tracks.current.x);
            assert_eq!(tracks.current.right(), tracks.unread.x);
            assert_eq!(tracks.unread.right(), tracks.auxiliary.x);
        }
        assert_eq!(menu.menu, menu.auxiliary);
        assert!(both.create.is_some());
        assert_eq!(both.create.unwrap().right() + 1, both.menu.x);
    }
}
