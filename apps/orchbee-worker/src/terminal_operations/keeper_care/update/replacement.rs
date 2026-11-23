//! The Keeper update and the retirement of a Keeper on Windows (`proposals/platform/host-handover.md`,
//! H4 design). The Worker starts a new Keeper from its own program in the other slot, has every Host
//! attach it, and commits once the new Keeper holds every holding the old one does; the old Keeper
//! is then retired. A failure before the commit retires the new Keeper instead, so the old one stays
//! in charge. Moves wait while a Keeper retires.
//!
//! Retiring a Keeper has each Host detach it before it ends: a Host whose Keeper link ends without a
//! detach reports that Keeper lost, which would replace the current one.

use std::{
    error::Error,
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use orchbee_local_runtime::{Holding, HoldingState, ProgramHolding};

use super::{
    super::{Cared, Keepers},
    KeeperUpdate, settled,
};
use crate::{
    diagnostic_events,
    session_keeper::{Serving, SessionKeeper},
};

/// How long the new Keeper may take to hold every holding the old one holds: a Host offers each
/// running terminal to a new link within 50 milliseconds.
const COMPARE_BUDGET: Duration = Duration::from_secs(2);
/// How long apart two comparisons of the Keepers' holdings lie.
const COMPARE_GAP: Duration = Duration::from_millis(20);

/// Replaces the current Keeper with a new one started from `program`, the Session runtime folder
/// and the Worker's Terminal Host, on its `attempt`.
pub(super) fn update(
    update: &mut KeeperUpdate,
    hosts: &[Cared<'_>],
    keepers: &mut Keepers<'_>,
    (runtime, executable): (&Path, &Path),
    attempt: u32,
    began: Instant,
) {
    let Some(old) = keepers.keeper.as_mut() else {
        return;
    };
    let slot = old.slot().other();
    let mut new = match SessionKeeper::start(runtime, slot, executable) {
        Ok(new) => new,
        Err(error) => return failed(update, "start", attempt, error.as_ref(), None, keepers),
    };
    for host in hosts.iter().filter(|host| host.connection.keeper()) {
        let attached = host
            .managed
            .host_identity()
            .map_err(Into::into)
            .and_then(|process| new.attach(host.connection, Serving::BetweenRequests, process));
        if let Err(error) = attached {
            return failed(
                update,
                "attach",
                attempt,
                error.as_ref(),
                Some(new),
                keepers,
            );
        }
    }
    let counts = match covered(old, &mut new) {
        Ok(counts) => counts,
        Err(error) => {
            return failed(
                update,
                "compare",
                attempt,
                error.as_ref(),
                Some(new),
                keepers,
            );
        }
    };
    if let Err(error) = new.make_current(runtime) {
        return failed(update, "commit", attempt, &error, Some(new), keepers);
    }
    update.judged = Some((new.identity(), false));
    if let Some(old) = keepers.keeper.replace(new) {
        *keepers.retiring = Some(old.retire());
    }
    diagnostic_events::keeper_updated(counts.0, counts.1, began.elapsed());
}

/// Records a failed update and retires the new Keeper, if one started, so the old one stays.
fn failed(
    update: &mut KeeperUpdate,
    step: &'static str,
    attempt: u32,
    error: &(dyn Error + 'static),
    new: Option<SessionKeeper>,
    keepers: &mut Keepers<'_>,
) {
    update.failed(step, attempt, error);
    if let Some(new) = new {
        *keepers.retiring = Some(new.retire());
    }
}

/// Has every Host detach the Keeper being retired, then ends it; a failed detach waits for the next
/// care.
pub(in super::super) fn retire(hosts: &[Cared<'_>], keepers: &mut Keepers<'_>) {
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
