//! Each link's `capacity` bookkeeping, under the hub state's lock. A link's last result answers
//! for 3 seconds; after that a request waits on one ask of the link, which every request for that
//! Machine shares, and reads `unknown` after 2 seconds. A link has one ask unanswered at a time and
//! is never asked when it did not negotiate `capacity`. This computer's answer is the hub's own
//! sampler's last reading.

use super::{super::capacity::Sampler, lifetime::Moment};
use orchbee_machine_protocol::{Id, Time, capacity::Capacity, hub::CapacityOutcome};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// How long a link's last result answers without asking the link again.
const REUSE: Duration = Duration::from_secs(3);
/// How long a request waits for the link's answer.
const WAIT: Duration = Duration::from_secs(2);

/// A request waiting for a link's answer.
pub(super) struct Waiter {
    /// The connection that asked.
    pub(super) peer: u64,
    pub(super) request: Id,
    /// When the request reads `unknown`.
    until: Instant,
}

/// Each carrier's capacity state, and the hub's own sampler.
#[derive(Default)]
pub(super) struct Capacities {
    sampler: Option<Arc<Sampler>>,
    links: BTreeMap<u64, Link>,
}

#[derive(Default)]
struct Link {
    /// The link negotiated `capacity`.
    serves: bool,
    /// The link's last result, when the hub received it on its clock and on its monotonic one.
    last: Option<(Capacity, Time, Instant)>,
    /// The ask the link has not answered yet.
    asked: Option<Id>,
    waiters: Vec<Waiter>,
}

impl Capacities {
    /// Answers this computer's requests from `sampler` from now on.
    pub(super) fn sample(&mut self, sampler: Option<Arc<Sampler>>) {
        self.sampler = sampler;
    }

    /// Records whether the link on `carrier` negotiated `capacity`.
    pub(super) fn answered(&mut self, carrier: u64, serves: bool) {
        self.links.entry(carrier).or_default().serves = serves;
    }

    /// The earliest moment a waiting request reads `unknown`.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        self.links
            .values()
            .flat_map(|link| link.waiters.iter().map(|waiter| waiter.until))
            .min()
    }

    /// This computer's answer: the sampler's last reading, or `unknown` before its first one.
    pub(super) fn local(&self) -> CapacityOutcome {
        self.sampler
            .as_ref()
            .and_then(|sampler| sampler.latest())
            .map_or(CapacityOutcome::Unknown, |sample| {
                CapacityOutcome::Sampled {
                    capacity: sample.capacity,
                    sampled_at: sample.sampled_at,
                }
            })
    }

    /// The request `request` of the connection `peer` for the Machine on `carrier`: answered from
    /// the link's last result, or `None` while it waits on the link's ask, which `ask` sends when
    /// none is unanswered. It reads `unknown` when the link does not serve `capacity` or the ask
    /// could not be sent.
    pub(super) fn request(
        &mut self,
        carrier: u64,
        peer: u64,
        request: Id,
        ask: impl FnOnce() -> Option<Id>,
    ) -> Option<CapacityOutcome> {
        let now = Instant::now();
        let link = self.links.entry(carrier).or_default();
        if !link.serves {
            return Some(CapacityOutcome::Unknown);
        }
        if let Some((capacity, sampled_at, at)) = &link.last
            && now.saturating_duration_since(*at) <= REUSE
        {
            return Some(CapacityOutcome::Sampled {
                capacity: capacity.clone(),
                sampled_at: *sampled_at,
            });
        }
        if link.asked.is_none() {
            let Some(asked) = ask() else {
                return Some(CapacityOutcome::Unknown);
            };
            link.asked = Some(asked);
        }
        link.waiters.push(Waiter {
            peer,
            request,
            until: now + WAIT,
        });
        None
    }

    /// The link on `carrier` answered the ask `request` with `capacity`, or with `unsupported`
    /// when it is `None`, after which the link is asked no more: the waiting requests and what they
    /// read. Nothing when the answer is not to the link's ask.
    pub(super) fn result(
        &mut self,
        carrier: u64,
        request: Id,
        capacity: Option<Capacity>,
    ) -> Option<(Vec<Waiter>, CapacityOutcome)> {
        let link = self.links.get_mut(&carrier)?;
        if link.asked != Some(request) {
            return None;
        }
        link.asked = None;
        let waiters = std::mem::take(&mut link.waiters);
        let sampled_at = Time::new(Moment::now().wall).ok();
        let outcome = match (capacity, sampled_at) {
            (None, _) => {
                link.serves = false;
                CapacityOutcome::Unknown
            }
            (Some(capacity), Some(sampled_at)) => {
                link.last = Some((capacity.clone(), sampled_at, Instant::now()));
                CapacityOutcome::Sampled {
                    capacity,
                    sampled_at,
                }
            }
            (Some(_), None) => CapacityOutcome::Unknown,
        };
        Some((waiters, outcome))
    }

    /// The requests waiting past their bound at `now`. The link's ask stays unanswered, so no
    /// second one is sent.
    pub(super) fn expire(&mut self, now: Instant) -> Vec<Waiter> {
        let mut expired = Vec::new();
        for link in self.links.values_mut() {
            let (gone, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut link.waiters)
                .into_iter()
                .partition(|waiter| waiter.until <= now);
            link.waiters = waiting;
            expired.extend(gone);
        }
        expired
    }

    /// The link on `carrier` ended: the requests that waited for it.
    pub(super) fn end(&mut self, carrier: u64) -> Vec<Waiter> {
        self.links
            .remove(&carrier)
            .map(|link| link.waiters)
            .unwrap_or_default()
    }

    /// Every waiting request, for the hub's closing.
    pub(super) fn drain(&mut self) -> Vec<Waiter> {
        self.links
            .values_mut()
            .flat_map(|link| std::mem::take(&mut link.waiters))
            .collect()
    }

    /// The connection `peer` is over: its waiting requests go.
    pub(super) fn forget(&mut self, peer: u64) {
        for link in self.links.values_mut() {
            link.waiters.retain(|waiter| waiter.peer != peer);
        }
    }
}
