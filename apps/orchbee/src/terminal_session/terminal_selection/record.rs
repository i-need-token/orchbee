//! Exact Host envelope validation at the selection and external-effect boundary.
use super::{Failure, Reply};
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, ConnectionControl, HostEpoch, MessageRole,
    PayloadEncoding, SelectionControl, SelectionExportControl, decode_connection_control,
    decode_selection_control, decode_selection_export_chunk, decode_selection_export_control,
    decode_transport_envelope,
};

pub(in crate::terminal_session) enum Event {
    Changed(orchbee_terminal_protocol::SelectionChanged),
    Chunk(orchbee_terminal_protocol::SelectionExportChunk),
    Finished(orchbee_terminal_protocol::SelectionExportFinished),
}

pub(in crate::terminal_session) fn reply(
    record: &[u8],
    attachment: AttachmentId,
    epoch: HostEpoch,
) -> Result<Reply, Failure> {
    let envelope = decode_transport_envelope(record).map_err(|_| Failure::Invalid)?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.host_epoch != Some(epoch)
        || envelope.optional_message
        || envelope.role != MessageRole::Response
        || envelope.correlation_id.is_none()
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(Failure::Invalid);
    }
    if envelope.kind == 5 {
        let ConnectionControl::TypedFailure(value) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(|_| Failure::Invalid)?
        else {
            return Err(Failure::Invalid);
        };
        return Ok(Reply::Failed {
            stale_generation: value.code == "stale-generation",
        });
    }
    if envelope.kind == 62 {
        let SelectionControl::ChangedResponse(value) =
            decode_selection_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(|_| Failure::Invalid)?
        else {
            return Err(Failure::Invalid);
        };
        return if value.attachment_id == attachment {
            Ok(Reply::Changed(value))
        } else {
            Err(Failure::Invalid)
        };
    }
    match decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
        .map_err(|_| Failure::Invalid)?
    {
        SelectionExportControl::StartedResponse(value) if value.attachment_id == attachment => {
            Ok(Reply::Started(value))
        }
        SelectionExportControl::FinishedResponse(value) if value.attachment_id == attachment => {
            Ok(Reply::Finished(value))
        }
        _ => Err(Failure::Invalid),
    }
}

pub(in crate::terminal_session) fn event(
    record: &[u8],
    attachment: AttachmentId,
    epoch: HostEpoch,
) -> Result<Option<Event>, Failure> {
    let envelope = decode_transport_envelope(record).map_err(|_| Failure::Invalid)?;
    if !matches!(envelope.kind, 62 | 65 | 67) {
        return Ok(None);
    }
    let encoding = if envelope.kind == 65 {
        PayloadEncoding::RawBytes
    } else {
        PayloadEncoding::ControlJson
    };
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.host_epoch != Some(epoch)
        || envelope.optional_message
        || envelope.role != MessageRole::Event
        || envelope.correlation_id.is_some()
        || envelope.payload_encoding != encoding
    {
        return Err(Failure::Invalid);
    }
    match envelope.kind {
        62 => {
            let SelectionControl::ChangedEvent(value) =
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(|_| Failure::Invalid)?
            else {
                return Err(Failure::Invalid);
            };
            if value.attachment_id != attachment {
                return Err(Failure::Invalid);
            }
            Ok(Some(Event::Changed(value)))
        }
        65 => {
            let value =
                decode_selection_export_chunk(envelope.kind, envelope.role, envelope.payload)
                    .map_err(|_| Failure::Invalid)?;
            if value.attachment_id != attachment {
                return Err(Failure::Invalid);
            }
            Ok(Some(Event::Chunk(value)))
        }
        67 => {
            let SelectionExportControl::FinishedEvent(value) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(|_| Failure::Invalid)?
            else {
                return Err(Failure::Invalid);
            };
            if value.attachment_id != attachment {
                return Err(Failure::Invalid);
            }
            Ok(Some(Event::Finished(value)))
        }
        _ => Err(Failure::Invalid),
    }
}
