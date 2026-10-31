//! Tab configuration admission and exact receipts within the existing canonical setup transaction.
use super::{
    OperationDeadlines, SetupOperation, WorkbenchAction, WorkbenchClientState, WorkbenchCommand,
    WorkbenchRequestId, WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, TabConfiguration, TerminalContext, configure_tabs,
};

impl WorkbenchClientState {
    pub(crate) fn plan_tab_configuration(
        &self,
        operation: TabConfiguration,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<SetupOperation> {
        if self.setup_blocks_structure()
            || !self.setup_authority_available()
            || !self.invariant_holds()
        {
            return None;
        }
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: self.cache.session_id().clone(),
            expected_revision: self.cache.revision(),
            action: WorkbenchAction::TabsConfigure {
                terminal_context_id: self.cache.terminal_context_id()?.clone(),
                operation,
            },
        };
        Some(SetupOperation {
            quick: None,
            generation,
            request: WorkbenchSetupRequest::try_from(command.clone()).ok()?,
            command,
            baseline: self.cache.clone(),
            deadlines,
        })
    }
}

impl SetupOperation {
    pub(super) fn configured_context(
        &self,
        commit: &WorkbenchSetupCommit,
    ) -> Option<TerminalContext> {
        let WorkbenchAction::TabsConfigure {
            terminal_context_id,
            operation,
        } = &self.command.action
        else {
            return None;
        };
        let WorkbenchSetupCommit::TabsConfigured {
            group_id,
            unchanged,
            ..
        } = commit
        else {
            return None;
        };
        if Some(terminal_context_id) != self.baseline.terminal_context_id() {
            return None;
        }
        // Group identities are Session-wide; a receipt cannot reuse one from another context.
        if let Some(group_id) = group_id
            && self
                .baseline
                .canonical_snapshot()
                .terminal_contexts
                .iter()
                .flat_map(|context| &context.groups)
                .any(|group| &group.group_id == group_id)
        {
            return None;
        }
        let baseline = self.baseline.canonical_context()?;
        let candidate = configure_tabs(baseline, operation, group_id.as_ref()).ok()?;
        if candidate.is_none() != *unchanged {
            return None;
        }
        Some(candidate.unwrap_or_else(|| baseline.clone()))
    }

    pub(super) fn validate_configuration_receipt(
        &self,
        commit: &WorkbenchSetupCommit,
    ) -> Option<bool> {
        self.configured_context(commit)?;
        let WorkbenchSetupCommit::TabsConfigured { unchanged, .. } = commit else {
            return None;
        };
        Some(*unchanged)
    }
}
