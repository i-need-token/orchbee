//! Closes one Tab, a set of Tabs or a Panel without its confirmation when the Server finds nothing
//! running in it.
//!
//! The confirmation opens first and stays hidden while the Server decides: the Client sends the
//! conditional close, and the Server reads every live terminal that close would end. Accepted,
//! the close goes on as a confirmed one; when the Server answers that the user must confirm, the
//! armed confirmation presents. A set of several Tabs asks the same way with one conditional Start
//! of the whole set. The rules are the Server's; see `docs/native-tab-close-sets.md`. A ReadOnly
//! Client and a Resume always present the confirmation and send nothing.

use super::{ActionBuildError, WorkbenchClientUi};
use crate::workbench_client::mailbox::{CloseAdmissionError, MoveAdmissionError};
use crate::workbench_state::{
    CloseConfirmationToken, ControllerMode, OPERATION_TOTAL, PanelCloseConfirmationToken,
    WorkbenchClientState,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Target {
    Tab(CloseConfirmationToken),
    Panel(PanelCloseConfirmationToken),
}

impl WorkbenchClientUi {
    /// Asks the Server whether the just-opened confirmation of `target` may close unasked, keeping
    /// it hidden until the answer. A close the Client cannot send presents the confirmation.
    pub(super) fn begin_idle_close(&mut self, target: Target) {
        let controlling = self.state.as_ref().is_some_and(|state| {
            matches!(
                state.controller_mode(),
                Some(ControllerMode::Controlling(_))
            )
        });
        if !self.idle_close || !controlling {
            return;
        }
        let Some(deadline) = self.clock.now().checked_add(OPERATION_TOTAL) else {
            return;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return;
        };
        let closed = match target {
            Target::Tab(token) => match mailbox.try_enqueue_close_for(
                state,
                deadline,
                &self.clock,
                |state, request_id, asked_at| {
                    state.plan_close_if_idle(token, request_id, deadline, asked_at)
                },
                WorkbenchClientState::begin_close_exchange,
            ) {
                Err(CloseAdmissionError::Closed(closed)) => Some(closed),
                _ => None,
            },
            Target::Panel(token) => match mailbox.try_enqueue_panel_for(
                state,
                deadline,
                &self.clock,
                |state, request_id| {
                    state
                        .plan_panel_close_if_idle(token, request_id, deadline)
                        .map_err(|_| ActionBuildError::Busy)
                },
                |state, operation| {
                    state
                        .begin_panel_close_if_idle(operation)
                        .map_err(|_| ActionBuildError::Busy)
                },
            ) {
                Err(MoveAdmissionError::Closed(closed)) => Some(closed),
                _ => None,
            },
        };
        if let Some(closed) = closed {
            self.retire(closed.failure());
        }
    }
}
