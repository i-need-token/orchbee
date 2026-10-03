//! Selection admission and record intake for the installed pane incarnation.
use super::{NativeProjectionStatus, NativeUi, NativeViewportSurface};
use crate::terminal_input::{PointerSelectionAdmission, PointerSelectionIntent};
use crate::terminal_session::terminal_selection::{
    self as selection, Command, Driver, Event, Failure, Reply,
};
use crate::terminal_session::{
    mailbox::{AdmissionError, CommandMailbox, QueuedCommand, Tagged},
    state::{CarrierFailure, PaneRef, SessionNotice},
};
use orchbee_client_server_protocol::TerminalViewerOutcome;
use std::sync::Arc;

impl NativeUi {
    pub(in crate::terminal_session) fn pointer_selection(
        &mut self,
        key: &PaneRef,
        intent: PointerSelectionIntent,
    ) -> PointerSelectionAdmission {
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return PointerSelectionAdmission::Rejected;
        };
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| &projection.key == key)
        else {
            return PointerSelectionAdmission::Rejected;
        };
        if intent == PointerSelectionIntent::Cancel {
            self.copies.cancel_pane(key);
            if let Some(driver) = self.projections[index].selection.as_mut()
                && let Err(failure) = driver.cancel()
            {
                self.settle_selection_failure(failure);
            }
            self.admit_selections(&mailbox);
            return PointerSelectionAdmission::Admitted;
        }
        if self.retirement_failure.is_some()
            || self.state.as_ref().is_none_or(|state| {
                state.ready().is_none()
                    || state.focus_pane_ref().as_ref() != Some(key)
                    || state.close_suppresses_attachment(key.pane().anchor())
            })
            || self.projections[index].status != NativeProjectionStatus::FrameAvailable
        {
            return PointerSelectionAdmission::Rejected;
        }
        let Some(frame) = self.projections[index].value.frame() else {
            return PointerSelectionAdmission::Rejected;
        };
        if frame.interaction.mouse_tracking() != orchbee_terminal_protocol::MouseTracking::None {
            return PointerSelectionAdmission::Rejected;
        }
        let (generation, columns, rows) = (
            frame.route.generation,
            frame.geometry.columns,
            frame.geometry.rows,
        );
        if self
            .view()
            .pane_viewport(key.pane().panel())
            .is_some_and(|surface| !matches!(surface, NativeViewportSurface::Window { .. }))
        {
            return PointerSelectionAdmission::Rejected;
        }
        let attachment = *self.projections[index].value.attachment();
        let driver = self.projections[index]
            .selection
            .get_or_insert_with(|| Driver::new(attachment));
        match driver.pointer(generation, columns, rows, intent) {
            Ok(true) => {
                if matches!(intent, PointerSelectionIntent::Start { .. }) {
                    self.copies.cancel_pane(key);
                }
                self.admit_selections(&mailbox);
                PointerSelectionAdmission::Admitted
            }
            Ok(false) => PointerSelectionAdmission::Rejected,
            Err(failure) => {
                self.settle_selection_failure(failure);
                PointerSelectionAdmission::Rejected
            }
        }
    }

    pub(super) fn admit_selections(&mut self, mailbox: &Arc<CommandMailbox>) {
        for index in 0..self.projections.len() {
            let key = self.projections[index].key.clone();
            let generation = self.projections[index]
                .value
                .frame()
                .map(|frame| frame.route.generation);
            let Some(driver) = self.projections[index].selection.as_mut() else {
                continue;
            };
            if let Some(generation) = generation
                && let Err(failure) = driver.observe_generation(generation)
            {
                self.settle_selection_failure(failure);
                return;
            }
            let Some(command) = driver.pending() else {
                continue;
            };
            match mailbox.try_enqueue(QueuedCommand::Selection(Tagged::new(key, command))) {
                Ok(_) => {
                    if let Err(failure) = driver.admitted(command) {
                        self.settle_selection_failure(failure);
                        return;
                    }
                }
                Err(
                    AdmissionError::Full | AdmissionError::Busy | AdmissionError::StaleAttachment,
                ) => {}
                Err(AdmissionError::Closed(closed)) => {
                    self.retire(closed.failure());
                    return;
                }
                Err(_) => {
                    self.retire(CarrierFailure::Protocol);
                    return;
                }
            }
        }
        self.collect_completed_copies();
    }

    pub(super) fn apply_selection_outcome(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        key: PaneRef,
        command: Command,
        outcome: TerminalViewerOutcome,
    ) -> bool {
        let Some(projection) = self
            .projections
            .iter_mut()
            .find(|projection| projection.key == key)
        else {
            return false;
        };
        let Some(driver) = projection.selection.as_mut() else {
            return false;
        };
        let reply = match outcome {
            TerminalViewerOutcome::Record(record) => selection::decode_reply(
                &record,
                *projection.value.attachment(),
                *projection.value.epoch(),
            ),
            TerminalViewerOutcome::FeedbackAccepted {} => Ok(Reply::Ack),
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::RowsRejected { .. } => {
                Ok(Reply::Failed {
                    stale_generation: false,
                })
            }
            _ => Err(Failure::Invalid),
        };
        let result = reply.and_then(|reply| driver.reply(command, reply));
        match result {
            Ok(changed) => {
                self.admit_selections(mailbox);
                changed
            }
            Err(failure) => {
                self.settle_selection_failure(failure);
                true
            }
        }
    }

    pub(super) fn apply_selection_event(
        &mut self,
        index: usize,
        record: &[u8],
    ) -> Result<bool, Failure> {
        let projection = &mut self.projections[index];
        let attachment = *projection.value.attachment();
        let Some(event) = selection::decode_event(record, attachment, *projection.value.epoch())?
        else {
            return Ok(false);
        };
        let driver = projection
            .selection
            .get_or_insert_with(|| Driver::new(attachment));
        match event {
            Event::Changed(value) => {
                if driver.changed(value)?
                    && let Some(state) = self.state.as_mut()
                {
                    state.set_notice(SessionNotice::SelectionEvicted);
                }
            }
            Event::Chunk(value) => driver.chunk(value)?,
            Event::Finished(value) => driver.finished(value)?,
        }
        Ok(true)
    }

    pub(super) fn settle_selection_failure(&mut self, failure: Failure) {
        match failure {
            Failure::Invalid => self.retire(CarrierFailure::Protocol),
            Failure::IdentityExhausted => self.retire(CarrierFailure::IdentityExhausted),
            Failure::Rejected => {
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(SessionNotice::CopyFailed);
                }
            }
        }
    }

    fn collect_completed_copies(&mut self) {
        for projection in &mut self.projections {
            let Some(driver) = projection.selection.as_mut() else {
                continue;
            };
            let Some((_, text)) = driver.take_copy() else {
                continue;
            };
            let Some(frame) = projection.value.frame() else {
                continue;
            };
            if !self
                .copies
                .enqueue(projection.key.clone(), frame.route.generation, text)
                && let Some(state) = self.state.as_mut()
            {
                state.set_notice(SessionNotice::CopyFailed);
            }
        }
        self.reconcile_copies();
    }

    fn reconcile_copies(&mut self) {
        let projections = &self.projections;
        let state = self.state.as_ref();
        self.copies.reconcile(|key, generation| {
            state.is_some_and(|state| {
                state.ready().is_some()
                    && state.panel_viewer_live(key.pane().panel())
                    && state
                        .pane_by_panel(key.pane().panel())
                        .is_some_and(|pane| pane.pane_ref() == *key)
                    && !state.close_suppresses_attachment(key.pane().anchor())
            }) && projections.iter().any(|projection| {
                &projection.key == key
                    && projection.status == NativeProjectionStatus::FrameAvailable
                    && projection
                        .value
                        .frame()
                        .is_some_and(|frame| frame.route.generation == generation)
                    && projection.selection.as_ref().is_some_and(|driver| {
                        matches!(
                            driver.selection(),
                            orchbee_terminal_protocol::SelectionChangedState::Active { .. }
                        )
                    })
            })
        });
    }

    pub(super) fn dispatch_copy_after_draw(&mut self) {
        self.reconcile_copies();
        if self.retirement_failure.is_none()
            && let Some(job) = self.copies.dispatch()
        {
            self.clipboard.submit(job);
        }
    }

    pub(super) fn settle_copy_delivery(&mut self) -> bool {
        use crate::terminal_session::terminal_copy::Outcome;
        self.reconcile_copies();
        let Some(outcome) = self.copies.take_outcome() else {
            return false;
        };
        let notice = match outcome {
            Outcome::Delivered | Outcome::Cancelled => None,
            Outcome::Failed => Some(SessionNotice::CopyFailed),
            Outcome::Unknown => Some(SessionNotice::CopyUnknown),
        };
        if let Some(notice) = notice
            && let Some(state) = self.state.as_mut()
        {
            state.set_notice(notice);
        }
        true
    }
}
