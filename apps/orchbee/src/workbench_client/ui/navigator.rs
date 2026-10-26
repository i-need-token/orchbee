//! Exact navigator admission shares the original mailbox and Client selection operations.
use super::{
    ActionBuildError, AdmissionError, NativeUi, OperationDeadlines, WorkbenchClientState,
    build_switch, finish_action,
};
use crate::client_view::{
    ClientAction,
    navigator::{Admission, Catalog, Row, Target},
};
use std::sync::Arc;

#[cfg(test)]
mod tests;

// Runs under the mailbox admission lock, before a no-op or generation is accepted.
fn admitted_catalog(state: &WorkbenchClientState, row: &Row) -> Result<Catalog, ActionBuildError> {
    let catalog = state.navigator_catalog();
    if catalog.source != row.source
        || !catalog.contains(&row.target)
        || !state.context_selection_available()
    {
        return Err(ActionBuildError::Busy);
    }
    Ok(catalog)
}

impl NativeUi {
    pub(in crate::workbench_client) fn navigator_catalog(&self) -> Option<Catalog> {
        if self.retirement.is_some() {
            return None;
        }
        self.state.as_ref().map(|state| state.navigator_catalog())
    }

    pub(in crate::workbench_client) fn activate_navigator(&mut self, row: &Row) -> Admission {
        if self.retirement.is_some() {
            return Admission::Refused;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            self.retire(super::CarrierFailure::Deadline);
            return Admission::Refused;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return Admission::Refused;
        };
        let result = match &row.target {
            Target::Location(location) => mailbox.try_enqueue_context_selection_for(
                state,
                deadlines,
                &self.clock,
                |state, generation| {
                    let catalog = admitted_catalog(state, row)?;
                    if location == &catalog.source.location {
                        return Err(ActionBuildError::NoOp);
                    }
                    let index = catalog
                        .locations
                        .iter()
                        .position(|id| id == location)
                        .ok_or(ActionBuildError::Busy)?;
                    state
                        .start_context_selection_at(generation, index, deadlines)
                        .then_some(())
                        .ok_or(ActionBuildError::Busy)
                },
                finish_action,
            ),
            Target::Tab(tab) => mailbox
                .try_enqueue_switch_for(
                    state,
                    |state, generation| {
                        let catalog = admitted_catalog(state, row)?;
                        // A navigator current-row selection only returns to the terminal.
                        // Ordinary SelectTab may deliberately retry this same unavailable Tab.
                        if catalog.current_tab.as_ref() == Some(tab) {
                            return Err(ActionBuildError::NoOp);
                        }
                        let index = catalog
                            .tabs
                            .iter()
                            .position(|id| id == tab)
                            .ok_or(ActionBuildError::Busy)?;
                        let position =
                            u8::try_from(index + 1).map_err(|_| ActionBuildError::Busy)?;
                        build_switch(
                            state,
                            ClientAction::SelectTab(position),
                            generation,
                            deadlines,
                        )
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
                )
                .map(Some),
        };
        match result {
            Ok(Some(generation)) => {
                if matches!(row.target, Target::Tab(_)) {
                    self.pending_draws.clear();
                    self.ordinary_poll_serial = None;
                }
                Admission::Admitted(generation)
            }
            Err(AdmissionError::Build(ActionBuildError::NoOp)) => Admission::ExactCurrent,
            Err(error) => {
                self.retire_for_admission(&error);
                Admission::Refused
            }
            Ok(None) => Admission::Refused,
        }
    }
}
