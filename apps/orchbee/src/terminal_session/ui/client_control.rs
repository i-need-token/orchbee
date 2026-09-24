//! Exact semantic navigation through the existing NativeUi owners.
use super::*;
use crate::terminal_session::client_control::{ControlStep, StepProgress};
use orchbee_workbench_protocol::{
    ClientViewControl, ClientViewControlReason as Reason, ClientViewIntent, ClientViewObservation,
    ClientViewPhase, ClientViewSelection, SiteId,
};

impl NativeUi {
    pub(in crate::terminal_session) fn control_modal(&self) -> bool {
        let view = self.view();
        view.close_confirmation().is_some()
            || view.close_status().is_some()
            || view.panel_close_confirmation().is_some()
            || view.rename_draft().is_some()
            || view.setup_draft().is_some()
            || view.agent_board().is_some()
            || self
                .projections
                .iter()
                .any(|projection| projection.viewport.is_some())
    }

    pub(in crate::terminal_session) fn control_observation(
        &self,
        shell_blocked: bool,
    ) -> Option<(SiteId, ClientViewObservation)> {
        let state = self.state.as_ref()?;
        let cache = state.cache();
        let tab = state
            .active()
            .and_then(|active| cache.tabs().get(active.index()));
        let phase = if shell_blocked
            || self.control_modal()
            || state.mutation_blocks_structure()
            || state.rename_draft_blocks_structure()
            || state.pending_refresh_generation().is_some()
        {
            ClientViewPhase::Busy
        } else {
            match state.phase_kind() {
                SessionPhaseKind::Empty | SessionPhaseKind::Ready => ClientViewPhase::Ready,
                SessionPhaseKind::Starting => ClientViewPhase::Starting,
                SessionPhaseKind::Creating
                | SessionPhaseKind::CreateCommittedNeedDetach
                | SessionPhaseKind::Switching => ClientViewPhase::Busy,
                _ => ClientViewPhase::Unavailable,
            }
        };
        let panel_count = u16::try_from(tab.map_or(0, |tab| tab.panel_count())).ok()?;
        let selection = ClientViewSelection {
            terminal_context_id: cache.terminal_context_id().clone(),
            tab_id: tab.map(|tab| tab.tab_id().clone()),
            panel_id: state.focus_panel_id().cloned(),
            zoomed: state.zoom_active(),
            panel_count,
        };
        let observation = ClientViewObservation {
            workbench_revision: cache.revision(),
            phase,
            selection: Some(selection),
        };
        observation.validate().ok()?;
        Some((cache.site_id().clone(), observation))
    }

    pub(in crate::terminal_session) fn control_step(
        &mut self,
        command: &ClientViewControl,
        progress: &mut StepProgress,
    ) -> ControlStep {
        let Some((_, view)) = self.control_observation(false) else {
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        };
        if self.retirement_failure.is_some()
            || matches!(
                self.state.as_ref().map(NativeSessionState::phase_kind),
                Some(SessionPhaseKind::Retired)
            )
        {
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        }
        if matches!(
            view.phase,
            ClientViewPhase::Starting | ClientViewPhase::Busy
        ) {
            return ControlStep::Waiting;
        }
        if view.workbench_revision < command.minimum_workbench_revision {
            if progress.refresh_requested {
                return ControlStep::Failed(Reason::ProjectionUnavailable);
            }
            let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
                return ControlStep::Failed(Reason::ProjectionUnavailable);
            };
            if let Some(state) = self.state.as_mut() {
                state.note_publication_hint();
            }
            self.drain_publication_refresh(&mailbox);
            if self
                .state
                .as_ref()
                .is_some_and(|state| state.pending_refresh_generation().is_some())
            {
                progress.refresh_requested = true;
                progress.had_effect = true;
                return ControlStep::Changed;
            }
            return ControlStep::Failed(Reason::Busy);
        }
        if command
            .intent
            .is_observed(&view, command.minimum_workbench_revision)
            || progress.toggle_origin.is_some_and(|previously_zoomed| {
                command.intent.is_toggle_observed(
                    &view,
                    command.minimum_workbench_revision,
                    previously_zoomed,
                )
            })
        {
            return ControlStep::Observed;
        }
        if command
            .intent
            .is_single_panel_zoom_observed(&view, command.minimum_workbench_revision)
        {
            return ControlStep::ZoomUnchanged;
        }
        let (context_id, tab_id, panel_id) = match &command.intent {
            ClientViewIntent::Layout { .. } | ClientViewIntent::FocusDirection { .. } => {
                return ControlStep::Failed(Reason::Unsupported);
            }
            ClientViewIntent::Context {
                terminal_context_id,
            } => (terminal_context_id, None, None),
            ClientViewIntent::Tab {
                terminal_context_id,
                tab_id,
            } => (terminal_context_id, Some(tab_id), None),
            ClientViewIntent::Panel {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | ClientViewIntent::ToggleZoom {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | ClientViewIntent::Zoom {
                terminal_context_id,
                tab_id,
                panel_id,
                ..
            } => (terminal_context_id, Some(tab_id), Some(panel_id)),
        };
        let Some(state) = self.state.as_ref() else {
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        };
        if state.cache().terminal_context_id() != context_id {
            if progress.context_attempted {
                return ControlStep::Failed(Reason::ProjectionUnavailable);
            }
            let Some(index) = state
                .cache()
                .navigation()
                .iter()
                .position(|context| context.location.terminal_context_id() == context_id)
            else {
                return ControlStep::Failed(Reason::TargetUnavailable);
            };
            self.select_context_at(index);
            if self
                .state
                .as_ref()
                .is_some_and(|state| state.context_selection_deadlines().is_some())
            {
                progress.context_attempted = true;
                progress.had_effect = true;
                return ControlStep::Changed;
            }
            return ControlStep::Failed(Reason::Busy);
        }
        let Some(tab_id) = tab_id else {
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        };
        let Some(index) = state.cache().tab_index(tab_id) else {
            return ControlStep::Failed(Reason::TargetUnavailable);
        };
        if panel_id.is_some_and(|panel| state.cache().tabs()[index].panel_index(panel).is_none()) {
            return ControlStep::Failed(Reason::TargetUnavailable);
        }
        if state
            .active()
            .is_none_or(|active| active.key().anchor().tab_id() != tab_id)
        {
            if progress.tab_attempted {
                return ControlStep::Failed(Reason::ProjectionUnavailable);
            }
            if self.control_select_tab(tab_id, panel_id) {
                progress.tab_attempted = true;
                progress.had_effect = true;
                return ControlStep::Changed;
            }
            return ControlStep::Failed(Reason::Busy);
        }
        if matches!(command.intent, ClientViewIntent::ToggleZoom { .. })
            && progress.toggle_origin.is_none()
        {
            progress.toggle_origin = Some(state.zoom_active());
        }
        if let Some(panel_id) = panel_id
            && state.focus_panel_id() != Some(panel_id)
        {
            // A context/Tab can be Ready before its sibling viewers converge.
            // Wait for the existing setup owner, never mint a second attachment.
            if !state.panel_viewer_live(panel_id) {
                return if state
                    .pane_by_panel(panel_id)
                    .is_some_and(|pane| !pane.attachable() || pane.fault().is_some())
                {
                    ControlStep::Failed(Reason::ProjectionUnavailable)
                } else {
                    ControlStep::Waiting
                };
            }
            self.move_panel_focus_to(panel_id);
            if self
                .state
                .as_ref()
                .is_some_and(|state| state.focus_panel_id() == Some(panel_id))
            {
                progress.had_effect = true;
                return ControlStep::Changed;
            }
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        }
        let enabled = match command.intent {
            ClientViewIntent::Zoom { enabled, .. } => Some(enabled),
            ClientViewIntent::ToggleZoom { .. } => progress.toggle_origin.map(|previous| !previous),
            _ => None,
        };
        if let Some(enabled) = enabled {
            self.toggle_zoom();
            if self
                .state
                .as_ref()
                .is_some_and(|state| state.zoom_active() == enabled)
            {
                progress.had_effect = true;
                return ControlStep::Changed;
            }
        }
        ControlStep::Failed(Reason::ProjectionUnavailable)
    }

    fn control_select_tab(
        &mut self,
        tab_id: &WorkbenchTabId,
        panel_id: Option<&WorkbenchPanelId>,
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
                let index = state
                    .cache()
                    .tab_index(tab_id)
                    .ok_or(ActionBuildError::Busy)?;
                if !matches!(
                    state.plan_selection_index(index),
                    SelectionPlan::Switch { .. }
                ) {
                    return Err(ActionBuildError::Busy);
                }
                let key = AttachmentKey::new(generation, tab_id.clone());
                let target = match panel_id {
                    Some(panel) => state.target_for_switch_for_panel(index, key, deadlines, panel),
                    None => state.target_for_switch(index, key, deadlines),
                }
                .ok_or(ActionBuildError::Protocol)?;
                if !state.begin_switch(target.clone()) {
                    return Err(ActionBuildError::Protocol);
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
