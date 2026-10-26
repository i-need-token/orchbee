use super::{NativeUi, WorkbenchClientView};
use crate::{
    client_view::ClientActionDisposition,
    workbench_client::{
        mailbox::{CommandMailbox, MutationResult, RefreshSettlement, UiReply, UiWaitKey},
        state::{
            CarrierFailure, ClientNotice, OperationDeadlines, RefreshDecision,
            control_takeover::{TakeoverEvidence, TakeoverStatus},
        },
    },
};
use orchbee_client_server_protocol::RequestId;
use std::sync::Arc;

impl WorkbenchClientView<'_> {
    pub(crate) fn takeover_status(&self) -> Option<(TakeoverStatus, &RequestId)> {
        self.state.as_ref()?.takeover_status()
    }
}
impl NativeUi {
    pub(super) fn request_takeover(&mut self) -> ClientActionDisposition {
        // An ended terminal accepts no controller; its retained exit fact stays visible
        // instead of a refusal the Server would certainly return.
        let view = self.view();
        let ended = view
            .focus_panel_id()
            .is_some_and(|panel| view.terminal_end(panel).is_some());
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
