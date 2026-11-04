//! Join Client-local Agent versions to the exact live panes actually painted.
use super::{ClientProjectionStatus, PaneRef, WorkbenchClientUi, WorkbenchClientView};
use crate::workbench_client::agent_attention::{AgentAttentionDraw, PaintedPane};
use orchbee_workbench_protocol::WorkbenchPanelId;

impl WorkbenchClientView<'_> {
    /// Exact physical versions with a Client completion projection; the boolean records input
    /// acknowledgement. Consumers never substitute these display facts for observed Agent status.
    pub fn completion_presentations(
        &self,
    ) -> Vec<(
        orchbee_agent_observation_protocol::AgentLaunchBinding,
        orchbee_workbench_protocol::WorkbenchRevision,
        bool,
    )> {
        self.attention
            .filter(|_| self.retirement.is_none())
            .map_or_else(Vec::new, |attention| attention.completion_presentations())
    }

    pub(crate) fn panel_completion_edited(&self, panel: &WorkbenchPanelId) -> bool {
        let Some(pane) = self
            .pane_ref_for_panel(panel)
            .and_then(|pane| self.bound_attention_pane(&pane))
        else {
            return false;
        };
        self.attention
            .is_some_and(|attention| attention.completion_edited(&pane))
    }

    pub(crate) fn panel_completion_pending(&self, panel: &WorkbenchPanelId) -> bool {
        let Some(pane) = self
            .pane_ref_for_panel(panel)
            .and_then(|pane| self.bound_attention_pane(&pane))
        else {
            return false;
        };
        self.attention
            .is_some_and(|attention| attention.completion_pending(&pane))
    }

    pub fn machine_attention_version(&self) -> Option<(u64, usize)> {
        if self.retirement.is_some() {
            return None;
        }
        self.attention?.physical_view_version()
    }

    pub fn machine_agent_attention(
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
            || self.pane_projection_status(panel_id) != ClientProjectionStatus::FrameAvailable
            || self.pane_projection(panel_id)?.frame().is_none()
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
        self.settings().is_none()
            && self.close_confirmation().is_none()
            && self.workspace_close_confirmation().is_none()
            && self.panel_close_confirmation().is_none()
            && self.rename_draft().is_none()
            && self.setup_draft().is_none()
            && self.agent_board().is_none()
            && self.worktree().is_none()
    }

    fn painted_attention_pane(&self, pane: &PaneRef) -> Option<PaintedPane> {
        if self.pane_viewport(pane.pane().panel()).is_some() {
            return None;
        }
        self.bound_attention_pane(pane)
    }

    fn bound_attention_pane(&self, pane: &PaneRef) -> Option<PaintedPane> {
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
    ) -> Option<AgentAttentionDraw> {
        if !self.attention_uncovered() {
            return None;
        }
        let panes = panes
            .filter_map(|pane| self.painted_attention_pane(pane))
            .collect();
        self.attention?.paint(
            panes,
            self.focus_panel_id()
                .and_then(|panel| self.pane_ref_for_panel(panel)),
        )
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

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn terminal_edited(&mut self) {
        let Some(pane) = self
            .state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .and_then(|pane| self.view().bound_attention_pane(&pane))
        else {
            return;
        };
        self.attention.terminal_edited(&pane);
    }

    pub(in crate::workbench_client) fn configure_agent_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_runtime::NotificationSettings,
    ) -> bool {
        self.notification_base = (revision, settings.clone());
        self.apply_notification_preferences()
    }

    pub(super) fn apply_notification_preferences(&mut self) -> bool {
        use orchbee_local_runtime::NotificationDelivery as Delivery;
        use orchbee_workbench_protocol::AgentReminderMode;
        let (revision, base) = &self.notification_base;
        let mut effective = base.clone();
        let enabled = self.preferences.agent_notification_mode == AgentReminderMode::Hidden;
        effective.agent_delivery = if enabled {
            if base.agent_delivery == Delivery::Off {
                Delivery::Tui
            } else {
                base.agent_delivery
            }
        } else {
            Delivery::Off
        };
        effective.sound_enabled = enabled && self.preferences.sound_enabled;
        self.attention
            .configure_notifications(*revision, &effective)
    }

    pub(in crate::workbench_client) fn agent_notification_requires_card(&self) -> bool {
        self.attention.notification_requires_card()
    }

    pub(in crate::workbench_client) fn take_agent_notification(
        &mut self,
        locale: crate::localization::Locale,
        sink_available: bool,
    ) -> Option<(
        orchbee_local_runtime::NotificationDelivery,
        orchbee_workbench_protocol::Notification,
    )> {
        use crate::workbench_client::agent_notifications::AgentNotificationKind;
        use orchbee_workbench_protocol::NotificationSound;

        let (delivery, command) = self.attention.take_notification(sink_available)?;
        debug_assert_eq!(
            command.popup,
            delivery != orchbee_local_runtime::NotificationDelivery::Off
        );
        debug_assert!(command.sequence > 0);
        let placement = self
            .agent_index
            .placement_by_presentation(&command.identity.presentation_id);
        let title = locale.agent_notification_title(
            &command.agent,
            matches!(command.kind, AgentNotificationKind::Request),
        );
        let body = placement.map(|placement| {
            locale.agent_notification_context(
                placement.workspace_label.as_deref(),
                placement.project_label.as_deref(),
                &placement.tab_label,
                &placement.panel_label,
            )
        });
        let sound = if command.sound {
            match command.kind {
                AgentNotificationKind::Done => NotificationSound::Done,
                AgentNotificationKind::Request => NotificationSound::Request,
            }
        } else {
            NotificationSound::None
        };
        let message = orchbee_workbench_protocol::Notification::normalize(
            &title,
            body.as_deref(),
            None,
            sound,
        )
        .ok()?;
        Some((delivery, message))
    }

    pub(in crate::workbench_client) fn read_terminal_agent_attention(
        &self,
        bindings: &[orchbee_agent_observation_protocol::AgentLaunchBinding],
    ) -> Option<orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot> {
        self.attention.read_physical(bindings, self.outer_focused)
    }
    pub(in crate::workbench_client) fn read_agent_attention(
        &self,
        sessions: &[orchbee_agent_observation_protocol::AgentSessionId],
    ) -> Option<orchbee_workbench_protocol::ClientAgentAttentionSnapshot> {
        self.attention.read(sessions, self.outer_focused)
    }
    pub(super) fn after_attention_draw(&mut self, draw: AgentAttentionDraw) {
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
        let focus = self
            .state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .filter(|pane| draw.focused_pane.as_ref() == Some(pane));
        self.attention
            .acknowledge(&draw, self.outer_focused, &visible, focus.as_ref());
    }
}

// These integration fixtures use Windows client control and legacy notification adapters.
#[cfg(test)]
pub(super) mod tests;
