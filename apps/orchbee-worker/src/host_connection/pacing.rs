//! Bounds Host exchanges below the Server's reply window and keeps the class of the Host failure
//! that ended the connection, so a slow Host is waited for and only a silent one is lost.
//!
//! The request bound is a start limit: an exchange starts only if its whole bound ends within the
//! request bound, and once started it keeps that whole bound. Work that no longer fits is refused
//! or left to a later cleanup without asking the Host, so it never counts against the Host.

use std::{
    cell::Cell,
    error::Error,
    fmt, io,
    time::{Duration, Instant},
};

use super::PeerFailure;
use crate::diagnostic_events::Site;

/// One Host exchange, its request written and its answer read, ends within this bound.
pub(crate) const EXCHANGE_BOUND: Duration = Duration::from_secs(10);
/// Every Host exchange of one Server request ends within this bound from when the request's first
/// bytes became readable. With the two-second result write that leaves the Server's
/// twenty-second reply window a three-second margin; with the exchange bound it means no exchange
/// starts later than five seconds after the request became readable.
pub(crate) const REQUEST_BOUND: Duration = Duration::from_secs(15);
/// An exchange that succeeds after longer than this is recorded as slow.
pub(crate) const SLOW_EXCHANGE: Duration = Duration::from_secs(1);

/// The failure of one Host exchange: which peer failed, whether the bound passed, and how long the
/// exchange waited within which bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HostFailure {
    pub(crate) class: PeerFailure,
    pub(crate) deadline: bool,
    pub(crate) waited: Duration,
    pub(crate) budget: Duration,
}

impl HostFailure {
    /// The I/O error that carries this failure, with the kind that names its class.
    pub(crate) fn into_io(self) -> io::Error {
        let kind = match (self.class, self.deadline) {
            (_, true) => io::ErrorKind::TimedOut,
            (PeerFailure::Protocol, false) => io::ErrorKind::InvalidData,
            (PeerFailure::Host | PeerFailure::Server, false) => io::ErrorKind::ConnectionAborted,
        };
        io::Error::new(kind, self)
    }

    /// The Host failure an error carries, directly or wrapped in an I/O error.
    pub(crate) fn find(error: &(dyn Error + 'static)) -> Option<Self> {
        let mut current = Some(error);
        while let Some(error) = current {
            if let Some(failure) = error.downcast_ref::<Self>() {
                return Some(*failure);
            }
            if let Some(inner) = error
                .downcast_ref::<io::Error>()
                .and_then(io::Error::get_ref)
            {
                current = Some(inner);
                continue;
            }
            current = error.source();
        }
        None
    }
}

impl fmt::Display for HostFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match (self.class, self.deadline) {
            (_, true) => "Host exchange passed its bound",
            (PeerFailure::Host, false) => "Host connection ended",
            (PeerFailure::Protocol, false) => "Host broke the terminal protocol",
            (PeerFailure::Server, false) => "Server connection ended during a Host exchange",
        })
    }
}

impl Error for HostFailure {}

/// An exchange the request bound left no room for. Nothing was written, so the Host is not at
/// fault and no failure is recorded: the caller answers a contained refusal or leaves the work to
/// a later cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NotStarted;

impl NotStarted {
    /// Whether an operation error is this refusal to start, which the operation contains.
    pub(crate) fn is(error: &(dyn Error + 'static)) -> bool {
        error.downcast_ref::<Self>().is_some()
    }

    /// An operation's result with this refusal split out as `None`.
    pub(crate) fn optional<T>(
        result: Result<T, Box<dyn Error>>,
    ) -> Result<Option<T>, Box<dyn Error>> {
        match result {
            Err(error) if Self::is(error.as_ref()) => Ok(None),
            result => result.map(Some),
        }
    }
}

impl fmt::Display for NotStarted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no room left in the request for another Host exchange")
    }
}

impl Error for NotStarted {}

/// Connection-scoped exchange bounds, the current request's deadline and site, and the first Host
/// failure observed on this connection.
pub(crate) struct Pacing {
    exchange_bound: Duration,
    request_bound: Duration,
    request_deadline: Cell<Option<Instant>>,
    site: Cell<Site>,
    failure: Cell<Option<HostFailure>>,
}

impl Default for Pacing {
    fn default() -> Self {
        Self::bounded(EXCHANGE_BOUND, REQUEST_BOUND)
    }
}

impl Pacing {
    /// Bounds a test may shorten so no evidence waits ten seconds.
    pub(crate) fn bounded(exchange_bound: Duration, request_bound: Duration) -> Self {
        Self {
            exchange_bound,
            request_bound,
            request_deadline: Cell::new(None),
            site: Cell::new(Site::ServerReceive),
            failure: Cell::new(None),
        }
    }

    pub(crate) const fn exchange_bound(&self) -> Duration {
        self.exchange_bound
    }

    /// The deadline of an exchange starting at `now` with at most `budget`, never past the
    /// exchange bound. It starts only if that whole deadline ends within the request it serves;
    /// otherwise nothing may be written.
    pub(crate) fn start(&self, now: Instant, budget: Duration) -> Result<Instant, NotStarted> {
        let deadline = now
            .checked_add(budget.min(self.exchange_bound))
            .ok_or(NotStarted)?;
        match self.request_deadline.get() {
            Some(request) if deadline > request => Err(NotStarted),
            _ => Ok(deadline),
        }
    }

    /// Starts the bound of the Server request whose first bytes became readable at `readable`.
    pub(crate) fn begin_request_at(&self, readable: Instant) {
        self.request_deadline
            .set(readable.checked_add(self.request_bound));
    }

    /// Ends the request bound before the loop waits for the next request, so idle reads and the
    /// next request never inherit it.
    pub(crate) fn end_request(&self) {
        self.request_deadline.set(None);
    }

    pub(crate) fn site(&self) -> Site {
        self.site.get()
    }

    pub(crate) fn set_site(&self, site: Site) {
        self.site.set(site);
        crate::diagnostic_events::enter(site);
    }

    /// Keeps the first failure: later ones follow from it.
    pub(crate) fn note_failure(&self, failure: HostFailure) {
        if self.failure.get().is_none() {
            self.failure.set(Some(failure));
        }
    }

    pub(crate) fn failure(&self) -> Option<HostFailure> {
        self.failure.get()
    }

    /// Forgets a failure its caller contained, so it cannot name the cause of a later exit.
    #[cfg(windows)]
    pub(crate) fn clear_failure(&self) {
        self.failure.set(None);
    }
}
