//! Exact borrowed/temporary primary Viewer sampling; caller policies own all directory fallback.
use super::{
    CarrierFailure, Instant, NativeExecutor, OperationDeadlines, PaneRef, Result,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, closed, submission,
};
use crate::workbench_client::workbench_projection::{NativeContextTabs, NativeTerminalBinding};

pub(super) use crate::workbench_client::primary_directory::RootDirectory;
use crate::workbench_client::primary_directory::decode_root;

impl NativeExecutor {
    fn source_viewer(
        &mut self,
        action: TerminalViewerAction,
        deadline: Instant,
    ) -> Result<TerminalViewerOutcome> {
        self.ensure_before(deadline)?;
        let request = self.mailbox.allocate_obcs_request_id().map_err(closed)?;
        self.client
            .submit_terminal_viewer_before(TerminalViewerRequest::new(request, action), deadline)
            .map(orchbee_client_server_protocol::TerminalViewerResult::into_outcome)
            .map_err(submission)
    }

    pub(super) fn read_source_root(
        &mut self,
        baseline: &NativeContextTabs,
        primary_pane: Option<&PaneRef>,
        deadlines: OperationDeadlines,
        binding: &NativeTerminalBinding,
    ) -> Result<RootDirectory> {
        if let Some(pane) = primary_pane {
            // A Client Connection may attach to a Terminal only once. Borrow its exact live Viewer;
            // the ordinary attachment owner retains it, including on a refused process sample.
            let Some(attached) = self.attachments.iter().find(|attached| {
                &attached.key == pane && attached.announced && attached.setup.is_none()
            }) else {
                return Ok(RootDirectory::Unavailable);
            };
            let (attachment_id, host_epoch, host_terminal_id) = (
                attached.attachment_id.clone(),
                attached.host_epoch.clone(),
                attached.host_terminal_id.clone(),
            );
            let response = self.source_viewer(
                TerminalViewerAction::ReadProcessInfo {
                    attachment_id: attachment_id.clone(),
                },
                deadlines.forward_cutoff(),
            )?;
            return decode_root(response, &attachment_id, &host_epoch, &host_terminal_id)
                .map_err(|_| CarrierFailure::Protocol);
        }
        let action = TerminalViewerAction::AttachBinding {
            site_id: orchbee_client_server_protocol::SiteId::parse(baseline.site_id().as_str())
                .map_err(|_| CarrierFailure::Protocol)?,
            worker_id: orchbee_client_server_protocol::WorkerId::parse(
                binding.worker_id().as_str(),
            )
            .map_err(|_| CarrierFailure::Protocol)?,
            terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                binding.terminal_id().as_str(),
            )
            .map_err(|_| CarrierFailure::Protocol)?,
        };
        let attached = self.source_viewer(action, deadlines.forward_cutoff())?;
        let TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
        } = attached
        else {
            return match attached {
                TerminalViewerOutcome::Rejected { .. }
                | TerminalViewerOutcome::Unavailable { .. } => Ok(RootDirectory::Unavailable),
                TerminalViewerOutcome::Attached { .. }
                | TerminalViewerOutcome::Empty { .. }
                | TerminalViewerOutcome::FeedbackAccepted { .. }
                | TerminalViewerOutcome::Detached { .. }
                | TerminalViewerOutcome::RowsRejected { .. }
                | TerminalViewerOutcome::Record(..) => Err(CarrierFailure::Protocol),
            };
        };
        // Every fallible read/decode returns through detach; a lost detach retires this Client.
        let response = self.source_viewer(
            TerminalViewerAction::ReadProcessInfo {
                attachment_id: attachment_id.clone(),
            },
            deadlines.forward_cutoff(),
        );
        let detached = self.source_viewer(
            TerminalViewerAction::Detach {
                attachment_id: attachment_id.clone(),
            },
            deadlines.total(),
        );
        if !matches!(detached?, TerminalViewerOutcome::Detached {}) {
            return Err(CarrierFailure::Protocol);
        }
        let response = response?;
        decode_root(response, &attachment_id, &host_epoch, &host_terminal_id)
            .map_err(|_| CarrierFailure::Protocol)
    }
}
