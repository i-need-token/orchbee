//! Join Client-local Agent versions to the exact live panes actually painted.
use super::{NativeProjectionStatus, NativeSessionView, NativeUi, PaneRef};
use crate::terminal_session::agent_attention::{NativeAgentAttentionDraw, PaintedPane};
use orchbee_workbench_protocol::WorkbenchPanelId;

impl NativeSessionView<'_> {
    pub(crate) fn machine_attention_version(&self) -> Option<(u64, usize)> {
        if self.retirement.is_some() {
            return None;
        }
        self.attention?.physical_view_version()
    }

    pub(crate) fn machine_agent_attention(
        &self,
        bindings: &[orchbee_agent_observation_protocol::AgentLaunchBinding],
    ) -> Option<orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot> {
        if self.retirement.is_some() {
            return None;
        }
        self.attention?.read_physical(bindings, None)
    }

    fn attention_presentation(&self, pane: &PaneRef) -> Option<String> {
        let panel_id = pane.pane().panel();
        if self.pane_ref_for_panel(panel_id).as_ref() != Some(pane)
            || self.pane_projection_status(panel_id) != NativeProjectionStatus::FrameAvailable
            || self.pane_projection(panel_id)?.frame().is_none()
            || self.pane_viewport(panel_id).is_some()
            || (self.zoom_active() && self.focus_panel_id() != Some(panel_id))
        {
            return None;
        }
        let tab = self.tab(self.active_position()?.checked_sub(1)?)?;
        (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|panel| panel.panel_id() == panel_id)
            .map(|panel| {
                self.selected_presentation(panel)
                    .presentation_id()
                    .as_str()
                    .to_owned()
            })
    }

    fn attention_uncovered(&self) -> bool {
        self.close_confirmation().is_none()
            && self.panel_close_confirmation().is_none()
            && self.rename_draft().is_none()
            && self.setup_draft().is_none()
            && self.agent_board().is_none()
    }

    fn painted_attention_pane(&self, pane: &PaneRef) -> Option<PaintedPane> {
        let presentation = self.attention_presentation(pane)?;
        let projection = self.pane_projection(pane.pane().panel())?;
        Some(PaintedPane {
            pane: pane.clone(),
            presentation,
            route: projection.frame()?.route,
            host_terminal_id: projection.host_terminal_id().map(str::to_owned),
        })
    }

    pub(crate) fn attention_draw<'a>(
        &self,
        panes: impl Iterator<Item = &'a PaneRef>,
    ) -> Option<NativeAgentAttentionDraw> {
        if !self.attention_uncovered() {
            return None;
        }
        let panes = panes
            .filter_map(|pane| self.painted_attention_pane(pane))
            .collect();
        self.attention?.paint(panes)
    }

    pub(crate) fn panel_unseen_count(&self, panel: &WorkbenchPanelId) -> usize {
        let Some(attention) = self.attention else {
            return 0;
        };
        let Some(tab) = self
            .active_position()
            .and_then(|position| position.checked_sub(1))
            .and_then(|index| self.tab(index))
        else {
            return 0;
        };
        (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .find(|entry| entry.panel_id() == panel)
            .map_or(0, |entry| {
                attention.unseen_in(entry.presentation_id().as_str())
            })
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn read_terminal_agent_attention(
        &self,
        bindings: &[orchbee_agent_observation_protocol::AgentLaunchBinding],
    ) -> Option<orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot> {
        self.attention.read_physical(bindings, self.outer_focused)
    }
    pub(in crate::terminal_session) fn read_agent_attention(
        &self,
        sessions: &[orchbee_agent_observation_protocol::AgentSessionId],
    ) -> Option<orchbee_workbench_protocol::ClientAgentAttentionSnapshot> {
        self.attention.read(sessions, self.outer_focused)
    }
    pub(super) fn after_attention_draw(&mut self, draw: NativeAgentAttentionDraw) {
        let view = self.view();
        if !view.attention_uncovered() {
            return;
        }
        let visible: Vec<_> = draw
            .panes
            .iter()
            .filter(|proof| view.painted_attention_pane(&proof.pane).as_ref() == Some(*proof))
            .cloned()
            .collect();
        self.attention
            .acknowledge(&draw, self.outer_focused, &visible);
    }
}

#[cfg(test)]
pub(super) mod tests;
