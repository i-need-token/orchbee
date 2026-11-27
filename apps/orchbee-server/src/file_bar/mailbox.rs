//! What waits to reach one Client connection's File Bar streams: the frames and closes the link
//! relays and those the Server answers itself. It retains at most 8 MiB of records, so a Client
//! that stops reading costs its own streams, not the Server's memory; a close always fits.

use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{MultiplexStreamId, StreamCloseReason, StreamFrameKind};
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
};

/// The record bytes one connection's mailbox retains.
const MAXIMUM_RETAINED_BYTES: usize = 8 * 1024 * 1024;

/// One frame or close for a File Bar stream.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Outbound {
    Frame {
        stream: MultiplexStreamId,
        sequence: NonZeroU64,
        kind: StreamFrameKind,
        record: Vec<u8>,
    },
    Close {
        stream: MultiplexStreamId,
        reason: StreamCloseReason,
    },
}

impl Outbound {
    pub(crate) const fn stream(&self) -> MultiplexStreamId {
        match self {
            Self::Frame { stream, .. } | Self::Close { stream, .. } => *stream,
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Self::Frame { record, .. } => record.len(),
            Self::Close { .. } => 0,
        }
    }
}

/// A frame the mailbox could not retain.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Full;

/// One connection's queue, shared by the link's reader, which fills it, and the connection's
/// dispatcher, which empties it.
#[derive(Debug, Default)]
pub(super) struct Mailbox {
    queue: Mutex<Queue>,
    /// Wakes the connection's dispatcher once it runs.
    wake: OnceLock<Arc<SessionChangeSignal>>,
}

#[derive(Debug, Default)]
struct Queue {
    items: VecDeque<Outbound>,
    bytes: usize,
}

impl Mailbox {
    /// Lets queued items wake the connection's dispatcher, which waits on `signal`.
    pub(super) fn bind(&self, signal: &Arc<SessionChangeSignal>) {
        let _ = self.wake.set(Arc::clone(signal));
    }

    /// Queues one frame, unless the mailbox would retain more than its bound.
    pub(super) fn frame(
        &self,
        stream: MultiplexStreamId,
        sequence: NonZeroU64,
        kind: StreamFrameKind,
        record: Vec<u8>,
    ) -> Result<(), Full> {
        let mut queue = self.lock();
        if queue.bytes.saturating_add(record.len()) > MAXIMUM_RETAINED_BYTES {
            return Err(Full);
        }
        queue.bytes += record.len();
        queue.items.push_back(Outbound::Frame {
            stream,
            sequence,
            kind,
            record,
        });
        drop(queue);
        self.notify();
        Ok(())
    }

    /// Queues a stream's close, after dropping what it still queued when `discard`.
    pub(super) fn close(
        &self,
        stream: MultiplexStreamId,
        reason: StreamCloseReason,
        discard: bool,
    ) {
        let mut queue = self.lock();
        if discard {
            queue.forget(stream);
        }
        queue.items.push_back(Outbound::Close { stream, reason });
        drop(queue);
        self.notify();
    }

    /// Drops everything queued for `stream`, which its Client closed.
    pub(super) fn forget(&self, stream: MultiplexStreamId) {
        self.lock().forget(stream);
    }

    /// Takes queued items in order: frames until `frames` of them or `bytes` record bytes are
    /// taken, the first whatever its size, and the closes up to the next frame left behind.
    pub(super) fn take(&self, frames: usize, bytes: usize) -> Vec<Outbound> {
        let mut queue = self.lock();
        let mut taken = Vec::new();
        let (mut frame_count, mut byte_count) = (0, 0);
        while let Some(item) = queue.items.front() {
            let frame = matches!(item, Outbound::Frame { .. });
            if frame
                && (frame_count >= frames || (frame_count > 0 && byte_count + item.bytes() > bytes))
            {
                break;
            }
            let Some(item) = queue.items.pop_front() else {
                break;
            };
            queue.bytes -= item.bytes();
            byte_count += item.bytes();
            frame_count += usize::from(frame);
            taken.push(item);
        }
        taken
    }

    /// Whether anything waits.
    pub(super) fn is_empty(&self) -> bool {
        self.lock().items.is_empty()
    }

    fn notify(&self) {
        if let Some(signal) = self.wake.get() {
            signal.wake();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Queue {
    fn forget(&mut self, stream: MultiplexStreamId) {
        let mut released = 0;
        self.items.retain(|item| {
            let kept = item.stream() != stream;
            if !kept {
                released += item.bytes();
            }
            kept
        });
        self.bytes -= released;
    }
}
