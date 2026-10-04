//! Read-only source-directory preparation under the ordinary bounded refresh lease.
use super::*;
use crate::terminal_session::{
    state::workspace_setup::quick::QuickPreparation, workbench_projection::NativeTerminalBinding,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, MessageRole, PayloadEncoding, ProcessInfoControl,
    decode_process_info_control, decode_transport_envelope,
};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

enum RootDirectory {
    Available(Option<WorkbenchDirectoryPath>),
    Unavailable,
}

impl NativeExecutor {
    pub(super) fn prepare_quick_cwd(
        &mut self,
        preparation: &QuickPreparation,
        outcome: &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorkbenchDirectoryPath>> {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        if preparation
            .baseline
            .project_snapshot(snapshot)
            .ok()
            .as_ref()
            != Some(&preparation.baseline)
        {
            return Ok(None);
        }
        if let Some(binding) = &preparation.primary {
            match self.read_quick_root(preparation, binding)? {
                RootDirectory::Available(Some(path)) => return Ok(Some(path)),
                RootDirectory::Available(None) => {}
                RootDirectory::Unavailable => return Ok(None),
            }
        }
        if let Some(directory) = preparation.baseline.directory() {
            return Ok(Some(directory.path.clone()));
        }
        #[cfg(windows)]
        {
            Ok(orchbee_local_site_runtime::normalize_windows_user_path(
                "~",
                |name| std::env::var_os(name),
                std::env::current_dir,
            )
            .ok()
            .and_then(|path| WorkbenchDirectoryPath::parse(&path).ok()))
        }
        #[cfg(not(windows))]
        {
            Ok(None)
        }
    }

    fn quick_viewer(
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

    fn read_quick_root(
        &mut self,
        preparation: &QuickPreparation,
        binding: &NativeTerminalBinding,
    ) -> Result<RootDirectory> {
        if let Some(pane) = &preparation.primary_pane {
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
            let response = self.quick_viewer(
                TerminalViewerAction::ReadProcessInfo {
                    attachment_id: attachment_id.clone(),
                },
                preparation.deadlines.forward_cutoff(),
            )?;
            return decode_root(response, &attachment_id, &host_epoch, &host_terminal_id);
        }
        let action = TerminalViewerAction::AttachBinding {
            site_id: orchbee_client_server_protocol::SiteId::parse(
                preparation.baseline.site_id().as_str(),
            )
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
        let attached = self.quick_viewer(action, preparation.deadlines.forward_cutoff())?;
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
        let response = self.quick_viewer(
            TerminalViewerAction::ReadProcessInfo {
                attachment_id: attachment_id.clone(),
            },
            preparation.deadlines.forward_cutoff(),
        );
        let detached = self.quick_viewer(
            TerminalViewerAction::Detach {
                attachment_id: attachment_id.clone(),
            },
            preparation.deadlines.total(),
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
