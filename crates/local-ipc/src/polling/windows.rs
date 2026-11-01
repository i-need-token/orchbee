use crate::{
    LocalListener, LocalStream, Readiness,
    windows::ReadWait,
    windows_native::{MAXIMUM_WAIT_EVENTS, wait_any},
};
use std::{
    io,
    ops::{BitOr, BitOrAssign},
    time::{Duration, Instant},
};

/// A borrowed named-pipe source. Never an OS file descriptor.
#[derive(Clone, Copy)]
pub enum LocalIo<'a> {
    Stream(&'a LocalStream),
    Listener(&'a LocalListener),
    Wake(&'a std::sync::atomic::AtomicU8, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollFlags(u8);
impl PollFlags {
    pub const POLLIN: Self = Self(1);
    pub const POLLOUT: Self = Self(2);
    pub const POLLHUP: Self = Self(4);
    pub const POLLERR: Self = Self(8);
    pub const POLLNVAL: Self = Self(16);
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}
impl BitOr for PollFlags {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl BitOrAssign for PollFlags {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PollTimeout(Option<Duration>);
impl PollTimeout {
    pub const NONE: Self = Self(None);
    pub const ZERO: Self = Self(Some(Duration::ZERO));
    pub const MAX: Self = Self(Some(Duration::from_millis(i32::MAX as u64)));
}
impl TryFrom<u128> for PollTimeout {
    type Error = io::Error;
    fn try_from(value: u128) -> io::Result<Self> {
        if value > i32::MAX as u128 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(Self(Some(Duration::from_millis(value as u64))))
    }
}
impl TryFrom<Duration> for PollTimeout {
    type Error = io::Error;
    fn try_from(value: Duration) -> io::Result<Self> {
        Self::try_from(
            value
                .as_millis()
                .saturating_add(u128::from(!value.subsec_nanos().is_multiple_of(1_000_000))),
        )
    }
}

pub struct PollFd<'a> {
    source: LocalIo<'a>,
    interest: PollFlags,
    events: PollFlags,
}
impl<'a> PollFd<'a> {
    pub fn new(source: LocalIo<'a>, interest: PollFlags) -> Self {
        Self {
            source,
            interest,
            events: PollFlags::empty(),
        }
    }
    pub fn revents(&self) -> Option<PollFlags> {
        Some(self.events)
    }
}

/// Waits on kernel events for peer bytes and closure. A source without one (a blocked writer, a
/// listener, a wake flag) keeps a bounded cadence. Never waits on FlushFileBuffers.
pub fn poll(sources: &mut [PollFd<'_>], timeout: PollTimeout) -> io::Result<usize> {
    const CADENCE: Duration = Duration::from_millis(2);
    let started = Instant::now();
    loop {
        let mut count = 0;
        for source in &mut *sources {
            source.events = PollFlags::empty();
            let ready = match source.source {
                LocalIo::Stream(stream) => stream.readiness(),
                LocalIo::Listener(listener) => listener.readiness(),
                LocalIo::Wake(flag, signalled) => Ok(
                    if flag.load(std::sync::atomic::Ordering::Acquire) == signalled {
                        Readiness::Readable
                    } else {
                        Readiness::Pending
                    },
                ),
            };
            let ready = match ready {
                Ok(ready) => ready,
                Err(_) => {
                    // A failed connection must not terminate unrelated peers
                    // sharing the listener's poll set.
                    source.events = PollFlags::POLLERR;
                    count += 1;
                    continue;
                }
            };
            if ready == Readiness::Closed {
                source.events |= PollFlags::POLLHUP;
            }
            if ready == Readiness::Readable && source.interest.contains(PollFlags::POLLIN) {
                source.events |= PollFlags::POLLIN;
            }
            // Advisory, since protocol receipts determine delivery. A reported full buffer
            // admits one attempt per cadence, because only the attempt can refute that report.
            if ready != Readiness::Closed
                && source.interest.contains(PollFlags::POLLOUT)
                && let LocalIo::Stream(stream) = source.source
            {
                match stream.writable_advisory() {
                    Ok(true) => source.events |= PollFlags::POLLOUT,
                    Ok(false) => {}
                    Err(_) => source.events = PollFlags::POLLERR,
                }
            }
            count += usize::from(!source.events.is_empty());
        }
        if count > 0 || timeout.0.is_some_and(|budget| started.elapsed() >= budget) {
            return Ok(count);
        }
        let mut events = Vec::with_capacity(sources.len());
        let mut cadence = sources.len() > MAXIMUM_WAIT_EVENTS;
        if !cadence {
            for source in &*sources {
                let LocalIo::Stream(stream) = source.source else {
                    cadence = true;
                    continue;
                };
                cadence |= source.interest.contains(PollFlags::POLLOUT);
                match stream.read_wait() {
                    Ok(ReadWait::Event(event)) => events.push(event),
                    // The next probe reports a failure as this source's own event.
                    Ok(ReadWait::Changed | ReadWait::Unwatchable) | Err(_) => cadence = true,
                }
            }
        }
        let remaining = timeout
            .0
            .map(|budget| budget.saturating_sub(started.elapsed()));
        wait_any(
            &events,
            if cadence || events.is_empty() {
                Some(remaining.map_or(CADENCE, |remaining| remaining.min(CADENCE)))
            } else {
                remaining
            },
        )?;
    }
}
