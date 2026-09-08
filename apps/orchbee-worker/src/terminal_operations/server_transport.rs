use std::{
    io::{self, Read, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, ReadMessageError, WriteMessageError, read_message, write_message,
};

use crate::host_connection::{HostBrokerConnection, consume_event};

const RECORD_BUDGET: Duration = Duration::from_secs(2);

pub(super) fn receive(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    let mut transport = ServerRecord {
        server,
        host,
        deadline: None,
        host_closed: false,
    };
    // Idle has no deadline. Once a record begins, every fragment shares one budget.
    transport.wait(PollFlags::POLLIN)?;
    read_message(&mut transport)
}

pub(super) fn send(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    result: &CapabilityNodeMessage,
) -> Result<bool, WriteMessageError> {
    let mut transport = ServerRecord {
        server,
        host,
        deadline: Some(Instant::now() + RECORD_BUDGET),
        host_closed: host.is_none(),
    };
    write_message(&mut transport, result)?;
    Ok(transport.host_closed)
}

struct ServerRecord<'a> {
    server: &'a UnixStream,
    host: Option<&'a HostBrokerConnection>,
    deadline: Option<Instant>,
    host_closed: bool,
}

impl ServerRecord<'_> {
    fn wait(&mut self, interest: PollFlags) -> io::Result<()> {
        loop {
            let timeout = match self.deadline {
                None => PollTimeout::NONE,
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    PollTimeout::try_from(remaining.as_millis().saturating_add(1))
                        .map_err(io::Error::other)?
                }
            };
            let mut descriptors = vec![PollFd::new(
                self.server.as_fd(),
                interest | PollFlags::POLLIN,
            )];
            if let Some(host) = self.host {
                descriptors.push(PollFd::new(host.stream().as_fd(), PollFlags::POLLIN));
            }
            match poll(&mut descriptors, timeout) {
                Ok(_) => {}
                Err(Errno::EINTR) => continue,
                Err(error) => return Err(error.into()),
            }
            let events = descriptors[0].revents().ok_or_else(super::peer_closed)?;
            if events.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL)
                || (interest == PollFlags::POLLOUT && events.contains(PollFlags::POLLIN))
            {
                return Err(super::peer_closed());
            }
            if self.deadline.is_none() && events.contains(PollFlags::POLLIN) {
                self.deadline = Some(Instant::now() + RECORD_BUDGET);
            }
            if descriptors
                .get(1)
                .is_some_and(|host| host.revents().is_none_or(|events| !events.is_empty()))
            {
                let event = self.host.ok_or_else(super::peer_closed).and_then(|host| {
                    let event_deadline = Instant::now() + Duration::from_secs(1);
                    let deadline = event_deadline.min(self.deadline.unwrap_or(event_deadline));
                    consume_event(host, self.server, deadline)
                });
                if event.is_err() {
                    if interest == PollFlags::POLLIN {
                        return Err(super::peer_closed());
                    }
                    // A known result still gets its bounded delivery attempt after
                    // Host loss. Admission stops as soon as that write finishes.
                    self.host = None;
                    self.host_closed = true;
                }
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(io::ErrorKind::TimedOut.into());
            }
            if events.contains(interest) {
                return Ok(());
            }
            if events.contains(PollFlags::POLLHUP) {
                return Err(super::peer_closed());
            }
        }
    }
}

impl Read for ServerRecord<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.wait(PollFlags::POLLIN)?;
            match self.server.read(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                result => return result,
            }
        }
    }
}

impl Write for ServerRecord<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.wait(PollFlags::POLLOUT)?;
            match self.server.write(bytes) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
