//! Route the setup launch through the existing one-shot WorkspaceLaunch validator.
use super::{
    Instant, LocalClient, WorkbenchCommand, WorkbenchSetupCommit, WorkbenchSetupNotSubmitted,
    WorkbenchSetupOutcomeUnknownReason, WorkbenchSetupPrecommit, WorkbenchSetupSubmission,
};
use crate::local_client::workbench::{
    WorkbenchTabCreateNotSubmitted, WorkbenchTabCreatePrecommit,
    workspace_launch::{WorkspaceLaunchRequest, WorkspaceLaunchSubmission},
};

impl LocalClient {
    pub(super) fn submit_setup_launch(
        &mut self,
        command: WorkbenchCommand,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchSetupSubmission {
        let request = WorkspaceLaunchRequest::try_from(command)
            .expect("setup launch action checked by caller");
        match self.submit_workspace_launch_with_wake(request, deadline, wake) {
            WorkspaceLaunchSubmission::Committed(commit) => WorkbenchSetupSubmission::Committed(
                WorkbenchSetupCommit::WorkspaceLaunched(Box::new(commit)),
            ),
            WorkspaceLaunchSubmission::DefinitelyNotSubmitted(reason) => {
                WorkbenchSetupSubmission::DefinitelyNotSubmitted(match reason {
                    WorkbenchTabCreateNotSubmitted::SessionUnavailable => {
                        WorkbenchSetupNotSubmitted::SessionUnavailable
                    }
                    WorkbenchTabCreateNotSubmitted::RequestEncoding(error) => {
                        WorkbenchSetupNotSubmitted::RequestEncoding(error)
                    }
                    WorkbenchTabCreateNotSubmitted::RequestNotSubmitted(error) => {
                        WorkbenchSetupNotSubmitted::RequestNotSubmitted(error)
                    }
                })
            }
            WorkspaceLaunchSubmission::DefinitePrecommit(reason) => {
                WorkbenchSetupSubmission::DefinitePrecommit(match reason {
                    WorkbenchTabCreatePrecommit::Denied => WorkbenchSetupPrecommit::Denied,
                    WorkbenchTabCreatePrecommit::Rejected { revision, reason } => {
                        WorkbenchSetupPrecommit::Rejected { revision, reason }
                    }
                    WorkbenchTabCreatePrecommit::Unavailable { reason } => {
                        WorkbenchSetupPrecommit::Unavailable { reason }
                    }
                    WorkbenchTabCreatePrecommit::ProfileUnavailable => {
                        WorkbenchSetupPrecommit::ProfileUnavailable
                    }
                    WorkbenchTabCreatePrecommit::ProfileAmbiguous { candidate_ids } => {
                        WorkbenchSetupPrecommit::ProfileAmbiguous { candidate_ids }
                    }
                })
            }
            WorkspaceLaunchSubmission::OutcomeUnknown { request_id, reason } => {
                WorkbenchSetupSubmission::OutcomeUnknown {
                    request_id,
                    reason: WorkbenchSetupOutcomeUnknownReason::Launch(Box::new(reason)),
                }
            }
        }
    }
}
