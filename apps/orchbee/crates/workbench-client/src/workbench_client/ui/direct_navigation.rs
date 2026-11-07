//! Admit verified sidebar Agent targets into the existing Client selection and cleanup owners.
use super::{Arc, OperationDeadlines, WorkbenchClientUi, finish_action};
use crate::workbench_client::ContextNavigationAdmission as Admission;
use crate::workbench_projection::ClientMemberTarget;
use crate::workbench_projection::{ClientMemberSelection, project_native_context_tabs};

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn navigate_agent(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> Admission {
        // Each retry re-decides whether background reads keep yielding to it.
        self.end_navigation_yield();
        if self.retirement_failure.is_some() || self.modal_owns_input() {
            return Admission::Unavailable;
        }
        let Some(state) = self.state.as_ref() else {
            return Admission::Unavailable;
        };
        if state.cache().session_id() != &target.context.session_id {
            return Admission::Unavailable;
        }
        // The displayed selection may still be A during A -> B. It cannot consume a new A intent.
        if !state.context_selection_available() {
            return self.context_navigation_busy();
        }
        let Ok(destination) = project_native_context_tabs(
            state.cache().canonical_snapshot(),
            &target.context.location,
        ) else {
            return Admission::Unavailable;
        };
        let Some(index) = destination.tab_index(&target.tab) else {
            return Admission::Unavailable;
        };
        let tab = &destination.tabs()[index];
        let Some(panel) = tab.panel_index(&target.panel).and_then(|i| tab.panel(i)) else {
            return Admission::Unavailable;
        };
        let Some(binding) = panel
            .presentation_index(&target.member)
            .and_then(|i| panel.presentation(i))
            .and_then(|member| member.binding())
        else {
            return Admission::Unavailable;
        };
        if binding.worker_id() != &target.terminal.worker_id
            || binding.terminal_id() != &target.terminal.terminal_id
        {
            return Admission::Unavailable;
        }
        let selection = ClientMemberSelection {
            member: ClientMemberTarget {
                tab: target.tab.clone(),
                panel: target.panel.clone(),
                presentation: target.member.clone(),
            },
            binding: target.terminal.clone(),
            expected_host: Some(target.physical.clone()),
        };
        self.retained_frames
            .fence_selection(&target.context, &selection);
        if destination.location() != state.cache().location() {
            // A Workspace an accepted close takes out of navigation is not a destination.
            let Some(context_index) = state.display_position(destination.location()) else {
                return Admission::Unavailable;
            };
            self.remember_history_positions();
            let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
                return Admission::Unavailable;
            };
            let (Some(mailbox), Some(state)) =
                (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
            else {
                return Admission::Unavailable;
            };
            let result = mailbox.try_enqueue_context_selection_for(
                state,
                deadlines,
                &self.clock,
                |state, generation| {
                    state
                        .start_context_selection_at_with_member(
                            generation,
                            context_index,
                            deadlines,
                            Some(selection),
                        )
                        .then_some(())
                        .ok_or(super::ActionBuildError::Busy)
                },
                finish_action,
            );
            return match result {
                Ok(Some(_)) => Admission::Admitted,
                Ok(None) => Admission::Unavailable,
                Err(error) => {
                    self.retire_for_admission(&error);
                    Admission::Unavailable
                }
            };
        }
        let already_selected = state
            .active()
            .is_some_and(|active| active.key().anchor().tab_id() == &target.tab)
            && state.selected_presentation(panel).presentation_id() == &target.member
            && state.panel_viewer_live(&target.panel)
            && state.pane_by_panel(&target.panel).is_some_and(|record| {
                self.projections.iter().any(|projection| {
                    projection.key == record.pane_ref()
                        && projection.value.epoch().to_text() == target.physical.host_epoch.as_str()
                        && projection.value.host_terminal_id()
                            == Some(target.physical.host_terminal_id.as_str())
                })
            });
        if already_selected {
            if state.focus_panel_id() == Some(&target.panel) {
                return Admission::Current;
            }
            self.remember_history_positions();
            return if self.move_panel_focus_admitted(&target.panel).is_some() {
                Admission::Admitted
            } else {
                Admission::Unavailable
            };
        }
        self.remember_history_positions();
        if self.select_presentation_with_host(
            index,
            &target.panel,
            &target.member,
            Some(target.physical.clone()),
        ) {
            Admission::Admitted
        } else {
            Admission::Unavailable
        }
    }

    /// Opens the Agent a notification card announced through the same revalidating navigation as
    /// a sidebar Agent row, never another target. An admitted opening marks that Agent's current
    /// attention read, as explicit focus does. The user's own landing switch or a background read
    /// holds the opening, which then replays through here; a stale target reports the Agent
    /// unavailable.
    pub(in crate::workbench_client) fn open_notification_target(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> crate::client_view::notifications::TargetOpening {
        use crate::client_view::notifications::TargetOpening;
        use crate::workbench_state::ClientNotice;

        match self.navigate_agent(target) {
            Admission::Current | Admission::Admitted => {
                self.attention.acknowledge_focused_presentations(
                    &std::iter::once(target.physical.presentation_id.as_str()).collect(),
                );
                TargetOpening::Opened
            }
            Admission::Busy if self.settling_for_user_action() => {
                self.hold_intent(super::held_intent::HeldIntent::AgentTarget(Box::new(
                    target.clone(),
                )));
                TargetOpening::Held
            }
            Admission::Busy => {
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::LocalBusy);
                }
                TargetOpening::Busy
            }
            Admission::Unavailable => {
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::AgentUnavailable);
                }
                TargetOpening::Unavailable
            }
        }
    }
}

#[cfg(test)]
mod tests;
