//! One deadline for same-user standby admission and exact handoff-ticket authentication.
use crate::host_connection::{HostBrokerConnection, consume_event};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, WorkerHandoffPeer, WorkerHandoffRefusal as Refusal,
    read_message, write_message,
};
use orchbee_local_ipc::{AsLocalIo, LocalStream, PollFd, PollFlags, PollTimeout, poll};
use std::{
    io::{self, Read, Write},
    path::Path,
    time::Instant,
};

pub(super) fn connect(
    runtime: &Path,
    peer: WorkerHandoffPeer,
    old: &LocalStream,
    host: Option<&HostBrokerConnection>,
    deadline: Instant,
) -> Result<LocalStream, Refusal> {
    let endpoint = orchbee_local_runtime::ModeCheckedLocalSessionRendezvous::handoff_endpoint_path(
        runtime,
        peer.target.handoff_id.as_bytes(),
    );
    let stream =
        LocalStream::connect_before(&endpoint, deadline).map_err(|_| Refusal::TargetUnavailable)?;
    orchbee_local_peer_credentials::require_same_effective_user(&stream)
        .map_err(|_| Refusal::Authentication)?;
    stream
        .set_nonblocking(true)
        .map_err(|_| Refusal::TargetUnavailable)?;
    let mut transport = Handshake {
        stream,
        old,
        host,
        deadline,
    };
    write_message(&mut transport, &Message::WorkerHandoffHello(peer))
        .map_err(|_| Refusal::TargetUnavailable)?;
    match read_message(&mut transport) {
        Ok(Message::WorkerHandoffReady(actual)) if actual == peer => {}
        Ok(_) => return Err(Refusal::Authentication),
        Err(_) => return Err(Refusal::TargetUnavailable),
    }
    transport
        .remaining()
        .map_err(|_| Refusal::TargetUnavailable)?;
    // No extra record, partial tail or early Owned may cross this gate.
    if !is_idle(&transport.stream).map_err(|_| Refusal::TargetUnavailable)? {
        return Err(Refusal::Authentication);
    }
    Ok(transport.stream)
}

pub(super) fn is_idle(stream: &LocalStream) -> io::Result<bool> {
    let mut fd = [PollFd::new(stream.as_local_io(), PollFlags::POLLIN)];
    poll(&mut fd, PollTimeout::ZERO)?;
    Ok(fd[0].revents().is_some_and(|events| events.is_empty()))
}

struct Handshake<'a> {
    stream: LocalStream,
    old: &'a LocalStream,
    host: Option<&'a HostBrokerConnection>,
    deadline: Instant,
}

impl Handshake<'_> {
    fn remaining(&self) -> io::Result<std::time::Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::ErrorKind::TimedOut.into())
    }

    fn wait(&self, interest: PollFlags) -> io::Result<()> {
        loop {
            let timeout = PollTimeout::try_from(self.remaining()?.as_millis().saturating_add(1))
                .map_err(io::Error::other)?;
            let mut descriptors = vec![
                PollFd::new(self.stream.as_local_io(), interest),
                PollFd::new(self.old.as_local_io(), PollFlags::POLLIN),
            ];
            if let Some(host) = self.host {
                descriptors.push(PollFd::new(host.stream().as_local_io(), PollFlags::POLLIN));
            }
            match poll(&mut descriptors, timeout) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
                Ok(_) => {}
            }
            self.remaining()?;
            if descriptors[1]
                .revents()
                .is_none_or(|events| !events.is_empty())
            {
                return Err(io::Error::other(
                    "old Server changed during handoff preparation",
                ));
            }
            if descriptors
                .get(2)
                .is_some_and(|host| host.revents().is_none_or(|events| !events.is_empty()))
            {
                consume_event(self.host.expect("polled Host"), self.old, self.deadline)?;
            }
            let events = descriptors[0]
                .revents()
                .ok_or_else(super::super::peer_closed)?;
            if events.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
                return Err(super::super::peer_closed());
            }
            if events.contains(interest) {
                return Ok(());
            }
            if events.contains(PollFlags::POLLHUP) {
                return Err(super::super::peer_closed());
            }
        }
    }
}

impl Read for Handshake<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.wait(PollFlags::POLLIN)?;
            match self.stream.read(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}

impl Write for Handshake<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.wait(PollFlags::POLLOUT)?;
            match self.stream.write(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.remaining().map(|_| ())
    }
}
