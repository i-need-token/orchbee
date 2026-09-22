use orchbee_local_ipc::AsLocalIo;
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    io::{self, Read},
    time::Instant,
};

use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, HostEpoch, LifecycleControl, MAXIMUM_TERMINAL_FRAME_BYTES,
    MessageRole, PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES, TerminalEffectEvent,
    decode_lifecycle_control, decode_terminal_effect_event, decode_transport_envelope,
};

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
        _ => match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
            Ok(TerminalEffectEvent::MetadataChanged(event)) => event.identity,
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

pub(crate) fn consume_event(
    connection: &super::HostBrokerConnection,
    server: &UnixStream,
    deadline: Instant,
) -> io::Result<()> {
    let mut reader = EventReader {
        host: connection.stream(),
        server,
        deadline,
    };
    let mut header = [0; TRANSPORT_ENVELOPE_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    let length = usize::try_from(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(invalid)?;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES
        ..=TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_TERMINAL_FRAME_BYTES)
        .contains(&length)
    {
        return Err(invalid(
            "Host event exceeds the live projection record bound",
        ));
    }
    let mut record = vec![0; length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    reader.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    connection.receive_event(record)
}

struct EventReader<'a> {
    host: &'a UnixStream,
    server: &'a UnixStream,
    deadline: Instant,
}

impl Read for EventReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let timeout =
                PollTimeout::try_from(remaining.as_millis().saturating_add(1)).map_err(invalid)?;
            let mut descriptors = [
                PollFd::new(self.host.as_local_io(), PollFlags::POLLIN),
                // A Server request may already be queued while idle; observe loss
                // without consuming or treating those request bytes as a response.
                PollFd::new(self.server.as_local_io(), PollFlags::empty()),
            ];
            match poll(&mut descriptors, timeout) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
            if descriptors[1]
                .revents()
                .is_none_or(|events| !events.is_empty())
            {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            let events = descriptors[0]
                .revents()
                .ok_or_else(|| invalid("invalid Host descriptor"))?;
            if events.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            if events.contains(PollFlags::POLLIN) {
                match self.host.read(bytes) {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                    result => return result,
                }
            }
            if events.contains(PollFlags::POLLHUP) {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests;
