//! Presentation navigation uses the existing switch admission and cleanup transaction.
use super::{
    ActionBuildError, Arc, ClientActionDisposition, ClientPhaseKind, OperationDeadlines, PaneRef,
    SelectionPlan, SwitchMarker, WorkbenchClientUi, WorkbenchClientView, WorkbenchPanelId,
    finish_action,
};
use crate::workbench_client::input::PointerUnboundPane;
use orchbee_workbench_protocol::TerminalPresentationId;

/// One presentation control exactly as it was drawn for a live pane source.
///
/// The destination alone is not authority. Attached controls retain the
/// physical pane incarnation that owned the header; unbound controls retain
/// the current logical pane and selected member. Release revalidates both the
/// source and the destination against current membership before switching.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DrawnPresentationTarget {
    Attached {
        pane: PaneRef,
        presentation: TerminalPresentationId,
    },
    Unbound {
        selected: PointerUnboundPane,
        presentation: TerminalPresentationId,
    },
}

impl DrawnPresentationTarget {
    pub(crate) const fn presentation(&self) -> &TerminalPresentationId {
        match self {
            Self::Attached { presentation, .. } | Self::Unbound { presentation, .. } => {
                presentation
            }
        }
    }

    fn panel(&self) -> &WorkbenchPanelId {
        match self {
            Self::Attached { pane, .. } => pane.pane().panel(),
            Self::Unbound { selected, .. } => selected.key.panel(),
        }
    }
}

impl WorkbenchClientUi {
    pub(super) fn select_presentation_in_active_tab(
        &mut self,
        panel_id: &WorkbenchPanelId,
        presentation_id: &TerminalPresentationId,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(state) = self.state.as_ref() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(active) = state.active() else {
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(index) = state
            .cache()
            .tabs()
            .iter()
            .position(|tab| tab.tab_id() == active.key().anchor().tab_id())
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        let tab = &state.cache().tabs()[index];
        let Some(panel) = (0..tab.panel_count())
            .filter_map(|i| tab.panel(i))
            .find(|p| p.panel_id() == panel_id)
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        if panel.presentation_index(presentation_id).is_none()
            || state.selected_presentation(panel).presentation_id() == presentation_id
        {
            return ClientActionDisposition::KeepNavigator;
        }
        if self.select_presentation(index, panel_id, presentation_id) {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    /// Activates one exact presentation control after revalidating its painted
    /// pane source and destination against the current active membership.
    pub(in crate::workbench_client) fn activate_drawn_presentation(
        &mut self,
        target: &DrawnPresentationTarget,
    ) -> ClientActionDisposition {
        if self.modal_owns_input() || self.retirement_failure.is_some() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some((index, panel)) = self.state.as_ref().and_then(|state| {
            if !matches!(
                state.phase_kind(),
                ClientPhaseKind::Ready | ClientPhaseKind::SelectedWithoutAttachment
            ) {
                return None;
            }
            let active = state.active()?;
            let tab = state.cache().tabs().get(active.index())?;
            let panel = tab.panel(tab.panel_index(target.panel())?)?;
            if panel.presentation_index(target.presentation()).is_none()
                || state.selected_presentation(panel).presentation_id() == target.presentation()
                || state.desired().map(|key| key.anchor()) != Some(active.key().anchor())
            {
                return None;
            }
            let source_is_current = match target {
                DrawnPresentationTarget::Attached { pane, .. } => {
                    pane.anchor() == active.key().anchor()
                        && state
                            .pane_by_panel(panel.panel_id())
                            .is_some_and(|record| record.pane_ref().eq(pane))
                }
                DrawnPresentationTarget::Unbound { selected, .. } => {
                    selected.key.anchor() == active.key().anchor()
                        && selected.key.panel() == panel.panel_id()
                        && state.selected_presentation(panel).binding().is_none()
                        && state.selected_presentation(panel).presentation_id()
                            == &selected.presentation
                }
            };
            source_is_current.then(|| (active.index(), panel.panel_id().clone()))
        }) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if self.select_presentation(index, &panel, target.presentation()) {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(super) fn cycle_presentation(&mut self, forward: bool) -> ClientActionDisposition {
        if self.modal_owns_input() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some((index, panel, presentation)) = self
            .state
            .as_ref()
            .and_then(|state| state.next_presentation(forward))
        else {
            return ClientActionDisposition::KeepNavigator;
        };
        if self.select_presentation(index, &panel, &presentation) {
            ClientActionDisposition::DismissNavigator
        } else {
            ClientActionDisposition::KeepNavigator
        }
    }

    pub(in crate::workbench_client) fn select_presentation(
        &mut self,
        index: usize,
        panel: &WorkbenchPanelId,
        presentation: &TerminalPresentationId,
    ) -> bool {
        self.select_presentation_with_host(index, panel, presentation, None)
    }

    pub(super) fn select_presentation_with_host(
        &mut self,
        index: usize,
        panel: &WorkbenchPanelId,
        presentation: &TerminalPresentationId,
        expected_host: Option<orchbee_agent_observation_protocol::AgentLaunchBinding>,
    ) -> bool {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return false;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return false;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| {
                if !state.invariant_holds() {
                    return Err(ActionBuildError::Protocol);
                }
                if !matches!(
                    state.plan_selection_index(index),
                    SelectionPlan::Switch { .. } | SelectionPlan::NoOp
                ) {
                    return Err(ActionBuildError::Busy);
                }
                let mut target = state
                    .target_for_presentation(index, panel, presentation, generation, deadlines)
                    .ok_or(ActionBuildError::Busy)?;
                target.expected_host = expected_host.map(Box::new);
                if !state.begin_switch(target.clone()) {
                    return Err(ActionBuildError::Busy);
                }
                Ok(SwitchMarker::new(target))
            },
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
                finish_action(state, result);
            },
        );
        match result {
            Ok(_) => {
                self.pending_draws.clear();
                self.ordinary_poll_serial = None;
                true
            }
            Err(error) => {
                self.retire_for_admission(&error);
                false
            }
        }
    }
}

impl<'a> WorkbenchClientView<'a> {
    pub(crate) fn drawn_presentation_target(
        &self,
        panel: &super::super::ClientPanel,
        index: usize,
    ) -> Option<DrawnPresentationTarget> {
        let presentation = panel.presentation(index)?.presentation_id().clone();
        Some(
            if let Some(pane) = self.pane_ref_for_panel(panel.panel_id()) {
                DrawnPresentationTarget::Attached { pane, presentation }
            } else {
                DrawnPresentationTarget::Unbound {
                    selected: self.unbound_pane(panel)?,
                    presentation,
                }
            },
        )
    }

    pub(crate) fn unbound_pane(
        &self,
        panel: &super::super::ClientPanel,
    ) -> Option<crate::workbench_client::input::PointerUnboundPane> {
        let state = self.state.as_ref()?;
        Some(crate::workbench_client::input::PointerUnboundPane {
            key: state.unbound_panel_key(panel)?,
            presentation: state.selected_presentation(panel).presentation_id().clone(),
        })
    }

    pub(crate) fn panel_is_unbound(&self, panel: &WorkbenchPanelId) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| state.panel_is_unbound(panel))
    }
    pub(crate) fn selected_is_unbound(&self) -> bool {
        self.focus_panel_id()
            .is_some_and(|id| self.panel_is_unbound(id))
    }
    pub(crate) fn selected_presentation(
        &self,
        panel: &'a super::super::ClientPanel,
    ) -> &'a super::super::ClientPresentation {
        self.state.as_ref().map_or_else(
            || panel.primary(),
            |state| state.selected_presentation(panel),
        )
    }
}

#[cfg(test)]
mod tests;
