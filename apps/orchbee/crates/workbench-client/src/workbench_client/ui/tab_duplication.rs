//! The selected-set adapter only admits the next ordinary Create; it owns no worker effects.
use super::{ActionBuildError, ClientActionDisposition, WorkbenchClientUi, build_create};
use crate::workbench_client::mailbox::{AdmissionError, CommandMailbox};
use crate::workbench_state::{DuplicateProgress, DuplicateStop};
use orchbee_workbench_protocol::{TabCreationPlacement, WorkbenchTabId};
use std::sync::Arc;
#[cfg(test)]
mod tests;

impl WorkbenchClientUi {
    pub(super) fn duplicate_tabs(&mut self, target: &WorkbenchTabId) -> ClientActionDisposition {
        let Some(state) = &mut self.state else {
            return ClientActionDisposition::KeepNavigator;
        };
        let sources = super::tab_groups::configuration_tab_ids(state, target);
        if sources.len() == 1 {
            return self.create_tab_placed(Some(TabCreationPlacement::Duplicate {
                tab_id: target.clone(),
            }));
        }
        if !state.begin_duplicate_sequence(sources, self.clock.now()) {
            return ClientActionDisposition::KeepNavigator;
        }
        if let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) {
            self.drive_duplicate_sequence(&mailbox);
        }
        ClientActionDisposition::DismissNavigator
    }

    pub(super) fn drive_duplicate_sequence(&mut self, mailbox: &CommandMailbox) -> bool {
        let Some(state) = &mut self.state else {
            return false;
        };
        let progress = state.duplicate_progress();
        if state.next_duplicate(self.clock.now()).is_none() {
            return progress != state.duplicate_progress();
        }
        let result = mailbox.try_enqueue_settled_duplicate_for(
            state,
            |state, generation, request_id| {
                let (source, deadlines) = state
                    .next_duplicate(self.clock.now())
                    .ok_or(ActionBuildError::Busy)?;
                let marker = build_create(
                    state,
                    generation,
                    request_id,
                    deadlines,
                    Some(TabCreationPlacement::Duplicate { tab_id: source }),
                )?;
                state.duplicate_submitted(marker.operation());
                Ok(marker)
            },
            |state, result| match result {
                Ok(_) => state.clear_notice(),
                Err(
                    AdmissionError::Busy | AdmissionError::Full | AdmissionError::BootstrapPending,
                ) => {}
                Err(AdmissionError::Build(_)) => {
                    state.stop_duplicate_sequence(DuplicateStop::NotCreated)
                }
                Err(_) => state.stop_duplicate_sequence(DuplicateStop::Recovery),
            },
        );
        let changed = progress != state.duplicate_progress();
        if let Err(error) = &result {
            self.retire_for_admission(error);
        }
        changed || result.is_ok()
    }
}

impl super::WorkbenchClientView<'_> {
    pub(crate) fn duplicate_progress(&self) -> Option<DuplicateProgress> {
        self.state.as_ref()?.duplicate_progress()
    }
}
