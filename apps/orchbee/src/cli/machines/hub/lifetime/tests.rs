//! The lifetime rules a real hub cannot reach on demand: a stale Session read, a read during
//! `presence`, and the read at the expiry deadline that a Session started in the last 30 seconds
//! depends on. The exercise in `tests/cli_windows_machine_hub.rs` covers the ordinary path through
//! a real process.

use super::{Lifetime, Moment, ReadDue, SESSION_READ_INTERVAL};
use orchbee_machine_protocol::hub::HubState;
use std::time::{Duration, Instant};

const EXPIRY: Duration = Duration::from_secs(600);

fn at(start: Instant, seconds: u64) -> Moment {
    Moment {
        at: start + Duration::from_secs(seconds),
        wall: 1_000_000 + seconds * 1_000,
    }
}

fn expiring_at(lifetime: &Lifetime) -> Option<u64> {
    match lifetime.state() {
        HubState::Expiring { expires_at } => Some(expires_at.get()),
        HubState::Held | HubState::Closing => None,
    }
}

#[test]
fn the_last_leave_holds_until_its_read_finds_nothing_and_expiry_counts_from_that_read() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(EXPIRY, false, at(start, 0));
    assert_eq!(expiring_at(&lifetime), Some(1_600_000));
    assert!(lifetime.attach());
    assert_eq!(lifetime.state(), HubState::Held);
    assert_eq!(lifetime.read_due(), None);
    lifetime.detach();
    assert_eq!(lifetime.state(), HubState::Held);
    assert_eq!(lifetime.read_due(), Some(ReadDue::Now));
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, false, at(start, 5)));
    assert_eq!(expiring_at(&lifetime), Some(1_605_000));
    assert_eq!(
        lifetime.read_due(),
        Some(ReadDue::At(at(start, 5).at + SESSION_READ_INTERVAL))
    );
}

#[test]
fn the_expiry_deadline_reads_the_sessions_first_and_closes_only_when_none_lives() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(Duration::from_secs(20), false, at(start, 0));
    // The deadline comes before the next periodic read, so the read is due there.
    assert_eq!(lifetime.read_due(), Some(ReadDue::At(at(start, 20).at)));
    // A Session started since the last read holds the hub at the deadline.
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, true, at(start, 20)));
    assert_eq!(lifetime.state(), HubState::Held);
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, false, at(start, 50)));
    assert_eq!(expiring_at(&lifetime), Some(1_070_000));
    assert_eq!(lifetime.read_due(), Some(ReadDue::At(at(start, 70).at)));
    let ticket = lifetime.begin_read();
    assert!(!lifetime.finish_read(ticket, false, at(start, 69)));
    assert_eq!(expiring_at(&lifetime), Some(1_070_000));
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, false, at(start, 70)));
    assert_eq!(lifetime.state(), HubState::Closing);
    assert!(!lifetime.attach());
    assert_eq!(lifetime.read_due(), None);
}

#[test]
fn a_read_overtaken_by_presence_or_a_later_leave_changes_nothing() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(EXPIRY, true, at(start, 0));
    lifetime.attach();
    lifetime.detach();
    let stale = lifetime.begin_read();
    lifetime.attach();
    assert!(!lifetime.finish_read(stale, false, at(start, 1)));
    lifetime.detach();
    assert!(!lifetime.finish_read(stale, false, at(start, 2)));
    assert_eq!(lifetime.state(), HubState::Held);
    assert_eq!(lifetime.read_due(), Some(ReadDue::Now));
    let current = lifetime.begin_read();
    assert!(!lifetime.finish_read(current, true, at(start, 3)));
    assert_eq!(lifetime.state(), HubState::Held);
}

#[test]
fn a_live_session_found_while_expiring_holds_again_and_drain_closes_once() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(EXPIRY, false, at(start, 0));
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, true, at(start, 30)));
    assert_eq!(lifetime.state(), HubState::Held);
    assert_eq!(lifetime.deadline(), None);
    assert!(lifetime.drain());
    assert!(!lifetime.drain());
    assert_eq!(lifetime.state(), HubState::Closing);
}

#[test]
fn an_online_connect_restarts_the_countdown_only_while_expiring() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(EXPIRY, false, at(start, 0));
    assert_eq!(expiring_at(&lifetime), Some(1_600_000));
    // The countdown counts from the latest connect that ended online.
    assert!(lifetime.connected(at(start, 100)));
    assert_eq!(expiring_at(&lifetime), Some(1_700_000));
    assert_eq!(lifetime.deadline(), Some(at(start, 700).at));
    // A read before the restarted deadline does not close; one at it does.
    let ticket = lifetime.begin_read();
    assert!(!lifetime.finish_read(ticket, false, at(start, 600)));
    assert_eq!(expiring_at(&lifetime), Some(1_700_000));
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, false, at(start, 700)));
    assert_eq!(lifetime.state(), HubState::Closing);
    // Closing never returns.
    assert!(!lifetime.connected(at(start, 701)));
    assert_eq!(lifetime.state(), HubState::Closing);
}

#[test]
fn a_connect_while_held_changes_nothing_and_the_later_leave_counts() {
    let start = Instant::now();
    let mut lifetime = Lifetime::start(EXPIRY, false, at(start, 0));
    assert!(lifetime.attach());
    assert!(!lifetime.connected(at(start, 10)));
    assert_eq!(lifetime.state(), HubState::Held);
    lifetime.detach();
    let ticket = lifetime.begin_read();
    assert!(lifetime.finish_read(ticket, false, at(start, 20)));
    assert_eq!(expiring_at(&lifetime), Some(1_620_000));
}
