use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    cell::RefCell,
    error::Error,
    io::{self, Read, Write},
    path::Path,
    rc::Rc,
    time::Duration,
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ClientId, ConnectionControl, CorrelationId, Envelope, Hello,
    MAXIMUM_INPUT_BYTES, MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TRANSPORT_RECORD_BYTES, MessageRole, PayloadEncoding, TRANSPORT_ENVELOPE_HEADER_BYTES,
    TerminalEffectEvent, decode_connection_control, decode_terminal_effect_event,
    decode_transport_envelope, encode_connection_control, encode_transport_envelope,
};

use crate::agent_drain::AgentDrain;

#[cfg(any(target_os = "linux", windows))]
mod admission;

mod events;
pub(crate) use events::{consume_event, validate_event};

mod controllers;
mod exchange;
mod target_cleanup;
mod titles;
mod viewers;

#[cfg(test)]
mod controller_takeover_tests;
#[cfg(test)]
mod read_rows_tests;
#[cfg(test)]
mod selection_actions_tests;
pub(crate) use exchange::{HostExchange, PeerFailure, exchange, exchange_before};
pub(crate) use target_cleanup::PreparedTargetCleanup;
use viewers::HostViewers;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_ID_BYTES: [u8; 16] = *b"orchbee-worker-1";
const HELLO_CORRELATION_BYTES: [u8; 16] = *b"worker-host-hi-1";

pub(crate) struct HostBrokerConnection {
    host_epoch: orchbee_terminal_protocol::HostEpoch,
    negotiated_limits: NegotiatedHostLimits,
    stream: UnixStream,
    pub(crate) viewers: RefCell<HostViewers>,
    pub(crate) controllers: RefCell<controllers::HostControllers>,
    retired_targets: RefCell<target_cleanup::RetiredTargets>,
    titles: RefCell<titles::Titles>,
    /// Shared handle to the admitted Server connection's Agent Drain queue.
    /// The broker only appends; the connection loop owns the watermark and
    /// outlives this Host attachment, so dropping the broker never resets it.
    drain: Rc<RefCell<AgentDrain>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NegotiatedHostLimits {
    pub(crate) frame_bytes: u32,
    pub(crate) input_bytes: u32,
    pub(crate) row_window_bytes: u32,
}

impl HostBrokerConnection {
    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn connect_before(
        endpoint: &Path,
        deadline: std::time::Instant,
        drain: Rc<RefCell<AgentDrain>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut transport = admission::Admission::connect(endpoint, deadline)?;
        let expected = requested_hello()?;
        let correlation_id = CorrelationId::from_bytes(HELLO_CORRELATION_BYTES)?;
        write_hello(&mut transport, &expected, correlation_id)?;
        let (host_epoch, negotiated_limits) =
            validate_hello_accepted(&mut transport, &expected, correlation_id)?;
        transport.remaining()?;
        Ok(Self {
            host_epoch,
            negotiated_limits,
            stream: transport.stream,
            viewers: RefCell::new(HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(titles::Titles::default()),
            drain,
        })
    }

    pub(crate) fn connect(
        endpoint: &Path,
        drain: Rc<RefCell<AgentDrain>>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut stream = UnixStream::connect(endpoint)?;
        require_same_effective_user(&stream)?;
        stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
        stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;

        let expected = requested_hello()?;
        let correlation_id = CorrelationId::from_bytes(HELLO_CORRELATION_BYTES)?;
        write_hello(&mut stream, &expected, correlation_id)?;
        let (host_epoch, negotiated_limits) =
            validate_hello_accepted(&mut stream, &expected, correlation_id)?;

        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(Self {
            host_epoch,
            negotiated_limits,
            stream,
            viewers: RefCell::new(HostViewers::default()),
            controllers: RefCell::new(controllers::HostControllers::default()),
            retired_targets: RefCell::new(target_cleanup::RetiredTargets::default()),
            titles: RefCell::new(titles::Titles::default()),
            drain,
        })
    }

    pub(crate) const fn host_epoch(&self) -> orchbee_terminal_protocol::HostEpoch {
        self.host_epoch
    }

    pub(crate) const fn negotiated_limits(&self) -> NegotiatedHostLimits {
        self.negotiated_limits
    }

    pub(crate) fn receive_event(&self, record: Vec<u8>) -> io::Result<()> {
        if self
            .retired_targets
            .borrow()
            .accept_event(&record, self.host_epoch)?
        {
            return Ok(());
        }
        let envelope = orchbee_terminal_protocol::decode_transport_envelope(&record)
            .map_err(io::Error::other)?;
        // Envelope validation precedes every consumer below, including the
        // Agent Drain intercept and the viewer borrow.
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(self.host_epoch)
            || envelope.optional_message
        {
            return Err(io::Error::other("unexpected Host event envelope"));
        }
        if envelope.kind == 23 {
            if envelope.host_epoch != Some(self.host_epoch)
                || envelope.protocol != CURRENT_PROTOCOL_VERSION
                || envelope.correlation_id.is_some()
                || envelope.optional_message
                || envelope.payload_encoding
                    != orchbee_terminal_protocol::PayloadEncoding::ControlJson
            {
                return Err(io::Error::other("unexpected controller event envelope"));
            }
            let event = orchbee_terminal_protocol::decode_attachment_control(
                envelope.kind,
                envelope.role,
                envelope.payload,
            )
            .map_err(io::Error::other)?;
            if let orchbee_terminal_protocol::AttachmentControl::ResizeLeaseChanged(event) = event
                && self.controllers.borrow().validate_lease_event(event)
            {
                return Ok(());
            }
            return Err(io::Error::other("unexpected controller resize lease event"));
        }
        if envelope.payload_encoding == PayloadEncoding::ControlJson
            && let Ok(TerminalEffectEvent::ApplicationSignal(signal)) =
                decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload)
        {
            // The Agent Drain producer intercepts application signals before
            // the viewer borrow so terminals with no attached viewer still
            // reach the queue. Decode failures and epoch mismatches keep their
            // existing fatal outcome for this connection.
            if signal.identity.host_epoch != self.host_epoch {
                return Err(io::Error::other(
                    "application signal has a different Host epoch",
                ));
            }
            let identity =
                crate::terminal_host_launch_adapter::capability_identity(signal.identity)
                    .map_err(io::Error::other)?;
            return self.drain.borrow_mut().append(identity, signal.payload);
        }
        if envelope.kind == 57 {
            events::validate_event(&record, self.host_epoch)?;
            let TerminalEffectEvent::MetadataChanged(event) =
                decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Err(io::Error::other("invalid title event"));
            };
            return self.titles.borrow_mut().observe(event);
        }
        self.viewers.borrow_mut().receive(record, self.host_epoch)
    }

    pub(crate) fn terminal_title(
        &self,
        identity: orchbee_capability_node_protocol::HostTerminalIdentity,
    ) -> orchbee_capability_node_protocol::TerminalTitleObservation {
        self.titles.borrow().read(identity)
    }

    pub(crate) const fn stream(&self) -> &UnixStream {
        &self.stream
    }
}

fn requested_hello() -> Result<Hello, Box<dyn Error>> {
    Ok(Hello {
        client_id: ClientId::from_bytes(CLIENT_ID_BYTES)?,
        frame_bytes: u32::try_from(MAXIMUM_TERMINAL_FRAME_BYTES)?,
        input_bytes: u32::try_from(MAXIMUM_INPUT_BYTES)?,
        protocol: CURRENT_PROTOCOL_VERSION,
        row_window_bytes: u32::try_from(MAXIMUM_ROW_WINDOW_BYTES)?,
        supported_optional_features: Vec::new(),
    })
}

fn write_hello(
    stream: &mut impl Write,
    hello: &Hello,
    correlation_id: CorrelationId,
) -> Result<(), Box<dyn Error>> {
    let control = encode_connection_control(&ConnectionControl::Hello(hello.clone()))?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation_id),
        host_epoch: None,
        kind: control.kind,
        optional_message: false,
        payload: &control.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: control.role,
    })?;
    stream.write_all(&record)?;
    Ok(())
}

fn validate_hello_accepted(
    stream: &mut impl Read,
    requested: &Hello,
    correlation_id: CorrelationId,
) -> Result<(orchbee_terminal_protocol::HostEpoch, NegotiatedHostLimits), Box<dyn Error>> {
    let record = read_transport_record(stream)?;
    let envelope = decode_transport_envelope(&record).map_err(io::Error::other)?;
    if envelope.role != MessageRole::Response
        || envelope.correlation_id != Some(correlation_id)
        || envelope.host_epoch.is_none()
        || envelope.optional_message
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
    {
        return Err(invalid_handshake());
    }

    let ConnectionControl::HelloAccepted(accepted) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)?
    else {
        return Err(invalid_handshake());
    };
    if envelope.host_epoch != Some(accepted.host_epoch)
        || accepted.protocol != CURRENT_PROTOCOL_VERSION
        || accepted.frame_bytes != requested.frame_bytes
        || accepted.input_bytes != requested.input_bytes
        || accepted.row_window_bytes != requested.row_window_bytes
        || !accepted.supported_optional_features.is_empty()
    {
        return Err(invalid_handshake());
    }
    Ok((
        accepted.host_epoch,
        NegotiatedHostLimits {
            frame_bytes: accepted.frame_bytes,
            input_bytes: accepted.input_bytes,
            row_window_bytes: accepted.row_window_bytes,
        },
    ))
}

fn read_transport_record(stream: &mut impl Read) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut header = [0_u8; TRANSPORT_ENVELOPE_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let total_length = usize::try_from(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))?;
    if !(TRANSPORT_ENVELOPE_HEADER_BYTES..=MAXIMUM_TRANSPORT_RECORD_BYTES).contains(&total_length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Terminal Host record length is outside the protocol boundary",
        )
        .into());
    }

    let mut record = vec![0_u8; total_length];
    record[..TRANSPORT_ENVELOPE_HEADER_BYTES].copy_from_slice(&header);
    stream.read_exact(&mut record[TRANSPORT_ENVELOPE_HEADER_BYTES..])?;
    Ok(record)
}

fn invalid_handshake() -> Box<dyn Error> {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Terminal Host rejected the exact Worker broker handshake",
    )
    .into()
}
