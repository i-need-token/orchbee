//! The Keeper update (`proposals/platform/host-handover.md`, the H4 design and H5's M7). A Worker
//! that answers for a Keeper running another installation's program updates it to its own once
//! every Host holding terminals attached it and nothing it holds is moving or orphaned. On Windows
//! a new Keeper takes every holding over and the old one retires (`replacement`); on macOS and
//! Linux the Keeper executes the Worker's program in its own process, keeping every holding and
//! connection. A failed update leaves the old Keeper in charge and is tried again later. Moves wait
//! while an update is due.

use std::{
    error::Error,
    path::Path,
    time::{Duration, Instant},
};

use orchbee_local_runtime::{HoldingState, KeeperStatus};

use super::{Cared, Keepers};
use crate::{diagnostic_events, session_keeper::SessionKeeper};

#[cfg(windows)]
mod replacement;

#[cfg(windows)]
pub(super) use replacement::retire;

/// How long after a failed update the next may begin.
const RETRY_SPACING: Duration = Duration::from_secs(30);
/// Failed updates after which this Worker tries none: moves then run under the old Keeper.
const ATTEMPTS: u32 = 3;

/// Whether the Session's Keeper is updated, judged once for each Keeper the Worker answers for.
pub(super) struct KeeperUpdate {
    /// The Keeper last judged, and whether it needs an update.
    judged: Option<((u32, u64), bool)>,
    /// Failed updates so far, and when the next may begin.
    failed: u32,
    next: Instant,
}

impl KeeperUpdate {
    pub(super) const fn new(now: Instant) -> Self {
        Self {
            judged: None,
            failed: 0,
            next: now,
        }
    }

    /// Whether an update is due or not yet judged for the current Keeper, which moves wait for.
    pub(super) fn pending(&self, keeper: Option<&SessionKeeper>) -> bool {
        let Some(keeper) = keeper else {
            return false;
        };
        match self.judged {
            Some((judged, due)) if judged == keeper.identity() => due && self.failed < ATTEMPTS,
            _ => true,
        }
    }

    /// Updates the Keeper when it runs another program than the Worker's own Host and nothing it
    /// holds is moving, orphaned, or unheld by a Host. Returns whether the Keeper stopped
    /// answering during the update, so it is replaced.
    pub(super) fn run(&mut self, hosts: &[Cared<'_>], keepers: &mut Keepers<'_>) -> bool {
        let Some(old) = keepers.keeper.as_mut() else {
            return false;
        };
        let due = match self.judged {
            Some((judged, due)) if judged == old.identity() => due,
            _ => {
                // A Keeper whose program cannot be read, one that cannot update, or a Worker that
                // knows no program to update to keeps the Keeper it has.
                let due = keepers.program.is_some_and(|(_, executable)| {
                    updatable(old) && old.runs(executable) == Some(false)
                });
                self.judged = Some((old.identity(), due));
                due
            }
        };
        let Some((runtime, executable)) = keepers.program else {
            return false;
        };
        let now = Instant::now();
        if !due || self.failed >= ATTEMPTS || now < self.next {
            return false;
        }
        #[cfg(windows)]
        if keepers.retiring.is_some() {
            return false;
        }
        let mut holding = hosts.iter().filter(|host| host.connection.keeper());
        if holding.any(|host| !host.watch.attached) {
            return false;
        }
        let status = match old.status() {
            Ok(status) if settled(&status) => status,
            _ => return false,
        };
        // A restart's successor Worker adopts the Hosts a few milliseconds before its Server's
        // commit names it in the Session record; a new Keeper started before then is refused, and
        // the update would wait out its spacing.
        #[cfg(windows)]
        if !SessionKeeper::may_start(runtime) {
            return false;
        }
        let attempt = self.failed + 1;
        self.apply(hosts, keepers, (runtime, executable), &status, attempt, now)
    }

    /// Replaces the Keeper with a new one started from the Worker's program.
    #[cfg(windows)]
    fn apply(
        &mut self,
        hosts: &[Cared<'_>],
        keepers: &mut Keepers<'_>,
        program: (&Path, &Path),
        _status: &KeeperStatus,
        attempt: u32,
        began: Instant,
    ) -> bool {
        replacement::update(self, hosts, keepers, program, attempt, began);
        false
    }

    /// Has the Keeper execute the Worker's program in its own process; one that does not answer
    /// within the update's bound is replaced.
    #[cfg(unix)]
    fn apply(
        &mut self,
        _hosts: &[Cared<'_>],
        keepers: &mut Keepers<'_>,
        (_, executable): (&Path, &Path),
        status: &KeeperStatus,
        attempt: u32,
        began: Instant,
    ) -> bool {
        let Some(keeper) = keepers.keeper.as_mut() else {
            return false;
        };
        match keeper.reexec(executable) {
            Ok(Ok(())) => {
                self.judged = Some((keeper.identity(), false));
                diagnostic_events::keeper_updated(
                    status.holdings.len(),
                    status.programs.len(),
                    began.elapsed(),
                );
                false
            }
            Ok(Err(refusal)) => {
                self.failed("reexec", attempt, &refusal);
                false
            }
            Err(error) => {
                self.failed("reexec", attempt, &error);
                true
            }
        }
    }

    /// Records a failed update; the next may begin after the retry spacing.
    fn failed(&mut self, step: &'static str, attempt: u32, error: &(dyn Error + 'static)) {
        diagnostic_events::keeper_update_failed(step, attempt, error);
        self.failed = attempt;
        self.next = Instant::now() + RETRY_SPACING;
    }
}

/// Whether `keeper` can be updated at all: every Windows Keeper can be replaced.
#[cfg(windows)]
const fn updatable(_keeper: &SessionKeeper) -> bool {
    true
}

/// Whether `keeper` can be updated at all: a macOS or Linux Keeper must offer the update in place.
#[cfg(unix)]
const fn updatable(keeper: &SessionKeeper) -> bool {
    keeper.reexecs()
}

/// Whether every holding `status` reports is held for a running Host: none moving, orphaned, or in
/// a state this build does not know, which an update would have to carry across.
fn settled(status: &KeeperStatus) -> bool {
    status
        .holdings
        .iter()
        .all(|holding| holding.state == HoldingState::Held)
        && status
            .programs
            .iter()
            .all(|program| program.state == HoldingState::Held)
}
