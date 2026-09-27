//! Bounded read-only navigation memory, owned by one interactive Client.
use super::{client_view::ClientViewObservation, json_fields::deserialize_nullable};
use crate::{ContractError, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAXIMUM_CLIENT_SELECTION_CONTEXTS: usize = 128;
pub const MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES: usize = 16 * 1024;

pub(super) fn validate_contexts(ids: &[WorkbenchTerminalContextId]) -> Result<(), ContractError> {
    if ids.is_empty()
        || ids.len() > MAXIMUM_CLIENT_SELECTION_CONTEXTS
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientContextSelection {
    pub terminal_context_id: WorkbenchTerminalContextId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub active_tab_id: Option<WorkbenchTabId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientNavigationSnapshot {
    pub workbench_revision: WorkbenchRevision,
    pub contexts: Vec<ClientContextSelection>,
}
impl ClientNavigationSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_contexts(
            &self
                .contexts
                .iter()
                .map(|entry| entry.terminal_context_id.clone())
                .collect::<Vec<_>>(),
        )?;
        let tabs = self
            .contexts
            .iter()
            .filter_map(|entry| entry.active_tab_id.as_ref())
            .collect::<Vec<_>>();
        if tabs.iter().collect::<BTreeSet<_>>().len() != tabs.len()
            || serde_json::to_vec(self)
                .map_err(|_| ContractError::InvalidDocument)?
                .len()
                > MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
    #[must_use]
    pub fn answers(
        &self,
        ids: &[WorkbenchTerminalContextId],
        view: &ClientViewObservation,
        minimum: WorkbenchRevision,
    ) -> bool {
        self.validate().is_ok()
            && view.phase == super::client_view::ClientViewPhase::Ready
            && self.workbench_revision >= minimum
            && self.workbench_revision >= view.workbench_revision
            && self.contexts.len() == ids.len()
            && self.contexts.iter().zip(ids).all(|(entry, id)| {
                &entry.terminal_context_id == id
                    && view.selection.as_ref().is_none_or(|selected| {
                        selected.terminal_context_id != *id
                            || selected.tab_id == entry.active_tab_id
                    })
            })
    }
}
