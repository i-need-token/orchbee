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

impl LocalClientOrigin {
    /// Register before the first domain read. A commit concurrent with that read remains dirty.
    pub(crate) fn watch_changes(&self) -> io::Result<ChangeSubscription> {
        use orchbee_workbench_protocol::{ProtocolVersion, SnapshotReadIntent, WorkbenchRequestId};
        let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("fffffffffffffffffffffffffffffffd")
                .expect("nonzero subscription identity"),
        });
        self.subscribe(&request, StreamMode::Invalidations)
            .map(ChangeSubscription)
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

pub(crate) struct ChangeSubscription(Subscription);
impl ChangeSubscription {
    pub(crate) fn changed(&self) -> io::Result<bool> {
        let mut changed = false;
        while self.0.poll()?.is_some() {
            changed = true;
        }
        Ok(changed)
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
