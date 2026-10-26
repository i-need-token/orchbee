//! One idle application-progress probe per carrier, with an absolute queue-to-reply deadline.
//! SSH keepalive proves transport reachability only. This bounded read proves dispatcher progress;
//! failure withdraws projections without claiming that a busy or unreachable Server has died.
use super::{Command, Shared};
use orchbee_client_server_protocol::{
    ClientMessage, FRAME_HEADER_BYTES, MAXIMUM_FRAME_PAYLOAD_BYTES, RequestId, ServerMessage,
    TerminalId, TerminalLaunchReadRequest, decode_server_frame, encode_client_frame,
};
use orchbee_workbench_protocol::{MultiplexMessage, MultiplexStreamId, StreamFrameKind};
use std::{
    io,
    num::NonZeroU64,
    time::{Duration, Instant},
};

pub(super) const RESERVED_BYTES: usize = 256 * 1024;
pub(super) const RESERVED_STREAMS: usize = 1;
const RESPONSE_BYTES: usize = MAXIMUM_FRAME_PAYLOAD_BYTES + FRAME_HEADER_BYTES;

#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub(super) idle: Duration,
    pub(super) response: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(5),
            response: Duration::from_secs(2),
        }
    }
}
struct Probe {
    stream: MultiplexStreamId,
    request: RequestId,
    handoff: Option<NonZeroU64>,
    deadline: Instant,
}
pub(super) struct Health {
    timing: Timing,
    idle_at: Instant,
    pending: Option<Probe>,
}
impl Health {
    pub(super) fn new(timing: Timing, now: Instant) -> Self {
        Self {
            timing,
            idle_at: now + timing.idle,
            pending: None,
        }
    }
    pub(super) fn reset(&mut self, now: Instant) {
        self.idle_at = now + self.timing.idle;
        self.pending = None;
    }
    pub(super) fn deadline(&self) -> Instant {
        self.pending
            .as_ref()
            .map_or(self.idle_at, |probe| probe.deadline)
    }
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn valid_until(&self) -> Instant {
        self.pending
            .as_ref()
            .map_or(self.idle_at + self.timing.response, |probe| probe.deadline)
    }
    pub(super) fn expired(&self, now: Instant) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|probe| now >= probe.deadline)
    }
    pub(super) fn enqueue(&mut self, shared: &Shared, now: Instant) -> io::Result<()> {
        if self.pending.is_some() || now < self.idle_at {
            return Ok(());
        }
        // Allocate and queue under the same lock as user streams. Inserting directly into the
        // actor's output could overtake an already allocated user id and violate high-water order.
        let mut commands = shared
            .commands
            .lock()
            .map_err(|_| io::ErrorKind::BrokenPipe)?;
        if commands.queue.len() >= orchbee_workbench_protocol::MAXIMUM_MULTIPLEX_STREAMS * 2 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let next = commands
            .next
            .checked_add(1)
            .ok_or(io::ErrorKind::OutOfMemory)?;
        let stream = MultiplexStreamId::new(next).ok_or(io::ErrorKind::InvalidData)?;
        let request = RequestId::parse(&format!("{next:032x}")).map_err(io::Error::other)?;
        // No invented Session identity is needed. A valid generated target is safe even if it
        // happens to exist: TerminalLaunchRead never attaches, mutates, or dispatches Host RPC.
        let target = probe_target()?;
        let request_frame = encode_client_frame(&ClientMessage::TerminalLaunchRead(
            TerminalLaunchReadRequest::new(request.clone(), target),
        ))
        .map_err(io::Error::other)?;
        commands.next = next;
        commands.queue.push_back(Command::Probe {
            id: stream,
            request: request_frame,
        });
        self.pending = Some(Probe {
            stream,
            request,
            handoff: None,
            deadline: now + self.timing.response,
        });
        drop(commands);
        shared.signal();
        Ok(())
    }
    pub(super) fn queued(&self, stream: MultiplexStreamId) -> Option<Instant> {
        self.pending
            .as_ref()
            .filter(|probe| probe.stream == stream)
            .map(|probe| probe.deadline)
    }
    pub(super) fn handoff(&mut self, stream: MultiplexStreamId, generation: NonZeroU64) {
        if let Some(probe) = &mut self.pending
            && probe.stream == stream
        {
            probe.handoff = Some(generation);
        }
    }
    /// Return true only for the current probe. Unrelated and retired frames cannot settle it.
    pub(super) fn receive(
        &mut self,
        message: &MultiplexMessage,
        generation: NonZeroU64,
        now: Instant,
    ) -> io::Result<bool> {
        let Some(probe) = &self.pending else {
            return Ok(false);
        };
        let MultiplexMessage::StreamFrame {
            stream_id,
            sequence,
            kind,
            record,
        } = message
        else {
            if matches!(message, MultiplexMessage::StreamClose { stream_id, .. } | MultiplexMessage::Gap { stream_id, .. } if *stream_id == probe.stream)
            {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            return Ok(false);
        };
        if *stream_id != probe.stream {
            return Ok(false);
        }
        if now >= probe.deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if probe.handoff != Some(generation)
            || sequence.get() != 1
            || *kind != StreamFrameKind::Result
            || record.len() > RESPONSE_BYTES
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let ServerMessage::TerminalLaunchReadResult(result) =
            decode_server_frame(record).map_err(|_| io::ErrorKind::InvalidData)?
        else {
            return Err(io::ErrorKind::InvalidData.into());
        };
        if result.request_id() != &probe.request {
            return Err(io::ErrorKind::InvalidData.into());
        }
        self.reset(now);
        Ok(true)
    }
    pub(super) fn business_progress(&mut self, now: Instant) {
        // Once queued, only the exact probe response can extend its total deadline.
        if self.pending.is_none() {
            self.idle_at = now + self.timing.idle;
        }
    }
}

fn probe_target() -> io::Result<TerminalId> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| io::ErrorKind::Other)?;
    TerminalId::parse(
        &bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .map_err(|_| io::ErrorKind::Other.into())
}
