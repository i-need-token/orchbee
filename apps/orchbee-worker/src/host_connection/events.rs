//! Validates Host event envelopes against the admitted epoch and reads Host records
//! incrementally: a record the Host has only begun stays partial across reads, so a Host pausing
//! inside one never holds the serving thread. It routes validated events to connection state
//! without issuing operations or interpreting Agent semantics.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    io::{self, Read},
    time::{Duration, Instant},
};

use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, HostEpoch, LifecycleControl, MAXIMUM_TERMINAL_FRAME_BYTES,
    MessageRole, PayloadEncoding, ProtocolErrorCode, TRANSPORT_ENVELOPE_HEADER_BYTES,
    TerminalEffectEvent, decode_lifecycle_control, decode_terminal_effect_event,
    decode_transport_envelope,
};

use super::{PeerFailure, pacing::HostFailure};

pub(crate) fn validate_event(record: &[u8], epoch: HostEpoch) -> io::Result<()> {
    let envelope = decode_transport_envelope(record).map_err(invalid)?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.role != MessageRole::Event
        || envelope.correlation_id.is_some()
        || envelope.host_epoch != Some(epoch)
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(invalid("unexpected Host event envelope"));
    }
    let identity = match decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload) {
        Ok(LifecycleControl::TerminalStarted(identity)) => identity,
        Ok(LifecycleControl::TerminalExited(event)) => event.identity,
        Ok(LifecycleControl::TerminalFailed(event)) => event.identity,
        // This lifecycle exchange requires its exact target-bearing acknowledgment; other control records cannot confirm it.
        _ => match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
            Ok(TerminalEffectEvent::MetadataChanged(event)) => event.identity,
            // This event stream observes only its declared lifecycle/effect records; unrelated records cannot advance it.
            _ => return Err(invalid("unexpected Host event payload")),
        },
    };
    if identity.host_epoch != epoch {
        return Err(invalid("Host event identity has a different epoch"));
    }
    // This lifecycle consumer has no lifecycle/metadata projection. Validation
    // drains notifications without turning them into operation acknowledgements
    // or adopting retained terminals. Viewer frames are routed separately.
    Ok(())
}

/// The Host record being read. Every reader of the Host stream continues it, so a record is
/// never split between readers and no reader waits for the rest of one.
pub(crate) struct PartialRecord {
    header: [u8; TRANSPORT_ENVELOPE_HEADER_BYTES],
    header_filled: usize,
    record: Vec<u8>,
    filled: usize,
    /// When the record's first byte arrived.
    began: Option<Instant>,
}

impl Default for PartialRecord {
    fn default() -> Self {
        Self {
            header: [0; TRANSPORT_ENVELOPE_HEADER_BYTES],
            header_filled: 0,
            record: Vec::new(),
            filled: 0,
            began: None,
        }
    }
}

impl PartialRecord {
    /// Reads what has arrived of the current record without waiting, never past its end. Returns
    /// the whole record once it is complete, or `None` when the Host has sent nothing more for
    /// now. A record that is not an event is bounded by `maximum_response_payload`.
    pub(crate) fn advance(
        &mut self,
        stream: &UnixStream,
        maximum_response_payload: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        loop {
            if self.header_filled < TRANSPORT_ENVELOPE_HEADER_BYTES {
                let Some(count) = read_available(stream, &mut self.header[self.header_filled..])?
                else {
                    return Ok(None);
                };
                self.began.get_or_insert_with(Instant::now);
                self.header_filled += count;
                if self.header_filled < TRANSPORT_ENVELOPE_HEADER_BYTES {
                    continue;
                }
                let length = usize::try_from(u32::from_le_bytes([
                    self.header[8],
                    self.header[9],
                    self.header[10],
                    self.header[11],
                ]))
                .map_err(invalid)?;
                // Events can carry one bounded opaque OBTF record between control responses;
                // anything else is a response bounded by its exchange's declared payload budget.
                let event = u16::from_le_bytes([self.header[6], self.header[7]]) & (1 << 2) != 0;
                let bound = TRANSPORT_ENVELOPE_HEADER_BYTES
                    .checked_add(if event {
                        MAXIMUM_TERMINAL_FRAME_BYTES
                    } else {
                        maximum_response_payload
                    })
                    .ok_or_else(|| invalid("Host record bound overflows"))?;
                if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=bound).contains(&length) {
                    return Err(invalid("Host record exceeds its bound"));
                }
                if lost_framing(&self.header, false) {
                    return Err(invalid("Host record lost its framing"));
                }
                self.record = vec![0; length];
                self.record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&self.header);
                self.filled = TRANSPORT_ENVELOPE_HEADER_BYTES;
            }
            if self.filled == self.record.len() {
                self.header_filled = 0;
                self.filled = 0;
                self.began = None;
                let record = std::mem::take(&mut self.record);
                if lost_framing(&record, true) {
                    return Err(invalid("Host record lost its framing"));
                }
                return Ok(Some(record));
            }
            let Some(count) = read_available(stream, &mut self.record[self.filled..])? else {
                return Ok(None);
            };
            self.filled += count;
        }
    }

    pub(crate) const fn began(&self) -> Option<Instant> {
        self.began
    }
}

/// Whether `bytes`, a `complete` record or only its fixed header, cannot have located its own end:
/// its magic is wrong or its length fields disagree. The length that found the next record is then
/// untrusted and the stream is out of step. Any other envelope fault leaves the stream in step, so
/// the record is routed and contained by its consumer.
fn lost_framing(bytes: &[u8], complete: bool) -> bool {
    decode_transport_envelope(bytes).is_err_and(|error| match error.code() {
        ProtocolErrorCode::InvalidMagic => true,
        // A header read alone always disagrees with a longer record's total length.
        ProtocolErrorCode::LengthMismatch => complete,
        _ => false,
    })
}

/// One read of bytes that have arrived; `None` when none have.
fn read_available(mut stream: &UnixStream, bytes: &mut [u8]) -> io::Result<Option<usize>> {
    loop {
        match stream.read(bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => return Ok(Some(count)),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// The class of a failed Host read: a record outside the protocol, or a Host that went away.
pub(crate) fn read_failure(error: &io::Error) -> PeerFailure {
    if error.kind() == io::ErrorKind::InvalidData {
        PeerFailure::Protocol
    } else {
        PeerFailure::Host
    }
}

/// Reads Host events that have arrived, without waiting, and routes each whole one, at most
/// `maximum_events`; an event the Host has only begun stays partial for the next read. Routing
/// contains every intact event, so only a failed read, a lost framing included, is an error; it is
/// recorded on the connection with its class.
pub(crate) fn drain_events(
    connection: &super::HostBrokerConnection,
    maximum_events: usize,
) -> io::Result<()> {
    let failed = |class| HostFailure {
        class,
        deadline: false,
        waited: Duration::ZERO,
        budget: Duration::ZERO,
    };
    for _ in 0..maximum_events {
        let advanced = connection
            .inbound
            .borrow_mut()
            .advance(connection.stream(), MAXIMUM_TERMINAL_FRAME_BYTES);
        let record = match advanced {
            Ok(Some(record)) => record,
            Ok(None) => return Ok(()),
            Err(error) => {
                connection.pacing.note_failure(failed(read_failure(&error)));
                return Err(error);
            }
        };
        connection.receive_event(record);
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests;
