//! Admits Client takeover actions and consumes correlated results into UI controller state
//! while retaining evidence across connection-task loss. It does not grant Server authority or
//! replay a submitted input effect. A fresh focus activation may reconcile an uncertain grant.

use super::{WorkbenchClientUi, WorkbenchClientView};
use crate::client_view::ClientActionDisposition;
use crate::workbench_client::mailbox::{
    CommandMailbox, MutationResult, RefreshSettlement, UiReply, UiWaitKey,
};
use crate::workbench_state::control_takeover::{TakeoverEvidence, TakeoverStatus};
use crate::workbench_state::{CarrierFailure, ClientNotice, OperationDeadlines, RefreshDecision};
use orchbee_client_server_protocol::RequestId;
use std::sync::Arc;

/// A focus activation admits at most one transfer. Failure never rearms it; a later host or
/// pane focus change is a new activation. Readiness may defer admission, never replay input.
#[derive(Default)]
pub(super) struct FocusControl {
    pane: Option<crate::workbench_state::PaneRef>,
    pending: Option<FocusActivation>,
    activated: bool,
    input: Option<FocusInput>,
}

struct FocusInput {
    pane: crate::workbench_state::PaneRef,
    deadline: std::time::Instant,
    bytes: Vec<u8>,
}

enum FocusActivation {
    Initial,
    Return,
}

impl FocusControl {
    pub(super) fn keyboard(&mut self) {
        if matches!(self.pending, Some(FocusActivation::Initial)) {
            self.pending = Some(FocusActivation::Return);
        }
    }
    pub(super) fn activate(&mut self, focused: bool) {
        self.input = None;
        self.pending = focused.then_some(if self.activated {
            FocusActivation::Return
        } else {
            FocusActivation::Initial
        });
        self.activated |= focused;
    }
}

impl WorkbenchClientView<'_> {
    pub(crate) fn takeover_status(&self) -> Option<(TakeoverStatus, &RequestId)> {
        self.state.as_ref()?.takeover_status()
    }
}
impl WorkbenchClientUi {
    pub(super) fn drive_focus_control(&mut self) -> bool {
        let pane = self.state.as_ref().and_then(|state| state.focus_pane_ref());
        if pane != self.focus_control.pane {
            if self
                .focus_control
                .input
                .as_ref()
                .is_some_and(|input| Some(&input.pane) != pane.as_ref())
            {
                self.focus_control.input = None;
            }
            self.focus_control.pane = pane;
            // Arm before Acquire has resolved the new pane's controller mode.
            if self.outer_focused == Some(true) {
                self.focus_control
                    .pending
                    .get_or_insert(FocusActivation::Return);
            }
        }
        if matches!(self.focus_control.pending, Some(FocusActivation::Initial))
            && self.state.as_ref().is_some_and(|state| {
                matches!(
                    state.controller_mode(),
                    Some(crate::workbench_state::ControllerMode::Controlling(_))
                )
            })
        {
            self.focus_control.pending = None;
        }
        if self.focus_control.pending.is_none()
            || self.outer_focused == Some(false)
            || self.modal_owns_input()
            || self.action_menu_active()
            || self.focus_viewport_non_live()
        {
            return false;
        }
        let view = self.view();
        if view
            .focus_panel_id()
            .is_some_and(|panel| view.settled_end(panel).is_some())
        {
            self.focus_control.pending = None;
            return false;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return false;
        };
        let (Some(mailbox), Some(state)) = (&self.mailbox, &mut self.state) else {
            return false;
        };
        if mailbox.try_enqueue_focus_activation(state, deadlines, &self.clock) {
            if self.focus_control.input.is_none() && !state.takeover_input_ready() {
                self.focus_control.input = state.focus_pane_ref().map(|pane| FocusInput {
                    pane,
                    deadline: deadlines.total(),
                    bytes: Vec::new(),
                });
            }
            self.focus_control.pending = None;
            return true;
        }
        false
    }

    /// These bytes have never been submitted. They belong only to the activation's exact pane,
    /// and are discarded on focus loss, replacement, refusal, retirement, or the fixed deadline.
    pub(super) fn hold_focus_input(&mut self, bytes: &[u8]) -> Option<bool> {
        // Known owners keep ordinary input admission. Queue the focus observation first even
        // when focus and typing arrive in one decoder packet; Server validation stays authoritative.
        if self.focus_control.input.is_none()
            && self.state.as_ref().is_some_and(|state| {
                state.takeover_input_ready()
                    && matches!(
                        state.controller_mode(),
                        Some(crate::workbench_state::ControllerMode::Controlling(_))
                    )
            })
        {
            self.drive_focus_control();
            if self
                .state
                .as_ref()
                .is_some_and(|state| state.takeover_input_ready())
            {
                return None;
            }
        }
        // A focus event and its first keys may arrive in one decoder packet, before the next tick.
        if self.focus_control.input.is_none()
            && self.focus_control.pending.is_some()
            && !(matches!(self.focus_control.pending, Some(FocusActivation::Initial))
                && self.state.as_ref().is_some_and(|state| {
                    matches!(
                        state.controller_mode(),
                        Some(crate::workbench_state::ControllerMode::Controlling(_))
                    )
                }))
            && let Some(pane) = self.state.as_ref().and_then(|state| state.focus_pane_ref())
            && let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now())
        {
            self.focus_control.input = Some(FocusInput {
                pane,
                deadline: deadlines.total(),
                bytes: Vec::new(),
            });
        }
        let input = self.focus_control.input.as_mut()?;
        let state = self.state.as_ref()?;
        if self.outer_focused == Some(false)
            || state.focus_pane_ref().as_ref() != Some(&input.pane)
            || self.clock.now() >= input.deadline
            || self.retirement_failure.is_some()
        {
            self.focus_control.input = None;
            return Some(false);
        }
        if input.bytes.len().saturating_add(bytes.len())
            > orchbee_client_server_protocol::MAXIMUM_CONTROL_INPUT_BYTES
        {
            return Some(false);
        }
        input.bytes.extend_from_slice(bytes);
        Some(true)
    }

    pub(super) fn drain_focus_input(&mut self) -> bool {
        let captured =
            self.modal_owns_input() || self.action_menu_active() || self.focus_viewport_non_live();
        let Some(input) = self.focus_control.input.as_ref() else {
            return false;
        };
        if self.outer_focused == Some(false)
            || captured
            || self.retirement_failure.is_some()
            || self.clock.now() >= input.deadline
            || self
                .state
                .as_ref()
                .and_then(|state| state.focus_pane_ref())
                .as_ref()
                != Some(&input.pane)
        {
            self.focus_control.input = None;
            return false;
        }
        if self.focus_control.pending.is_some() {
            return false;
        }
        let Some(accepted) = self
            .state
            .as_ref()
            .and_then(|state| state.focus_input_settlement())
        else {
            return false;
        };
        let Some(input) = self.focus_control.input.take() else {
            return false;
        };
        if !accepted || input.bytes.is_empty() {
            return false;
        }
        if self.forward(&input.bytes) {
            return true;
        }
        // Admission refusal is known to have no effect; the same bounded activation still owns it.
        self.focus_control.input = Some(input);
        false
    }

    pub(super) fn request_takeover(&mut self) -> ClientActionDisposition {
        // An ended terminal accepts no controller; its retained exit fact stays visible
        // instead of a refusal the Server would certainly return.
        let view = self.view();
        let ended = view
            .focus_panel_id()
            .is_some_and(|panel| view.settled_end(panel).is_some());
        if ended || self.modal_owns_input() || self.focus_viewport_non_live() {
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) = (&self.mailbox, &mut self.state) else {
            return ClientActionDisposition::KeepNavigator;
        };
        if mailbox.try_enqueue_takeover_for(state, deadlines, &self.clock) {
            self.focus_control.pending = None;
            ClientActionDisposition::DismissNavigator
        } else {
            state.set_notice(ClientNotice::LocalBusy);
            ClientActionDisposition::KeepNavigator
        }
    }
    pub(in crate::workbench_client) fn takeover_worker_loss(&self) -> Option<TakeoverEvidence> {
        self.mailbox
            .as_ref()
            .and_then(|mailbox| mailbox.takeover_worker_loss())
    }
    pub(in crate::workbench_client) fn finalize_takeover_worker(
        &mut self,
        evidence: Option<TakeoverEvidence>,
    ) -> bool {
        evidence.is_none_or(|evidence| {
            self.state
                .as_mut()
                .is_some_and(|state| state.finish_takeover(&evidence, self.clock.now()))
        })
    }
    pub(super) fn apply_takeover_event(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: UiWaitKey,
        evidence: TakeoverEvidence,
    ) -> bool {
        let Some(state) = self.state.as_mut() else {
            self.retire(CarrierFailure::Protocol);
            return true;
        };
        // Retain even when reply deposition cannot proceed after this boundary.
        if !state.retain_takeover(&evidence) {
            self.retire(CarrierFailure::Protocol);
            return true;
        }
        let result = mailbox.deposit_ui_reply_before(&wait, &self.clock, |transaction| {
            if !state.finish_takeover(&evidence, transaction.observed_at()) {
                return Err(CarrierFailure::Protocol);
            }
            Ok(UiReply::Refresh(RefreshSettlement::new(
                RefreshDecision::Continue,
                state.desired().cloned(),
            )))
        });
        match result {
            Ok(MutationResult::Applied) => self.validate_state(),
            Ok(MutationResult::Stale) => {
                self.retire(CarrierFailure::Protocol);
                true
            }
            Err(error) => self.handle_event_error(error),
        }
    }
}
