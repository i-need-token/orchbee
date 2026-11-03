//! Painted-source facts that pointer drags resolve against: the sidebar's navigation snapshot and
//! the Tab strip's members. The port reports them; gesture policy stays with `tui_pointer`.
use crate::{action_menu::DrawnTabCreate, machine_picker::SidebarSource, machines::Choice};
use orchbee_workbench_protocol::{
    Navigation, SessionId, Snapshot, WorkbenchTabGroupId, WorkbenchTabId,
};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarReorderSource(Arc<Snapshot>);
impl SidebarReorderSource {
    pub(crate) fn from_snapshot(snapshot: Arc<Snapshot>) -> Self {
        Self(snapshot)
    }
    pub(crate) fn matches_sidebar(&self, source: &SidebarSource, machine: &Choice) -> bool {
        source.matches_navigation(machine, self.session_id(), self.navigation())
    }
    pub(crate) fn matches_menu_source(&self, source: &crate::action_menu::DrawnMenuSource) -> bool {
        source.session == self.0.session_id && source.revision == self.0.revision
    }
    pub(crate) fn navigation(&self) -> &Navigation {
        &self.0.navigation
    }
    pub(crate) fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.0)
    }
    pub(crate) fn session_id(&self) -> &SessionId {
        &self.0.session_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TabDragTab {
    pub(crate) id: WorkbenchTabId,
    pub(crate) pinned: bool,
    pub(crate) group: Option<WorkbenchTabGroupId>,
    pub(crate) selected: bool,
    pub(crate) collapsed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabDragSource {
    pub(crate) drawn: DrawnTabCreate,
    pub(crate) tabs: Vec<TabDragTab>,
}
