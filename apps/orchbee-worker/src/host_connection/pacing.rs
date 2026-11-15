//! Bounds Host exchanges below the Server's reply window and keeps the class of the Host failure
//! that ended the connection, so a slow Host is waited for and only a silent one is lost.

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
/// Every Host exchange of one Server request ends within this bound from the request's arrival,
/// which leaves room to deliver the result inside the Server's twenty-second reply window.
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

    /// The deadline of an exchange starting now with at most `budget`: never past the exchange
    /// bound, nor past the deadline of the request it serves.
    pub(crate) fn deadline(&self, now: Instant, budget: Duration) -> Instant {
        let deadline = now
            .checked_add(budget.min(self.exchange_bound))
            .unwrap_or(now);
        self.request_deadline
            .get()
            .map_or(deadline, |request| deadline.min(request))
    }

    /// Starts the bound of the Server request that has just arrived.
    pub(crate) fn begin_request(&self) {
        self.begin_request_at(Instant::now());
    }

    /// Starts the bound of a Server request that arrived at `arrived`, before this connection
    /// existed.
    pub(crate) fn begin_request_at(&self, arrived: Instant) {
        self.request_deadline
            .set(arrived.checked_add(self.request_bound));
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
}
