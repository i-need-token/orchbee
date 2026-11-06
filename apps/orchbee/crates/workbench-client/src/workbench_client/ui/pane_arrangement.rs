//! The Client view's side of the layout port: pane geometry reads the sidebar, the active Tab's
//! Panels, zoom, and focus through it.

use std::io;

use orchbee_client_server_protocol::Geometry;
use orchbee_workbench_protocol::{
    ClientSplitRatio, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchSplitId,
};

use super::{ClientTabView, WorkbenchClientView};
use crate::{
    client_view::{ClientView, SidebarSurface},
    terminal_device::WindowDimensions,
    tui_layout::{self, PaneArrangement, TabPanes},
    workbench_client::{ClientPanel, ClientPanelTree},
};

impl WorkbenchClientView<'_> {
    /// Every visible leaf's `(panel_id, panel_index, geometry)` under this Client, projected by
    /// the shared pane layout: the membership observation that gives each pane its own viewer.
    pub fn pane_geometries(
        &self,
        dimensions: WindowDimensions,
        view: &ClientView,
    ) -> io::Result<Vec<(WorkbenchPanelId, usize, Geometry)>> {
        tui_layout::pane_geometries(dimensions, view, self)
    }
}

impl<'a> PaneArrangement for WorkbenchClientView<'a> {
    type Tab = ClientTabView<'a>;

    fn sidebar_surface(&self, surface: SidebarSurface) -> SidebarSurface {
        Self::sidebar_surface(self, surface)
    }

    fn active_tab(&self) -> Option<ClientTabView<'a>> {
        self.tab(self.active_position()?.checked_sub(1)?)
    }

    fn zoom_active(&self) -> bool {
        Self::zoom_active(self)
    }

    fn focus_panel_id(&self) -> Option<&WorkbenchPanelId> {
        Self::focus_panel_id(self)
    }
}

impl TabPanes for ClientTabView<'_> {
    fn drawn_panel_count(&self) -> usize {
        Self::drawn_panel_count(self)
    }

    fn panel_tree(&self) -> &ClientPanelTree {
        Self::panel_tree(self)
    }

    fn active_panel(&self) -> Option<usize> {
        Self::active_panel(self)
    }

    fn split_ratio(
        &self,
        split: &WorkbenchSplitId,
        first: &WorkbenchPanelSlotId,
        second: &WorkbenchPanelSlotId,
    ) -> ClientSplitRatio {
        Self::split_ratio(self, split, first, second)
    }

    fn panel_id(&self, index: usize) -> Option<&WorkbenchPanelId> {
        self.panel(index).map(ClientPanel::panel_id)
    }
}
