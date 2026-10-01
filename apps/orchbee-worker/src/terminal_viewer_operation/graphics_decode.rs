use super::{HostBrokerConnection, HostExchange, UnixStream, invalid};
use orchbee_capability_node_protocol::{
    TerminalGraphicsDecodeRequest, TerminalViewerOutcome as Outcome, TerminalViewerResult,
    ViewerRejection,
};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope,
    GRAPHICS_DECODE_RESULT_HEADER_BYTES, GraphicsDecodeRequest, MAXIMUM_GRAPHICS_RGBA_BYTES,
    MessageRole, PayloadEncoding, decode_connection_control, decode_graphics_decode_result,
    decode_transport_envelope, encode_graphics_decode_request, encode_transport_envelope,
};
use std::{error::Error, num::NonZeroU64};

pub(crate) fn execute(
    request: TerminalGraphicsDecodeRequest,
    worker_id: orchbee_local_site_protocol::WorkerId,
    generation: NonZeroU64,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<TerminalViewerResult, Box<dyn Error>> {
    let outcome = if request.worker_id.as_bytes() != worker_id.as_bytes() {
        Outcome::Rejected(ViewerRejection::WrongWorker)
    } else if request.worker_generation.get() != generation.get() {
        Outcome::Rejected(ViewerRejection::StaleWorkerGeneration)
    } else {
        read(&request, host, server)?
    };
    Ok(TerminalViewerResult {
        request_id: request.request_id,
        attachment_id: request.attachment_id,
        outcome,
    })
}
fn read(
    request: &TerminalGraphicsDecodeRequest,
    host: &HostBrokerConnection,
    server: &UnixStream,
) -> Result<Outcome, Box<dyn Error>> {
    request.validate()?;
    let Some(target) = host.viewers.borrow().target(request.attachment_id) else {
        return Ok(Outcome::Rejected(ViewerRejection::AttachmentUnavailable));
    };
    let attachment_id = AttachmentId::from_bytes(request.attachment_id.as_bytes())?;
    let correlation = CorrelationId::from_bytes(request.request_id.as_bytes())?;
    let payload = encode_graphics_decode_request(&GraphicsDecodeRequest {
        attachment_id,
        width: request.width,
        height: request.height,
        png: &request.png,
    })?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind: 80,
        optional_message: false,
        payload: &payload,
        payload_encoding: PayloadEncoding::GraphicsImage,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    let mut exchange = HostExchange::new(host, server, std::time::Duration::from_secs(5))
        .ok_or_else(|| invalid("graphics decode budget is invalid"))?;
    exchange
        .prepare()
        .map_err(|_| invalid("graphics decode preflight failed"))?;
    let record = exchange
        .round_trip_bounded(
            &record,
            MAXIMUM_GRAPHICS_RGBA_BYTES + GRAPHICS_DECODE_RESULT_HEADER_BYTES,
        )
        .map_err(|_| invalid("graphics decode exchange failed"))?;
    let envelope = decode_transport_envelope(&record)?;
    if envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(host.host_epoch())
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
    {
        return Err(invalid("graphics decode response envelope disagrees").into());
    }
    if envelope.kind == 81 && envelope.payload_encoding == PayloadEncoding::GraphicsImage {
        let image = decode_graphics_decode_result(envelope.payload)?;
        if image.attachment_id != attachment_id
            || image.terminal_id.as_bytes() != &target.as_bytes()
            || image.width != request.width
            || image.height != request.height
        {
            return Err(invalid("graphics decode response target or dimensions disagree").into());
        }
    } else if envelope.payload_encoding != PayloadEncoding::ControlJson
        || !matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        )
    {
        return Err(invalid("graphics decode response kind disagrees").into());
    }
    Ok(Outcome::Record(record))
}
