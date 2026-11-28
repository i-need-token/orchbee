//! One Client connection's File Bar streams. A stream's OBFB record is checked only for its
//! operation, the capability it needs, its Workspace and its grant class, then travels to
//! `orchbee-files` unchanged; Put records are never parsed. A request that needs `files-edit` from
//! a connection whose answer did not list it closes as unsupported, as on a Server without it. A connection holds at most 24 File Bar streams, 16 of them carrying Puts or
//! read chunks. Its dispatcher writes a File Bar record only when no terminal or Workbench record
//! waits, but at least one every 100 milliseconds, so neither starves the other.

use super::{Call, FileBar, mailbox::Mailbox, mailbox::Outbound};
use orchbee_files_protocol::{
    CallId, ConnectionId, ErrorCode, FilesCapability, FilesError, FilesRecord, GrantClass, LaneId,
    Operation, OperationStream, RecordPlace, RequestError, decode_files_record, parse_request,
};
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{
    MultiplexLaneId, MultiplexStreamId, StreamCloseReason, StreamMode,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

/// File Bar streams one connection may hold open.
const MAXIMUM_STREAMS: usize = 24;
/// Of those, the streams that carry a Put or a read chunk.
const MAXIMUM_DATA_STREAMS: usize = 16;
/// The longest a File Bar record waits behind terminal and Workbench records.
const PRIORITY_INTERVAL: Duration = Duration::from_millis(100);
/// The record bytes one pass of the dispatcher writes while nothing else waits.
const PASS_BYTES: usize = 1024 * 1024;

struct Open {
    call: CallId,
    lane: MultiplexLaneId,
    data: bool,
}

/// The File Bar streams of one Client connection, owned by its dispatcher. Dropping it ends
/// everything the connection held in `orchbee-files`.
pub(crate) struct Relay {
    bar: Arc<FileBar>,
    connection: ConnectionId,
    grant: Option<GrantClass>,
    /// Whether the connection's handshake answer lists `files-edit`.
    edit: bool,
    mailbox: Arc<Mailbox>,
    streams: BTreeMap<MultiplexStreamId, Open>,
    lanes: BTreeSet<MultiplexLaneId>,
    /// Whether a call went to the File Bar, which then holds state for this connection.
    used: bool,
    last_delivery: Option<Instant>,
    yielded: bool,
}

impl Relay {
    pub(super) fn new(
        bar: Arc<FileBar>,
        connection: ConnectionId,
        grant: Option<GrantClass>,
        edit: bool,
    ) -> Self {
        Self {
            bar,
            connection,
            grant,
            edit,
            mailbox: Arc::default(),
            streams: BTreeMap::new(),
            lanes: BTreeSet::new(),
            used: false,
            last_delivery: None,
            yielded: false,
        }
    }

    /// Lets File Bar records wake the dispatcher that waits on `signal`.
    pub(crate) fn bind(&self, signal: &Arc<SessionChangeSignal>) {
        self.mailbox.bind(signal);
    }

    /// Takes the File Bar stream the Client opened with `record`; the error is the close that
    /// refuses it at once.
    pub(crate) fn open(
        &mut self,
        stream: MultiplexStreamId,
        lane: MultiplexLaneId,
        mode: StreamMode,
        record: Vec<u8>,
    ) -> Result<(), StreamCloseReason> {
        if self.streams.len() >= MAXIMUM_STREAMS {
            return Err(StreamCloseReason::Capacity);
        }
        let (place, events) = match mode {
            StreamMode::Request => (RecordPlace::RequestOpen, false),
            StreamMode::Events => (RecordPlace::EventsOpen, true),
            StreamMode::State | StreamMode::Invalidations => {
                return Err(StreamCloseReason::InvalidRequest);
            }
        };
        let (workspace, needed, data) = match decode_files_record(&record, place) {
            Ok(FilesRecord::Request(json)) => {
                let request = parse_request(&json).map_err(|error| match error {
                    RequestError::Unsupported => StreamCloseReason::Unsupported,
                    RequestError::Invalid => StreamCloseReason::InvalidRequest,
                })?;
                if request.capability() == FilesCapability::FilesEdit && !self.edit {
                    return Err(StreamCloseReason::Unsupported);
                }
                let operation = request.operation();
                if (operation.stream() == OperationStream::Events) != events {
                    return Err(StreamCloseReason::InvalidRequest);
                }
                (
                    Some(request.workspace()),
                    operation.grant(),
                    operation == Operation::FsReadChunk,
                )
            }
            Ok(FilesRecord::Put(_)) => (None, GrantClass::Lifecycle, true),
            Ok(_) | Err(_) => return Err(StreamCloseReason::InvalidRequest),
        };
        if data && self.streams.values().filter(|open| open.data).count() >= MAXIMUM_DATA_STREAMS {
            return Err(StreamCloseReason::Capacity);
        }
        let lane_id = LaneId::new(lane.value()).ok_or(StreamCloseReason::InvalidRequest)?;
        let call = Call {
            id: self.bar.next_call(),
            connection: self.connection,
            lane: lane_id,
            workspace,
            grant: needed,
            record,
            events,
            mutation: needed == GrantClass::Lifecycle,
            stream,
            mailbox: Arc::clone(&self.mailbox),
        };
        self.streams.insert(
            stream,
            Open {
                call: call.id,
                lane,
                data,
            },
        );
        self.lanes.insert(lane);
        let permitted = matches!(
            (self.grant, needed),
            (Some(GrantClass::Lifecycle), _)
                | (Some(GrantClass::TerminalCreate), GrantClass::TerminalCreate)
        );
        if permitted {
            self.used = true;
            self.bar.submit(call);
        } else {
            call.answer(FilesError::new(ErrorCode::Denied));
        }
        Ok(())
    }

    /// Ends the File Bar stream the Client cancelled; false when `stream` is none of them.
    pub(crate) fn cancel(&mut self, stream: MultiplexStreamId) -> bool {
        let Some(open) = self.streams.remove(&stream) else {
            return false;
        };
        self.mailbox.forget(stream);
        self.bar.cancel(open.call);
        true
    }

    /// Ends the File Bar streams of a lane the Client closed, with every transfer on it, and
    /// returns them.
    pub(crate) fn close_lane(&mut self, lane: MultiplexLaneId) -> Vec<MultiplexStreamId> {
        let closed: Vec<_> = self
            .streams
            .iter()
            .filter(|(_, open)| open.lane == lane)
            .map(|(stream, _)| *stream)
            .collect();
        for stream in &closed {
            self.cancel(*stream);
        }
        if self.lanes.remove(&lane)
            && let Some(lane) = LaneId::new(lane.value())
        {
            self.bar.lane_closed(self.connection, lane);
        }
        closed
    }

    /// The File Bar frames and closes to write now: everything that fits one pass when nothing
    /// else waits (`others_wait` false), otherwise one record once 100 milliseconds passed since
    /// the last.
    pub(crate) fn take(&mut self, now: Instant, others_wait: bool) -> Vec<Outbound> {
        let due = self
            .last_delivery
            .is_none_or(|at| now.duration_since(at) >= PRIORITY_INTERVAL);
        self.yielded = others_wait;
        let frames = match (others_wait, due) {
            (false, _) => usize::MAX,
            (true, true) => 1,
            (true, false) => return Vec::new(),
        };
        let mut delivered = Vec::new();
        for item in self.mailbox.take(frames, PASS_BYTES) {
            let open = match &item {
                Outbound::Frame { stream, .. } => self.streams.contains_key(stream),
                Outbound::Close { stream, .. } => self.streams.remove(stream).is_some(),
            };
            // Anything for a stream the Client already closed is dropped.
            if open {
                delivered.push(item);
            }
        }
        if delivered
            .iter()
            .any(|item| matches!(item, Outbound::Frame { .. }))
        {
            self.last_delivery = Some(now);
        }
        delivered
    }

    /// How long the dispatcher may wait before File Bar records are due, `None` while none wait.
    pub(crate) fn due(&self, now: Instant) -> Option<Duration> {
        if self.mailbox.is_empty() {
            return None;
        }
        if !self.yielded {
            return Some(Duration::ZERO);
        }
        Some(self.last_delivery.map_or(Duration::ZERO, |at| {
            PRIORITY_INTERVAL.saturating_sub(now.duration_since(at))
        }))
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        if self.used {
            let calls: Vec<_> = self.streams.values().map(|open| open.call).collect();
            self.bar.connection_lost(self.connection, &calls);
        }
    }
}

#[cfg(test)]
mod tests;
