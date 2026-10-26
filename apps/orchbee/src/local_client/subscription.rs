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
pub(crate) struct Request {
    stream: Subscription,
    deadline: Instant,
    clock: super::LocalOperationClock,
}
impl Request {
    pub(crate) fn poll(&self) -> Option<Result<WorkbenchResponse, bool>> {
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
    pub(crate) fn request(
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
    pub(crate) fn watch_changes(&self) -> io::Result<ChangeSubscription> {
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
    pub(crate) fn subscribe(
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

pub(crate) struct ChangeSubscription {
    stream: Subscription,
    ended: std::sync::atomic::AtomicBool,
}
impl ChangeSubscription {
    pub(crate) fn changed(&self) -> io::Result<bool> {
        use std::sync::atomic::Ordering;
        if self.ended.load(Ordering::Acquire) {
            return Ok(false);
        }
        let mut changed = false;
        loop {
            match self.stream.poll() {
                Ok(Some(_)) => changed = true,
                Ok(None) => return Ok(changed),
                Err(error) => {
                    self.ended.store(true, Ordering::Release);
                    return Err(error);
                }
            }
        }
    }
}

pub(crate) fn notify(stream: &LocalStream) -> io::Result<()> {
    match (&*stream).write(&[1]) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error),
    }
}

/// `None` means the domain mailbox changed; this owner drains only its own wake channel.
pub(crate) fn next_workbench(
    subscription: &Subscription,
    wake: &LocalStream,
) -> io::Result<Option<WorkbenchResponse>> {
    loop {
        match subscription.poll()? {
            Some(StreamEvent::Record(bytes)) => {
                return read_workbench_response(&mut Cursor::new(bytes))
                    .map(Some)
                    .map_err(io::Error::other);
            }
            Some(StreamEvent::Gap | StreamEvent::Disconnected) => continue,
            None => {}
        }
        let mut fds = [
            PollFd::new(subscription.readiness(), PollFlags::POLLIN),
            PollFd::new(wake.as_local_io(), PollFlags::POLLIN),
        ];
        poll(&mut fds, PollTimeout::NONE)?;
        let flags = fds[1].revents().unwrap_or(PollFlags::empty());
        if !flags.is_empty() {
            let mut buffer = [0; 64];
            match (&*wake).read(&mut buffer) {
                Ok(0) => return Err(io::ErrorKind::BrokenPipe.into()),
                Ok(_) => return Ok(None),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
    }
}
