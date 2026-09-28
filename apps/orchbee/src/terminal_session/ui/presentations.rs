//! Presentation navigation uses the existing switch admission and cleanup transaction.
use super::*;
use orchbee_workbench_protocol::TerminalPresentationId;

impl NativeUi {
    /// Empty sessions have no terminal Poll on which to piggyback publication reads.
    pub(super) fn schedule_unbound_refresh(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let Some(state) = self.state.as_mut().filter(|state| {
            state.phase_kind() == SessionPhaseKind::SelectedWithoutAttachment
                && state.selected_is_unbound()
                && !state.has_attachments()
        }) else {
            self.unbound_refresh_at = None;
            return false;
        };
        let now = self.clock.now();
        let Some(later) = now.checked_add(std::time::Duration::from_secs(1)) else {
            return false;
        };
        let next = self.unbound_refresh_at.get_or_insert(later);
        if now < *next {
            return false;
        }
        *next = later;
        let Some(deadline) = now.checked_add(super::super::state::OPERATION_TOTAL) else {
            return false;
        };
        let result = mailbox.try_enqueue_refresh_for(
            state,
            deadline,
            |state, generation| {
                state
                    .start_pending_refresh(generation)
                    .then_some(())
                    .ok_or(ActionBuildError::Busy)
            },
            |_state, _result| {},
        );
        let admitted = result.is_ok();
        if let Err(error) = result {
            self.retire_for_admission(&error);
        }
        admitted
    }

    pub(super) fn select_presentation_in_active_tab(
        &mut self,
        panel_id: &WorkbenchPanelId,
        presentation_id: &TerminalPresentationId,
    ) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some(state) = self.state.as_ref() else {
            return SessionActionDisposition::KeepNavigator;
        };
        let Some(active) = state.active() else {
            return SessionActionDisposition::KeepNavigator;
        };
        let Some(index) = state
            .cache()
            .tabs()
            .iter()
            .position(|tab| tab.tab_id() == active.key().anchor().tab_id())
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        let tab = &state.cache().tabs()[index];
        let Some(panel) = (0..tab.panel_count())
            .filter_map(|i| tab.panel(i))
            .find(|p| p.panel_id() == panel_id)
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        if panel.presentation_index(presentation_id).is_none()
            || state.selected_presentation(panel).presentation_id() == presentation_id
        {
            return SessionActionDisposition::KeepNavigator;
        }
        if self.select_presentation(index, panel_id, presentation_id) {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(super) fn cycle_presentation(&mut self, forward: bool) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some((index, panel, presentation)) = self
            .state
            .as_ref()
            .and_then(|state| state.next_presentation(forward))
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        if self.select_presentation(index, &panel, &presentation) {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(in crate::terminal_session) fn select_presentation(
        &mut self,
        index: usize,
        panel: &WorkbenchPanelId,
        presentation: &TerminalPresentationId,
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
                let target = state
                    .target_for_presentation(index, panel, presentation, generation, deadlines)
                    .ok_or(ActionBuildError::Busy)?;
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

impl<'a> NativeSessionView<'a> {
    pub(crate) fn unbound_pane(
        &self,
        panel: &super::super::NativePanel,
    ) -> Option<crate::terminal_input::PointerUnboundPane> {
        let state = self.state.as_ref()?;
        Some(crate::terminal_input::PointerUnboundPane {
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
        panel: &'a super::super::NativePanel,
    ) -> &'a super::super::NativePresentation {
        self.state.as_ref().map_or_else(
            || panel.primary(),
            |state| state.selected_presentation(panel),
        )
    }
}

#[cfg(all(test, windows))]
mod tests;
