//! Correlated PNG decode on the sampler's own retained temporary viewer.
use super::{
    ViewerAttachment, WorkerOperationRequest, WorkerOperationResult, WorkerOutcome, WorkerRequest,
};
use orchbee_capability_node_protocol::TerminalGraphicsDecodeRequest;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, MessageRole, PayloadEncoding,
    decode_connection_control, decode_graphics_decode_result, decode_transport_envelope,
};
use orchbee_workbench_protocol::PaneGraphicsFailure as Failure;

pub(in crate::local_terminal_service) fn graphics_png(
    viewer: &ViewerAttachment<'_>,
    template: WorkerRequest,
    png: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, Failure> {
    if !viewer.service.viewer_worker_is_current(&viewer.handle)
        || viewer.service.target_lifecycle(viewer.target)
            != Some(super::super::TargetLifecycle::Unfenced)
    {
        return Err(Failure::SourceChanged);
    }
    let request_id = viewer
        .service
        .next_viewer_operation()
        .ok_or(Failure::Unavailable)?;
    // Required completion is always consumed before the temporary-viewer owner can settle cleanup.
    let result = viewer
        .handle
        .request(WorkerOperationRequest::GraphicsDecode(
            TerminalGraphicsDecodeRequest {
                request_id,
                worker_id: template.worker_id,
                worker_generation: template.worker_generation,
                attachment_id: template.attachment_id,
                width,
                height,
                png: png.to_vec(),
            },
        ))
        .map_err(|_| Failure::SourceUnavailable)?;
    let WorkerOperationResult::Viewer(result) = result else {
        return Err(Failure::SourceUnavailable);
    };
    if result.request_id != request_id || result.attachment_id != template.attachment_id {
        return Err(Failure::SourceUnavailable);
    }
    let WorkerOutcome::Record(record) = result.outcome else {
        return Err(Failure::SourceUnavailable);
    };
    let envelope = decode_transport_envelope(&record).map_err(|_| Failure::SourceUnavailable)?;
    if envelope.correlation_id != CorrelationId::from_bytes(request_id.as_bytes()).ok()
        || envelope.host_epoch != Some(viewer.projection.host_epoch)
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Response
        || envelope.optional_message
    {
        return Err(Failure::SourceUnavailable);
    }
    if envelope.kind == 5 && envelope.payload_encoding == PayloadEncoding::ControlJson {
        if let Ok(ConnectionControl::TypedFailure(failure)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            return Err(if failure.code == "invalid-image" {
                Failure::InvalidImage
            } else {
                Failure::SourceUnavailable
            });
        }
        return Err(Failure::SourceUnavailable);
    }
    if envelope.kind != 81 || envelope.payload_encoding != PayloadEncoding::GraphicsImage {
        return Err(Failure::SourceUnavailable);
    }
    let image =
        decode_graphics_decode_result(envelope.payload).map_err(|_| Failure::SourceUnavailable)?;
    if image.attachment_id != viewer.projection.attachment_id
        || image.terminal_id.as_bytes() != &viewer.target.identity.terminal_id().as_bytes()
        || image.width != width
        || image.height != height
    {
        return Err(Failure::SourceUnavailable);
    }
    Ok(image.rgba.to_vec())
}
