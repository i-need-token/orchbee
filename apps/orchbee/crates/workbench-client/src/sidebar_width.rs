//! Bounded sidebar width preference and its keyboard resize steps; no geometry projection.

/// The narrowest content a docked sidebar leaves beside itself: the Compact viewport minimum.
pub(crate) const COMPACT_MINIMUM_COLUMNS: u16 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarWidth(u16);

impl SidebarWidth {
    pub(crate) const DEFAULT: Self = Self(30);
    pub(crate) const MINIMUM: u16 = 24;
    pub(crate) const MAXIMUM: u16 = 60;

    pub(crate) const fn new(columns: u16) -> Option<Self> {
        if columns >= Self::MINIMUM && columns <= Self::MAXIMUM {
            Some(Self(columns))
        } else {
            None
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    pub const fn columns(self) -> u16 {
        self.0
    }
    /// The narrowest host that docks this sidebar beside Compact-minimum content.
    pub(crate) const fn docked_host_columns(self) -> u16 {
        self.0 + COMPACT_MINIMUM_COLUMNS
    }
    /// The columns this preference occupies on a host of `host_columns`: never more than half.
    pub(crate) fn effective_columns(self, host_columns: u16) -> u16 {
        self.0.min(host_columns / 2)
    }
}

impl Default for SidebarWidth {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarWidthChange {
    Narrower,
    Wider,
    Reset,
}
