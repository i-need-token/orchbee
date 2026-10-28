//! Bounded sidebar width and reversible painted-edge geometry; no input or terminal effects.
use super::{SidebarArea, project_shell_areas};
use crate::client_view::SidebarSurface;
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarWidth(u16);

impl SidebarWidth {
    pub const DEFAULT: Self = Self(30);
    const MINIMUM: u16 = 24;
    const MAXIMUM: u16 = 60;

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
        self.0 + super::COMPACT_MINIMUM_COLUMNS
    }
    pub(super) fn effective_columns(self, host_columns: u16) -> u16 {
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

/// Both preferred surface and effective rectangle belong to this exact painted source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrawnSidebarWidth {
    pub(crate) viewport: Rect,
    pub(crate) surface: SidebarSurface,
    pub(crate) sidebar: SidebarArea,
}

impl DrawnSidebarWidth {
    pub fn new(viewport: Rect, surface: SidebarSurface) -> Option<Self> {
        let sidebar = project_shell_areas(viewport, surface).sidebar?;
        Some(Self {
            viewport,
            surface,
            sidebar,
        })
    }
    pub(crate) fn preferred(self) -> SidebarWidth {
        match self.surface {
            SidebarSurface::Docked(width) | SidebarSurface::Overlay(width) => width,
            SidebarSurface::Hidden => unreachable!("a drawn sidebar has a visible surface"),
        }
    }
    pub(crate) fn with_width(self, width: SidebarWidth) -> Self {
        let surface = match self.surface {
            SidebarSurface::Docked(_) => SidebarSurface::Docked(width),
            SidebarSurface::Overlay(_) => SidebarSurface::Overlay(width),
            SidebarSurface::Hidden => unreachable!("a drawn sidebar has a visible surface"),
        };
        Self::new(self.viewport, surface).expect("changing width preserves sidebar visibility")
    }
    pub fn edge(self) -> Rect {
        self.edge_for(self.preferred())
    }
    fn edge_for(self, width: SidebarWidth) -> Rect {
        Rect::new(
            self.sidebar
                .area
                .x
                .saturating_add(width.effective_columns(self.viewport.width))
                .saturating_sub(1),
            self.sidebar.area.y,
            1,
            self.sidebar.area.height,
        )
    }
    fn displaced(self, delta: i32) -> SidebarWidth {
        let maximum = SidebarWidth::MAXIMUM.min(self.viewport.width / 2);
        let effective = self.sidebar.area.width;
        let next = (i32::from(effective) + delta)
            .clamp(i32::from(SidebarWidth::MINIMUM), i32::from(maximum));
        if next == i32::from(effective) {
            // A constrained no-op must not erase the preference waiting for a larger host.
            self.preferred()
        } else {
            SidebarWidth::new(u16::try_from(next).expect("bounded positive columns"))
                .expect("displacement is clamped to the admitted width interval")
        }
    }
    pub(crate) fn adjusted(self, change: SidebarWidthChange) -> SidebarWidth {
        match change {
            SidebarWidthChange::Narrower => self.displaced(-2),
            SidebarWidthChange::Wider => self.displaced(2),
            SidebarWidthChange::Reset => SidebarWidth::DEFAULT,
        }
    }
}

/// The pointer owner holds or drops this preview; only it can admit a matching release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarWidthDrag {
    pub(crate) target: DrawnSidebarWidth,
    pub(crate) painted: DrawnSidebarWidth,
    pressed_column: u16,
    pub(crate) width: SidebarWidth,
    pub(crate) preview: Rect,
}

impl SidebarWidthDrag {
    pub(crate) fn begin(target: DrawnSidebarWidth, column: u16, row: u16) -> Option<Self> {
        target
            .edge()
            .contains((column, row).into())
            .then_some(Self {
                target,
                painted: target,
                pressed_column: column,
                width: target.preferred(),
                preview: target.edge(),
            })
    }
    pub(crate) fn update(&mut self, column: u16) {
        self.width = self
            .target
            .displaced(i32::from(column) - i32::from(self.pressed_column));
        self.preview = self.target.edge_for(self.width);
    }
    pub(crate) fn projected_target(&self) -> DrawnSidebarWidth {
        self.target.with_width(self.width)
    }
}

#[cfg(test)]
mod tests;
