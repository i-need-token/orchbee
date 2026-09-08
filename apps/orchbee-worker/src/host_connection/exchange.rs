use std::{
    error::Error,
    io::{self, Read, Write},
    net::Shutdown,
    os::{fd::AsFd, unix::net::UnixStream},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MAXIMUM_TERMINAL_FRAME_BYTES, MessageRole,
    PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES, decode_transport_envelope,
    encode_transport_envelope,
};

use crate::host_connection::{HostBrokerConnection, consume_event};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerFailure {
    Host,
    Server,
}

pub(crate) struct HostExchange<'a> {
    host: &'a HostBrokerConnection,
    server: &'a UnixStream,
    deadline: Instant,
}

impl<'a> HostExchange<'a> {
    pub(crate) fn new(
        host: &'a HostBrokerConnection,
        server: &'a UnixStream,
        budget: Duration,
    ) -> Option<Self> {
        if budget.is_zero() {
            return None;
        }
        Some(Self {
            host,
            server,
            deadline: Instant::now().checked_add(budget)?,
        })
    }

    pub(crate) fn prepare(&self) -> Result<(), PeerFailure> {
        self.host
            .stream()
            .set_nonblocking(true)
            .map_err(|_error| PeerFailure::Host)?;
        loop {
            let (server, host) = self.poll(PollFlags::POLLIN, PollTimeout::ZERO)?;
            if !server.is_empty() {
                return Err(PeerFailure::Server);
            }
            if host.contains(PollFlags::POLLIN) {
                if consume_event(self.host, self.server, self.deadline).is_err() {
                    let (server, _) = self.poll(PollFlags::POLLIN, PollTimeout::ZERO)?;
                    return Err(if server.is_empty() {
                        PeerFailure::Host
                    } else {
                        PeerFailure::Server
                    });
                }
                continue;
            }
            if !host.is_empty() {
                return Err(PeerFailure::Host);
            }
            return Ok(());
        }
    }

    pub(crate) fn round_trip(&mut self, record: &[u8]) -> Result<Vec<u8>, PeerFailure> {
        let mut remaining = record;
        while !remaining.is_empty() {
            self.wait(PollFlags::POLLOUT)?;
            match self.host.stream().write(remaining) {
                Ok(0) => return Err(PeerFailure::Host),
                Ok(count) => remaining = &remaining[count..],
                Err(error) if transient(&error) => {}
                Err(_error) => return Err(PeerFailure::Host),
            }
        }
        loop {
            let record = self.read_record()?;
            let envelope = decode_transport_envelope(&record).map_err(|_| PeerFailure::Host)?;
            if envelope.role != MessageRole::Event {
                return Ok(record);
            }
            self.host
                .receive_event(record)
                .map_err(|_| PeerFailure::Host)?;
        }
    }

    fn read_record(&mut self) -> Result<Vec<u8>, PeerFailure> {
        let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
        self.read_exact(&mut header)?;
        let length = usize::try_from(u32::from_le_bytes([
            header[8], header[9], header[10], header[11],
        ]))
        .map_err(|_error| PeerFailure::Host)?;
        // Events can carry one bounded opaque OBTF record between control responses.
        if !(TRANSPORT_ENVELOPE_HEADER_BYTES
            ..=TRANSPORT_ENVELOPE_HEADER_BYTES + MAXIMUM_TERMINAL_FRAME_BYTES)
            .contains(&length)
        {
            return Err(PeerFailure::Host);
        }
        let mut record = vec![0; length];
        record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
        self.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
        Ok(record)
    }

    fn read_exact(&mut self, mut destination: &mut [u8]) -> Result<(), PeerFailure> {
        while !destination.is_empty() {
            self.wait(PollFlags::POLLIN)?;
            match self.host.stream().read(destination) {
                Ok(0) => return Err(PeerFailure::Host),
                Ok(count) => destination = &mut destination[count..],
                Err(error) if transient(&error) => {}
                Err(_error) => return Err(PeerFailure::Host),
            }
        }
        Ok(())
    }

    fn wait(&self, interest: PollFlags) -> Result<(), PeerFailure> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(PeerFailure::Host);
            }
            // Round up; a sub-millisecond remainder must not become an idle spin.
            let milliseconds = remaining
                .as_millis()
                .saturating_add(1)
                .min(i32::MAX as u128);
            let timeout =
                PollTimeout::try_from(milliseconds).map_err(|_error| PeerFailure::Host)?;
            let (server, host) = self.poll(interest, timeout)?;
            if !server.is_empty() {
                return Err(PeerFailure::Server);
            }
            if host.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
                return Err(PeerFailure::Host);
            }
            // A complete response queued before EOF may still be read exactly once.
            if host.contains(interest) {
                return Ok(());
            }
            if host.contains(PollFlags::POLLHUP) {
                return Err(PeerFailure::Host);
            }
        }
    }

    fn poll(
        &self,
        interest: PollFlags,
        timeout: PollTimeout,
    ) -> Result<(PollFlags, PollFlags), PeerFailure> {
        let mut descriptors = [
            PollFd::new(self.server.as_fd(), PollFlags::POLLIN),
            PollFd::new(self.host.stream().as_fd(), interest),
        ];
        match poll(&mut descriptors, timeout) {
            Ok(_) => Ok((
                descriptors[0].revents().ok_or(PeerFailure::Server)?,
                descriptors[1].revents().ok_or(PeerFailure::Host)?,
            )),
            Err(Errno::EINTR) => Ok((PollFlags::empty(), PollFlags::empty())),
            Err(_error) => Err(PeerFailure::Host),
        }
    }

    pub(crate) fn close(&self, failure: PeerFailure) {
        let _closed = self.host.stream().shutdown(Shutdown::Both);
        if failure == PeerFailure::Server {
            let _closed = self.server.shutdown(Shutdown::Both);
        }
    }

    pub(crate) fn deadline_elapsed(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

pub(crate) fn exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
    encoding: PayloadEncoding,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind,
        optional_message: false,
        payload,
        payload_encoding: encoding,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    let mut exchange = HostExchange::new(host, server, Duration::from_secs(1))
        .ok_or_else(|| io::Error::other("invalid Host request budget"))?;
    exchange
        .prepare()
        .map_err(|_| io::Error::other("Host request preflight failed"))?;
    let record = exchange
        .round_trip(&record)
        .map_err(|_| io::Error::other("Host request exchange failed"))?;
    let envelope = decode_transport_envelope(&record)?;
    if envelope.correlation_id != Some(correlation)
        || envelope.host_epoch != Some(host.host_epoch())
        || envelope.role != MessageRole::Response
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(io::Error::other("unexpected Host request response envelope").into());
    }
    Ok(record)
}
