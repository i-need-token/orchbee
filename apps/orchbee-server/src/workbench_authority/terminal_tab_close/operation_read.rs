//! Identity lookup reads the retained ledger, including closed-resource tombstones.
use super::{CloseScope, RetainedRequest, WorkbenchAuthority};
use crate::workbench_authority::WorkbenchGrant;
use orchbee_workbench_protocol::{
    CloseOperationOutcome, CloseOperationRead, CloseOperationResult, CloseOperationTarget,
    ProtocolVersion,
};

impl WorkbenchAuthority {
    #[must_use]
    pub fn read_close_operation(
        &self,
        grant: Option<&WorkbenchGrant>,
        request: CloseOperationRead,
    ) -> CloseOperationResult {
        let outcome = if !grant.is_some_and(|grant| {
            grant.site_id == *self.site_id() && request.site_id == *self.site_id()
        }) {
            CloseOperationOutcome::Denied {}
        } else if let Some(RetainedRequest::TabClose(record)) =
            self.ledger.get(&request.operation_id)
        {
            let target = match &record.scope {
                CloseScope::Context { workspace_id } => Some(CloseOperationTarget::Workspace {
                    workspace_id: workspace_id.clone(),
                }),
                CloseScope::Tab => record.tab_id().map(|tab_id| CloseOperationTarget::Tab {
                    terminal_context_id: record.terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                }),
                CloseScope::Panel => record.tab_id().map(|tab_id| CloseOperationTarget::Panel {
                    terminal_context_id: record.terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                }),
            };
            target.map_or(CloseOperationOutcome::Unavailable {}, |target| {
                CloseOperationOutcome::Available {
                    target,
                    generation: record.generation,
                    status: record.status(),
                }
            })
        } else {
            CloseOperationOutcome::NotFound {}
        };
        CloseOperationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            operation_id: request.operation_id,
            outcome,
        }
    }
}
