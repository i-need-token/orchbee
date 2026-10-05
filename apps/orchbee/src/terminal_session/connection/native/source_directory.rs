//! Exact borrowed/temporary primary Viewer sampling; caller policies own all directory fallback.
use super::{
    CarrierFailure, HostEpoch, Instant, NativeExecutor, OperationDeadlines, PaneRef, Result,
    TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, closed, submission,
};
use crate::terminal_session::workbench_projection::{NativeContextTabs, NativeTerminalBinding};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, MessageRole, PayloadEncoding, ProcessInfoControl,
    decode_process_info_control, decode_transport_envelope,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

pub(super) enum RootDirectory {
    Available(Option<WorkbenchDirectoryPath>),
    Unavailable,
}

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
            // A Session may attach to a Terminal only once. Borrow its exact live Viewer;
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
            return decode_root(response, &attachment_id, &host_epoch, &host_terminal_id);
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
                _ => Err(CarrierFailure::Protocol),
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
    }
}

fn decode_root(
    response: TerminalViewerOutcome,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    host_epoch: &orchbee_client_server_protocol::HostEpoch,
    host_terminal_id: &orchbee_client_server_protocol::HostTerminalId,
) -> Result<RootDirectory> {
    let TerminalViewerOutcome::Record(record) = response else {
        return match response {
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                Ok(RootDirectory::Unavailable)
            }
            _ => Err(CarrierFailure::Protocol),
        };
    };
    if record.len() > 64 + orchbee_terminal_protocol::MAXIMUM_CONTROL_JSON_BYTES {
        return Err(CarrierFailure::Protocol);
    }
    let envelope = decode_transport_envelope(&record).map_err(|_| CarrierFailure::Protocol)?;
    let epoch = HostEpoch::parse(host_epoch.as_str()).map_err(|_| CarrierFailure::Protocol)?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
        || envelope.host_epoch != Some(epoch)
        || envelope.correlation_id.is_none()
    {
        return Err(CarrierFailure::Protocol);
    }
    let Ok(ProcessInfoControl::ProcessInfo(info)) =
        decode_process_info_control(envelope.kind, envelope.role, envelope.payload)
    else {
        if let Ok(orchbee_terminal_protocol::ConnectionControl::TypedFailure(failure)) =
            orchbee_terminal_protocol::decode_connection_control(
                envelope.kind,
                envelope.role,
                envelope.payload,
            )
            && matches!(
                failure.code.as_str(),
                "limit-exceeded"
                    | "missing-resource"
                    | "process-sample-unsupported"
                    | "process-sample-changed"
                    | "process-sample-unavailable"
            )
        {
            return Ok(RootDirectory::Unavailable);
        }
        return Err(CarrierFailure::Protocol);
    };
    if info.attachment_id.to_text() != attachment_id.as_str()
        || info.terminal_id.to_text() != host_terminal_id.as_str()
    {
        return Err(CarrierFailure::Protocol);
    }
    let Some(root) = info
        .processes
        .iter()
        .find(|process| process.pid == info.root_pid)
    else {
        return Ok(RootDirectory::Unavailable);
    };
    let cwd = match root.current_directory.as_deref() {
        None => None,
        Some(path) if std::path::Path::new(path).is_absolute() => {
            match WorkbenchDirectoryPath::parse(path) {
                Ok(path) => Some(path),
                Err(_) => return Ok(RootDirectory::Unavailable),
            }
        }
        _ => return Ok(RootDirectory::Unavailable),
    };
    Ok(RootDirectory::Available(cwd))
}
