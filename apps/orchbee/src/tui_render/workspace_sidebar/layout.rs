//! Pure Sidebar tree tracks derived from the actual painted Sidebar width.

use crate::machine_picker::SidebarTarget;
use ratatui::layout::Rect;

const NAVIGATION_WIDTH: u16 = 1;
const DISCLOSURE_WIDTH: u16 = 1;
const DISCLOSURE_GAP: u16 = 1;
/// Agent rows open with a two-cell tree connector (`├─`/`└─`) under their Workspace state.
const BRANCH_WIDTH: u16 = 2;
const CURRENT_WIDTH: u16 = 1;
const STATE_WIDTH: u16 = 1;
const STATE_GAP: u16 = 1;
const UNREAD_WIDTH: u16 = 1;
const METADATA_GAP: u16 = 1;
const COMFORTABLE_MINIMUM: u16 = 52;
const COMPACT_MINIMUM: u16 = 28;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RowKind {
    /// A Project or ungrouped heading: identity starts at the list edge without a state slot.
    Group,
    /// A Workspace: state at the list edge, identity after it.
    Owner,
    /// An Agent: tree connector under the Workspace state, then state and identity.
    Agent,
}

impl RowKind {
    pub(super) const fn for_target(target: &SidebarTarget) -> Self {
        match target {
            SidebarTarget::Agent { .. } => Self::Agent,
            SidebarTarget::Project { .. } => Self::Group,
            SidebarTarget::Context { .. } => Self::Owner,
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

    const fn create_width(self) -> u16 {
        match self {
            Self::Comfortable => 11,
            Self::Compact | Self::Narrow => 3,
        }
    }
}

/// Auxiliary controls owned by the exact Sidebar source for one row.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct AuxiliaryActions {
    create: bool,
}

impl AuxiliaryActions {
    pub(super) const fn new(create: bool) -> Self {
        Self { create }
    }

    fn width(self, density: Density) -> u16 {
        if self.create {
            density.create_width()
        } else {
            0
        }
    }

    fn create(self, density: Density, auxiliary: Rect) -> Option<Rect> {
        self.create.then(|| {
            Rect::new(
                auxiliary.x,
                auxiliary.y,
                density.create_width(),
                auxiliary.height,
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Tracks {
    density: Density,
    pub(super) band: Rect,
    pub(super) content_y: u16,
    pub(super) navigation: Rect,
    pub(super) disclosure: Rect,
    pub(super) branch: Rect,
    pub(super) current: Rect,
    pub(super) state: Rect,
    pub(super) unread: Rect,
    pub(super) primary: Rect,
    pub(super) auxiliary: Rect,
    pub(super) create: Option<Rect>,
}

impl Tracks {
    #[cfg(test)]
    pub(super) fn resolve(
        band: Rect,
        leading_gap: u16,
        actions: AuxiliaryActions,
        kind: RowKind,
    ) -> Option<Self> {
        Self::resolve_for_width(band, leading_gap, band.width, actions, kind)
    }

    pub(super) fn resolve_for_width(
        band: Rect,
        leading_gap: u16,
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
        // The keyboard marker owns the list edge; every kind starts its content right after it.
        let navigation = Rect::new(
            band.x,
            band.y,
            NAVIGATION_WIDTH.min(band.width),
            band.height,
        );
        let branch = clipped_track(
            band,
            navigation.right(),
            if kind == RowKind::Agent {
                BRANCH_WIDTH
            } else {
                0
            },
        );
        // A Workspace state shares the connector column, so an Agent state aligns with its
        // Workspace identity. Headings reserve no state slot.
        let state = clipped_track(
            band,
            branch.right(),
            if kind == RowKind::Group {
                0
            } else {
                STATE_WIDTH
            },
        );
        let metadata_width = METADATA_GAP + CURRENT_WIDTH + UNREAD_WIDTH;
        let primary_right = auxiliary.x.checked_sub(metadata_width)?;
        let primary_x = if kind == RowKind::Group {
            state.right()
        } else {
            state.right().saturating_add(STATE_GAP)
        };
        if primary_x >= primary_right {
            return None;
        }
        let current = clipped_track(band, primary_right + METADATA_GAP, CURRENT_WIDTH);
        let unread = clipped_track(band, current.right(), UNREAD_WIDTH);
        let primary = Rect::new(primary_x, band.y, primary_right - primary_x, band.height);
        let disclosure = Rect::new(primary.right(), content_y, 0, 0);
        let create = actions.create(density, auxiliary);
        Some(Self {
            density,
            band,
            content_y,
            navigation,
            disclosure,
            branch,
            current,
            state,
            unread,
            primary,
            auxiliary,
            create,
        })
    }

    pub(super) const fn density(self) -> Density {
        self.density
    }

    pub(super) fn text_width(self, has_disclosure: bool) -> u16 {
        if has_disclosure && self.primary.width > DISCLOSURE_GAP + DISCLOSURE_WIDTH {
            self.primary.width - DISCLOSURE_GAP - DISCLOSURE_WIDTH
        } else {
            self.primary.width
        }
    }

    pub(super) fn with_disclosure_after(mut self, first_line_width: usize) -> Self {
        if self.primary.width > DISCLOSURE_GAP + DISCLOSURE_WIDTH {
            let identity_width = first_line_width.min(usize::from(self.text_width(true))) as u16;
            self.disclosure = Rect::new(
                self.primary.x + identity_width + DISCLOSURE_GAP,
                self.content_y,
                DISCLOSURE_WIDTH,
                1,
            );
        }
        self
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
mod tests;
