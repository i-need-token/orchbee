use super::{NativeExecutor, Result};
use crate::{
    local_client::workbench::workspace_launch::{
        WorkspaceLaunchRequest, WorkspaceLaunchSubmission,
    },
    terminal_session::{
        NativeMemberTarget, NativeStartupTarget,
        startup_directory::{Progress, validate_workspace},
        state::CarrierFailure,
        workbench_projection::{NativeContextTabs, project_native_standalone_tabs},
    },
};
use orchbee_client_server_protocol::{Geometry, OpenOutcome, TerminalOpenRequest};
use orchbee_workbench_protocol::{
    LaunchOptions, ProtocolVersion, Snapshot, WorkbenchAction, WorkbenchCommand,
    WorkbenchSnapshotOutcome, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkspacePlacement,
};
use std::{sync::Arc, time::Instant};

impl NativeExecutor {
    pub(super) fn directory_startup(
        &mut self,
        snapshot: Snapshot,
        geometry: Geometry,
        deadline: Instant,
    ) -> Result<(Snapshot, Option<(NativeContextTabs, NativeMemberTarget)>)> {
        let Some(NativeStartupTarget::LocalDirectory {
            directory,
            position,
            ..
        }) = &self.initial_context
        else {
            return Ok((snapshot, None));
        };
        let directory = (**directory).clone();
        let has_position = position.is_some();
        // Validate the canonical projection before granting any automatic mutation.
        project_native_standalone_tabs(&snapshot)
            .map_err(|_| CarrierFailure::AuthorityDivergence)?;
        if !directory.admits(&snapshot, has_position) {
            if has_position {
                return Ok((snapshot, None));
            }
            self.ensure_before(deadline)?;
            let request = self
                .mailbox
                .allocate_obcs_request_id()
                .map_err(super::closed)?;
            let opened = self
                .client
                .submit_terminal_open(TerminalOpenRequest::new(request, geometry))
                .map_err(super::submission)?;
            match opened.into_outcome() {
                OpenOutcome::Opened { .. } => {}
                OpenOutcome::OutcomeUnknown {} => return Err(CarrierFailure::OutcomeUnknown),
                OpenOutcome::Unavailable { .. } | OpenOutcome::Failed { .. } => {
                    return Err(CarrierFailure::Transport);
                }
            }
            return self
                .directory_snapshot(deadline)
                .map(|snapshot| (snapshot, None));
        }
        let parameters = directory.parameters()?;
        self.ensure_before(deadline)?;
        let workspace_request = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(super::closed)?;
        let geometry = WorkbenchTerminalGeometry::new(
            geometry.columns(),
            geometry.rows(),
            geometry.cell_pixel_width(),
            geometry.cell_pixel_height(),
        )
        .map_err(|_| CarrierFailure::Protocol)?;
        let command = WorkbenchCommand {
            protocol_version: ProtocolVersion,
            site_id: snapshot.site_id.clone(),
            request_id: workspace_request.clone(),
            expected_revision: snapshot.revision,
            action: WorkbenchAction::WorkspaceLaunch {
                placement: WorkspacePlacement::Ungrouped {},
                label: parameters.workspace_label.clone(),
                geometry,
                launch: None,
                profile: None,
                options: Box::new(LaunchOptions {
                    cwd: Some(parameters.cwd.clone()),
                    ..LaunchOptions::default()
                }),
            },
        };
        let request =
            WorkspaceLaunchRequest::try_from(command).map_err(|_| CarrierFailure::Protocol)?;
        directory.retain(Progress::WorkspaceUnknown(workspace_request))?;
        let commit = match self.client.submit_workspace_launch(request, deadline) {
            WorkspaceLaunchSubmission::Committed(commit) => Arc::new(commit),
            WorkspaceLaunchSubmission::DefinitelyNotSubmitted(_)
            | WorkspaceLaunchSubmission::DefinitePrecommit(_) => {
                directory.retain(Progress::Initial)?;
                return Err(CarrierFailure::Transport);
            }
            WorkspaceLaunchSubmission::OutcomeUnknown { .. } => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
        };
        directory.retain(Progress::Workspace(Arc::clone(&commit)))?;
        match &commit.terminal_outcome {
            Some(WorkbenchTerminalOutcome::Started { .. }) => {}
            Some(WorkbenchTerminalOutcome::OutcomeUnknown {}) => {
                return Err(CarrierFailure::OutcomeUnknown);
            }
            _ => return Err(CarrierFailure::Transport),
        }
        let fresh = self.directory_snapshot(deadline)?;
        let created = validate_workspace(&fresh, &snapshot.site_id, &commit, &parameters)?;
        Ok((fresh, Some(created)))
    }

    fn directory_snapshot(&mut self, deadline: Instant) -> Result<Snapshot> {
        self.ensure_before(deadline)?;
        let request = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(super::closed)?;
        match self.read_workbench_before(request, deadline)? {
            WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
            _ => Err(CarrierFailure::Transport),
        }
    }
}
