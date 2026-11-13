//! `stop-session` runs: the hub stops a Machine's remote Session the way an admitted Client of
//! that Session does, with the Session's own stop request over link `session` channels. A run's
//! thread performs its exchanges through a relay pair; the hub state owns the run, its waiting
//! requests and whether its request was submitted, and the first end that decides the run answers
//! them. How each end of an exchange reads is a set of pure functions of what the run saw.
//! `docs/machines.md#stopping-a-session` owns the rules.

use super::{
    relay::Held,
    state::{Hub, Shared},
};
use orchbee_connection_handshake::Offer;
use orchbee_local_client_protocol::{
    ConnectClientError, SessionStopExchangeError, SessionStopRequest, SessionStopResult,
    SessionStopStatus, StopAction, StopId, exchange_session_stop_over,
};
use orchbee_machine_protocol::{
    Id, MachineId, ResetReason,
    hub::{Reply, StopSessionOutcome},
};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    sync::{Arc, MutexGuard},
    thread,
    time::{Duration, Instant},
};

/// How long a run watches for the Session's end after its start: the default wait of `server stop`.
const WAIT: Duration = Duration::from_secs(65);
/// The pause between two `status` exchanges.
const POLL: Duration = Duration::from_millis(250);
/// The longest one exchange takes, past the link helper's 20-second connect window.
const EXCHANGE: Duration = Duration::from_secs(30);

/// Every run until its outcome, by its number.
#[derive(Debug, Default)]
pub(super) struct Stops {
    runs: BTreeMap<u64, Run>,
    next: u64,
}

#[derive(Debug)]
struct Run {
    machine: MachineId,
    /// The carrier the run's exchanges go over; its end decides the run.
    carrier: u64,
    /// The requests waiting for the outcome, each with the connection that sent it.
    waiters: Vec<(u64, Id)>,
    /// A byte of the stop request was committed to the link, so it may have reached the Session.
    submitted: bool,
}

/// What one exchange showed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Seen {
    /// The Session answered the request.
    Answered(SessionStopStatus),
    /// The far end reset the channel before the request was submitted.
    Reset(ResetReason),
    /// The Session's admission refused the offer, or the Session rejected the request.
    Refused,
    /// No answer otherwise; `submitted` when the request may have reached the Session.
    Broken { submitted: bool },
}

/// How far the stop got with the Session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    /// The `start` was submitted and never answered.
    Unanswered,
    /// The Session recorded the stop and answered `pending`.
    Recorded,
}

/// What follows an exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Next {
    Watch(Stage),
    Done(StopSessionOutcome),
}

/// Reads one exchange from its result, the far end's reset of its channel, and whether the request
/// was submitted. A far reset says nothing ran only before submission.
pub(super) fn seen(
    result: Result<SessionStopResult, SessionStopExchangeError>,
    reset: Option<ResetReason>,
    submitted: bool,
) -> Seen {
    match result {
        Ok(result) => Seen::Answered(result.status),
        Err(error) => match (error.failure, reset) {
            (ConnectClientError::Refused(_) | ConnectClientError::Rejected(_), _) => Seen::Refused,
            (_, Some(reason)) if !submitted => Seen::Reset(reason),
            _ => Seen::Broken { submitted },
        },
    }
}

/// What the `start` exchange decides.
pub(super) const fn after_start(seen: Seen) -> Next {
    match seen {
        Seen::Answered(SessionStopStatus::Pending { .. }) => Next::Watch(Stage::Recorded),
        Seen::Broken { submitted: true } => Next::Watch(Stage::Unanswered),
        Seen::Reset(ResetReason::SessionNotRunning) => Next::Done(StopSessionOutcome::NotRunning),
        Seen::Answered(_) | Seen::Refused | Seen::Reset(_) | Seen::Broken { submitted: false } => {
            Next::Done(StopSessionOutcome::Failed)
        }
    }
}

/// What a `status` exchange decides, the stop being at `stage`.
pub(super) const fn after_status(stage: Stage, seen: Seen) -> Next {
    match seen {
        Seen::Answered(SessionStopStatus::Pending { .. }) => Next::Watch(Stage::Recorded),
        // A Server keeps its stop entry until it ends, so after `pending` a `status` it does not
        // know comes from a later Server.
        Seen::Answered(SessionStopStatus::NotRecorded) => Next::Done(match stage {
            Stage::Recorded => StopSessionOutcome::Stopped,
            Stage::Unanswered => StopSessionOutcome::Unknown,
        }),
        Seen::Answered(
            SessionStopStatus::Failed { .. }
            | SessionStopStatus::Busy { .. }
            | SessionStopStatus::Unmanaged,
        ) => Next::Done(StopSessionOutcome::Failed),
        Seen::Reset(ResetReason::SessionNotRunning) => Next::Done(StopSessionOutcome::Stopped),
        Seen::Reset(_) | Seen::Refused | Seen::Broken { .. } => Next::Watch(stage),
    }
}

/// The outcome of a run whose carrier ended.
pub(super) const fn carrier_lost(submitted: bool) -> StopSessionOutcome {
    if submitted {
        StopSessionOutcome::Unknown
    } else {
        StopSessionOutcome::NotOnline
    }
}

/// The outcome of a run still deciding when a closing hub reaches its allowance.
pub(super) const fn allowance_passed(submitted: bool) -> StopSessionOutcome {
    if submitted {
        StopSessionOutcome::Unknown
    } else {
        StopSessionOutcome::Stopping
    }
}

/// Answers a `stop-session` for `machine` from the connection `peer` at once, or returns `None`
/// when the run it starts or joins answers later.
pub(super) fn request(
    shared: &Arc<Shared>,
    peer: u64,
    request: Id,
    machine: MachineId,
) -> Option<StopSessionOutcome> {
    let mut hub = shared.lock();
    if hub.lifetime.is_closing() {
        return Some(StopSessionOutcome::Stopping);
    }
    if hub.machines.state(&machine).is_none() {
        return Some(StopSessionOutcome::NotFound);
    }
    if let Some(run) = hub
        .stops
        .runs
        .values_mut()
        .find(|run| run.machine == machine)
    {
        run.waiters.push((peer, request));
        return None;
    }
    let Some((carrier, _)) = hub.machines.online_link(&machine) else {
        return Some(StopSessionOutcome::NotOnline);
    };
    hub.stops.next += 1;
    let number = hub.stops.next;
    hub.stops.runs.insert(
        number,
        Run {
            machine,
            carrier,
            waiters: vec![(peer, request)],
            submitted: false,
        },
    );
    drop(hub);
    let worker = shared.clone();
    let started = thread::Builder::new()
        .name("orchbee-hub-stop".into())
        .spawn(move || {
            if let Some(outcome) = execute(&worker, number) {
                worker.lock().conclude(number, outcome);
                worker.wake();
            }
        });
    if started.is_err() {
        // No exchange began, so nothing reached the Session.
        shared.lock().conclude(number, StopSessionOutcome::Failed);
        shared.wake();
    }
    None
}

/// Performs the run `number` and returns its outcome, or `None` once another end decided the run.
/// The outcome counts only while no other end decided it first.
fn execute(shared: &Shared, number: u64) -> Option<StopSessionOutcome> {
    let mut bytes = [0; 16];
    let Some(id) = getrandom::fill(&mut bytes)
        .ok()
        .and_then(|()| StopId::new(bytes).ok())
    else {
        return Some(StopSessionOutcome::Failed);
    };
    let offer = crate::local_client::client_offer(None);
    let deadline = Instant::now() + WAIT;
    let start = SessionStopRequest {
        id,
        action: StopAction::Start,
    };
    let mut next = after_start(exchange(shared, number, &offer, start, deadline));
    loop {
        let stage = match next {
            Next::Done(outcome) => return Some(outcome),
            Next::Watch(stage) => stage,
        };
        if !pause(shared, number, Instant::now() + POLL) {
            return None;
        }
        if Instant::now() >= deadline {
            return Some(StopSessionOutcome::Unknown);
        }
        let status = SessionStopRequest {
            id,
            action: StopAction::Status,
        };
        next = after_status(stage, exchange(shared, number, &offer, status, deadline));
    }
}

/// Waits until `until` without a lock; false when another end decided the run meanwhile.
fn pause(shared: &Shared, number: u64, until: Instant) -> bool {
    let mut hub = shared.lock();
    while hub.stops.runs.contains_key(&number) {
        if Instant::now() >= until {
            return true;
        }
        hub = shared.wait_until(hub, until);
    }
    false
}

/// One exchange over a new link `session` channel, which ends with it.
fn exchange(
    shared: &Shared,
    number: u64,
    offer: &Offer,
    request: SessionStopRequest,
    deadline: Instant,
) -> Seen {
    let Some(key) = shared.lock().open_stop_exchange(number) else {
        // No channel opened, so nothing was sent.
        return Seen::Broken { submitted: false };
    };
    let mut channel = Channel {
        shared,
        number,
        key,
        deadline: deadline.min(Instant::now() + EXCHANGE),
        submitting: false,
        submitted: false,
    };
    let result = exchange_session_stop_over(&mut channel, offer, request, |channel| {
        channel.submitting = true;
    });
    let reset = shared.lock().close_stop_channel(key);
    seen(result, reset, channel.submitted)
}

/// One exchange's link `session` channel as the byte stream the stop exchange speaks over, bounded
/// by `deadline` and by the run's life.
struct Channel<'a> {
    shared: &'a Shared,
    number: u64,
    key: u64,
    deadline: Instant,
    /// The writes from now on carry the stop request.
    submitting: bool,
    /// A byte of the stop request was committed to the link.
    submitted: bool,
}

impl<'a> Channel<'a> {
    /// Waits on the hub state's signal, which the carrier's reader fires after each frame; an error
    /// once the deadline passed or another end decided the run.
    fn wait(&self, hub: MutexGuard<'a, Hub>) -> io::Result<MutexGuard<'a, Hub>> {
        if !hub.stops.runs.contains_key(&self.number) {
            return Err(io::ErrorKind::ConnectionAborted.into());
        }
        if Instant::now() >= self.deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(self.shared.wait_until(hub, self.deadline))
    }
}

impl Read for Channel<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let shared = self.shared;
        let mut hub = shared.lock();
        loop {
            match hub.read_stop_channel(self.key, buffer) {
                Held::Bytes(count) => return Ok(count),
                Held::Over => return Ok(0),
                Held::Waiting => hub = self.wait(hub)?,
            }
        }
    }
}

impl Write for Channel<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let shared = self.shared;
        let mut hub = shared.lock();
        loop {
            match hub.write_stop_channel(self.key, bytes) {
                None => return Err(io::ErrorKind::BrokenPipe.into()),
                Some(0) => hub = self.wait(hub)?,
                Some(count) => {
                    // Marked under the same lock as the commit, so a carrier's end that follows
                    // reads the run as submitted.
                    if self.submitting {
                        self.submitted = true;
                        if let Some(run) = hub.stops.runs.get_mut(&self.number) {
                            run.submitted = true;
                        }
                    }
                    return Ok(count);
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Hub {
    /// Work in flight for the hub's lifetime: open routes and the runs still deciding.
    pub(super) fn routes(&self) -> usize {
        self.relays.routes() + self.stops.runs.len()
    }

    /// Decides the run `number` with `outcome` and answers every request waiting for it; a run
    /// already decided keeps its outcome.
    pub(super) fn conclude(&mut self, number: u64, outcome: StopSessionOutcome) {
        let Some(run) = self.stops.runs.remove(&number) else {
            return;
        };
        for (peer, request) in run.waiters {
            if let Some(peer) = self.peers.get(&peer) {
                let mut wire = peer.line.lock();
                if wire.open {
                    let _replied = wire.connection.reply(request, Reply::StopSession(outcome));
                }
                drop(wire);
                peer.line.wake();
            }
        }
    }

    /// Opens the next exchange's channel for the run `number`, while it still decides.
    fn open_stop_exchange(&mut self, number: u64) -> Option<u64> {
        let run = self.stops.runs.get(&number)?;
        let (machine, carrier) = (run.machine.clone(), run.carrier);
        self.open_stop_channel(number, &machine, carrier)
    }

    /// The carrier `carrier` ended: every run on it ends by whether its request was submitted.
    pub(super) fn end_stops_on(&mut self, carrier: u64) {
        for (number, submitted) in self.stops_where(|run| run.carrier == carrier) {
            self.conclude(number, carrier_lost(submitted));
        }
    }

    /// A closing hub reached its allowance: every run ends by whether its request was submitted.
    pub(super) fn end_stops_closing(&mut self) {
        for (number, submitted) in self.stops_where(|_| true) {
            self.conclude(number, allowance_passed(submitted));
        }
    }

    /// The connection `peer` is over: its waiting requests leave their runs, which go on.
    pub(super) fn forget_stop_waiters(&mut self, peer: u64) {
        for run in self.stops.runs.values_mut() {
            run.waiters.retain(|(owner, _)| *owner != peer);
        }
    }

    fn stops_where(&self, pick: impl Fn(&Run) -> bool) -> Vec<(u64, bool)> {
        self.stops
            .runs
            .iter()
            .filter(|(_, run)| pick(run))
            .map(|(number, run)| (*number, run.submitted))
            .collect()
    }
}

#[cfg(test)]
mod tests;
