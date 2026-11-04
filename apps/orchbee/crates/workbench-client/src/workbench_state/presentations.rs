//! Client-local Presentation choice, resolved independently of canonical Panel geometry.
use super::{
    AttachTarget, AttachmentKey, ClientPhase, CreateTerminalProvenance, OperationDeadlines,
    PaneRef, WorkbenchClientState, WorkbenchPanelId,
};
use crate::workbench_projection::{ClientMemberTarget, ClientPanel, ClientPresentation};
use orchbee_workbench_protocol::TerminalPresentationId;

impl WorkbenchClientState {
    /// The focused Panel and the Presentation this Client shows in it.
    pub(crate) fn focused_member(&self) -> Option<ClientMemberTarget> {
        let active = self.active()?;
        let tab = self.cache().tabs().get(active.index())?;
        let panel = tab.panel(active.panel_index())?;
        Some(ClientMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: self.selected_presentation(panel).presentation_id().clone(),
        })
    }
    pub(crate) fn selected_presentation<'a>(
        &self,
        panel: &'a ClientPanel,
    ) -> &'a ClientPresentation {
        if let ClientPhase::Switching(setup) = &self.phase
            && setup.target.selected.key().panel() == panel.panel_id()
            && let Some(member) = panel
                .presentation_index(&setup.target.presentation_id)
                .and_then(|i| panel.presentation(i))
        {
            return member;
        }
        self.pane_by_panel(panel.panel_id())
            .and_then(|record| panel.bound_presentation(record.binding()))
            .unwrap_or_else(|| {
                self.navigation_memory
                    .presentation(self.cache.location(), panel)
            })
    }

    pub(crate) fn panel_is_unbound(&self, id: &WorkbenchPanelId) -> bool {
        self.active()
            .and_then(|active| self.cache.tabs().get(active.index()))
            .and_then(|tab| tab.panel_index(id).and_then(|index| tab.panel(index)))
            .is_some_and(|panel| self.selected_presentation(panel).binding().is_none())
    }

    pub(crate) fn selected_is_unbound(&self) -> bool {
        self.active()
            .is_some_and(|active| self.panel_is_unbound(active.key().panel()))
    }

    pub(super) fn member_provenance(
        &self,
        tab_index: usize,
        panel: &ClientPanel,
        member: &ClientPresentation,
    ) -> CreateTerminalProvenance {
        if member.presentation_id() == panel.presentation_id() {
            self.provenance
                .get(tab_index)
                .cloned()
                .unwrap_or(CreateTerminalProvenance::BindingOnly)
        } else {
            CreateTerminalProvenance::BindingOnly
        }
        .observed_for(member)
    }

    pub(super) fn target_provenance_valid(&self, target: &AttachTarget) -> bool {
        self.cache
            .tabs()
            .get(target.selected.index())
            .and_then(|tab| tab.panel(target.selected.panel_index()))
            .filter(|panel| panel.panel_id() == target.selected.key().panel())
            .and_then(|panel| {
                panel
                    .presentation_index(&target.presentation_id)
                    .and_then(|index| panel.presentation(index))
                    .filter(|member| member.binding() == target.binding.as_ref())
                    .map(|member| (panel, member))
            })
            .is_some_and(|(panel, member)| {
                self.member_provenance(target.selected.index(), panel, member) == target.provenance
            })
    }

    pub(super) fn pane_provenance_matches(
        &self,
        key: &PaneRef,
        index: usize,
        provenance: &CreateTerminalProvenance,
    ) -> bool {
        self.live_pane_record(key).map_or_else(
            || self.provenance.get(index) == Some(provenance),
            |record| record.index == index && &record.provenance == provenance,
        )
    }

    pub(crate) fn target_for_presentation(
        &mut self,
        tab_index: usize,
        panel_id: &WorkbenchPanelId,
        presentation_id: &TerminalPresentationId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<AttachTarget> {
        let tab = self.cache.tabs().get(tab_index)?;
        let panel_index = tab.panel_index(panel_id)?;
        let panel = tab.panel(panel_index)?;
        let member = panel.presentation(panel.presentation_index(presentation_id)?)?;
        let binding = member.binding().cloned();
        let provenance = self.member_provenance(tab_index, panel, member);
        let key = AttachmentKey::new(generation, tab.tab_id().clone());
        let mut target = self.target_for_switch_for_panel(tab_index, key, deadlines, panel_id)?;
        target.binding = binding;
        target.presentation_id = Box::new(presentation_id.clone());
        target.provenance = provenance;
        Some(target)
    }

    pub(crate) fn next_presentation(
        &self,
        forward: bool,
    ) -> Option<(usize, WorkbenchPanelId, TerminalPresentationId)> {
        let active = self.active()?;
        let panel = self
            .cache
            .tabs()
            .get(active.index())?
            .panel(active.panel_index())?;
        let count = panel.presentation_count();
        if count == 1 {
            return None;
        }
        let current =
            panel.presentation_index(self.selected_presentation(panel).presentation_id())?;
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        Some((
            active.index(),
            panel.panel_id().clone(),
            panel.presentation(next)?.presentation_id().clone(),
        ))
    }
}
