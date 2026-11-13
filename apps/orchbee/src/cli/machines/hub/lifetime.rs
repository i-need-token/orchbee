//! The hub's lifetime as a state machine over its holders, with the time an argument: `held` while a
//! `presence` connection is attached or the last Session read found a live Session, `expiring`
//! from the moment the hub learned that nothing holds it, and `closing` once and for ever, entered
//! by `drain` or by a Session read at or after the expiry deadline that finds no live Session. A
//! `connect` answered `online` while `expiring` starts the countdown again.
//! `docs/machines.md#lifetime` owns the rules; the supervisor alone reads the clocks.

use orchbee_machine_protocol::{MAXIMUM_COUNT, Time, hub::HubState};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The time between Session reads while no `presence` connection is attached.
pub(super) const SESSION_READ_INTERVAL: Duration = Duration::from_secs(30);

/// One reading of both clocks: monotonic for deadlines, the wall clock for `expiresAt`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Moment {
    pub(super) at: Instant,
    /// Milliseconds since the Unix epoch.
    pub(super) wall: u64,
}

impl Moment {
    pub(super) fn now() -> Self {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        Self {
            at: Instant::now(),
            wall,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Held,
    Expiring { deadline: Instant, expires_at: Time },
    Closing,
}

/// When the next Session read is due.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NextRead {
    /// None runs while a `presence` connection is attached, nor while one runs.
    None,
    /// The last `presence` connection left: read at once.
    Owed,
    At(Instant),
}

/// When the supervisor reads the Sessions next.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadDue {
    Now,
    At(Instant),
}

/// The leave a read started after: a read whose result arrives after a later leave is stale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadTicket(u64);

#[derive(Debug)]
pub(super) struct Lifetime {
    expiry: Duration,
    presence: usize,
    phase: Phase,
    next_read: NextRead,
    /// Counts the leaves of the last `presence` connection.
    leaves: u64,
}

impl Lifetime {
    /// The lifetime after the first Session read at `now`, which found a live Session or not.
    pub(super) fn start(expiry: Duration, live: bool, now: Moment) -> Self {
        Self {
            expiry,
            presence: 0,
            phase: if live {
                Phase::Held
            } else {
                expiring(expiry, now)
            },
            next_read: NextRead::At(now.at + SESSION_READ_INTERVAL),
            leaves: 0,
        }
    }

    /// The state a `hub` event and `attach-result` carry.
    pub(super) fn state(&self) -> HubState {
        match self.phase {
            Phase::Held => HubState::Held,
            Phase::Expiring { expires_at, .. } => HubState::Expiring { expires_at },
            Phase::Closing => HubState::Closing,
        }
    }

    pub(super) fn is_closing(&self) -> bool {
        self.phase == Phase::Closing
    }

    /// A connection attached with `presence`, outside `closing`; true when the state changed.
    pub(super) fn attach(&mut self) -> bool {
        if self.is_closing() {
            return false;
        }
        self.presence += 1;
        self.next_read = NextRead::None;
        let changed = matches!(self.phase, Phase::Expiring { .. });
        self.phase = Phase::Held;
        changed
    }

    /// A `presence` connection left or dropped `presence`. The last one leaves the hub `held` until
    /// the read it owes finds no live Session.
    pub(super) fn detach(&mut self) {
        self.presence = self.presence.saturating_sub(1);
        if self.presence == 0 && !self.is_closing() {
            self.leaves += 1;
            self.next_read = NextRead::Owed;
        }
    }

    /// When the next Session read is due, if one is. The expiry deadline is one: the hub closes
    /// only after a read there finds no live Session.
    pub(super) fn read_due(&self) -> Option<ReadDue> {
        if self.presence > 0 || self.is_closing() {
            return None;
        }
        match self.next_read {
            NextRead::None => None,
            NextRead::Owed => Some(ReadDue::Now),
            NextRead::At(at) => Some(ReadDue::At(
                self.deadline().map_or(at, |deadline| deadline.min(at)),
            )),
        }
    }

    pub(super) fn begin_read(&mut self) -> ReadTicket {
        self.next_read = NextRead::None;
        ReadTicket(self.leaves)
    }

    /// Applies a read that ended at `now`, unless a `presence` connection is attached, the hub is
    /// closing, or a later leave owes a newer read; true when the state changed. A read that finds
    /// no live Session at or after the expiry deadline enters `closing`.
    pub(super) fn finish_read(&mut self, ticket: ReadTicket, live: bool, now: Moment) -> bool {
        if self.presence > 0 || self.is_closing() || ticket.0 != self.leaves {
            return false;
        }
        self.next_read = NextRead::At(now.at + SESSION_READ_INTERVAL);
        match (self.phase, live) {
            (Phase::Held, false) => {
                self.phase = expiring(self.expiry, now);
                true
            }
            (Phase::Expiring { .. }, true) => {
                self.phase = Phase::Held;
                true
            }
            (Phase::Expiring { deadline, .. }, false) if now.at >= deadline => {
                self.phase = Phase::Closing;
                true
            }
            _ => false,
        }
    }

    /// The expiry deadline while `expiring`.
    pub(super) fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Expiring { deadline, .. } => Some(deadline),
            Phase::Held | Phase::Closing => None,
        }
    }

    /// A `connect` was answered `online` at `now`: while `expiring`, the countdown starts again from
    /// `now`; true when the state changed. `held` and `closing` are left as they are.
    pub(super) fn connected(&mut self, now: Moment) -> bool {
        if !matches!(self.phase, Phase::Expiring { .. }) {
            return false;
        }
        let restarted = expiring(self.expiry, now);
        let changed = restarted != self.phase;
        self.phase = restarted;
        changed
    }

    /// Enters `closing` on `drain`; true when it was not closing already.
    pub(super) fn drain(&mut self) -> bool {
        let changed = !self.is_closing();
        self.phase = Phase::Closing;
        changed
    }
}

fn expiring(expiry: Duration, now: Moment) -> Phase {
    let milliseconds = u64::try_from(expiry.as_millis()).unwrap_or(u64::MAX);
    let wall = now.wall.saturating_add(milliseconds);
    Phase::Expiring {
        deadline: now.at + expiry,
        expires_at: Time::new(wall.min(MAXIMUM_COUNT)).expect("bounded by the count range"),
    }
}

#[cfg(test)]
mod tests;
