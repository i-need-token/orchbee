//! UI-owned remote navigation. The lane transports observations; only this owner advances UI steps.
use super::ui::NativeUi;
use crate::local_client::{LocalClientOrigin, LocalOperationClock};
use orchbee_workbench_protocol::{
    ClientLayoutAdjustment, ClientViewCompletion, ClientViewCompletionOutcome, ClientViewControl,
    ClientViewControlReason as Reason, ClientViewIntent, ClientViewObservation, ClientViewPhase,
    ClientViewSequence,
};
use std::time::Instant;

mod lane;
use lane::{ClientControlLane, Sample};

#[derive(Default)]
pub(super) struct StepProgress {
    pub(super) refresh_requested: bool,
    pub(super) context_attempted: bool,
    pub(super) tab_attempted: bool,
    pub(super) had_effect: bool,
    pub(super) toggle_origin: Option<bool>,
}
pub(super) enum ControlStep {
    Waiting,
    Changed,
    Observed,
    ZoomUnchanged,
    Failed(Reason),
}
enum Execution {
    Navigation {
        target: ClientViewControl,
        progress: StepProgress,
    },
    Adjustment {
        adjustment: ClientLayoutAdjustment,
        before: ClientViewObservation,
    },
}
impl Execution {
    fn had_effect(&self) -> bool {
        match self {
            Self::Navigation { progress, .. } => progress.had_effect,
            Self::Adjustment { .. } => true,
        }
    }
    fn step(&mut self, native: &mut NativeUi) -> ControlStep {
        match self {
            Self::Navigation { target, progress } => native.control_step(target, progress),
            Self::Adjustment { adjustment, before } => {
                if native.adjustment_observed(adjustment, before) {
                    ControlStep::Observed
                } else {
                    ControlStep::Failed(Reason::StaleView)
                }
            }
        }
    }
    fn outcome(
        &self,
        native: &NativeUi,
        command: &ClientViewControl,
        view: &ClientViewObservation,
    ) -> Option<ClientViewCompletionOutcome> {
        match self {
            Self::Adjustment { adjustment, before } => native
                .adjustment_observed(adjustment, before)
                .then(|| ClientViewCompletionOutcome::LayoutAdjusted {
                    adjustment: adjustment.clone(),
                }),
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
pub(super) struct ClientControl {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    lane: Option<ClientControlLane>,
    stopped: bool,
    heartbeat: u64,
    revision: u64,
    observed: Option<ClientViewObservation>,
    pending: Option<Pending>,
    shell_blocked: bool,
    viewport: Option<(ratatui::layout::Rect, crate::client_view::SidebarSurface)>,
}
impl ClientControl {
    pub(super) const fn new(origin: LocalClientOrigin, clock: LocalOperationClock) -> Self {
        Self {
            origin,
            clock,
            lane: None,
            stopped: false,
            heartbeat: 0,
            revision: 0,
            observed: None,
            pending: None,
            shell_blocked: false,
            viewport: None,
        }
    }
    pub(super) fn observe_geometry(
        &mut self,
        viewport: ratatui::layout::Rect,
        sidebar: crate::client_view::SidebarSurface,
    ) {
        self.viewport = Some((viewport, sidebar));
    }
    fn sample(&mut self, view: ClientViewObservation) -> Option<Sample> {
        self.heartbeat = self.heartbeat.checked_add(1)?;
        if self.observed.as_ref() != Some(&view) {
            self.revision = self.revision.checked_add(1)?;
            self.observed = Some(view.clone());
        }
        Some(Sample {
            heartbeat: ClientViewSequence::new(self.heartbeat).ok()?,
            revision: ClientViewSequence::new(self.revision).ok()?,
            view,
        })
    }
    pub(super) fn interrupt(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.interrupted = true;
        }
    }
    pub(super) fn tick(&mut self, native: &mut NativeUi, shell_blocked: bool) -> bool {
        self.shell_blocked = shell_blocked;
        if self.stopped {
            return false;
        }
        let Some((site_id, view)) = native.control_observation(shell_blocked) else {
            return false;
        };
        let view_changed = self.observed.as_ref() != Some(&view);
        let Some(sample) = self.sample(view) else {
            self.close();
            return false;
        };
        if self.lane.is_none() {
            match ClientControlLane::spawn(self.origin.clone(), self.clock.clone(), site_id) {
                Ok(lane) => self.lane = Some(lane),
                Err(_) => {
                    self.close();
                    return false;
                }
            }
        }
        let Some(lane) = &self.lane else {
            return false;
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
            let reason = if command.expected_view_revision != sample.revision {
                Some(Reason::StaleView)
            } else if !command.intent.is_passive_read()
                && matches!(
                    sample.view.phase,
                    ClientViewPhase::Starting | ClientViewPhase::Busy
                )
            {
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
                ClientViewIntent::Resize { .. } | ClientViewIntent::SetSplitRatio { .. }
            ) {
                let prepared = delivery
                    .layout_snapshot
                    .as_ref()
                    .ok_or(Reason::ProjectionUnavailable)
                    .and_then(|snapshot| {
                        let adjustment =
                            native.prepare_layout_adjustment(&command, snapshot, self.viewport)?;
                        native.commit_layout_adjustment(snapshot, &adjustment)?;
                        Ok(adjustment)
                    });
                match prepared {
                    Ok(adjustment) if adjustment.selected && adjustment.change.changed() => {
                        Execution::Adjustment {
                            adjustment,
                            before: sample.view.clone(),
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
                    delivery.layout_snapshot.as_ref(),
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
                deadline: self.clock.now() + super::state::OPERATION_TOTAL,
                interrupted: false,
                awaiting_draw: false,
            });
        }
        let Some(pending) = &mut self.pending else {
            return false;
        };
        let step = if pending.interrupted || shell_blocked || native.control_modal() {
            ControlStep::Failed(Reason::Interrupted)
        } else if self.clock.now() >= pending.deadline {
            ControlStep::Failed(Reason::Expired)
        } else {
            pending.execution.step(native)
        };
        match step {
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
                let Some(sample) = self.sample(view) else {
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
    pub(super) fn after_draw(&mut self, native: &NativeUi) {
        if self.stopped {
            return;
        }
        let Some((_, view)) = native.control_observation(self.shell_blocked) else {
            return;
        };
        let completion = self
            .pending
            .as_ref()
            .filter(|pending| pending.awaiting_draw && !pending.interrupted)
            .and_then(|pending| {
                let outcome = pending.execution.outcome(native, &pending.command, &view)?;
                Some(ClientViewCompletion {
                    operation_id: pending.command.operation_id.clone(),
                    outcome,
                })
            });
        let Some(sample) = self.sample(view) else {
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
    pub(super) fn close(&mut self) {
        self.stopped = true;
        self.pending = None;
        if let Some(mut lane) = self.lane.take() {
            lane.close();
        }
    }
}

/// Resolve geometry exactly once, before any navigation. Err is a terminal receipt,
/// including successful reads and no-neighbor observations that require no draw.
fn prepare_control(
    native: &NativeUi,
    command: &ClientViewControl,
    snapshot: Option<&orchbee_workbench_protocol::Snapshot>,
    viewport: Option<(ratatui::layout::Rect, crate::client_view::SidebarSurface)>,
) -> Result<ClientViewControl, Box<ClientViewCompletionOutcome>> {
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
