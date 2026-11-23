//! The old side of a Server restart that keeps terminals, on Windows. A `prepare` holds the Session
//! for the successor its record names: it settles intents, counts background programs, publishes an
//! exact capture, releases the store and the Worker, and starts a lease. A `commit` ends this Server
//! once the record names the successor; an `abort`, the lease, or the end of the successor or its
//! Worker decides the restart under the record's writer lock, and takes the Session back while the
//! record still names this generation. Whoever removes the successor entry first ends what it names
//! by its recorded identity; holding nothing, this Server removes an entry its successor abandoned.
use super::{Owners, Watch, checkpoint_worker, end_worker, own_stop_id};
use crate::{
    local_client_service::ManagedClients,
    session_shutdown::{HoldRefusal, SessionShutdown},
    terminal_runtime::TerminalRuntimeState,
};
use orchbee_connection_handshake::{
    HandoverOutcome, HandoverRefusal, HandoverStep, HandoverTicket, ProcessState,
};
use orchbee_local_runtime::{ExactProcess, Fenced, LogLevel, LogValue, Successor};
use std::{
    io, thread,
    time::{Duration, Instant},
};

/// How long the successor has, from `prepared`, to commit.
const LEASE: Duration = Duration::from_secs(30);
/// How long a `prepare` waits for admitted intents and pending operations to settle.
const SETTLE: Duration = Duration::from_secs(5);
/// How long a `prepare` waits for the Worker's count of background programs.
const PROGRAMS_DUE: Duration = Duration::from_secs(2);
/// How long the Worker has to release its Host before it is ended.
const RELEASE: Duration = Duration::from_secs(10);
/// How long a process ended by its recorded identity, or a store hold, has to go.
const END: Duration = Duration::from_secs(5);
/// How long one decision waits for the Session record's writer lock.
const FENCE_WAIT: Duration = Duration::from_secs(5);
/// How long a held restart may stay undecided before this Server ends it and leaves.
const UNDECIDED: Duration = Duration::from_secs(30);
/// How long an entry may wait for its `prepare` before it counts as abandoned.
const ABANDONED: Duration = Duration::from_secs(120);
/// How often a Server that holds nothing looks for an abandoned entry.
const LOOK: Duration = Duration::from_secs(5);
/// The stop loop's tick while a restart holds the Session.
pub(super) const HELD_TICK: Duration = Duration::from_millis(100);
/// How long a Server that leaves to its successor gives its jobs and Client listener.
const LEAVE: Duration = Duration::from_secs(10);

/// What the stop loop does after a restart step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Next {
    Serve,
    /// The Session record names the successor, or the restart stayed undecided: this Server ends
    /// without a receipt, and the next start recovers whatever no successor holds.
    Leave,
}

/// One Server's side of the restarts it is asked to take part in.
#[derive(Default)]
pub(super) struct Restart {
    held: Option<Held>,
    /// The ticket of an entry seen while holding nothing, and when it was first seen.
    seen: Option<([u8; 16], Instant)>,
    next_look: Option<Instant>,
}

/// A restart this Server holds the Session for.
struct Held {
    ticket: HandoverTicket,
    successor: Successor,
    /// The successor Server and its Worker, opened on their recorded identities with the right to
    /// end them.
    server: ExactProcess,
    worker: ExactProcess,
    lease: Instant,
    /// Since when no decision could be made.
    undecided: Option<Instant>,
}

enum Fence {
    Kept,
    Committed,
    Undecided,
}

impl Restart {
    pub(super) fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// Answers one `handover` step to the Client waiting on it.
    pub(super) fn step(
        &mut self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
        clients: &ManagedClients,
        watch: &mut Watch<'_>,
        step: HandoverStep,
        ticket: HandoverTicket,
    ) -> Next {
        let held = self.held.as_ref().map(|held| held.ticket == ticket);
        let (answer, next) = match (step, held) {
            (HandoverStep::Prepare, Some(true)) => (HandoverOutcome::Prepared, None),
            (HandoverStep::Prepare | HandoverStep::Abort, Some(false)) => {
                (HandoverOutcome::Refused(HandoverRefusal::Busy), None)
            }
            (HandoverStep::Prepare, None) => {
                match prepare(owners, shutdown, clients, watch, ticket) {
                    Ok(held) => {
                        self.held = Some(held);
                        record("prepared");
                        (HandoverOutcome::Prepared, None)
                    }
                    Err(refusal) => (HandoverOutcome::Refused(refusal), None),
                }
            }
            (HandoverStep::Commit, Some(true)) => {
                let successor = self.held.as_ref().map(|held| held.successor);
                let committed = successor.is_some_and(|successor| {
                    matches!(
                        owners
                            .session_record
                            .names_committed(&successor, Instant::now() + FENCE_WAIT),
                        Ok(true)
                    )
                });
                if committed {
                    (HandoverOutcome::Committed, Some(Decided::Committed))
                } else {
                    (HandoverOutcome::Refused(HandoverRefusal::NotRecorded), None)
                }
            }
            (HandoverStep::Commit, _) => {
                (HandoverOutcome::Refused(HandoverRefusal::NotRecorded), None)
            }
            (HandoverStep::Abort, Some(true)) => match self.fence(owners) {
                Fence::Kept => (HandoverOutcome::Aborted, Some(Decided::Kept)),
                Fence::Committed => (HandoverOutcome::Committed, Some(Decided::Committed)),
                // The next tick decides again; the requester may ask again.
                Fence::Undecided => (HandoverOutcome::Refused(HandoverRefusal::Busy), None),
            },
            (HandoverStep::Abort, None) => (HandoverOutcome::Aborted, None),
            (HandoverStep::Unknown, _) => {
                (HandoverOutcome::Refused(HandoverRefusal::UnknownStep), None)
            }
        };
        shutdown.answer_handover(answer);
        match next {
            None => Next::Serve,
            Some(Decided::Committed) => {
                record("committed");
                Next::Leave
            }
            Some(Decided::Kept) => self.take_back(owners, shutdown, watch),
        }
    }

    /// One tick while holding: the lease, and the successor and its Worker. A restart past its
    /// lease, whose successor or Worker ended, or that could not be decided before, is decided now.
    pub(super) fn tick(
        &mut self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
        watch: &mut Watch<'_>,
    ) -> Next {
        let Some(held) = &self.held else {
            return Next::Serve;
        };
        let ended = |process: &ExactProcess| process.exited().unwrap_or(true);
        let due = Instant::now() >= held.lease
            || held.undecided.is_some()
            || ended(&held.server)
            || ended(&held.worker);
        if !due {
            return Next::Serve;
        }
        match self.fence(owners) {
            Fence::Kept => self.take_back(owners, shutdown, watch),
            Fence::Committed => {
                record("committed");
                Next::Leave
            }
            Fence::Undecided => self.undecided(),
        }
    }

    /// Before a stop or an `exit`: a held restart, which only an `exit` meets since the hold refuses
    /// stops, is decided first, and the Session taken back without a replacement Worker, so the
    /// `exit` goes on as without a Worker; holding nothing, an entry is removed after what it names
    /// is ended.
    pub(super) fn settle_before_ending(
        &mut self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
    ) -> Next {
        if self.held.is_none() {
            let deadline = Instant::now() + END;
            if let Err(error) = owners.session_record.clear_successor(
                deadline,
                |_| true,
                |entry| ExactProcess::end_successor(entry, deadline),
            ) {
                diagnostic("clear", &error);
            }
            return Next::Serve;
        }
        loop {
            match self.fence(owners) {
                Fence::Kept => {
                    self.held = None;
                    record("taken-back");
                    if !retake(owners) {
                        return Next::Leave;
                    }
                    lift(owners, shutdown);
                    return Next::Serve;
                }
                Fence::Committed => {
                    record("committed");
                    return Next::Leave;
                }
                Fence::Undecided => {
                    if self.undecided() == Next::Leave {
                        return Next::Leave;
                    }
                    thread::sleep(HELD_TICK);
                }
            }
        }
    }

    /// Holding nothing, every [`LOOK`]: an entry whose successor is gone, or that waited
    /// [`ABANDONED`] for its `prepare`, is removed after what it names is ended.
    pub(super) fn look(&mut self, owners: &Owners) {
        let now = Instant::now();
        if self.next_look.is_some_and(|next| now < next) {
            return;
        }
        self.next_look = Some(now + LOOK);
        let Ok(Some(entry)) = owners.session_record.successor() else {
            self.seen = None;
            return;
        };
        let ticket = entry.ticket();
        let first = match self.seen {
            Some((seen, at)) if seen == ticket => at,
            _ => {
                self.seen = Some((ticket, now));
                now
            }
        };
        let gone = matches!(ExactProcess::successor_server(&entry), Ok(None));
        if !gone && now.duration_since(first) < ABANDONED {
            return;
        }
        let deadline = now + END;
        match owners.session_record.clear_successor(
            deadline,
            |entry| entry.ticket() == ticket,
            |entry| ExactProcess::end_successor(entry, deadline),
        ) {
            Ok(cleared) => {
                self.seen = None;
                if cleared.is_some() {
                    record("abandoned");
                }
            }
            // The next look tries again.
            Err(error) => diagnostic("clear", &error),
        }
    }

    /// Decides the held restart under the Session record's writer lock. While the record names
    /// this generation, the successor's Worker and then the successor are ended inside the same
    /// hold, before the entry goes.
    fn fence(&mut self, owners: &Owners) -> Fence {
        let Some(held) = self.held.as_mut() else {
            return Fence::Kept;
        };
        let decided =
            owners
                .session_record
                .fence(&held.successor, Instant::now() + FENCE_WAIT, || {
                    let deadline = Instant::now() + END;
                    held.worker.end_before(deadline)?;
                    held.server.end_before(deadline)
                });
        match decided {
            Ok(Fenced::Kept) => Fence::Kept,
            Ok(Fenced::Committed) => Fence::Committed,
            Err(error) => {
                diagnostic("fence", &error);
                held.undecided.get_or_insert_with(Instant::now);
                Fence::Undecided
            }
        }
    }

    /// A restart that stays undecided for [`UNDECIDED`] ends: this Server ends the successor and
    /// its Worker through the handles it holds and leaves without taking the Session back, so the
    /// next start recovers it, adopting the Host as after a crash.
    fn undecided(&mut self) -> Next {
        let Some(held) = &self.held else {
            return Next::Serve;
        };
        if held
            .undecided
            .is_none_or(|since| since.elapsed() < UNDECIDED)
        {
            return Next::Serve;
        }
        let deadline = Instant::now() + END;
        let _ = held.worker.end_before(deadline);
        let _ = held.server.end_before(deadline);
        record("undecided");
        Next::Leave
    }

    /// Takes the Session back after a restart the record does not name: the store, the saver, the
    /// lifted hold, and a replacement Worker that adopts the Host as after a loss.
    fn take_back(
        &mut self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
        watch: &mut Watch<'_>,
    ) -> Next {
        self.held = None;
        record("taken-back");
        if !retake(owners) {
            return Next::Leave;
        }
        restart_saver(owners);
        lift(owners, shutdown);
        if watch.supervision.take_back(owners, shutdown)
            && !shutdown.stopping()
            && let Some(id) = own_stop_id()
        {
            shutdown.begin(id);
        }
        Next::Serve
    }
}

enum Decided {
    Kept,
    Committed,
}

/// Holds the Session for `ticket`'s successor and releases the store and the Worker to it.
fn prepare(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    clients: &ManagedClients,
    watch: &mut Watch<'_>,
    ticket: HandoverTicket,
) -> Result<Held, HandoverRefusal> {
    if shutdown.ending() || shutdown.offer_state() != ProcessState::Ready {
        return Err(HandoverRefusal::Busy);
    }
    let successor = match owners.session_record.successor() {
        Ok(Some(successor))
            if successor.ticket() == *ticket.as_bytes() && successor.worker_recorded() =>
        {
            successor
        }
        _ => return Err(HandoverRefusal::NotRecorded),
    };
    // The hold ends the pair should the record keep naming this generation, so it needs the right.
    let endable =
        |process: io::Result<Option<ExactProcess>>| process?.map(ExactProcess::endable).transpose();
    let (Ok(Some(server)), Ok(Some(worker))) = (
        endable(ExactProcess::successor_server(&successor)),
        endable(ExactProcess::successor_worker(&successor)),
    ) else {
        return Err(HandoverRefusal::NotRecorded);
    };
    if owners.checkpoint.is_none()
        || !owners.handle.is_active()
        || owners.carrier.is_finished()
        || !matches!(owners.worker.try_wait(), Ok(None))
    {
        return Err(HandoverRefusal::WorkerUnavailable);
    }
    let worker_id = checkpoint_worker(&owners.handle).map_err(|_| HandoverRefusal::Busy)?;
    hold(owners, shutdown, clients)?;
    if let Err(refusal) = capture(owners, shutdown, &worker_id) {
        let _ = retake(owners);
        restart_saver(owners);
        lift(owners, shutdown);
        return Err(refusal);
    }
    // From here the successor may hold the store, so the Session is only ever taken back.
    owners
        .terminals
        .terminal_runtime()
        .set(TerminalRuntimeState::Reconnecting);
    if let Err(error) = end_worker(owners, Instant::now() + RELEASE) {
        // A Worker that does not release its Host in time is ended; the Host keeps every terminal.
        diagnostic("release", &error);
        let deadline = Instant::now() + END;
        let _ = owners.worker.terminate_before(deadline);
        while !owners.carrier.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    owners.serving.set(None);
    watch.loss.released();
    Ok(Held {
        ticket,
        successor,
        server,
        worker,
        lease: Instant::now() + LEASE,
        undecided: None,
    })
}

/// Settles what the hold admitted and publishes the exact capture the successor starts from, then
/// releases the store.
fn capture(
    owners: &mut Owners,
    shutdown: &SessionShutdown,
    worker: &orchbee_workbench_protocol::WorkerId,
) -> Result<(), HandoverRefusal> {
    let deadline = Instant::now() + SETTLE;
    if !shutdown.settle_intents(deadline) {
        return Err(HandoverRefusal::Busy);
    }
    while owners.terminals.operation_pending() {
        if Instant::now() >= deadline {
            return Err(HandoverRefusal::Busy);
        }
        thread::sleep(Duration::from_millis(10));
    }
    if owners
        .terminals
        .background_programs(Instant::now() + PROGRAMS_DUE)
        != Some(0)
    {
        return Err(HandoverRefusal::BackgroundPrograms);
    }
    let kept = owners
        .checkpoint
        .as_mut()
        .ok_or(HandoverRefusal::WorkerUnavailable)?;
    kept.stop_saver_before(Instant::now() + SETTLE)
        .map_err(|_| HandoverRefusal::Busy)?;
    // The committed state with each terminal's last sampled directory: the terminals keep running
    // in the Host, so nothing is sampled again or relaunched, and the record matches exactly.
    let (_, bytes) = owners
        .terminals
        .cached_checkpoint(worker)
        .map_err(|_| HandoverRefusal::Busy)?;
    kept.publish_exact(&owners.terminals, worker, &bytes)
        .map_err(|_| HandoverRefusal::Busy)?;
    kept.release_store().map_err(|_| HandoverRefusal::Busy)
}

/// Holds the Worker, stops, every intent other than terminal records, and Clients that did not
/// offer `reconnect` for a restart. Refused, holding nothing, `busy` once a stop, an `exit` or a
/// `drain` came first, and `clients` while such a Client is connected or being admitted: the
/// successor's check saw none, but one may have connected since.
fn hold(
    owners: &Owners,
    shutdown: &SessionShutdown,
    clients: &ManagedClients,
) -> Result<(), HandoverRefusal> {
    shutdown
        .enter_hold(|| clients.rejoin())
        .map_err(|refusal| match refusal {
            HoldRefusal::Busy => HandoverRefusal::Busy,
            HoldRefusal::Clients => HandoverRefusal::Clients,
        })?;
    hold_registry(owners, true);
    Ok(())
}

fn lift(owners: &Owners, shutdown: &SessionShutdown) {
    shutdown.lift_hold();
    hold_registry(owners, false);
}

fn hold_registry(owners: &Owners, held: bool) {
    match owners.registry.lock() {
        Ok(mut registry) => registry.hold(held),
        Err(poisoned) => poisoned.into_inner().hold(held),
    }
}

/// Holds the store again; false when it stays out of reach.
fn retake(owners: &Owners) -> bool {
    let Some(kept) = &owners.checkpoint else {
        return true;
    };
    match kept.retake_store(Instant::now() + END) {
        Ok(()) => true,
        Err(error) => {
            diagnostic("retake", &error);
            false
        }
    }
}

/// Starts a saver in place of the stopped one, and move turns in place of stopped ones; a saver
/// that does not start is reported only.
fn restart_saver(owners: &mut Owners) {
    let Some(kept) = &mut owners.checkpoint else {
        return;
    };
    let started = kept
        .stop_saver_before(Instant::now() + END)
        .and_then(|()| checkpoint_worker(&owners.handle))
        .and_then(|worker| {
            kept.start_saver(&owners.terminals, worker.clone())?;
            if kept.turns_started() {
                kept.start_turns(&owners.terminals, &owners.handle, worker);
            }
            Ok(())
        });
    if let Err(error) = started {
        diagnostic("saver", &error);
    }
}

/// Ends this Server once its successor holds the Session, or a restart stayed undecided: no
/// receipt and no `exit` mark, so the Session record keeps naming whoever holds it.
pub(super) fn leave(
    mut owners: Owners,
    mut clients: ManagedClients,
    shutdown: &SessionShutdown,
) -> io::Result<()> {
    let deadline = Instant::now() + LEAVE;
    if let Err(error) = super::stop_background_jobs(&mut owners, deadline) {
        diagnostic("leave", &error);
    }
    shutdown.end(deadline);
    clients.stop_before(deadline)
}

fn record(step: &'static str) {
    orchbee_local_runtime::log_event(
        LogLevel::Warning,
        "handover",
        &[("step", LogValue::Token(step))],
    );
}

fn diagnostic(stage: &str, error: &io::Error) {
    eprintln!(
        "orchbee-server diagnostic component=lifecycle operation=handover stage={stage} cause=io:{:?}",
        error.kind()
    );
}
