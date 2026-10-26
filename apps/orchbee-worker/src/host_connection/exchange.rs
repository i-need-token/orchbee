//! Performs deadline-bounded Host round trips while draining interleaved events and
//! classifying failure before or after possible delivery. It owns exchange I/O, not operation
//! authorization, effect replay, or Host process lifetime.

use orchbee_local_ipc::AsLocalIo;
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    error::Error,
    io::{self, Read, Write},
    net::Shutdown,
    time::{Duration, Instant},
};

use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MAXIMUM_TERMINAL_FRAME_BYTES, MessageRole,
    PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES, decode_transport_envelope,
    encode_transport_envelope,
};

use crate::host_connection::{HostBrokerConnection, consume_event};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PeerFailure {
    Host,
    /// The peer emitted bytes that violate the terminal protocol itself —
    /// a malformed frame or an envelope that fails contract validation —
    /// distinct from a carrier that simply stopped answering.
    Protocol,
    Server,
}

pub(crate) struct HostExchange<'a> {
    host: &'a HostBrokerConnection,
    server: &'a UnixStream,
    deadline: Instant,
}

impl<'a> HostExchange<'a> {
    /// Shares an existing caller deadline without renewing its budget.
    #[cfg(windows)]
    pub(crate) fn before(
        host: &'a HostBrokerConnection,
        server: &'a UnixStream,
        deadline: Instant,
    ) -> Self {
        Self {
            host,
            server,
            deadline,
        }
    }

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
        self.consume_pending(None)
    }

    /// A read-only cache observation must yield to a continuously active Host stream.
    #[cfg(windows)]
    pub(crate) fn observe_pending(&self, maximum_events: usize) -> Result<(), PeerFailure> {
        self.consume_pending(Some(maximum_events))
    }

    fn consume_pending(&self, maximum_events: Option<usize>) -> Result<(), PeerFailure> {
        self.host
            .stream()
            .set_nonblocking(true)
            .map_err(|_error| PeerFailure::Host)?;
        let mut consumed = 0usize;
        loop {
            if maximum_events.is_some_and(|maximum| consumed >= maximum) {
                return Ok(());
            }
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
                consumed = consumed.saturating_add(1);
                continue;
            }
            if !host.is_empty() {
                return Err(PeerFailure::Host);
            }
            return Ok(());
        }
    }

    pub(crate) fn round_trip(&mut self, record: &[u8]) -> Result<Vec<u8>, PeerFailure> {
        self.round_trip_bounded(record, MAXIMUM_TERMINAL_FRAME_BYTES)
    }

    /// Same single exchange with a tighter bound on the correlated response
    /// payload; interleaved events keep the existing frame bound because a
    /// queued frame may exceed a row request's byte budget legitimately.
    pub(crate) fn round_trip_bounded(
        &mut self,
        record: &[u8],
        maximum_response_payload: usize,
    ) -> Result<Vec<u8>, PeerFailure> {
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
            let record = self.read_record(maximum_response_payload)?;
            let envelope = decode_transport_envelope(&record).map_err(|_| PeerFailure::Protocol)?;
            if envelope.role != MessageRole::Event {
                return Ok(record);
            }
            self.host
                .receive_event(record)
                .map_err(|_| PeerFailure::Host)?;
        }
    }

    fn read_record(&mut self, maximum_response_payload: usize) -> Result<Vec<u8>, PeerFailure> {
        let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
        self.read_exact(&mut header)?;
        let length = usize::try_from(u32::from_le_bytes([
            header[8], header[9], header[10], header[11],
        ]))
        .map_err(|_error| PeerFailure::Host)?;
        // Events can carry one bounded opaque OBTF record between control
        // responses; anything else is a response bounded by this exchange's
        // declared payload budget.
        let event = u16::from_le_bytes([header[6], header[7]]) & (1 << 2) != 0;
        let bound = TRANSPORT_ENVELOPE_HEADER_BYTES
            .checked_add(if event {
                MAXIMUM_TERMINAL_FRAME_BYTES
            } else {
                maximum_response_payload
            })
            .ok_or(PeerFailure::Protocol)?;
        if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=bound).contains(&length) {
            return Err(PeerFailure::Protocol);
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
            PollFd::new(self.server.as_local_io(), PollFlags::POLLIN),
            PollFd::new(self.host.stream().as_local_io(), interest),
        ];
        match poll(&mut descriptors, timeout) {
            Ok(_) => Ok((
                descriptors[0].revents().ok_or(PeerFailure::Server)?,
                descriptors[1].revents().ok_or(PeerFailure::Host)?,
            )),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                Ok((PollFlags::empty(), PollFlags::empty()))
            }
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
    exchange_before(
        host,
        server,
        Envelope {
            correlation_id: Some(correlation),
            host_epoch: Some(host.host_epoch()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: encoding,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        },
        Instant::now() + Duration::from_secs(1),
    )
}

pub(crate) fn exchange_before(
    host: &HostBrokerConnection,
    server: &UnixStream,
    request: Envelope<'_>,
    deadline: Instant,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let correlation = request.correlation_id;
    if correlation.is_none()
        || request.host_epoch != Some(host.host_epoch())
        || request.protocol != CURRENT_PROTOCOL_VERSION
        || request.role != MessageRole::Request
        || request.optional_message
    {
        return Err(io::Error::other("invalid Host request envelope").into());
    }
    let record = encode_transport_envelope(&request)?;
    let mut exchange = HostExchange::new(
        host,
        server,
        deadline.saturating_duration_since(Instant::now()),
    )
    .ok_or_else(|| io::Error::other("invalid Host request budget"))?;
    exchange
        .prepare()
        .map_err(|_| io::Error::other("Host request preflight failed"))?;
    let record = exchange
        .round_trip(&record)
        .map_err(|_| io::Error::other("Host request exchange failed"))?;
    let envelope = decode_transport_envelope(&record)?;
    if envelope.correlation_id != correlation
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
