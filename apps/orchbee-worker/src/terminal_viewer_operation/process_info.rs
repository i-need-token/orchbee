//! Requests process information through an existing read attachment and validates the Host's
//! correlated response and target. It does not query processes directly or grant input and
//! lifecycle authority from the sample.

use super::{
    AttachmentId, AttachmentTarget, CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId,
    Envelope, Error, HostBrokerConnection, HostExchange, MessageRole, PayloadEncoding,
    TerminalViewerOutcome, UnexpectedAnswer, UnixStream, ViewerRejection,
    decode_connection_control, encode_transport_envelope, response_envelope,
};
use orchbee_terminal_protocol::{
    MAXIMUM_CONTROL_JSON_BYTES, ProcessInfoControl, ReadProcessInfo, decode_process_info_control,
    encode_process_info_control,
};

pub(super) fn read(
    host: &HostBrokerConnection,
    server: &UnixStream,
    attachment: orchbee_capability_node_protocol::OperationId,
    correlation: CorrelationId,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    let Some(target) = host.viewers.borrow().target(attachment) else {
        return Ok(TerminalViewerOutcome::Rejected(
            ViewerRejection::AttachmentUnavailable,
        ));
    };
    let id = AttachmentId::from_bytes(attachment.as_bytes())?;
    let command =
        encode_process_info_control(&ProcessInfoControl::ReadProcessInfo(ReadProcessInfo {
            target: AttachmentTarget { attachment_id: id },
            // A Host that confirms prompts also records how it launched the root, and the
            // Client's Agent start reads that record instead of the root's command line. A Host
            // without the gate never sees the field.
            include_launch_kind: host.prompt_gated_input(),
        }))?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind: command.kind,
        optional_message: false,
        payload: &command.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    host.pacing
        .set_site(crate::diagnostic_events::Site::ProcessRead);
    let mut exchange = HostExchange::open(host, server)?;
    exchange.prepare().map_err(|class| exchange.error(class))?;
    let record = exchange
        .round_trip_bounded(&record, MAXIMUM_CONTROL_JSON_BYTES)
        .map_err(|class| exchange.error(class))?;
    let envelope = response_envelope(&record, correlation, host.host_epoch())?;
    if envelope.payload_encoding != PayloadEncoding::ControlJson {
        return Err(UnexpectedAnswer::of(&record).into());
    }
    match decode_process_info_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(ProcessInfoControl::ProcessInfo(info))
            if info.attachment_id == id && info.terminal_id.as_bytes() == &target.as_bytes() => {}
        _ if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) => {}
        // Only the requested Host acknowledgment can confirm this attachment/process operation.
        _ => return Err(UnexpectedAnswer::of(&record).into()),
    }
    Ok(TerminalViewerOutcome::Record(record))
}
