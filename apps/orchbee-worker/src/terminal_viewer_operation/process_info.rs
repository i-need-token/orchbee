//! Requests process information through an existing read attachment and validates the Host's
//! correlated response and target. It does not query processes directly or grant input and
//! lifecycle authority from the sample.

use super::{
    AttachmentId, AttachmentTarget, CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId,
    Envelope, Error, HostBrokerConnection, HostExchange, MessageRole, PayloadEncoding,
    TerminalViewerOutcome, UnixStream, ViewerRejection, decode_connection_control,
    decode_transport_envelope, encode_transport_envelope, invalid,
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
            include_launch_kind: false,
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
    let mut exchange = HostExchange::new(host, server, std::time::Duration::from_secs(1))
        .ok_or_else(|| invalid("process read budget is invalid"))?;
    exchange
        .prepare()
        .map_err(|_| invalid("process read preflight failed"))?;
    let record = exchange
        .round_trip_bounded(&record, MAXIMUM_CONTROL_JSON_BYTES)
        .map_err(|_| invalid("process read exchange failed"))?;
    let envelope = decode_transport_envelope(&record)?;
    if envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(host.host_epoch())
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.payload_encoding != PayloadEncoding::ControlJson
        || envelope.optional_message
    {
        return Err(invalid("process read response has the wrong envelope").into());
    }
    match decode_process_info_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(ProcessInfoControl::ProcessInfo(info))
            if info.attachment_id == id && info.terminal_id.as_bytes() == &target.as_bytes() => {}
        _ if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) => {}
        // Only the requested Host acknowledgment can confirm this attachment/process operation.
        _ => return Err(invalid("process read response has the wrong target or payload").into()),
    }
    Ok(TerminalViewerOutcome::Record(record))
}
