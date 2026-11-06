//! Sidebar moves share the canonical setup transaction, without opening a modal.
use super::{
    OperationDeadlines, SetupOperation, WorkbenchAction, WorkbenchClientState, WorkbenchCommand,
    WorkbenchRequestId, WorkbenchSetupCommit, WorkbenchSetupRequest,
};
use crate::machine_spaces::navigation_projection::SidebarNavigationProjection;
use orchbee_workbench_protocol::{
    Navigation, NavigationConfiguration, ProtocolVersion, configure_navigation,
};

impl super::super::WorkbenchClientView<'_> {
    pub(crate) fn sidebar_navigation(&self) -> SidebarNavigationProjection {
        let state = self.state;
        let operation = state
            .setup
            .uncertain
            .is_none()
            .then(|| {
                state
                    .setup
                    .pending
                    .as_ref()
                    .or_else(|| state.setup.known.as_ref().map(|known| &known.operation))
            })
            .flatten();
        let pending = operation.and_then(|operation| {
            if operation.command.expected_revision != state.cache.revision() {
                return None;
            }
            match &operation.command.action {
                WorkbenchAction::NavigationConfigure { operation } => Some(operation.clone()),
                // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
                _ => None,
            }
        });
        SidebarNavigationProjection {
            snapshot: state.cache.shared_canonical_snapshot(),
            pending,
            leaving: state.leaving_workspace_ids(),
        }
    }
}

impl WorkbenchClientState {
    pub(crate) fn plan_navigation_configuration(
        &self,
        operation: NavigationConfiguration,
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
        // Reject unavailable or no-op drops before allocating transport work.
        configure_navigation(&self.cache.canonical_snapshot().navigation, &operation).ok()??;
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id,
            session_id: self.cache.session_id().clone(),
            expected_revision: self.cache.revision(),
            action: WorkbenchAction::NavigationConfigure { operation },
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
    pub(super) fn configured_navigation(
        &self,
        commit: &WorkbenchSetupCommit,
    ) -> Option<Navigation> {
        let WorkbenchAction::NavigationConfigure { operation } = &self.command.action else {
            return None;
        };
        let WorkbenchSetupCommit::NavigationConfigured { unchanged, .. } = commit else {
            return None;
        };
        let baseline = &self.baseline.canonical_snapshot().navigation;
        let candidate = configure_navigation(baseline, operation).ok()?;
        if candidate.is_none() != *unchanged {
            return None;
        }
        Some(candidate.unwrap_or_else(|| baseline.clone()))
    }
    pub(super) fn validate_navigation_receipt(
        &self,
        commit: &WorkbenchSetupCommit,
    ) -> Option<bool> {
        self.configured_navigation(commit)?;
        let WorkbenchSetupCommit::NavigationConfigured { unchanged, .. } = commit else {
            return None;
        };
        Some(*unchanged)
    }
}
