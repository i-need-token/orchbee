//! Performs deadline-bounded Host round trips while draining interleaved events and
//! classifying failure before or after possible delivery. It owns exchange I/O, not operation
//! authorization, effect replay, or Host process lifetime.
//!
//! An exchange starts only if its whole bound fits in the request it serves ([`Pacing::start`]);
//! one that cannot start writes nothing and records no failure. Bytes that have already arrived
//! are read before a deadline is checked, so a late Worker never refuses an answer that is
//! waiting, and a deadline ends an exchange only while it waits for bytes that have not come.
//!
//! [`Pacing::start`]: super::Pacing::start

use orchbee_local_ipc::AsLocalIo;
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    error::Error,
    io::{self, Write},
    net::Shutdown,
    time::{Duration, Instant},
};

use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, MAXIMUM_TERMINAL_FRAME_BYTES, MessageRole,
    PayloadEncoding, decode_transport_envelope, encode_transport_envelope,
};

use super::events::read_failure;
use super::pacing::{HostFailure, NotStarted, SLOW_EXCHANGE};
use crate::host_connection::{HostBrokerConnection, drain_events};

/// A preflight drain reads at most this many queued events, then lets the request go ahead; the
/// exchange reads any later event while it waits for its answer.
const MAXIMUM_PREFLIGHT_EVENTS: usize = 64;
/// After its deadline an exchange still reads records that have already arrived, but at most this
/// many, so a Host that floods events without answering cannot hold the serving thread.
const MAXIMUM_LATE_RECORDS: usize = 64;

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
    started: Instant,
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
            started: Instant::now(),
            deadline,
        }
    }

    /// An exchange with at most `budget`, never past the connection's exchange bound. It starts
    /// only if that whole bound ends within the current request.
    pub(crate) fn new(
        host: &'a HostBrokerConnection,
        server: &'a UnixStream,
        budget: Duration,
    ) -> Result<Self, NotStarted> {
        let started = Instant::now();
        Ok(Self {
            host,
            server,
            started,
            deadline: host.pacing.start(started, budget)?,
        })
    }

    /// An exchange with the connection's whole exchange bound, if that fits in the current
    /// request.
    pub(crate) fn open(
        host: &'a HostBrokerConnection,
        server: &'a UnixStream,
    ) -> Result<Self, NotStarted> {
        Self::new(host, server, host.pacing.exchange_bound())
    }

    /// Observes events that have already arrived and writes no request, so no start limit applies.
    #[cfg(windows)]
    pub(crate) fn observer(host: &'a HostBrokerConnection, server: &'a UnixStream) -> Self {
        let started = Instant::now();
        Self {
            host,
            server,
            started,
            deadline: started,
        }
    }

    pub(crate) fn prepare(&self) -> Result<(), PeerFailure> {
        self.consume_pending(MAXIMUM_PREFLIGHT_EVENTS)
    }

    /// A read-only cache observation must yield to a continuously active Host stream.
    #[cfg(windows)]
    pub(crate) fn observe_pending(&self, maximum_events: usize) -> Result<(), PeerFailure> {
        self.consume_pending(maximum_events)
    }

    /// Reads queued events that have arrived, at most `maximum_events` whole ones, without
    /// waiting; one the Host has only begun stays partial for the exchange to finish.
    fn consume_pending(&self, maximum_events: usize) -> Result<(), PeerFailure> {
        self.host
            .stream()
            .set_nonblocking(true)
            .map_err(|_error| self.fail(PeerFailure::Host))?;
        let (server, host) = self.poll(PollFlags::POLLIN, PollTimeout::ZERO)?;
        if !server.is_empty() {
            return Err(self.fail(PeerFailure::Server));
        }
        if host.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
            return Err(self.fail(PeerFailure::Host));
        }
        if host.intersects(PollFlags::POLLIN | PollFlags::POLLHUP)
            && drain_events(self.host, maximum_events).is_err()
        {
            // The drain recorded its own failure first; this exchange reports the Host.
            return Err(self.fail(PeerFailure::Host));
        }
        Ok(())
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
        self.write_request(record, maximum_response_payload)?;
        let mut late = 0_usize;
        loop {
            let record = self.read_record(maximum_response_payload)?;
            if let Some(response) = self.receive_interleaved(record, true)? {
                let waited = self.started.elapsed();
                if waited > SLOW_EXCHANGE {
                    crate::diagnostic_events::host_slow(self.host.pacing.site(), waited);
                }
                return Ok(response);
            }
            if self.deadline_elapsed() {
                late += 1;
                if late > MAXIMUM_LATE_RECORDS {
                    return Err(self.fail(PeerFailure::Host));
                }
            }
        }
    }

    /// Writes the whole request. A request abandoned part way, at its deadline or on an error,
    /// leaves the Host stream out of step for good, so the connection is closed at once and never
    /// used again.
    fn write_request(
        &self,
        record: &[u8],
        maximum_response_payload: usize,
    ) -> Result<(), PeerFailure> {
        let mut written = 0;
        while let Some(remaining) = record.get(written..).filter(|rest| !rest.is_empty()) {
            match self.write_step(remaining, maximum_response_payload) {
                Ok(count) => written += count,
                Err(failure) => {
                    if written > 0 {
                        let _closed = self.host.stream().shutdown(Shutdown::Both);
                    }
                    return Err(failure);
                }
            }
        }
        Ok(())
    }

    /// Writes what the Host takes now and returns how much that was. The Host may be writing to
    /// this Worker while its own input is full; reading its events here keeps both directions
    /// moving instead of stalling the Host's writer.
    fn write_step(
        &self,
        remaining: &[u8],
        maximum_response_payload: usize,
    ) -> Result<usize, PeerFailure> {
        let ready = self.wait(PollFlags::POLLOUT | PollFlags::POLLIN)?;
        if ready.contains(PollFlags::POLLOUT) {
            match self.host.stream().write(remaining) {
                Ok(0) => return Err(self.fail(PeerFailure::Host)),
                Ok(count) => return Ok(count),
                Err(error) if transient(&error) => {}
                Err(_error) => return Err(self.fail(PeerFailure::Host)),
            }
        }
        if ready.contains(PollFlags::POLLIN)
            && let Some(event) = self.advance(maximum_response_payload)?
        {
            self.receive_interleaved(event, false)?;
        }
        Ok(0)
    }

    /// Routes one record read during the exchange: an event goes to connection state; a response
    /// is returned once the request has been written, and before that breaks the protocol.
    fn receive_interleaved(
        &self,
        record: Vec<u8>,
        request_written: bool,
    ) -> Result<Option<Vec<u8>>, PeerFailure> {
        let envelope =
            decode_transport_envelope(&record).map_err(|_| self.fail(PeerFailure::Protocol))?;
        if envelope.role != MessageRole::Event {
            return if request_written {
                Ok(Some(record))
            } else {
                Err(self.fail(PeerFailure::Protocol))
            };
        }
        if self.host.receive_event(record).is_err() {
            self.host
                .pacing
                .note_failure(self.failure_of(PeerFailure::Protocol));
            return Err(self.fail(PeerFailure::Host));
        }
        Ok(None)
    }

    /// Reads the next whole record, continuing one the connection has begun.
    fn read_record(&self, maximum_response_payload: usize) -> Result<Vec<u8>, PeerFailure> {
        loop {
            if let Some(record) = self.advance(maximum_response_payload)? {
                return Ok(record);
            }
            self.wait(PollFlags::POLLIN)?;
        }
    }

    /// Reads what has arrived of the connection's current record without waiting.
    fn advance(&self, maximum_response_payload: usize) -> Result<Option<Vec<u8>>, PeerFailure> {
        let advanced = self
            .host
            .inbound
            .borrow_mut()
            .advance(self.host.stream(), maximum_response_payload);
        advanced.map_err(|error| self.fail(read_failure(&error)))
    }

    /// Waits until the Host stream is ready for one of `interest` and returns its readiness.
    /// Readiness is taken before the deadline is checked, so arrived bytes are always read.
    fn wait(&self, interest: PollFlags) -> Result<PollFlags, PeerFailure> {
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            // Round up; a sub-millisecond remainder must not become an idle spin.
            let milliseconds = if remaining.is_zero() {
                0
            } else {
                remaining
                    .as_millis()
                    .saturating_add(1)
                    .min(i32::MAX as u128)
            };
            let timeout = PollTimeout::try_from(milliseconds)
                .map_err(|_error| self.fail(PeerFailure::Host))?;
            let (server, host) = self.poll(interest, timeout)?;
            if !server.is_empty() {
                return Err(self.fail(PeerFailure::Server));
            }
            if host.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
                return Err(self.fail(PeerFailure::Host));
            }
            // A complete response queued before EOF may still be read exactly once.
            if host.intersects(interest) {
                return Ok(host);
            }
            if host.contains(PollFlags::POLLHUP) || remaining.is_zero() {
                return Err(self.fail(PeerFailure::Host));
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
                descriptors[0]
                    .revents()
                    .ok_or_else(|| self.fail(PeerFailure::Server))?,
                descriptors[1]
                    .revents()
                    .ok_or_else(|| self.fail(PeerFailure::Host))?,
            )),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                Ok((PollFlags::empty(), PollFlags::empty()))
            }
            Err(_error) => Err(self.fail(PeerFailure::Host)),
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

    /// The error a caller returns for this exchange's failure, carrying its class, whether the
    /// bound passed, and how long the exchange waited.
    pub(crate) fn error(&self, class: PeerFailure) -> io::Error {
        self.failure_of(class).into_io()
    }

    fn failure_of(&self, class: PeerFailure) -> HostFailure {
        HostFailure {
            class,
            deadline: self.deadline_elapsed(),
            waited: self.started.elapsed(),
            budget: self.deadline.saturating_duration_since(self.started),
        }
    }

    /// Records a failure of this exchange on the connection, including one its caller finds in
    /// an answer that arrived intact.
    pub(crate) fn note(&self, class: PeerFailure) {
        self.host.pacing.note_failure(self.failure_of(class));
    }

    /// Records the failure on the connection and returns its class.
    fn fail(&self, class: PeerFailure) -> PeerFailure {
        self.note(class);
        class
    }
}

fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// One correlated request and its ControlJson answer, with the connection's whole exchange bound.
/// [`NotStarted`] is returned, boxed, when the request has no room left for it; nothing was
/// written then.
pub(crate) fn exchange(
    host: &HostBrokerConnection,
    server: &UnixStream,
    correlation: CorrelationId,
    kind: u16,
    payload: &[u8],
    encoding: PayloadEncoding,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut exchange = HostExchange::open(host, server)?;
    exchange.prepare().map_err(|class| exchange.error(class))?;
    exchange.request(correlation, kind, payload, encoding)
}

impl HostExchange<'_> {
    /// Writes one correlated request and returns its ControlJson answer, whose envelope must be in
    /// step with it.
    pub(crate) fn request(
        &mut self,
        correlation: CorrelationId,
        kind: u16,
        payload: &[u8],
        encoding: PayloadEncoding,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let host = self.host;
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
        let record = self
            .round_trip(&record)
            .map_err(|class| self.error(class))?;
        let envelope = decode_transport_envelope(&record)?;
        if envelope.correlation_id != Some(correlation)
            || envelope.host_epoch != Some(host.host_epoch())
            || envelope.role != MessageRole::Response
            || envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            self.note(PeerFailure::Protocol);
            return Err(io::Error::other("unexpected Host request response envelope").into());
        }
        Ok(record)
    }
}
