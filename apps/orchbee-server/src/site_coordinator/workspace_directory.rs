use super::SiteCoordinator;
use orchbee_server::WorkbenchGrant;
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkerId,
};

impl SiteCoordinator {
    pub(crate) fn preflight_workspace_directory(
        &self,
        command: &WorkbenchCommand,
    ) -> Option<WorkbenchCommandResult> {
        let Some(authority) = self.workbench.as_ref() else {
            return Some(unavailable(command));
        };
        let grant = WorkbenchGrant::manage_navigation(authority.site_id().clone())
            .with_directory_association();
        if matches!(
            command.action,
            orchbee_workbench_protocol::WorkbenchAction::WorkspaceLaunch { .. }
        ) {
            let grant =
                WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone())
                    .with_directory_association();
            authority.preflight_workspace_launch(Some(&grant), command)
        } else {
            authority.preflight_workspace_directory(Some(&grant), command)
        }
    }

    pub(crate) fn accept_workspace_directory(
        &mut self,
        command: WorkbenchCommand,
        worker: WorkerId,
    ) -> WorkbenchCommandResult {
        let Some(authority) = self.workbench.as_mut() else {
            return unavailable(&command);
        };
        let grant = WorkbenchGrant::manage_navigation(authority.site_id().clone())
            .with_directory_association();
        authority.accept_workspace_directory(Some(&grant), command, worker)
    }
}

fn unavailable(command: &WorkbenchCommand) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome: WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::ServiceUnavailable,
        },
    }
}
