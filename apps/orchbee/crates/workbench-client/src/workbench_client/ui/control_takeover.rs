//! Admits Client takeover actions and consumes correlated results into UI controller state
//! while retaining evidence across connection-task loss. It does not grant Server authority or
//! submit a second takeover after unknown delivery.

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
}

enum FocusActivation {
    Initial,
    Return,
}

impl FocusControl {
    pub(super) fn activate(&mut self, focused: bool) {
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
            self.focus_control.pane = pane;
            // Selecting an already controlled pane needs no barrier. In particular a fresh
            // startup grant must not fence the first click or its selection/drag continuation.
            if self.outer_focused == Some(true)
                && self.state.as_ref().is_some_and(|state| {
                    matches!(
                        state.controller_mode(),
                        Some(crate::workbench_state::ControllerMode::ReadOnly)
                    )
                })
            {
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
        if mailbox.try_enqueue_takeover_for(state, deadlines, &self.clock) {
            self.focus_control.pending = None;
            return true;
        }
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
