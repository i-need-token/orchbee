//! Exact native origin and primary source sampling facts, independent of Git execution.
use super::{WorktreeMode, WorktreeToken};
use crate::workbench_client::{
    state::{ClientPhaseKind, OperationDeadlines, PaneRef, WorkbenchClientState},
    workbench_projection::{ClientContextTabs, ClientMemberTarget, ClientTerminalBinding},
};
use orchbee_client_server_protocol::Geometry;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct Origin {
    pub cache: ClientContextTabs,
    pub member: Option<ClientMemberTarget>,
    pub zoom: bool,
    pub geometry: Geometry,
    pub primary_pane: Option<PaneRef>,
}
impl Origin {
    pub fn unchanged(&self, current: &Self) -> bool {
        self.cache.session_id() == current.cache.session_id()
            && self.cache.location() == current.cache.location()
            && self.cache.directory() == current.cache.directory()
            && self.member == current.member
            && self.zoom == current.zoom
            && self.geometry == current.geometry
            && self.primary_pane == current.primary_pane
            && self.primary_binding() == current.primary_binding()
    }
    pub(super) fn primary_binding(&self) -> Option<&ClientTerminalBinding> {
        let member = self.member.as_ref()?;
        let tab = self.cache.tabs().get(self.cache.tab_index(&member.tab)?)?;
        tab.panel(tab.panel_index(&member.panel)?)?
            .primary()
            .binding()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct Preparation {
    pub generation: u64,
    pub token: WorktreeToken,
    pub mode: WorktreeMode,
    pub origin: Origin,
    pub primary: Option<ClientTerminalBinding>,
    pub primary_pane: Option<PaneRef>,
    pub deadlines: OperationDeadlines,
}
impl WorkbenchClientState {
    pub(in crate::workbench_client) fn worktree_origin(&self) -> Option<Origin> {
        if self.mutation_blocks_structure()
            || self.pending_refresh_generation().is_some()
            || !matches!(
                self.phase_kind(),
                ClientPhaseKind::Ready | ClientPhaseKind::Empty
            )
        {
            return None;
        }
        let member = self.active().and_then(|active| {
            let tab = self.cache().tabs().get(active.index())?;
            let panel = tab.panel(active.panel_index())?;
            Some(ClientMemberTarget {
                tab: tab.tab_id().clone(),
                panel: panel.panel_id().clone(),
                presentation: self.selected_presentation(panel).presentation_id().clone(),
            })
        });
        let primary = self.active().and_then(|active| {
            self.cache()
                .tabs()
                .get(active.index())?
                .panel(active.panel_index())?
                .primary()
                .binding()
        });
        let primary_pane = self
            .focus_record()
            .filter(|record| Some(record.binding()) == primary)
            .map(super::super::PaneRecord::pane_ref);
        Some(Origin {
            cache: self.cache().clone(),
            member,
            zoom: self.zoom_active(),
            geometry: self.latest_geometry(),
            primary_pane,
        })
    }
    pub(in crate::workbench_client) fn worktree_preparation(
        &self,
        mode: WorktreeMode,
        generation: u64,
        token: WorktreeToken,
        deadlines: OperationDeadlines,
    ) -> Option<Preparation> {
        let origin = self.worktree_origin()?;
        let primary = self
            .active()
            .and_then(|active| {
                self.cache()
                    .tabs()
                    .get(active.index())?
                    .panel(active.panel_index())?
                    .primary()
                    .binding()
            })
            .cloned();
        let primary_pane = origin.primary_pane.clone();
        Some(Preparation {
            generation,
            token,
            mode,
            origin,
            primary,
            primary_pane,
            deadlines,
        })
    }
}
