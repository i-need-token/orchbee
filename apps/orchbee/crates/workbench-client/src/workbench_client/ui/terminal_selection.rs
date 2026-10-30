//! Selection admission and record intake for the installed pane incarnation.
use super::{ClientProjectionStatus, ViewportSurface, WorkbenchClientUi};
use crate::workbench_client::input::{PaneAdmission, PointerSelectionIntent};
use crate::workbench_client::terminal_selection::{
    self as selection, Command, Driver, Event, Failure, Reply,
};
use crate::workbench_client::{
    mailbox::{AdmissionError, CommandMailbox, QueuedCommand, Tagged},
    state::{CarrierFailure, ClientNotice, PaneRef},
};
use orchbee_client_server_protocol::TerminalViewerOutcome;
use std::sync::Arc;

impl WorkbenchClientUi {
    /// Per-projection ordering gate shared by copy entry/exit and every
    /// ordinary viewport admission path.
    pub(super) fn viewport_admission_blocked(&self, key: &PaneRef) -> bool {
        self.projections
            .iter()
            .find(|projection| &projection.key == key)
            .and_then(|projection| projection.selection.as_ref())
            .is_some_and(Driver::viewport_blocked)
    }

    pub(super) fn semantic_copy_selection(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        key: &PaneRef,
        source: Option<&crate::terminal_content::ContentSource>,
        effect: crate::terminal_copy_mode::SelectionEffect,
    ) -> bool {
        use crate::{
            terminal_copy_mode::SelectionEffect, workbench_client::input::PointerSelectionCell,
            workbench_client::terminal_selection::SemanticIntent,
        };

        let Some(index) = self
            .projections
            .iter()
            .position(|projection| &projection.key == key)
        else {
            return false;
        };
        if self.workspace_creation_replaces_content() && !matches!(effect, SelectionEffect::Cancel)
            || self.retirement_failure.is_some()
            || self.state.as_ref().is_none_or(|state| {
                state.ready().is_none()
                    || state.focus_pane_ref().as_ref() != Some(key)
                    || state.close_suppresses_attachment(key.pane().anchor())
            })
            || self.projections[index].status != ClientProjectionStatus::FrameAvailable
        {
            return false;
        }
        let cleanup = if matches!(effect, SelectionEffect::Cancel) {
            let Some(source) = source.filter(|source| &source.pane == key) else {
                return false;
            };
            let Some(cleanup) = self.copy_selection_cleanup(key, source) else {
                return false;
            };
            Some(cleanup)
        } else {
            None
        };
        let attachment = *self.projections[index].value.attachment();
        let driver = self.projections[index]
            .selection
            .get_or_insert_with(|| Driver::new(attachment));
        let result = match effect {
            SelectionEffect::ExportActive => driver.export_active(),
            SelectionEffect::Cancel => {
                let (generation, columns, rows, cell) = cleanup.expect("validated cleanup");
                driver
                    .cancel_copy_selection(generation, columns, rows, cell)
                    .map(|_| true)
            }
            SelectionEffect::Start(point)
            | SelectionEffect::Update(point)
            | SelectionEffect::End(point) => {
                let Some(source) = source.filter(|source| &source.pane == key) else {
                    return false;
                };
                let Some(row) = u64::from(point.row)
                    .checked_sub(source.rows.start_row)
                    .and_then(|row| u16::try_from(row).ok())
                    .filter(|row| *row < source.rows.row_count)
                else {
                    return false;
                };
                let cell = PointerSelectionCell {
                    column: point.col,
                    row,
                };
                let intent = match effect {
                    SelectionEffect::Start(_) => SemanticIntent::Start(cell),
                    SelectionEffect::Update(_) => SemanticIntent::Update(cell),
                    SelectionEffect::End(_) => SemanticIntent::End(cell),
                    SelectionEffect::Cancel | SelectionEffect::ExportActive => unreachable!(),
                };
                driver.semantic(
                    source.rows.generation,
                    source.rows.columns,
                    source.rows.row_count,
                    intent,
                )
            }
        };
        match result {
            Ok(true) => {
                if matches!(effect, SelectionEffect::Start(_)) {
                    self.effects.cancel_selection(key);
                }
                self.admit_selections(mailbox);
                true
            }
            Ok(false) => false,
            Err(failure) => {
                self.settle_selection_failure(failure);
                false
            }
        }
    }

    pub(in crate::workbench_client) fn pointer_selection(
        &mut self,
        key: &PaneRef,
        intent: PointerSelectionIntent,
    ) -> PaneAdmission {
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return PaneAdmission::Rejected;
        };
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| &projection.key == key)
        else {
            return PaneAdmission::Rejected;
        };
        if intent == PointerSelectionIntent::Cancel {
            self.effects.cancel_selection(key);
            if let Some(driver) = self.projections[index].selection.as_mut()
                && let Err(failure) = driver.cancel()
            {
                self.settle_selection_failure(failure);
            }
            self.admit_selections(&mailbox);
            return PaneAdmission::Admitted;
        }
        if self.workspace_creation_replaces_content()
            || self.retirement_failure.is_some()
            || self.state.as_ref().is_none_or(|state| {
                state.ready().is_none()
                    || state.focus_pane_ref().as_ref() != Some(key)
                    || state.close_suppresses_attachment(key.pane().anchor())
            })
            || self.projections[index].status != ClientProjectionStatus::FrameAvailable
        {
            return PaneAdmission::Rejected;
        }
        let Some(frame) = self.projections[index].value.frame() else {
            return PaneAdmission::Rejected;
        };
        if frame.interaction.mouse_tracking() != orchbee_terminal_protocol::MouseTracking::None {
            return PaneAdmission::Rejected;
        }
        let (generation, columns, rows) = (
            frame.route.generation,
            frame.geometry.columns,
            frame.geometry.rows,
        );
        if self.projections[index].tail.is_some() {
            if self.view().pane_tail_window(key.pane().panel()).is_none() {
                return PaneAdmission::Rejected;
            }
            // A user selection pins the displayed window; automatic following must not move it.
            self.projections[index].tail = None;
        }
        if self
            .view()
            .pane_viewport(key.pane().panel())
            .is_some_and(|surface| !matches!(surface, ViewportSurface::Window { .. }))
        {
            return PaneAdmission::Rejected;
        }
        let intent = if let Some(crop) = self.view().pane_live_view(key.pane().panel()) {
            let map = |mut cell: crate::workbench_client::input::PointerSelectionCell| {
                cell.column = cell.column.saturating_add(crop.column);
                cell.row = cell.row.saturating_add(crop.row);
                cell
            };
            match intent {
                PointerSelectionIntent::Start { click_count, cell } => {
                    PointerSelectionIntent::Start {
                        click_count,
                        cell: map(cell),
                    }
                }
                PointerSelectionIntent::Update { cell } => {
                    PointerSelectionIntent::Update { cell: map(cell) }
                }
                PointerSelectionIntent::End { cell } => {
                    PointerSelectionIntent::End { cell: map(cell) }
                }
                PointerSelectionIntent::Cancel => PointerSelectionIntent::Cancel,
            }
        } else {
            intent
        };
        let attachment = *self.projections[index].value.attachment();
        let driver = self.projections[index]
            .selection
            .get_or_insert_with(|| Driver::new(attachment));
        driver.set_copy_on_select(self.preferences.copy_on_select);
        match driver.pointer(generation, columns, rows, intent) {
            Ok(true) => {
                if matches!(intent, PointerSelectionIntent::Start { .. }) {
                    self.effects.cancel_selection(key);
                }
                self.admit_selections(&mailbox);
                PaneAdmission::Admitted
            }
            Ok(false) => PaneAdmission::Rejected,
            Err(failure) => {
                self.settle_selection_failure(failure);
                PaneAdmission::Rejected
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
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty { .. }
            | TerminalViewerOutcome::Detached { .. }
            | TerminalViewerOutcome::Unavailable { .. } => Err(Failure::Invalid),
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
        let evicted = {
            let projection = &mut self.projections[index];
            let attachment = *projection.value.attachment();
            let Some(event) =
                selection::decode_event(record, attachment, *projection.value.epoch())?
            else {
                return Ok(false);
            };
            let driver = projection
                .selection
                .get_or_insert_with(|| Driver::new(attachment));
            match event {
                Event::Changed(value) => driver.changed(value)?,
                Event::Chunk(value) => {
                    driver.chunk(value)?;
                    false
                }
                Event::Finished(value) => {
                    driver.finished(value)?;
                    false
                }
            }
        };
        if evicted {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::SelectionEvicted);
            }
            {
                let key = self.projections[index].key.clone();
                self.copy_selection_evicted(&key);
            }
        }
        Ok(true)
    }

    pub(super) fn settle_selection_failure(&mut self, failure: Failure) {
        match failure {
            Failure::Invalid => self.retire(CarrierFailure::Protocol),
            Failure::IdentityExhausted => self.retire(CarrierFailure::IdentityExhausted),
            Failure::Rejected => {
                self.copy_selection_failed();
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::CopyFailed);
                }
            }
        }
    }

    fn collect_completed_copies(&mut self) {
        let mut completed = Vec::new();
        for projection in &mut self.projections {
            let Some(driver) = projection.selection.as_mut() else {
                continue;
            };
            let Some(copy) = driver.take_copy() else {
                continue;
            };
            let Some(frame) = projection.value.frame() else {
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::CopyFailed);
                }
                continue;
            };
            completed.push((projection.key.clone(), frame.route.generation, copy));
        }
        for (key, generation, copy) in completed {
            let accepted = self.copy_export_ready(&key, generation, copy.0, copy.1);

            if !accepted && let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::CopyFailed);
            }
        }
        self.reconcile_effects();
    }
}
