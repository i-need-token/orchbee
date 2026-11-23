//! The Keeper update and the retirement of a Keeper on Windows (`proposals/platform/host-handover.md`,
//! H4 design). A Worker that adopted a Keeper running another installation's program starts a new
//! Keeper from its own in the other slot, has every Host attach it, and commits once the new Keeper
//! holds every holding the old one does; the old Keeper is then retired. A failure before the commit
//! retires the new Keeper instead, so the old one stays in charge, and the update is tried again
//! later. Moves wait while an update is due and while a Keeper retires.
//!
//! Retiring a Keeper has each Host detach it before it ends: a Host whose Keeper link ends without a
//! detach reports that Keeper lost, which would replace the current one.

use std::{
    error::Error,
    io, thread,
    time::{Duration, Instant},
};

use orchbee_local_runtime::{Holding, HoldingState, KeeperStatus, ProgramHolding};

use super::{Cared, Keepers};
use crate::{
    diagnostic_events,
    session_keeper::{Serving, SessionKeeper},
};

/// How long the new Keeper may take to hold every holding the old one holds: a Host offers each
/// running terminal to a new link within 50 milliseconds.
const COMPARE_BUDGET: Duration = Duration::from_secs(2);
/// How long apart two comparisons of the Keepers' holdings lie.
const COMPARE_GAP: Duration = Duration::from_millis(20);
/// How long after a rolled-back update the next may begin.
const RETRY_SPACING: Duration = Duration::from_secs(30);
/// Rolled-back updates after which this Worker tries none: moves then run under the old Keeper.
const ATTEMPTS: u32 = 3;

/// Whether the Session's Keeper is updated, judged once for each Keeper the Worker answers for.
pub(super) struct KeeperUpdate {
    /// The Keeper last judged, and whether it needs an update.
    judged: Option<((u32, u64), bool)>,
    /// Rolled-back updates so far, and when the next may begin.
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
    /// holds is moving, orphaned, or unheld by a Host.
    pub(super) fn run(&mut self, hosts: &[Cared<'_>], keepers: &mut Keepers<'_>) {
        let Some(old) = keepers.keeper.as_mut() else {
            return;
        };
        let due = match self.judged {
            Some((judged, due)) if judged == old.identity() => due,
            _ => {
                // A Keeper whose program cannot be read, or a Worker that knows no program to start
                // one from, keeps the Keeper it has.
                let due = keepers
                    .program
                    .is_some_and(|(_, executable)| old.runs(executable) == Some(false));
                self.judged = Some((old.identity(), due));
                due
            }
        };
        let Some((runtime, executable)) = keepers.program else {
            return;
        };
        let now = Instant::now();
        if !due || self.failed >= ATTEMPTS || now < self.next || keepers.retiring.is_some() {
            return;
        }
        let holding = hosts.iter().filter(|host| host.connection.keeper());
        if holding.clone().any(|host| !host.watch.attached) {
            return;
        }
        match old.status() {
            Ok(status) if settled(&status) => {}
            _ => return,
        }
        let attempt = self.failed + 1;
        let slot = old.slot().other();
        let mut new = match SessionKeeper::start(runtime, slot, executable) {
            Ok(new) => new,
            Err(error) => return self.failed("start", attempt, error.as_ref(), None, keepers),
        };
        for host in holding {
            let attached = host
                .managed
                .host_identity()
                .map_err(Into::into)
                .and_then(|process| new.attach(host.connection, Serving::BetweenRequests, process));
            if let Err(error) = attached {
                return self.failed("attach", attempt, error.as_ref(), Some(new), keepers);
            }
        }
        let counts = match covered(old, &mut new) {
            Ok(counts) => counts,
            Err(error) => {
                return self.failed("compare", attempt, error.as_ref(), Some(new), keepers);
            }
        };
        if let Err(error) = new.make_current(runtime) {
            return self.failed("commit", attempt, &error, Some(new), keepers);
        }
        self.judged = Some((new.identity(), false));
        if let Some(old) = keepers.keeper.replace(new) {
            *keepers.retiring = Some(old.retire());
        }
        diagnostic_events::keeper_updated(counts.0, counts.1, now.elapsed());
    }

    /// Records a failed update and retires the new Keeper, if one started, so the old one stays.
    fn failed(
        &mut self,
        step: &'static str,
        attempt: u32,
        error: &(dyn Error + 'static),
        new: Option<SessionKeeper>,
        keepers: &mut Keepers<'_>,
    ) {
        diagnostic_events::keeper_update_failed(step, attempt, error);
        self.failed = attempt;
        self.next = Instant::now() + RETRY_SPACING;
        if let Some(new) = new {
            *keepers.retiring = Some(new.retire());
        }
    }
}

/// Has every Host detach the Keeper being retired, then ends it; a failed detach waits for the next
/// care.
pub(super) fn retire(hosts: &[Cared<'_>], keepers: &mut Keepers<'_>) {
    let Some(retiring) = keepers.retiring.as_mut() else {
        return;
    };
    let Some((runtime, _)) = keepers.program else {
        return;
    };
    let detaching = hosts
        .iter()
        .filter(|host| host.connection.keeper())
        .filter_map(|host| Some((host.connection, host.managed.host_identity().ok()?)));
    if retiring.retire(runtime, detaching) {
        *keepers.retiring = None;
    }
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

/// Waits until `new` holds every holding `old` does, the same terminal or program under the same
/// holding id for the same Host with the same processes, and answers how many terminal and program
/// holdings that is.
fn covered(
    old: &mut SessionKeeper,
    new: &mut SessionKeeper,
) -> Result<(usize, usize), Box<dyn Error>> {
    let deadline = Instant::now() + COMPARE_BUDGET;
    loop {
        let (before, after) = (old.status()?, new.status()?);
        if !settled(&before) {
            return Err(io::Error::other("a holding began to move during the update").into());
        }
        let terminals = before
            .holdings
            .iter()
            .all(|held| after.holdings.iter().any(|copy| same_holding(held, copy)));
        let programs = before
            .programs
            .iter()
            .all(|held| after.programs.iter().any(|copy| same_program(held, copy)));
        if terminals && programs {
            return Ok((before.holdings.len(), before.programs.len()));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the new Keeper did not hold every holding in time",
            )
            .into());
        }
        thread::sleep(COMPARE_GAP);
    }
}

fn same_holding(held: &Holding, copy: &Holding) -> bool {
    copy.state == HoldingState::Held
        && copy.holding == held.holding
        && copy.owner == held.owner
        && copy.terminal == held.terminal
        && copy.program == held.program
        && copy.console == held.console
}

fn same_program(held: &ProgramHolding, copy: &ProgramHolding) -> bool {
    copy.state == HoldingState::Held
        && copy.holding == held.holding
        && copy.owner == held.owner
        && copy.process == held.process
        && copy.program_id == held.program_id
}
