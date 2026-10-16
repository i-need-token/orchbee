//! A read-only local geometry proposal; the pointer or acknowledgement owner retains its lease.
use crate::client_view::SidebarSurface;
use orchbee_workbench_protocol::{
    ClientSplitRatio, WorkbenchPanelSlotId, WorkbenchSplitId, WorkbenchTabId,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum ResizeProjection {
    #[default]
    Canonical,
    #[cfg(windows)]
    Divider {
        source: super::DrawnDivider,
        ratio: ClientSplitRatio,
    },
    #[cfg(windows)]
    Sidebar {
        source: super::DrawnSidebarWidth,
        width: super::SidebarWidth,
    },
}

impl ResizeProjection {
    #[cfg(windows)]
    pub(crate) fn divider(&self) -> Option<super::DrawnDivider> {
        let Self::Divider { source, ratio } = self else {
            return None;
        };
        let mut drawn = source.clone();
        drawn.ratio = *ratio;
        drawn.divider = super::divide_panes(source.area, source.orientation, *ratio)?.divider;
        Some(drawn)
    }

    #[cfg(windows)]
    pub(crate) fn sidebar_target(&self) -> Option<super::DrawnSidebarWidth> {
        match self {
            Self::Sidebar { source, width } => Some(source.with_width(*width)),
            _ => None,
        }
    }

    pub(crate) fn sidebar(&self, canonical: SidebarSurface) -> SidebarSurface {
        match self {
            #[cfg(windows)]
            Self::Sidebar { source, width } => source.with_width(*width).surface,
            _ => canonical,
        }
    }

    pub(crate) fn ratio(
        &self,
        _tab: &WorkbenchTabId,
        _split: &WorkbenchSplitId,
        _first: &WorkbenchPanelSlotId,
        _second: &WorkbenchPanelSlotId,
        canonical: ClientSplitRatio,
    ) -> ClientSplitRatio {
        match self {
            #[cfg(windows)]
            Self::Divider { source, ratio }
                if &source.source.tab == _tab
                    && &source.split == _split
                    && &source.first == _first
                    && &source.second == _second =>
            {
                *ratio
            }
            _ => canonical,
        }
    }
}
