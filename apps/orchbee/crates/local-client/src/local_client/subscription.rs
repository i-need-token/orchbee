//! Semantic subscriptions share the connection actor's cancellation and fresh-snapshot recovery.
use super::{
    LocalClientOrigin,
    multiplex::{StreamEvent, Subscription},
};
use orchbee_local_ipc::{AsLocalIo, LocalStream, PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    StreamMode, WorkbenchRequest, WorkbenchResponse, read_workbench_response,
    write_workbench_request,
};
use std::io::{self, Cursor, Read, Write};
use std::time::Instant;

/// An asynchronous one-shot request. Polling reads only the actor mailbox, never the socket.
pub struct Request {
    stream: Subscription,
    deadline: Instant,
    clock: super::LocalOperationClock,
}
impl Request {
    pub fn poll(&self) -> Option<Result<WorkbenchResponse, bool>> {
        if self.clock.now() >= self.deadline {
            return Some(Err(self.stream.cancel()));
        }
        match self.stream.poll() {
            Ok(Some(StreamEvent::Record(bytes))) => {
                Some(read_workbench_response(&mut Cursor::new(bytes)).map_err(|_| true))
            }
            Ok(None) => None,
            Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) | Err(_) => {
                Some(Err(self.stream.cancel()))
            }
        }
    }
}

impl LocalClientOrigin {
    pub fn request(
        &self,
        request: &WorkbenchRequest,
        deadline: Instant,
        clock: super::LocalOperationClock,
    ) -> io::Result<Request> {
        if clock.now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.subscribe(request, StreamMode::Request)
            .map(|stream| Request {
                stream,
                deadline,
                clock,
            })
    }
    /// Register before the first domain read. A commit concurrent with that read remains dirty.
    pub fn watch_changes(&self) -> io::Result<ChangeSubscription> {
        use orchbee_workbench_protocol::{ProtocolVersion, SnapshotReadIntent, WorkbenchRequestId};
        let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("fffffffffffffffffffffffffffffffd")
                .expect("nonzero subscription identity"),
        });
        self.subscribe(&request, StreamMode::Invalidations)
            .map(|stream| ChangeSubscription {
                stream,
                ended: std::sync::atomic::AtomicBool::new(false),
            })
    }
    pub fn subscribe(
        &self,
        request: &WorkbenchRequest,
        mode: StreamMode,
    ) -> io::Result<Subscription> {
        let connection = self.multiplex.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        let mut bytes = Vec::new();
        write_workbench_request(&mut bytes, request).map_err(io::Error::other)?;
        connection.subscribe(bytes, mode)
    }
}

pub struct ChangeSubscription {
    stream: Subscription,
    ended: std::sync::atomic::AtomicBool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeEvent {
    Published,
    Reset,
}
impl ChangeSubscription {
    /// A reset invalidates reads already in flight; a subsequent publication permits a fresh read.
    pub fn poll(&self) -> io::Result<Option<ChangeEvent>> {
        use std::sync::atomic::Ordering;
        if self.ended.load(Ordering::Acquire) {
            return Ok(None);
        }
        match self.stream.poll() {
            Ok(Some(StreamEvent::Record(_))) => Ok(Some(ChangeEvent::Published)),
            Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) => Ok(Some(ChangeEvent::Reset)),
            Ok(None) => Ok(None),
            Err(error) => {
                self.ended.store(true, Ordering::Release);
                Err(error)
            }
        }
    }
    /// Wait for a committed invalidation or this domain owner's command wake.
    /// The subscription never consumes a semantic command or receipt.
    pub fn wait(&self, wake: &LocalStream) -> io::Result<bool> {
        loop {
            if self.changed()? {
                return Ok(true);
            }
            let mut fds = [
                PollFd::new(self.stream.readiness(), PollFlags::POLLIN),
                PollFd::new(wake.as_local_io(), PollFlags::POLLIN),
            ];
            poll(&mut fds, PollTimeout::NONE)?;
            if !fds[1].revents().unwrap_or(PollFlags::empty()).is_empty() {
                let mut buffer = [0; 64];
                match (&*wake).read(&mut buffer) {
                    Ok(0) => return Err(io::ErrorKind::BrokenPipe.into()),
                    Ok(_) => return Ok(false),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    pub fn changed(&self) -> io::Result<bool> {
        let mut changed = false;
        loop {
            match self.poll() {
                Ok(Some(_)) => changed = true,
                Ok(None) => return Ok(changed),
                Err(error) => return Err(error),
            }
        }
    }
}

pub fn notify(stream: &LocalStream) -> io::Result<()> {
    match (&*stream).write(&[1]) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error),
    }
}
