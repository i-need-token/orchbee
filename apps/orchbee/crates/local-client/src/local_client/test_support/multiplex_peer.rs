//! Scripted peers keep domain assertions while exercising the real single-carrier framing.
use orchbee_local_client_protocol::{LocalClientMessage, write_message};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{
    MultiplexLaneId, MultiplexMessage, MultiplexStreamId, ProtocolVersion, StreamFrameKind,
    StreamMode, WorkbenchPublicationDocument, WorkbenchPublicationOutcome,
    WorkbenchPublicationResult, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, read_multiplex_message,
    read_workbench_request, read_workbench_response, write_multiplex_message,
    write_workbench_response,
};
use std::{
    collections::BTreeMap,
    io::{self, Cursor, Read, Write},
    num::NonZeroU64,
    time::Duration,
};

pub struct Open {
    pub id: MultiplexStreamId,
    pub mode: StreamMode,
    pub request: Vec<u8>,
}
pub fn admit(stream: &mut LocalStream, generation: u64) -> io::Result<()> {
    super::handshake::expect_hello(stream)?;
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(generation).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
pub fn read_open(stream: &mut LocalStream) -> io::Result<Open> {
    loop {
        match read_multiplex_message(stream)? {
            MultiplexMessage::StreamOpen {
                stream_id,
                mode,
                request,
                ..
            } => {
                return Ok(Open {
                    id: stream_id,
                    mode,
                    request,
                });
            }
            MultiplexMessage::LaneOpen { .. }
            | MultiplexMessage::LaneClose { .. }
            | MultiplexMessage::StreamClose { .. } => {}
            _ => return Err(io::ErrorKind::InvalidData.into()),
        }
    }
}
pub fn respond(
    stream: &mut LocalStream,
    open: &Open,
    response: &WorkbenchResponse,
    sequence: u64,
) -> io::Result<()> {
    let mut record = Vec::new();
    write_workbench_response(&mut record, response).map_err(io::Error::other)?;
    write_multiplex_message(
        stream,
        &MultiplexMessage::StreamFrame {
            stream_id: open.id,
            sequence: NonZeroU64::new(sequence).unwrap(),
            kind: if open.mode == StreamMode::Request {
                StreamFrameKind::Result
            } else if sequence == 1 {
                StreamFrameKind::Snapshot
            } else {
                StreamFrameKind::Update
            },
            record,
        },
    )
}

/// A Client may close its carrier before an unsolicited subscription frame or probe answer
/// arrives, as it may against a real Server; a scripted reply it awaits stays strict.
fn tolerate_closed_carrier(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
            ) =>
        {
            Ok(())
        }
        result => result,
    }
}

/// Domain scripts can run directly or over the TUI's actual multiplexer. Snapshot subscriptions
/// observe the same snapshots that the script publishes; they never open an extra Server socket.
pub struct Peer {
    stream: LocalStream,
    admitted: bool,
    multiplex: Option<bool>,
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    reply: Option<Open>,
    snapshots: BTreeMap<MultiplexStreamId, (WorkbenchRequestId, StreamMode, u64)>,
    latest: Option<WorkbenchSnapshotOutcome>,
    /// `Some` scripts only the first opened lane; `Some(None)` awaits that lane.
    primary: Option<Option<MultiplexLaneId>>,
    probes: bool,
}
impl Peer {
    pub fn new(stream: LocalStream) -> Self {
        Self {
            stream,
            admitted: false,
            multiplex: None,
            input: Cursor::new(Vec::new()),
            output: Vec::new(),
            reply: None,
            snapshots: BTreeMap::new(),
            latest: None,
            primary: None,
            probes: false,
        }
    }
    /// Scripts only the Client's first logical lane. Observation lanes opened later on the same
    /// carrier stay unanswered until they close, as against a Server that never replies to them.
    pub fn primary_lane(stream: LocalStream) -> Self {
        Self {
            primary: Some(None),
            ..Self::new(stream)
        }
    }
    /// From now on, answers multiplexed `publication.read` probes from the latest scripted
    /// snapshot, as a journal would, once the script expects no further explicit refresh.
    pub fn answer_probes(&mut self) {
        self.probes = true;
    }
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }
    fn snapshot(&mut self, id: MultiplexStreamId) -> io::Result<()> {
        let Some(outcome) = self.latest.clone() else {
            return Ok(());
        };
        let Some((request_id, mode, sequence)) = self.snapshots.get_mut(&id) else {
            return Ok(());
        };
        *sequence += 1;
        let open = Open {
            id,
            mode: *mode,
            request: Vec::new(),
        };
        tolerate_closed_carrier(respond(
            &mut self.stream,
            &open,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                outcome,
            }),
            *sequence,
        ))
    }
    fn probe(&mut self, id: MultiplexStreamId, request: &[u8]) -> io::Result<bool> {
        let Ok(WorkbenchRequest::PublicationRead(intent)) =
            read_workbench_request(&mut Cursor::new(request))
        else {
            return Ok(false);
        };
        let outcome = match self.latest.clone() {
            Some(WorkbenchSnapshotOutcome::Available { snapshot })
                if snapshot.revision > intent.known_revision =>
            {
                WorkbenchPublicationOutcome::Published {
                    publications: vec![WorkbenchPublicationDocument::Snapshot(snapshot)],
                }
            }
            Some(WorkbenchSnapshotOutcome::Available { snapshot }) => {
                WorkbenchPublicationOutcome::Current {
                    revision: snapshot.revision,
                }
            }
            Some(WorkbenchSnapshotOutcome::Denied {}) => WorkbenchPublicationOutcome::Denied {},
            Some(WorkbenchSnapshotOutcome::Unavailable { reason }) => {
                WorkbenchPublicationOutcome::Unavailable { reason }
            }
            None => WorkbenchPublicationOutcome::Current {
                revision: intent.known_revision,
            },
        };
        let open = Open {
            id,
            mode: StreamMode::Request,
            request: Vec::new(),
        };
        tolerate_closed_carrier(respond(
            &mut self.stream,
            &open,
            &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                protocol_version: ProtocolVersion,
                request_id: intent.request_id,
                outcome,
            }),
            1,
        ))?;
        Ok(true)
    }
    fn accept(&mut self, message: MultiplexMessage) -> io::Result<bool> {
        match message {
            MultiplexMessage::StreamOpen {
                stream_id,
                lane_id,
                mode,
                request,
            } => {
                if mode != StreamMode::Request
                    && let Ok(WorkbenchRequest::SnapshotRead(intent)) =
                        read_workbench_request(&mut Cursor::new(&request))
                {
                    self.snapshots
                        .insert(stream_id, (intent.request_id, mode, 0));
                    self.snapshot(stream_id)?;
                    return Ok(false);
                }
                if self
                    .primary
                    .is_some_and(|primary| primary.is_some_and(|primary| primary != lane_id))
                {
                    return Ok(false);
                }
                if self.probes && mode == StreamMode::Request && self.probe(stream_id, &request)? {
                    return Ok(false);
                }
                self.reply = Some(Open {
                    id: stream_id,
                    mode,
                    request: Vec::new(),
                });
                self.input = Cursor::new(request);
                Ok(true)
            }
            MultiplexMessage::StreamClose { stream_id, .. } => {
                self.snapshots.remove(&stream_id);
                Ok(false)
            }
            MultiplexMessage::LaneOpen { lane_id } => {
                if self.primary == Some(None) {
                    self.primary = Some(Some(lane_id));
                }
                Ok(false)
            }
            MultiplexMessage::LaneClose { .. } => Ok(false),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }
}
impl Read for Peer {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if !self.admitted {
            return self.stream.read(buffer);
        }
        loop {
            if self.input.position() < self.input.get_ref().len() as u64 {
                return self.input.read(buffer);
            }
            if self.multiplex == Some(false) {
                return self.stream.read(buffer);
            }
            let message = if self.multiplex.is_none() {
                let mut header = [0; 12];
                match self.stream.read_exact(&mut header) {
                    Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(0),
                    result => result?,
                }
                if &header[..4] != b"OBWP"
                    || !matches!(u16::from_be_bytes([header[6], header[7]]), 64 | 70)
                {
                    self.multiplex = Some(false);
                    self.input = Cursor::new(header.to_vec());
                    continue;
                }
                self.multiplex = Some(true);
                read_multiplex_message(&mut Cursor::new(header).chain(&mut self.stream))
            } else {
                read_multiplex_message(&mut self.stream)
            };
            match message {
                Ok(message) => {
                    self.accept(message)?;
                }
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(0),
                Err(error) => return Err(error),
            }
        }
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.admitted {
            let count = self.stream.write(bytes)?;
            if bytes.starts_with(b"OBLC") {
                self.admitted = true;
            }
            return Ok(count);
        }
        if self.multiplex != Some(true) {
            return self.stream.write(bytes);
        }
        if bytes.len() < 8
            && self.output.is_empty()
            && let Some(open) = self.reply.take()
        {
            write_multiplex_message(
                &mut self.stream,
                &MultiplexMessage::StreamFrame {
                    stream_id: open.id,
                    sequence: NonZeroU64::MIN,
                    kind: StreamFrameKind::Result,
                    record: bytes.to_vec(),
                },
            )?;
            return Ok(bytes.len());
        }
        self.output.extend_from_slice(bytes);
        let length = if self.output.len() < 8 {
            return Ok(bytes.len());
        } else if self.output.starts_with(b"OBCS") {
            8 + usize::from(u16::from_be_bytes([self.output[6], self.output[7]]))
        } else if self.output.len() >= 12 {
            12 + u32::from_be_bytes(self.output[8..12].try_into().unwrap()) as usize
        } else {
            return Ok(bytes.len());
        };
        if self.output.len() < length {
            return Ok(bytes.len());
        }
        let record = std::mem::take(&mut self.output);
        let mut changed = false;
        if let Ok(WorkbenchResponse::Snapshot(reply)) =
            read_workbench_response(&mut Cursor::new(&record))
        {
            changed = self.latest.as_ref() != Some(&reply.outcome);
            self.latest = Some(reply.outcome);
        }
        let open = self.reply.take().ok_or(io::ErrorKind::InvalidData)?;
        write_multiplex_message(
            &mut self.stream,
            &MultiplexMessage::StreamFrame {
                stream_id: open.id,
                sequence: NonZeroU64::MIN,
                kind: if open.mode == StreamMode::Request {
                    StreamFrameKind::Result
                } else {
                    StreamFrameKind::Snapshot
                },
                record,
            },
        )?;
        if changed {
            for id in self.snapshots.keys().copied().collect::<Vec<_>>() {
                self.snapshot(id)?;
            }
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
