//! Exact semantic navigation through the existing NativeUi owners.
use super::*;
use crate::terminal_session::client_control::{ControlStep, StepProgress};
use orchbee_workbench_protocol::{
    ClientViewControl, ClientViewControlReason as Reason, ClientViewIntent, ClientViewObservation,
    ClientViewPhase, ClientViewSelection, SiteId,
};

impl NativeUi {
    pub(in crate::terminal_session) fn configured_command_completed(
        &mut self,
        completion: crate::terminal_session::client_control::CommandCompletion,
    ) {
        use super::NativeSessionNotice as N;
        use orchbee_workbench_protocol::{
            ConfiguredCommandFailure as F, ConfiguredCommandOutcome as O,
            WorkbenchCommandOutcome as W, WorkbenchTerminalOutcome as T,
        };
        let notice = match completion.reply {
            Err(true)
            | Ok(orchbee_workbench_protocol::ConfiguredCommandResult {
                outcome:
                    O::Failed {
                        reason: F::PhysicalUnknown,
                    },
                ..
            }) => N::CommandUnknown,
            Err(false)
            | Ok(orchbee_workbench_protocol::ConfiguredCommandResult {
                outcome: O::Failed { .. },
                ..
            }) => N::CommandFailed,
            Ok(reply) => match reply.outcome {
                O::Pane { result, .. } => match result.outcome {
                    W::PanelSplitCreated {
                        terminal_outcome: T::Started { .. },
                        ..
                    } => N::CommandAccepted,
                    W::PanelSplitCreated {
                        terminal_outcome: T::OutcomeUnknown {},
                        ..
                    } => N::CommandUnknown,
                    _ => N::CommandFailed,
                },
                O::Plugin { result, .. } => match result.outcome {
                    orchbee_workbench_protocol::PluginOutcome::Failed { .. } => N::CommandFailed,
                    orchbee_workbench_protocol::PluginOutcome::Unknown { .. } => N::CommandUnknown,
                    _ => N::CommandAccepted,
                },
                O::PopupControl { result, .. } => {
                    if matches!(
                        result.outcome,
                        orchbee_workbench_protocol::ClientViewOutcome::Operation { ref operation } if matches!(operation.status, orchbee_workbench_protocol::ClientViewControlStatus::Queued {} | orchbee_workbench_protocol::ClientViewControlStatus::Delivered {})
                    ) {
                        N::CommandAccepted
                    } else {
                        N::CommandFailed
                    }
                }
                O::ShellStarted { .. } => N::CommandAccepted,
                _ => N::CommandFailed,
            },
        };
        let notice = match completion.focus {
            Some(Err(true)) => N::CommandUnknown,
            Some(Err(false)) => N::CommandFailed,
            Some(Ok(result))
                if !matches!(
                    result.outcome,
                    orchbee_workbench_protocol::ClientViewOutcome::Operation { ref operation } if matches!(operation.status, orchbee_workbench_protocol::ClientViewControlStatus::Queued {} | orchbee_workbench_protocol::ClientViewControlStatus::Delivered {})
                ) =>
            {
                N::CommandFailed
            }
            _ => notice,
        };
        self.command_notice = Some((notice, self.clock.now() + std::time::Duration::from_secs(5)));
    }
    /// Canonical membership is checked before any Panel mode cancellation.
    pub(in crate::terminal_session) fn validate_panel_focus(
        &self,
        command: &ClientViewControl,
        snapshot: &orchbee_workbench_protocol::Snapshot,
    ) -> Result<(), Reason> {
        use crate::terminal_session::workbench_projection::{
            project_native_context_tabs, project_native_standalone_tabs,
        };
        let ClientViewIntent::Panel {
            terminal_context_id,
            tab_id,
            panel_id,
        } = &command.intent
        else {
            return Err(Reason::Unsupported);
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
            || snapshot.revision < state.cache().revision()
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let navigation =
            project_native_standalone_tabs(snapshot).map_err(|_| Reason::ProjectionUnavailable)?;
        let location = &navigation
            .navigation()
            .iter()
            .find(|entry| entry.location.terminal_context_id() == terminal_context_id)
            .ok_or(Reason::TargetUnavailable)?
            .location;
        let candidate = project_native_context_tabs(snapshot, location)
            .map_err(|_| Reason::ProjectionUnavailable)?;
        let tab = candidate
            .tab_index(tab_id)
            .and_then(|index| candidate.tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        tab.panel_index(panel_id).ok_or(Reason::TargetUnavailable)?;
        Ok(())
    }

    /// One preflight precedes every cancellation. Pending operations keep their owners/receipts.
    pub(in crate::terminal_session) fn dismiss_focus_modes(
        &mut self,
        command: &ClientViewControl,
    ) -> Result<bool, Reason> {
        self.agent_focus_source(command)?;
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        let setup = self.view().setup_draft().map(|draft| draft.token());
        if self.retirement_failure.is_some()
            || !matches!(
                state.phase_kind(),
                super::SessionPhaseKind::Ready | super::SessionPhaseKind::Empty
            ) && !(state.phase_kind() == super::SessionPhaseKind::SelectedWithoutAttachment
                && state.selected_is_unbound())
            || state.pending_refresh_generation().is_some()
            || state.mutation_in_progress()
            || (state.mutation_blocks_structure() && setup.is_none())
            || self.view().close_status().is_some()
        {
            return Err(Reason::Busy);
        }
        let history = self.control_history();
        let mailbox = self.mailbox.as_ref().cloned();
        if history && mailbox.is_none() {
            return Err(Reason::ProjectionUnavailable);
        }
        let changed = self.control_modal() || state.rename_draft_blocks_structure();
        self.cancel_close();
        self.cancel_panel_close();
        if let Some(state) = self.state.as_mut() {
            state.cancel_rename_draft();
        }
        if let Some(token) = setup {
            self.cancel_setup_draft(token);
        }
        self.worktrees.dismiss_focus();
        self.board.close();
        if let Some(mailbox) = mailbox {
            for index in 0..self.projections.len() {
                if self.retirement_failure.is_some() {
                    break;
                }
                let key = self.projections[index].key.clone();
                if self.projections[index].tail.is_some() {
                    continue;
                }
                if let Some(viewport) = self.projections[index].viewport.as_mut() {
                    let effect = viewport.nav(super::ViewportNav::Exit);
                    self.settle_viewport_effect(&mailbox, &key, effect);
                }
            }
        }
        Ok(changed)
    }

    /// Called after the resolved focus target has been drawn without expiry or interruption.
    pub(in crate::terminal_session) fn finish_focus(
        &mut self,
        command: &ClientViewControl,
    ) -> Result<(), Reason> {
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if view.phase != ClientViewPhase::Ready
            || !command
                .intent
                .is_observed(&view, command.minimum_workbench_revision)
        {
            return Err(Reason::StaleView);
        }
        if !self.agent_focus_ready(command)? {
            return Err(Reason::ProjectionUnavailable);
        }
        let tab_id = match &command.intent {
            ClientViewIntent::Context { .. } => {
                let Some(tab_id) = view
                    .selection
                    .as_ref()
                    .and_then(|selection| selection.tab_id.as_ref())
                else {
                    return Ok(());
                };
                tab_id
            }
            ClientViewIntent::Tab { tab_id, .. }
            | ClientViewIntent::Panel { tab_id, .. }
            | ClientViewIntent::AgentFocus { tab_id, .. } => tab_id,
            _ => return Err(Reason::Unsupported),
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        let tab = state
            .cache()
            .tab_index(tab_id)
            .and_then(|index| state.cache().tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        let presentations: Vec<_> = (0..tab.panel_count())
            .filter_map(|index| tab.panel(index))
            .flat_map(|panel| {
                panel
                    .presentations()
                    .map(|member| member.presentation_id().as_str().to_owned())
            })
            .collect();
        self.attention
            .acknowledge_focused_tab(&presentations.iter().map(String::as_str).collect());
        Ok(())
    }

    pub(in crate::terminal_session) fn control_modal(&self) -> bool {
        self.control_dialog() || self.control_history()
    }

    pub(in crate::terminal_session) fn control_history(&self) -> bool {
        self.projections
            .iter()
            .any(|projection| projection.viewport.is_some() && projection.tail.is_none())
    }

    pub(in crate::terminal_session) fn control_dialog(&self) -> bool {
        let view = self.view();
        view.close_confirmation().is_some()
            || view.workspace_close_confirmation().is_some()
            || view.close_status().is_some()
            || view.panel_close_confirmation().is_some()
            || view.rename_draft().is_some()
            || view.setup_draft().is_some()
            || view.worktree().is_some()
            || view.agent_board().is_some()
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
        let phase = if self.popup.captured()
            || shell_blocked
            || self.control_modal()
            || state.mutation_blocks_structure()
            || state.rename_draft_blocks_structure()
            || state.pending_refresh_generation().is_some()
        {
            ClientViewPhase::Busy
        } else {
            match state.phase_kind() {
                SessionPhaseKind::Empty | SessionPhaseKind::Ready => ClientViewPhase::Ready,
                SessionPhaseKind::SelectedWithoutAttachment if state.selected_is_unbound() => {
                    ClientViewPhase::Ready
                }
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
            split_revision: Some(state.split_revision()),
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
        if let Err(reason) = self.agent_focus_source(command) {
            return ControlStep::Failed(reason);
        }
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
        if matches!(command.intent, ClientViewIntent::Overlay { .. })
            && !progress.overlay_registered
        {
            let Some(state) = self.state.as_mut() else {
                return ControlStep::Failed(Reason::ProjectionUnavailable);
            };
            if let Err(reason) = state.register_plugin_overlay(&command.intent) {
                return ControlStep::Failed(reason);
            }
            progress.overlay_registered = true;
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
            if let ClientViewIntent::AgentFocus {
                tab_id,
                panel_id,
                presentation_id,
                ..
            } = &command.intent
            {
                let Some(state) = self.state.as_ref() else {
                    return ControlStep::Failed(Reason::ProjectionUnavailable);
                };
                let Some(index) = state.cache().tab_index(tab_id) else {
                    return ControlStep::Failed(Reason::TargetUnavailable);
                };
                let tab = &state.cache().tabs()[index];
                let Some(panel) = tab.panel_index(panel_id).and_then(|index| tab.panel(index))
                else {
                    return ControlStep::Failed(Reason::TargetUnavailable);
                };
                if panel.presentation_index(presentation_id).is_none() {
                    return ControlStep::Failed(Reason::TargetUnavailable);
                }
                if state.selected_presentation(panel).presentation_id() != presentation_id {
                    return if self.select_presentation(index, panel_id, presentation_id) {
                        progress.had_effect = true;
                        ControlStep::Changed
                    } else {
                        ControlStep::Failed(Reason::Busy)
                    };
                }
            }
            return match self.agent_focus_ready(command) {
                Ok(true) => ControlStep::Observed,
                Ok(false) => ControlStep::Waiting,
                Err(reason) => ControlStep::Failed(reason),
            };
        }
        if command
            .intent
            .is_single_panel_zoom_observed(&view, command.minimum_workbench_revision)
        {
            return ControlStep::ZoomUnchanged;
        }
        let (context_id, tab_id, panel_id) = match &command.intent {
            ClientViewIntent::Selections { .. }
            | ClientViewIntent::Presentation { .. }
            | ClientViewIntent::Viewports { .. }
            | ClientViewIntent::AgentAttention { .. }
            | ClientViewIntent::TerminalAgentAttention { .. }
            | ClientViewIntent::Layout { .. }
            | ClientViewIntent::FocusDirection { .. }
            | ClientViewIntent::Resize { .. }
            | ClientViewIntent::SetSplitRatio { .. }
            | ClientViewIntent::CommandPopupOpen { .. }
            | ClientViewIntent::PopupOpen { .. }
            | ClientViewIntent::EditorOpen { .. }
            | ClientViewIntent::PopupClose { .. } => {
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
            | ClientViewIntent::AgentFocus {
                terminal_context_id,
                tab_id,
                panel_id,
                ..
            }
            | ClientViewIntent::ToggleZoom {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | ClientViewIntent::Overlay {
                terminal_context_id,
                tab_id,
                panel_id,
                ..
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
            if !state.panel_viewer_live(panel_id) && !state.panel_is_unbound(panel_id) {
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
            ClientViewIntent::Overlay { .. } => Some(true),
            ClientViewIntent::Zoom { enabled, .. } => Some(enabled),
            ClientViewIntent::ToggleZoom { .. } => progress.toggle_origin.map(|previous| !previous),
            _ => None,
        };
        if let Some(enabled) = enabled {
            self.toggle_zoom();
            if let ClientViewIntent::Overlay { panel_id, .. } = &command.intent
                && let Some(state) = self.state.as_mut()
            {
                state.arm_overlay_zoom(panel_id);
            }
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
