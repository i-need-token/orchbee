//! Read-only process attachment, correlated Host decoding and one fallible-work release boundary.
use super::binding::Binding;
use crate::local_client::{LocalClient, SubmissionError};
use orchbee_client_server_protocol::{
    RequestId, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerRequest, ViewerId,
    ViewerRejectedReason,
};
use orchbee_local_ipc::LocalIo;
use orchbee_terminal_protocol::{
    AttachmentId, ConnectionControl, HostEpoch, MessageRole, PayloadEncoding, ProcessInfoControl,
    TerminalId, TerminalProcessInfo, decode_connection_control, decode_process_info_control,
    decode_transport_envelope,
};
use std::time::Instant;

/// Preserve request/exchange failures separately from invalid Host evidence and admitted Host codes.
#[derive(Debug)]
pub(crate) enum Error {
    Identity,
    Exchange(SubmissionError),
    Timeout,
    Rejected(ViewerRejectedReason),
    Unavailable,
    Protocol,
    Host(HostFailure),
}

/// The closed set of Host failures admitted for a process sample; unknown codes are protocol errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostFailure {
    LimitExceeded,
    MissingResource,
    Unsupported,
    Changed,
    Unavailable,
}
impl HostFailure {
    /// Return the original wire code for a caller's diagnostics without reclassifying the failure.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::LimitExceeded => "limit-exceeded",
            Self::MissingResource => "missing-resource",
            Self::Unsupported => "process-sample-unsupported",
            Self::Changed => "process-sample-changed",
            Self::Unavailable => "process-sample-unavailable",
        }
    }
}

/// Correlated viewer identities returned by the Server, with Client-owned teardown as fallback.
/// This descriptor does not detach on drop; its caller owns explicit viewer release.
pub(crate) struct Attachment {
    pub(crate) viewer: ViewerId,
    pub(crate) id: AttachmentId,
    pub(crate) epoch: HostEpoch,
    pub(crate) terminal: TerminalId,
    pub(crate) host_terminal: orchbee_client_server_protocol::HostTerminalId,
}

/// A validated Host process sample and its separate OBCS and Host correlation identities.
/// The evidence describes one attachment and epoch, not a guarantee of future process identity.
pub(crate) struct Observation {
    pub(crate) info: TerminalProcessInfo,
    pub(crate) request: RequestId,
    pub(crate) host_request: String,
    pub(crate) epoch: HostEpoch,
    pub(crate) attachment: AttachmentId,
}

/// Attach once to the exact physical binding on the borrowed Client, without selecting a target.
/// A successful caller owns the returned viewer until explicit detach or Client teardown.
///
/// # Errors
/// Preserves identity and exchange failures, Server refusals, and unexpected response variants.
pub(crate) fn attach(
    client: &mut LocalClient,
    deadline: Instant,
    binding: &Binding,
    wake: Option<LocalIo<'_>>,
) -> Result<Attachment, Error> {
    let request = TerminalViewerRequest::new(
        super::obcs_identity().map_err(|_| Error::Identity)?,
        TerminalViewerAction::AttachBinding {
            session_id: binding.session.clone(),
            worker_id: binding.worker.clone(),
            terminal_id: binding.terminal.clone(),
        },
    );
    let result = client
        .submit_terminal_viewer_cancellable(request, Some(deadline), wake)
        .map_err(Error::Exchange)?;
    match result.outcome() {
        TerminalViewerOutcome::Attached {
            attachment_id,
            host_epoch,
            host_terminal_id,
        } => Ok(Attachment {
            viewer: attachment_id.clone(),
            host_terminal: host_terminal_id.clone(),
            id: AttachmentId::parse(attachment_id.as_str()).map_err(|_| Error::Identity)?,
            epoch: HostEpoch::parse(host_epoch.as_str()).map_err(|_| Error::Identity)?,
            terminal: TerminalId::parse(host_terminal_id.as_str()).map_err(|_| Error::Identity)?,
        }),
        TerminalViewerOutcome::Rejected { reason } => Err(Error::Rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => Err(Error::Unavailable),
        TerminalViewerOutcome::Empty { .. }
        | TerminalViewerOutcome::FeedbackAccepted { .. }
        | TerminalViewerOutcome::Detached { .. }
        | TerminalViewerOutcome::RowsRejected { .. }
        | TerminalViewerOutcome::Record(..) => Err(Error::Protocol),
    }
}

/// Attach, read one process sample, then attempt detach after all fallible read work has settled.
/// Detach uses the same deadline and wake; failure cannot replace the sample result, and Client
/// teardown remains the final release fallback. No alternate binding or retry is chosen.
///
/// # Errors
/// Returns the attachment or read failure, including correlation and Host-code validation errors.
pub(crate) fn read(
    client: &mut LocalClient,
    deadline: Instant,
    binding: &Binding,
    wake: Option<LocalIo<'_>>,
) -> Result<Observation, Error> {
    let attachment = attach(client, deadline, binding, wake)?;
    // All fallible post-attachment work settles before the one best-effort detach.
    // Dropping the caller-owned Client remains the final teardown fallback.
    let outcome = read_attached(
        client,
        deadline,
        (&attachment.viewer, attachment.id, attachment.epoch),
        wake,
    );
    if let Ok(request_id) = super::obcs_identity() {
        let _ = client.submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                request_id,
                TerminalViewerAction::Detach {
                    attachment_id: attachment.viewer,
                },
            ),
            Some(deadline),
            wake,
        );
    }
    outcome
}

/// Read through a borrowed viewer and validate the Host epoch and attachment in its bounded reply.
/// The caller retains viewer release ownership; this operation never detaches it.
///
/// # Errors
/// Distinguishes identity/exchange failure, post-read timeout, Server refusal, invalid Host
/// envelopes or correlation, and the closed set of admitted Host sample failures.
pub(crate) fn read_attached(
    client: &mut LocalClient,
    deadline: Instant,
    lease: (&ViewerId, AttachmentId, HostEpoch),
    wake: Option<LocalIo<'_>>,
) -> Result<Observation, Error> {
    let (viewer, attachment, epoch) = lease;
    let request_id = super::obcs_identity().map_err(|_| Error::Identity)?;
    let result = client
        .submit_terminal_viewer_cancellable(
            TerminalViewerRequest::new(
                request_id.clone(),
                TerminalViewerAction::ReadProcessInfo {
                    attachment_id: viewer.clone(),
                },
            ),
            Some(deadline),
            wake,
        )
        .map_err(Error::Exchange)?;
    if Instant::now() >= deadline {
        return Err(Error::Timeout);
    }
    let record = match result.outcome() {
        TerminalViewerOutcome::Record(record) => record,
        TerminalViewerOutcome::Rejected { reason } => return Err(Error::Rejected(*reason)),
        TerminalViewerOutcome::Unavailable { .. } => return Err(Error::Unavailable),
        TerminalViewerOutcome::Attached { .. }
        | TerminalViewerOutcome::Empty { .. }
        | TerminalViewerOutcome::FeedbackAccepted { .. }
        | TerminalViewerOutcome::Detached { .. }
        | TerminalViewerOutcome::RowsRejected { .. } => return Err(Error::Protocol),
    };
    let (info, host_request) = decode(record, attachment, epoch)?;
    Ok(Observation {
        info,
        request: request_id,
        host_request,
        epoch,
        attachment,
    })
}

fn decode(
    record: &[u8],
    attachment: AttachmentId,
    epoch: HostEpoch,
) -> Result<(TerminalProcessInfo, String), Error> {
    if record.len() > 64 + orchbee_terminal_protocol::MAXIMUM_CONTROL_JSON_BYTES {
        return Err(Error::Protocol);
    }
    let envelope = decode_transport_envelope(record).map_err(|_| Error::Protocol)?;
    if envelope.protocol != orchbee_terminal_protocol::CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
        || envelope.host_epoch != Some(epoch)
    {
        return Err(Error::Protocol);
    }
    // The Server owns the Worker/Host correlation and physical Terminal binding.
    // That correlation differs from the caller's OBCS request identity.
    let correlation = envelope.correlation_id.ok_or(Error::Protocol)?;
    if let Ok(ProcessInfoControl::ProcessInfo(info)) =
        decode_process_info_control(envelope.kind, envelope.role, envelope.payload)
    {
        if info.attachment_id != attachment {
            return Err(Error::Protocol);
        }
        return Ok((info, correlation.to_text()));
    }
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        let failure = match failure.code.as_str() {
            "limit-exceeded" => HostFailure::LimitExceeded,
            "missing-resource" => HostFailure::MissingResource,
            "process-sample-unsupported" => HostFailure::Unsupported,
            "process-sample-changed" => HostFailure::Changed,
            "process-sample-unavailable" => HostFailure::Unavailable,
            _ => return Err(Error::Protocol),
        };
        return Err(Error::Host(failure));
    }
    Err(Error::Protocol)
}

#[cfg(test)]
mod tests;
