//! An admitted primary root-process cwd sample; unavailable is never absence.
#[cfg(test)]
pub(crate) mod tests;
use orchbee_workbench_protocol::WorkbenchDirectoryPath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InvalidRootSample;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RootDirectory {
    Available(Option<WorkbenchDirectoryPath>),
    Unavailable,
}

use orchbee_client_server_protocol::TerminalViewerOutcome;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, MessageRole, PayloadEncoding, ProcessInfoControl,
    decode_process_info_control, decode_transport_envelope,
};

pub(crate) fn decode_root(
    response: TerminalViewerOutcome,
    attachment_id: &orchbee_client_server_protocol::ViewerId,
    host_epoch: &orchbee_client_server_protocol::HostEpoch,
    host_terminal_id: &orchbee_client_server_protocol::HostTerminalId,
) -> Result<RootDirectory, InvalidRootSample> {
    let TerminalViewerOutcome::Record(record) = response else {
        return match response {
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. } => {
                Ok(RootDirectory::Unavailable)
            }
            TerminalViewerOutcome::Attached { .. }
            | TerminalViewerOutcome::Empty { .. }
            | TerminalViewerOutcome::FeedbackAccepted { .. }
            | TerminalViewerOutcome::Detached { .. }
            | TerminalViewerOutcome::RowsRejected { .. }
            | TerminalViewerOutcome::Record(..) => Err(InvalidRootSample),
        };
    };
    if record.len() > 64 + orchbee_terminal_protocol::MAXIMUM_CONTROL_JSON_BYTES {
        return Err(InvalidRootSample);
    }
    let envelope = decode_transport_envelope(&record).map_err(|_| InvalidRootSample)?;
    let epoch = orchbee_terminal_protocol::HostEpoch::parse(host_epoch.as_str())
        .map_err(|_| InvalidRootSample)?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
        || envelope.host_epoch != Some(epoch)
        || envelope.correlation_id.is_none()
    {
        return Err(InvalidRootSample);
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
                    | "process-sample-unavailable"
            )
        {
            return Ok(RootDirectory::Unavailable);
        }
        return Err(InvalidRootSample);
    };
    if info.attachment_id.to_text() != attachment_id.as_str()
        || info.terminal_id.to_text() != host_terminal_id.as_str()
    {
        return Err(InvalidRootSample);
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
