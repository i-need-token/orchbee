//! Bounded process admission for suites with short protocol deadlines.
use super::peer_support::Fixture;
use std::{
    ops::Deref,
    sync::{Condvar, Mutex},
};

struct LiveFixtures {
    count: usize,
    exclusive: bool,
    exclusive_waiters: usize,
}
static LIVE_FIXTURES: Mutex<LiveFixtures> = Mutex::new(LiveFixtures {
    count: 0,
    exclusive: false,
    exclusive_waiters: 0,
});
static FIXTURE_AVAILABLE: Condvar = Condvar::new();

struct FixtureSlot {
    exclusive: bool,
}
impl Drop for FixtureSlot {
    fn drop(&mut self) {
        let mut live = LIVE_FIXTURES.lock().unwrap();
        live.count -= 1;
        if self.exclusive {
            live.exclusive = false;
        }
        drop(live);
        FIXTURE_AVAILABLE.notify_all();
    }
}

pub(super) struct CohortFixture {
    // Settle the private files before returning the admission slot.
    fixture: Fixture,
    _slot: FixtureSlot,
}
impl CohortFixture {
    pub(super) fn new() -> Self {
        Self::acquire(false)
    }

    pub(super) fn deadline_sensitive() -> Self {
        // Reach the intended protocol phase without sibling process startup consuming its budget.
        Self::acquire(true)
    }

    fn acquire(exclusive: bool) -> Self {
        let mut live = LIVE_FIXTURES.lock().unwrap();
        if exclusive {
            live.exclusive_waiters += 1;
        }
        while if exclusive {
            live.count != 0
        } else {
            live.exclusive || live.exclusive_waiters != 0 || live.count >= 4
        } {
            live = FIXTURE_AVAILABLE.wait(live).unwrap();
        }
        if exclusive {
            live.exclusive_waiters -= 1;
            live.exclusive = true;
        }
        live.count += 1;
        drop(live);
        let slot = FixtureSlot { exclusive };
        Self {
            fixture: Fixture::new(),
            _slot: slot,
        }
    }
}
impl Deref for CohortFixture {
    type Target = Fixture;
    fn deref(&self) -> &Self::Target {
        &self.fixture
    }
}
