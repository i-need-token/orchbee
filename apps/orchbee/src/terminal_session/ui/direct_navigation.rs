//! Admit verified sidebar Agent targets into the existing Session selection and cleanup owners.
use super::{Arc, NativeUi, OperationDeadlines, finish_action};
use crate::terminal_session::{
    ContextNavigationAdmission as Admission, NativeMemberTarget,
    workbench_projection::{NativeMemberSelection, project_native_context_tabs},
};

impl NativeUi {
    pub(in crate::terminal_session) fn navigate_agent(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> Admission {
        if self.retirement_failure.is_some() || self.modal_owns_input() {
            return Admission::Unavailable;
        }
        let Some(state) = self.state.as_ref() else {
            return Admission::Unavailable;
        };
        if state.cache().site_id() != &target.context.site_id {
            return Admission::Unavailable;
        }
        // The displayed selection may still be A during A -> B. It cannot consume a new A intent.
        if !state.context_selection_available() {
            return Admission::Busy;
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
        let selection = NativeMemberSelection {
            member: NativeMemberTarget {
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
            let Some(context_index) = state
                .cache()
                .navigation()
                .iter()
                .position(|entry| &entry.location == destination.location())
            else {
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
}

#[cfg(test)]
mod tests;
