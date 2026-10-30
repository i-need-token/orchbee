//! UI-owned remote navigation. The lane transports observations; only this owner advances UI steps.
use super::WorkbenchClientUi;
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_workbench_protocol::{
    ClientLayoutAdjustment, ClientViewCompletion, ClientViewCompletionOutcome, ClientViewControl,
    ClientViewControlReason as Reason, ClientViewIntent, ClientViewObservation, ClientViewSequence,
};
use std::time::Instant;

mod adjustment;
mod local_resize;
use crate::workbench_client::client_control::{ClientControlLane, Sample};
use adjustment::SharedAdjustment;
pub(in crate::workbench_client) use local_resize::LocalResizeFeedback;
use local_resize::LocalResizePending;

#[derive(Default)]
enum FocusModes {
    #[default]
    Unprepared,
    Exiting,
    Finished,
}

#[derive(Debug, PartialEq)]
enum ModeStep {
    Prepare,
    Wait,
    Navigate,
    Interrupted,
}

impl FocusModes {
    fn decide(&self, shell_mode: bool, dialog: bool, history: bool) -> ModeStep {
        match self {
            Self::Unprepared => ModeStep::Prepare,
            _ if shell_mode || dialog => ModeStep::Interrupted,
            Self::Exiting if history => ModeStep::Wait,
            Self::Finished if history => ModeStep::Interrupted,
            _ => ModeStep::Navigate,
        }
    }
}

#[derive(Default)]
pub(in crate::workbench_client) struct StepProgress {
    pub(in crate::workbench_client) refresh_requested: bool,
    pub(in crate::workbench_client) overlay_registered: bool,
    pub(in crate::workbench_client) context_attempted: bool,
    pub(in crate::workbench_client) tab_attempted: bool,
    pub(in crate::workbench_client) had_effect: bool,
    pub(in crate::workbench_client) toggle_origin: Option<bool>,
    focus_modes: FocusModes,
}
pub(in crate::workbench_client) enum ControlStep {
    Waiting,
    Changed,
    Observed,
    ZoomUnchanged,
    Failed(Reason),
    Completed(ClientViewCompletionOutcome),
}
enum Execution {
    Popup {
        target: ClientViewControl,
    },
    Navigation {
        target: ClientViewControl,
        progress: StepProgress,
    },
    Adjustment(SharedAdjustment),
}
impl Execution {
    fn focus_target(&self) -> Option<&ClientViewControl> {
        match self {
            Self::Navigation { target, .. }
                if matches!(
                    target.intent,
                    ClientViewIntent::Context { .. }
                        | ClientViewIntent::Tab { .. }
                        | ClientViewIntent::Panel { .. }
                        | ClientViewIntent::AgentFocus { .. }
                ) =>
            {
                Some(target)
            }
            // This handler implements its own ClientView intent subset; other intents belong to their feature handlers.
            _ => None,
        }
    }
    fn had_effect(&self) -> bool {
        match self {
            Self::Popup { .. } => true,
            Self::Navigation { progress, .. } => progress.had_effect,
            Self::Adjustment(flow) => flow.had_effect(),
        }
    }
    fn step(
        &mut self,
        native: &mut WorkbenchClientUi,
        shell_mode: bool,
        too_small: bool,
        dismiss_shell_modes: &mut bool,
    ) -> ControlStep {
        match self {
            Self::Popup { target } => {
                if shell_mode || too_small {
                    return ControlStep::Failed(Reason::Interrupted);
                }
                if native.popup_control_outcome(target).is_some() {
                    ControlStep::Observed
                } else {
                    ControlStep::Waiting
                }
            }
            Self::Navigation { target, progress }
                if matches!(
                    target.intent,
                    ClientViewIntent::AgentFocus { .. } | ClientViewIntent::Panel { .. }
                ) =>
            {
                if let Err(reason) = native.agent_focus_source(target) {
                    return ControlStep::Failed(reason);
                }
                if too_small {
                    return ControlStep::Failed(Reason::Busy);
                }
                match progress.focus_modes.decide(
                    shell_mode,
                    native.control_dialog(),
                    native.control_history(),
                ) {
                    ModeStep::Prepare => {
                        let changed = match native.dismiss_focus_modes(target) {
                            Ok(changed) => changed,
                            Err(reason) => return ControlStep::Failed(reason),
                        };
                        progress.focus_modes = FocusModes::Exiting;
                        progress.had_effect |= changed || shell_mode;
                        *dismiss_shell_modes = shell_mode;
                        if changed || shell_mode {
                            return ControlStep::Changed;
                        }
                    }
                    ModeStep::Wait => return ControlStep::Waiting,
                    ModeStep::Interrupted => return ControlStep::Failed(Reason::Interrupted),
                    ModeStep::Navigate => {}
                }
                progress.focus_modes = FocusModes::Finished;
                native.control_step(target, progress)
            }
            _ if shell_mode || too_small || native.control_modal() => {
                ControlStep::Failed(Reason::Interrupted)
            }
            Self::Navigation { target, progress } => native.control_step(target, progress),
            Self::Adjustment(flow) => flow.step(native),
        }
    }
    fn outcome(
        &self,
        native: &WorkbenchClientUi,
        command: &ClientViewControl,
        view: &ClientViewObservation,
    ) -> Option<ClientViewCompletionOutcome> {
        match self {
            Self::Popup { target } => native.popup_control_outcome(target),
            Self::Adjustment(flow) => flow.outcome(native),
            Self::Navigation { target, progress } => {
                if target
                    .intent
                    .is_observed(view, command.minimum_workbench_revision)
                {
                    Some(match (&command.intent, &target.intent) {
                        (
                            ClientViewIntent::FocusDirection { .. },
                            ClientViewIntent::Panel { panel_id, .. },
                        ) => ClientViewCompletionOutcome::DirectionFocused {
                            panel_id: panel_id.clone(),
                        },
                        // This handler implements its own ClientView intent subset; other intents belong to their feature handlers.
                        _ => ClientViewCompletionOutcome::Applied {},
                    })
                } else if let Some(previously_zoomed) = progress.toggle_origin
                    && command.intent.is_toggle_observed(
                        view,
                        command.minimum_workbench_revision,
                        previously_zoomed,
                    )
                {
                    Some(ClientViewCompletionOutcome::ZoomToggled { previously_zoomed })
                } else if command
                    .intent
                    .is_single_panel_zoom_observed(view, command.minimum_workbench_revision)
                {
                    Some(ClientViewCompletionOutcome::ZoomUnchanged {})
                } else {
                    None
                }
            }
        }
    }
}
struct Pending {
    command: ClientViewControl,
    execution: Execution,
    deadline: Instant,
    interrupted: bool,
    awaiting_draw: bool,
}
pub(in crate::workbench_client) struct ClientControl {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    lane: Option<ClientControlLane>,
    stopped: bool,
    heartbeat: u64,
    revision: u64,
    observed: Option<ClientViewObservation>,
    pending: Option<Pending>,
    local_resize: Option<LocalResizePending>,
    local_resize_feedback: Option<LocalResizeFeedback>,
    shell_blocked: bool,
    dismiss_shell_modes: bool,
    viewport: Option<(ratatui::layout::Rect, crate::client_view::SidebarSurface)>,
}
impl ClientControl {
    pub(in crate::workbench_client) fn invoke_configured(
        &mut self,
        native: &WorkbenchClientUi,
        id: String,
    ) -> Result<(), orchbee_workbench_protocol::ConfiguredCommandFailure> {
        use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
        if self.pending.is_some() || self.local_resize.is_some() || self.stopped {
            return Err(F::Busy);
        }
        let Some((source, view)) = native.configured_command_source() else {
            return Err(F::StaleSource);
        };
        let sample = self.sample(view, native).ok_or(F::Unavailable)?;
        self.lane
            .as_ref()
            .ok_or(F::Unavailable)?
            .invoke_configured(id, source, sample)
    }
    pub(in crate::workbench_client) const fn new(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
    ) -> Self {
        Self {
            origin,
            clock,
            lane: None,
            stopped: false,
            heartbeat: 0,
            revision: 0,
            observed: None,
            pending: None,
            local_resize: None,
            local_resize_feedback: None,
            shell_blocked: false,
            dismiss_shell_modes: false,
            viewport: None,
        }
    }
    pub(in crate::workbench_client) fn observe_geometry(
        &mut self,
        viewport: ratatui::layout::Rect,
        sidebar: crate::client_view::SidebarSurface,
    ) {
        self.viewport = Some((viewport, sidebar));
    }
    fn sample(
        &mut self,
        view: ClientViewObservation,
        native: &WorkbenchClientUi,
    ) -> Option<Sample> {
        self.heartbeat = self.heartbeat.checked_add(1)?;
        if self.observed.as_ref() != Some(&view) {
            self.revision = self.revision.checked_add(1)?;
            self.observed = Some(view.clone());
        }
        Some(Sample {
            heartbeat: ClientViewSequence::new(self.heartbeat).ok()?,
            revision: ClientViewSequence::new(self.revision).ok()?,
            graphics: native.graphics_observation(&view),
            view,
        })
    }
    pub(in crate::workbench_client) fn interrupt(&mut self) {
        self.interrupt_local_resize();
        if let Some(pending) = &mut self.pending {
            pending.interrupted = true;
        }
    }
    pub(in crate::workbench_client) fn interrupt_focus(&mut self) {
        if let Some(pending) = &mut self.pending
            && pending.execution.focus_target().is_some()
        {
            pending.interrupted = true;
        }
    }
    pub(in crate::workbench_client) fn take_shell_mode_dismissal(&mut self) -> bool {
        std::mem::take(&mut self.dismiss_shell_modes)
    }
    pub(in crate::workbench_client) fn tick(
        &mut self,
        native: &mut WorkbenchClientUi,
        shell_mode: bool,
    ) -> bool {
        let too_small = self.viewport.is_some_and(|(viewport, sidebar)| {
            crate::tui_layout::project_shell_areas(viewport, sidebar).tier
                == crate::tui_layout::ViewportTier::TooSmall
        });
        let shell_blocked = shell_mode || too_small;
        self.shell_blocked = shell_blocked;
        if self.stopped {
            return false;
        }
        let Some((session_id, view)) =
            native.control_observation(shell_blocked || self.local_resize.is_some())
        else {
            return false;
        };
        let view_changed = self.observed.as_ref() != Some(&view);
        let Some(sample) = self.sample(view, native) else {
            self.close();
            return false;
        };
        if self.lane.is_none() {
            match ClientControlLane::spawn(
                self.origin.clone(),
                self.clock.clone(),
                session_id.clone(),
            ) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    self.close();
                    return false;
                }
            }
        }
        if let Some(result) = self
            .lane
            .as_ref()
            .and_then(ClientControlLane::take_layout_result)
        {
            if let Some(pending) = self.local_resize.as_mut()
                && pending.request_id == result.operation_id
            {
                pending.flow.settle(result.reply);
            } else if let Some(pending) = self.pending.as_mut()
                && pending.command.operation_id == result.operation_id
                && let Execution::Adjustment(flow) = &mut pending.execution
            {
                flow.settle(result.reply);
            }
        }
        let local_resize_changed = self.advance_local_resize(native, too_small);
        let Some(lane) = &self.lane else {
            return local_resize_changed;
        };
        let command_changed = if let Some(result) = lane.take_invocation_result() {
            native.configured_command_completed(result);
            true
        } else {
            false
        };
        if lane.failed() || !lane.publish(sample.clone()) {
            self.close();
            return false;
        }
        if let Some(delivery) = lane.take_control() {
            let command = delivery.command;
            if self.pending.is_some() {
                self.close();
                return false;
            }
            let reason = if self.local_resize.is_some() {
                Some(Reason::Busy)
            } else if command.expected_view_revision != sample.revision {
                Some(Reason::StaleView)
            } else if command.intent.blocked_by_busy_phase(sample.view.phase) {
                Some(Reason::Busy)
            } else {
                None
            };
            if let Some(reason) = reason {
                if !lane.complete(
                    sample,
                    ClientViewCompletion {
                        operation_id: command.operation_id,
                        outcome: ClientViewCompletionOutcome::Rejected { reason },
                    },
                ) {
                    self.close();
                }
                return false;
            }
            let execution = if matches!(
                command.intent,
                ClientViewIntent::CommandPopupOpen { .. }
                    | ClientViewIntent::PopupOpen { .. }
                    | ClientViewIntent::PopupClose {}
                    | ClientViewIntent::EditorOpen { .. }
            ) {
                if let Err(reason) =
                    native.begin_popup_control(&command, delivery.workbench_snapshot.as_ref())
                {
                    if !lane.complete(
                        sample,
                        ClientViewCompletion {
                            operation_id: command.operation_id,
                            outcome: ClientViewCompletionOutcome::Rejected { reason },
                        },
                    ) {
                        self.close();
                    }
                    return false;
                }
                Execution::Popup {
                    target: command.clone(),
                }
            } else if matches!(
                command.intent,
                ClientViewIntent::Resize { .. } | ClientViewIntent::SetSplitRatio { .. }
            ) {
                let prepared = delivery
                    .workbench_snapshot
                    .as_ref()
                    .ok_or(Reason::ProjectionUnavailable)
                    .and_then(|snapshot| {
                        if shell_blocked || native.control_modal() {
                            return Err(Reason::Interrupted);
                        }
                        native.prepare_layout_adjustment(&command, snapshot, self.viewport)
                    });
                match prepared {
                    Ok(adjustment) if adjustment.change.changed() => {
                        let prepared = SharedAdjustment::prepare(
                            session_id,
                            command.operation_id.clone(),
                            adjustment,
                            sample.view.clone(),
                        )
                        .and_then(|(flow, mutation)| lane.submit_layout(mutation).map(|()| flow));
                        match prepared {
                            Ok(flow) => Execution::Adjustment(flow),
                            Err(reason) => {
                                if !lane.complete(
                                    sample,
                                    ClientViewCompletion {
                                        operation_id: command.operation_id,
                                        outcome: ClientViewCompletionOutcome::Rejected { reason },
                                    },
                                ) {
                                    self.close();
                                }
                                return false;
                            }
                        }
                    }
                    result => {
                        let outcome = match result {
                            Ok(adjustment) => {
                                ClientViewCompletionOutcome::LayoutAdjusted { adjustment }
                            }
                            Err(reason) => ClientViewCompletionOutcome::Rejected { reason },
                        };
                        if !lane.complete(
                            sample,
                            ClientViewCompletion {
                                operation_id: command.operation_id,
                                outcome,
                            },
                        ) {
                            self.close();
                        }
                        return false;
                    }
                }
            } else {
                let target = match prepare_control(
                    native,
                    &command,
                    delivery.workbench_snapshot.as_ref(),
                    self.viewport,
                ) {
                    Ok(execution) => execution,
                    Err(outcome) => {
                        if !lane.complete(
                            sample,
                            ClientViewCompletion {
                                operation_id: command.operation_id,
                                outcome: *outcome,
                            },
                        ) {
                            self.close();
                        }
                        // Layout reads and no-neighbor decisions have no display side effects.
                        return false;
                    }
                };
                Execution::Navigation {
                    target,
                    progress: StepProgress::default(),
                }
            };
            self.pending = Some(Pending {
                command,
                execution,
                deadline: self.clock.now() + crate::workbench_client::state::OPERATION_TOTAL,
                interrupted: false,
                awaiting_draw: false,
            });
        }
        let Some(pending) = &mut self.pending else {
            return command_changed || local_resize_changed;
        };
        let step = if pending.interrupted {
            ControlStep::Failed(Reason::Interrupted)
        } else if self.clock.now() >= pending.deadline {
            ControlStep::Failed(Reason::Expired)
        } else {
            pending
                .execution
                .step(native, shell_mode, too_small, &mut self.dismiss_shell_modes)
        };
        match step {
            ControlStep::Completed(outcome) => {
                let completion = ClientViewCompletion {
                    operation_id: pending.command.operation_id.clone(),
                    outcome,
                };
                self.pending = None;
                let Some((_, view)) = native.control_observation(shell_blocked) else {
                    self.close();
                    return false;
                };
                let Some(sample) = self.sample(view, native) else {
                    self.close();
                    return false;
                };
                if !self
                    .lane
                    .as_ref()
                    .is_some_and(|lane| lane.complete(sample, completion))
                {
                    self.close();
                }
                view_changed
            }
            ControlStep::Waiting => view_changed,
            ControlStep::Changed => true,
            ControlStep::Observed | ControlStep::ZoomUnchanged => {
                pending.awaiting_draw = true;
                true
            }
            ControlStep::Failed(reason) => {
                let completion = ClientViewCompletion {
                    operation_id: pending.command.operation_id.clone(),
                    outcome: if pending.execution.had_effect() {
                        ClientViewCompletionOutcome::Incomplete { reason }
                    } else {
                        ClientViewCompletionOutcome::Rejected { reason }
                    },
                };
                self.pending = None;
                // The observation is resampled after the last attempted semantic step.
                let Some((_, view)) = native.control_observation(shell_blocked) else {
                    self.close();
                    return true;
                };
                let Some(sample) = self.sample(view, native) else {
                    self.close();
                    return true;
                };
                if !self
                    .lane
                    .as_ref()
                    .is_some_and(|lane| lane.complete(sample, completion))
                {
                    self.close();
                }
                true
            }
        }
    }
    pub(in crate::workbench_client) fn after_draw(&mut self, native: &mut WorkbenchClientUi) {
        if self.stopped {
            return;
        }
        self.local_resize_after_draw(native);
        let Some((_, view)) =
            native.control_observation(self.shell_blocked || self.local_resize.is_some())
        else {
            return;
        };
        let completion = self
            .pending
            .as_ref()
            .filter(|pending| pending.awaiting_draw && !pending.interrupted)
            .and_then(|pending| {
                let mut outcome = pending.execution.outcome(native, &pending.command, &view)?;
                if let Some(target) = pending.execution.focus_target() {
                    let result = if self.clock.now() >= pending.deadline {
                        Err(Reason::Expired)
                    } else {
                        native.finish_focus(target)
                    };
                    if let Err(reason) = result {
                        outcome = if pending.execution.had_effect() {
                            ClientViewCompletionOutcome::Incomplete { reason }
                        } else {
                            ClientViewCompletionOutcome::Rejected { reason }
                        };
                    }
                }
                Some(ClientViewCompletion {
                    operation_id: pending.command.operation_id.clone(),
                    outcome,
                })
            });
        let Some(sample) = self.sample(view, native) else {
            self.close();
            return;
        };
        let Some(lane) = &self.lane else {
            return;
        };
        if let Some(completion) = completion {
            if lane.complete(sample, completion) {
                self.pending = None;
            } else {
                self.close();
            }
        } else if !lane.publish(sample) {
            self.close();
        }
    }
    pub(in crate::workbench_client) fn close(&mut self) {
        self.stopped = true;
        if self.local_resize.take().is_some() {
            self.local_resize_feedback = Some(LocalResizeFeedback::Notice(
                crate::client_view::Notice::ResizeUnknown,
            ));
        }
        self.pending = None;
        self.dismiss_shell_modes = false;
        if let Some(mut lane) = self.lane.take() {
            lane.close();
        }
    }
    /// Process exit: asks the control lane to stop without waiting.
    pub(in crate::workbench_client) fn request_stop(&mut self) {
        self.stopped = true;
        if let Some(lane) = &mut self.lane {
            lane.request_stop();
        }
    }
    /// Process exit: observes the control lane against the shared deadline.
    pub(in crate::workbench_client) fn stop_before(
        &mut self,
        deadline: std::time::Instant,
    ) -> std::io::Result<()> {
        self.request_stop();
        self.lane
            .as_mut()
            .map_or(Ok(()), |lane| lane.stop_before(deadline))
    }
}

/// Resolve geometry exactly once, before any navigation. Err is a terminal receipt,
/// including successful reads and no-neighbor observations that require no draw.
fn prepare_control(
    native: &WorkbenchClientUi,
    command: &ClientViewControl,
    snapshot: Option<&orchbee_workbench_protocol::Snapshot>,
    viewport: Option<(ratatui::layout::Rect, crate::client_view::SidebarSurface)>,
) -> Result<ClientViewControl, Box<ClientViewCompletionOutcome>> {
    if native.popup_captured() && !command.intent.is_passive_read() {
        return Err(Box::new(ClientViewCompletionOutcome::Rejected {
            reason: Reason::Busy,
        }));
    }
    native
        .agent_focus_source(command)
        .map_err(|reason| Box::new(ClientViewCompletionOutcome::Rejected { reason }))?;
    if matches!(command.intent, ClientViewIntent::Panel { .. }) {
        snapshot
            .ok_or(Reason::ProjectionUnavailable)
            .and_then(|snapshot| native.validate_panel_focus(command, snapshot))
            .map_err(|reason| Box::new(ClientViewCompletionOutcome::Rejected { reason }))?;
    }
    if matches!(command.intent, ClientViewIntent::Presentation { .. }) {
        return Err(Box::new(
            snapshot
                .ok_or(Reason::ProjectionUnavailable)
                .and_then(|snapshot| native.read_client_presentation(command, snapshot))
                .map_or_else(
                    |reason| ClientViewCompletionOutcome::Rejected { reason },
                    |presentation| ClientViewCompletionOutcome::PresentationRead {
                        presentation: Box::new(presentation),
                    },
                ),
        ));
    }
    if matches!(command.intent, ClientViewIntent::Viewports { .. }) {
        return Err(Box::new(
            snapshot
                .ok_or(Reason::ProjectionUnavailable)
                .and_then(|snapshot| native.read_client_viewports(command, snapshot))
                .map_or_else(
                    |reason| ClientViewCompletionOutcome::Rejected { reason },
                    |viewport| ClientViewCompletionOutcome::ViewportRead {
                        viewport: Box::new(viewport),
                    },
                ),
        ));
    }
    if matches!(command.intent, ClientViewIntent::Selections { .. }) {
        return Err(Box::new(
            snapshot
                .ok_or(Reason::ProjectionUnavailable)
                .and_then(|snapshot| native.read_client_selections(command, snapshot))
                .map_or_else(
                    |reason| ClientViewCompletionOutcome::Rejected { reason },
                    |navigation| ClientViewCompletionOutcome::NavigationRead {
                        navigation: Box::new(navigation),
                    },
                ),
        ));
    }
    if let ClientViewIntent::TerminalAgentAttention { bindings } = &command.intent {
        return Err(Box::new(
            native.read_terminal_agent_attention(bindings).map_or(
                ClientViewCompletionOutcome::Rejected {
                    reason: Reason::ProjectionUnavailable,
                },
                |attention| ClientViewCompletionOutcome::TerminalAttentionRead {
                    attention: Box::new(attention),
                },
            ),
        ));
    }
    if let ClientViewIntent::AgentAttention { session_ids } = &command.intent {
        return Err(Box::new(native.read_agent_attention(session_ids).map_or(
            ClientViewCompletionOutcome::Rejected {
                reason: Reason::ProjectionUnavailable,
            },
            |attention| ClientViewCompletionOutcome::AttentionRead {
                attention: Box::new(attention),
            },
        )));
    }
    if !matches!(
        command.intent,
        ClientViewIntent::Layout { .. } | ClientViewIntent::FocusDirection { .. }
    ) {
        return Ok(command.clone());
    }
    let layout = match (snapshot, viewport) {
        (Some(snapshot), Some((viewport, sidebar))) => {
            native.read_client_layout(command, snapshot, viewport, sidebar)
        }
        _ => Err(Reason::ProjectionUnavailable),
    }
    .map_err(|reason| ClientViewCompletionOutcome::Rejected { reason })?;
    let ClientViewIntent::FocusDirection { direction, .. } = &command.intent else {
        return Err(Box::new(ClientViewCompletionOutcome::LayoutRead {
            layout: Box::new(layout),
        }));
    };
    if !layout.source_available {
        return Err(Box::new(ClientViewCompletionOutcome::Rejected {
            reason: Reason::ProjectionUnavailable,
        }));
    }
    let Some(destination) = layout.neighbors.get(*direction) else {
        return Err(Box::new(ClientViewCompletionOutcome::DirectionUnchanged {
            layout_revision: layout.workbench_revision,
        }));
    };
    let intent = command.intent.directional_target(destination).ok_or(
        ClientViewCompletionOutcome::Rejected {
            reason: Reason::ProjectionUnavailable,
        },
    )?;
    Ok(ClientViewControl {
        intent,
        ..command.clone()
    })
}

#[cfg(test)]
mod tests;
