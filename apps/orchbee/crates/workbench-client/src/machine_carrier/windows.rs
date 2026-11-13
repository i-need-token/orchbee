//! Windows carrier endpoints: a private same-user pipe whose every accepted connection is one hub
//! `session` channel, copied up and down on two threads of its own. No thread holds the hub
//! owner's lock while it reads or writes the pipe, and each channel's credit returns only as its
//! own pipe takes the bytes.
use super::{Gate, StartedCarrier};
use crate::{
    machine_hub::{ChannelDelivery, ChannelHandle, HubClient},
    machines::ProfileId,
};
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_machine_protocol::{MachineId, hub::Channel};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// The hub owner the carrier opens its channels on.
pub(crate) type Hub = Arc<HubClient>;

/// Connections one endpoint carries at once; a further one closes at once.
const CONNECTIONS: usize = 16;
/// How often the accept loop looks for a connection and for its stop flag.
const ACCEPT_PAUSE: Duration = Duration::from_millis(10);
/// How often a connection's threads look for the stop flag while they wait.
const POLL: Duration = Duration::from_millis(100);
/// How long opening a channel may wait for the owner to re-attach.
const OPEN: Duration = Duration::from_secs(10);
/// How long the channel may still deliver after the local Client closed its pipe.
const LINGER: Duration = Duration::from_secs(5);
/// The most bytes one read of the pipe takes.
const READ_BYTES: usize = 64 * 1024;

pub(super) fn start(hub: &Hub, machine: &ProfileId, online: Gate) -> io::Result<StartedCarrier> {
    let machine = MachineId::new(machine.as_str())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let id = ProfileId::generate()
        .map_err(|_| io::Error::other("machine endpoint identity unavailable"))?;
    let endpoint = PathBuf::from(format!(r"\\.\pipe\orchbee-machine-{}", id.as_str()));
    let listener = LocalListener::bind(&endpoint)?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let task = {
        let hub = hub.clone();
        let stop = stop.clone();
        thread::Builder::new()
            .name("orchbee-machine-carrier".into())
            .spawn(move || {
                let open = Arc::new(AtomicUsize::new(0));
                while !stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, ())) => {
                            // A Machine that is not online, or a full endpoint, closes only this
                            // candidate; a Client that keeps reconnecting costs no hub request.
                            if online() && open.load(Ordering::Acquire) < CONNECTIONS {
                                let lane = Lane {
                                    hub: hub.clone(),
                                    channel: Channel::Session {
                                        machine: machine.clone(),
                                    },
                                    stop: stop.clone(),
                                    _counted: Counted::new(&open),
                                };
                                // A thread that cannot start drops the lane and its count.
                                let _started = thread::Builder::new()
                                    .name("orchbee-machine-lane".into())
                                    .spawn(move || lane.carry(stream));
                            }
                        }
                        // No candidate, or one whose admission failed: it closed by itself.
                        Err(_) => thread::sleep(ACCEPT_PAUSE),
                    }
                }
                Ok(())
            })?
    };
    Ok(StartedCarrier {
        endpoint,
        stop,
        task,
    })
}

/// One open connection's place among the endpoint's connections.
struct Counted(Arc<AtomicUsize>);

impl Counted {
    fn new(open: &Arc<AtomicUsize>) -> Self {
        open.fetch_add(1, Ordering::AcqRel);
        Self(open.clone())
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Lane {
    hub: Hub,
    channel: Channel,
    stop: Arc<AtomicBool>,
    _counted: Counted,
}

impl Lane {
    /// Opens the channel and copies both ways until either side ends or the endpoint stops.
    fn carry(self, stream: LocalStream) {
        // An unopened channel closes the pipe, which the Client reads as a lost connection.
        let Ok(handle) = self.hub.open_channel(&self.channel, Instant::now() + OPEN) else {
            return;
        };
        if stream.set_read_timeout(Some(POLL)).is_err()
            || stream.set_write_timeout(Some(POLL)).is_err()
        {
            self.hub.cancel(handle);
            return;
        }
        let local_ended = Arc::new(AtomicBool::new(false));
        let up = {
            let hub = self.hub.clone();
            let stream = stream.clone();
            let stop = self.stop.clone();
            let local_ended = local_ended.clone();
            thread::Builder::new()
                .name("orchbee-machine-lane-up".into())
                .spawn(move || up(&hub, handle, &stream, &stop, &local_ended))
        };
        let Ok(up) = up else {
            self.hub.cancel(handle);
            let _closed = stream.shutdown(Shutdown::Both);
            return;
        };
        down(&self.hub, handle, &stream, &self.stop, &local_ended);
        // Ends the up thread's read too.
        let _closed = stream.shutdown(Shutdown::Both);
        let _joined = up.join();
    }
}

/// The pipe's bytes into the channel under its credit; the pipe's end sends `end`.
fn up(
    hub: &HubClient,
    handle: ChannelHandle,
    mut stream: &LocalStream,
    stop: &AtomicBool,
    local_ended: &AtomicBool,
) {
    let mut buffer = vec![0; READ_BYTES];
    loop {
        if stop.load(Ordering::Acquire) {
            hub.cancel(handle);
            return;
        }
        let read = match stream.read(&mut buffer) {
            Ok(0) => {
                // The channel may already be over; its deliveries tell the down thread how.
                let _ended = hub.end(handle);
                local_ended.store(true, Ordering::Release);
                return;
            }
            Ok(read) => read,
            Err(error) if waiting(&error) => continue,
            Err(_) => {
                hub.cancel(handle);
                let _closed = stream.shutdown(Shutdown::Both);
                return;
            }
        };
        let mut sent = 0;
        while sent < read {
            if stop.load(Ordering::Acquire) {
                hub.cancel(handle);
                return;
            }
            match hub.write(handle, &buffer[sent..read], Instant::now() + POLL) {
                Ok(committed) => sent += committed,
                // The channel is over: the down thread reads its last delivery.
                Err(_) => return,
            }
        }
    }
}

/// The channel's deliveries into the pipe; taking data returns its credit. Returns when the channel
/// is over, the pipe fails, the endpoint stops, or the local Client closed its pipe and the far end
/// did not close the channel within [`LINGER`].
fn down(
    hub: &HubClient,
    handle: ChannelHandle,
    mut stream: &LocalStream,
    stop: &AtomicBool,
    local_ended: &AtomicBool,
) {
    let mut ended_at = None;
    loop {
        if stop.load(Ordering::Acquire) {
            hub.cancel(handle);
            return;
        }
        if local_ended.load(Ordering::Acquire)
            && ended_at.get_or_insert_with(Instant::now).elapsed() >= LINGER
        {
            hub.cancel(handle);
            return;
        }
        match hub.receive(handle, Instant::now() + POLL) {
            None | Some(ChannelDelivery::Accepted | ChannelDelivery::Ended) => {}
            Some(ChannelDelivery::Data(bytes)) => {
                if !deliver(&mut stream, &bytes, stop) {
                    hub.cancel(handle);
                    return;
                }
            }
            Some(
                ChannelDelivery::Closed(_) | ChannelDelivery::Reset(_) | ChannelDelivery::Lost,
            ) => {
                return;
            }
        }
    }
}

/// Writes all of `bytes` to the pipe; false when the pipe failed or the endpoint stopped.
fn deliver(stream: &mut &LocalStream, mut bytes: &[u8], stop: &AtomicBool) -> bool {
    while !bytes.is_empty() {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        match stream.write(bytes) {
            Ok(0) => return false,
            Ok(written) => bytes = &bytes[written..],
            Err(error) if waiting(&error) => {}
            Err(_) => return false,
        }
    }
    true
}

/// A read or write that found nothing to do within its budget.
fn waiting(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}
