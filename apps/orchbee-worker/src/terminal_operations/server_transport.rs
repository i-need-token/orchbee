//! Reads and writes one Server protocol record under bounded deadlines while continuing to
//! consume admitted Host events. Host events are read as they arrive, never waited for, so a Host
//! pausing inside one cannot hold a Server record; a Host stopped inside one past the exchange
//! bound is lost. It owns transport progress without dispatching requests, issuing
//! authorization, or recovering a lost connection.

use orchbee_local_ipc::AsLocalIo;
use orchbee_local_ipc::LocalStream as UnixStream;
use std::{
    io::{self, Read, Write},
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, ReadMessageError, TerminalViewerRecordReady, WriteMessageError,
    encode_message, read_framed_message, write_message,
};
use orchbee_local_ipc::{PollFd, PollFlags, PollTimeout, poll};

use crate::host_connection::{HostBrokerConnection, drain_events};

const RECORD_BUDGET: Duration = Duration::from_secs(2);
/// Whole Host events read per wake-up before the Server is looked at again.
const EVENTS_PER_WAKE: usize = 64;

pub(super) fn receive(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
) -> Result<CapabilityNodeMessage, ReadMessageError> {
    receive_timed(server, host).map(|(request, _readable)| request)
}

/// A request with the instant its first bytes became readable, which starts its bound.
pub(super) fn receive_timed(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
) -> Result<(CapabilityNodeMessage, Instant), ReadMessageError> {
    receive_until_timed(server, host, None)?.ok_or_else(|| super::peer_closed().into())
}

/// Only an idle cutoff can return None. Once input is readable, a partial record
/// remains a fatal record-budget failure and cannot be discarded as lease expiry. A whole frame
/// this Worker cannot decode is answered as unreadable here, and the wait goes on to the next
/// request.
pub(super) fn receive_until_timed(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    idle_deadline: Option<Instant>,
) -> Result<Option<(CapabilityNodeMessage, Instant)>, ReadMessageError> {
    loop {
        let mut transport = ServerRecord {
            server,
            host,
            deadline: idle_deadline,
            host_closed: false,
            readable: None,
        };
        // Idle has no deadline. Once a record begins, every fragment shares one budget.
        if let Err(error) = transport.wait(PollFlags::POLLIN) {
            if error.kind() == io::ErrorKind::TimedOut
                && idle_deadline.is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Ok(None);
            }
            return Err(error.into());
        }
        let readable = transport.readable.unwrap_or_else(Instant::now);
        transport.deadline = Some(readable + RECORD_BUDGET);
        match read_framed_message(&mut transport)? {
            Ok(request) => return Ok(Some((request, readable))),
            // The frame ended where its header said, so the stream sits at the next one: one
            // record fails, not the connection.
            Err(_undecodable) => answer_unreadable(server, host)?,
        }
    }
}

/// Answers a whole frame this Worker could not decode, such as a request kind newer than it. The
/// frame names no identity to trust, so the answer matches by position: the Server waits on one
/// request at a time, and this is the only one this Worker has not answered.
fn answer_unreadable(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
) -> Result<(), ReadMessageError> {
    crate::diagnostic_events::server_unexpected(
        crate::diagnostic_events::ServerUnexpected::Unreadable,
    );
    match send(
        server,
        host,
        &CapabilityNodeMessage::WorkerRequestUnreadable,
    ) {
        // The Host ended while the answer was written, which a receive treats as Host loss.
        // Before the first Launch there is no Host, and the send reports it as closed.
        Ok(true) if host.is_some() => return Err(super::peer_closed().into()),
        Ok(_) => {}
        Err(WriteMessageError::Io(error)) => return Err(error.into()),
        Err(WriteMessageError::Protocol(error)) => return Err(ReadMessageError::Protocol(error)),
    }
    if let Some(host) = host {
        host.pacing
            .set_site(crate::diagnostic_events::Site::ServerReceive);
    }
    Ok(())
}

pub(super) fn send(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    result: &CapabilityNodeMessage,
) -> Result<bool, WriteMessageError> {
    send_until(server, host, result, Instant::now() + RECORD_BUDGET)
}

/// A handoff lease can shorten, but never extend, the existing record budget.
pub(super) fn send_until(
    server: &UnixStream,
    host: Option<&HostBrokerConnection>,
    result: &CapabilityNodeMessage,
    deadline: Instant,
) -> Result<bool, WriteMessageError> {
    let deadline = deadline.min(Instant::now() + RECORD_BUDGET);
    if let Some(host) = host {
        host.pacing
            .set_site(crate::diagnostic_events::Site::ServerSend);
    }
    let mut transport = ServerRecord {
        server,
        host,
        deadline: Some(deadline),
        host_closed: host.is_none(),
        readable: None,
    };
    write_message(&mut transport, result)?;
    if Instant::now() >= deadline {
        return Err(io::Error::from(io::ErrorKind::TimedOut).into());
    }
    Ok(transport.host_closed)
}

struct ServerRecord<'a> {
    server: &'a UnixStream,
    host: Option<&'a HostBrokerConnection>,
    deadline: Option<Instant>,
    host_closed: bool,
    /// When the Server's bytes were first seen readable.
    readable: Option<Instant>,
}

impl ServerRecord<'_> {
    /// Whether the Server's next request is already readable, which starts its bound now.
    fn server_readable(&mut self) -> io::Result<bool> {
        let mut descriptors = [PollFd::new(self.server.as_local_io(), PollFlags::POLLIN)];
        match poll(&mut descriptors, PollTimeout::ZERO) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(false),
            Err(error) => return Err(error),
        }
        let readable = descriptors[0]
            .revents()
            .is_some_and(|events| events.contains(PollFlags::POLLIN));
        if readable {
            self.readable.get_or_insert_with(Instant::now);
        }
        Ok(readable)
    }

    /// Called only while idle between requests, with no record open in either direction, so a
    /// hint cannot split one: viewer readiness first, then at most one title note. A prepared
    /// handoff waits under a deadline and therefore sends none. A request that becomes readable
    /// while hints are written starts its bound then, so hint time counts against it.
    fn send_ready_hints(&mut self) -> io::Result<()> {
        let Some(host) = self.host else {
            return Ok(());
        };
        let ready = host.viewers.borrow_mut().take_ready();
        let titles = host.take_title_note();
        if ready.is_empty() && titles.is_none() {
            return Ok(());
        }
        // Hints stay far below the transport buffer, so this deadline only ends a stalled Server.
        let deadline = Instant::now() + RECORD_BUDGET;
        let hints = ready
            .into_iter()
            .map(|attachment_id| {
                CapabilityNodeMessage::TerminalViewerRecordReady(TerminalViewerRecordReady {
                    attachment_id,
                })
            })
            .chain(titles.map(CapabilityNodeMessage::TerminalTitlesChanged));
        for hint in hints {
            let record = encode_message(&hint).map_err(io::Error::other)?;
            write_hint(self.server, &record, deadline, &mut self.readable)?;
        }
        Ok(())
    }

    fn wait(&mut self, interest: PollFlags) -> io::Result<()> {
        loop {
            if self.deadline.is_none() && interest == PollFlags::POLLIN {
                // A request already readable keeps the pending hints for the next idle wait.
                if !self.server_readable()? {
                    self.send_ready_hints()?;
                }
                // One hard bound from the first readable byte, whatever the wait did since.
                if let Some(readable) = self.readable {
                    self.deadline = Some(readable + RECORD_BUDGET);
                }
            }
            let now = Instant::now();
            if self.deadline.is_some_and(|deadline| now >= deadline) {
                return Err(io::ErrorKind::TimedOut.into());
            }
            // A Host record begun but not finished also wakes the wait when its bound passes.
            let stall = self.host.and_then(HostBrokerConnection::partial_deadline);
            let wake = match (self.deadline, stall) {
                (Some(deadline), Some(stall)) => Some(deadline.min(stall)),
                (deadline, stall) => deadline.or(stall),
            };
            let timeout = match wake {
                None => PollTimeout::NONE,
                Some(wake) => PollTimeout::try_from(
                    wake.saturating_duration_since(now)
                        .as_millis()
                        .saturating_add(1),
                )
                .map_err(io::Error::other)?,
            };
            let mut descriptors = vec![PollFd::new(
                self.server.as_local_io(),
                interest | PollFlags::POLLIN,
            )];
            if let Some(host) = self.host {
                descriptors.push(PollFd::new(host.stream().as_local_io(), PollFlags::POLLIN));
            }
            match poll(&mut descriptors, timeout) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
            let events = descriptors[0].revents().ok_or_else(super::peer_closed)?;
            if events.intersects(PollFlags::POLLERR | PollFlags::POLLNVAL)
                || (interest == PollFlags::POLLOUT && events.contains(PollFlags::POLLIN))
            {
                return Err(super::peer_closed());
            }
            if events.contains(PollFlags::POLLIN) {
                let now = Instant::now();
                if interest == PollFlags::POLLIN {
                    self.readable.get_or_insert(now);
                }
                if self.deadline.is_none() {
                    self.deadline = Some(now + RECORD_BUDGET);
                }
            }
            if let Some(host) = self.host {
                let ready = descriptors
                    .get(1)
                    .is_some_and(|host| host.revents().is_none_or(|events| !events.is_empty()));
                let read = if ready {
                    drain_events(host, EVENTS_PER_WAKE)
                } else {
                    Ok(())
                };
                let read = read.and_then(|()| match host.partial_deadline() {
                    Some(stall) if Instant::now() >= stall => Err(host.stalled()),
                    _ => Ok(()),
                });
                if read.is_err() {
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

/// Unlike a result, a hint races the Server's next request, so readable input is no error here;
/// the first time it is seen readable is stamped in `readable`, which starts that request's bound.
fn write_hint(
    server: &UnixStream,
    mut record: &[u8],
    deadline: Instant,
    readable: &mut Option<Instant>,
) -> io::Result<()> {
    let mut writer = server;
    while !record.is_empty() {
        match writer.write(record) {
            Ok(0) => return Err(super::peer_closed()),
            Ok(written) => record = &record[written..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                let timeout = PollTimeout::try_from(remaining.as_millis().saturating_add(1))
                    .map_err(io::Error::other)?;
                // Readable input stays readable, so it is watched only until first seen.
                let interest = if readable.is_none() {
                    PollFlags::POLLOUT | PollFlags::POLLIN
                } else {
                    PollFlags::POLLOUT
                };
                let mut descriptors = [PollFd::new(server.as_local_io(), interest)];
                match poll(&mut descriptors, timeout) {
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error),
                }
                if descriptors[0]
                    .revents()
                    .is_some_and(|events| events.contains(PollFlags::POLLIN))
                {
                    readable.get_or_insert_with(Instant::now);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
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
