//! Exact Host effect admission with independent bell and clipboard sequence cursors.
use super::Payload;
use orchbee_terminal_protocol::{
    AttachmentId, CURRENT_PROTOCOL_VERSION, HostEpoch, MessageRole, PayloadEncoding,
    TerminalEffectEvent, decode_terminal_effect_event, decode_transport_envelope,
};

#[derive(Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct Invalid;

pub(in crate::workbench_client) struct Stream {
    attachment: AttachmentId,
    epoch: HostEpoch,
    bell_sequence: u64,
    clipboard_sequence: u64,
}
impl Stream {
    pub(in crate::workbench_client) const fn new(
        attachment: AttachmentId,
        epoch: HostEpoch,
    ) -> Self {
        Self {
            attachment,
            epoch,
            bell_sequence: 0,
            clipboard_sequence: 0,
        }
    }

    pub(in crate::workbench_client) fn receive(
        &mut self,
        record: &[u8],
    ) -> Result<Option<Payload>, Invalid> {
        let envelope = decode_transport_envelope(record).map_err(|_| Invalid)?;
        if !matches!(envelope.kind, 55 | 56) {
            return Ok(None);
        }
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.host_epoch != Some(self.epoch)
            || envelope.optional_message
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            return Err(Invalid);
        }
        let (attachment, sequence, cursor, payload) =
            match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload)
                .map_err(|_| Invalid)?
            {
                TerminalEffectEvent::Bell(event) => (
                    event.attachment_id,
                    event.sequence,
                    &mut self.bell_sequence,
                    Payload::Bell,
                ),
                TerminalEffectEvent::ClipboardWriteRequest(event) => {
                    if !super::valid_program_text(&event.text) {
                        return Err(Invalid);
                    }
                    (
                        event.attachment_id,
                        event.sequence,
                        &mut self.clipboard_sequence,
                        Payload::Clipboard(event.text),
                    )
                }
                _ => return Err(Invalid),
            };
        if attachment != self.attachment || sequence.get() <= *cursor {
            return Err(Invalid);
        }
        *cursor = sequence.get();
        Ok(Some(payload))
    }
}
