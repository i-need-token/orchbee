//! Supervises the managed Session's Worker. At a loss it proves the Worker gone through the handle
//! it started that Worker with, then lets an admitted stop go on, gives up, has the Server stop
//! itself, or replaces the Worker with one that adopts the running Terminal Host, under a cap of
//! [`CAP_ATTEMPTS`] replacement attempts in any trailing [`CAP_WINDOW`]. It records `worker-lost`
//! and `worker-replaced` and states the terminal runtime to Clients. It never ends a Host.

use std::{
    collections::VecDeque,
    ffi::OsStr,
    io,
    process::ExitStatus,
    thread,
    time::{Duration, Instant},
};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptHostFailure, AdoptHostOutcome, AdoptHostPage, AdoptHostPageKey,
    AdoptHostProcess, AdoptHostRequest, AdoptHostTarget, MAXIMUM_ADOPT_HOST_PAGE_BINDINGS,
    OperationId, TerminalId, WorkerGeneration, WorkerId,
};
use orchbee_local_runtime::{HostPresence, LogField, LogLevel, LogValue, RetainedChild};

use super::stop::Owners;
use crate::{
    session_shutdown::SessionShutdown,
    terminal_coordinator::WorkerPlacement,
    terminal_runtime::TerminalRuntimeState,
    worker_obcn_carrier::{
        WorkerOperationRequest, WorkerOperationResult, WorkerRequestError, WorkerRequestHandle,
        WorkerResponseFailure,
    },
};

/// How long a lost Worker has to report its exit before the Server ends it.
const LOSS_EXIT_WAIT: Duration = Duration::from_secs(2);
/// How long a Worker the Server ended has to exit, and its connection's thread to finish.
const END_WAIT: Duration = Duration::from_secs(5);
/// Replacement attempts allowed in any trailing [`CAP_WINDOW`] of one Server process.
const CAP_ATTEMPTS: usize = 3;
const CAP_WINDOW: Duration = Duration::from_secs(10 * 60);
/// The pause after a failed attempt before the next one.
const RETRY_SPACING: Duration = Duration::from_secs(1);
/// The admission deadline of one replacement, from its creation.
const ADMISSION_BUDGET: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(10);

const WORKER_LOST: &str = "worker-lost";
const WORKER_REPLACED: &str = "worker-replaced";

/// Supervises the Worker the stop loop serves with, across its replacements.
pub(super) struct Supervision {
    cohort: String,
    /// When each replacement attempt in the cap's window started, oldest first.
    attempts: VecDeque<Instant>,
    /// The generation whose loss left no serving replacement.
    unreplaced: Option<std::num::NonZeroU64>,
}

impl Supervision {
    pub(super) fn new(cohort: &str) -> Self {
        Self {
            cohort: cohort.to_owned(),
            attempts: VecDeque::new(),
            unreplaced: None,
        }
    }

    /// One look of the stop loop. A loss of the serving Worker is handled here, which may block
    /// for one replacement's attempts; a stop or an `exit` ends a replacement at its next step
    /// boundary. Returns whether the Server should begin its own stop: a Worker lost without a
    /// serving replacement whose Host is gone.
    pub(super) fn look(&mut self, owners: &mut Owners, shutdown: &SessionShutdown) -> bool {
        let generation = owners.handle.generation();
        if self.unreplaced == Some(generation) {
            return super::stop::host_absent(owners);
        }
        if owners.handle.is_active() && !owners.carrier.is_finished() {
            return false;
        }
        self.unreplaced = Some(generation);
        self.lost(owners, shutdown)
    }

    fn lost(&mut self, owners: &mut Owners, shutdown: &SessionShutdown) -> bool {
        owners.serving.set(None);
        // Clients show Reconnecting from the loss itself, not after the seconds that proving the
        // Worker gone may take; giving up then turns it to GivenUp, and a stop ends the Session.
        if !shutdown.stopping() {
            owners
                .terminals
                .terminal_runtime()
                .set(TerminalRuntimeState::Reconnecting);
        }
        let proof = prove_gone(&mut owners.worker, &owners.carrier);
        self.prune(Instant::now());
        let lost = WorkerPlacement::of(&owners.handle);
        let decision = match (shutdown.stopping(), proof, lost) {
            (true, ..) => Decision::Stop(Reason::StopAdmitted),
            (false, Proof::NotGone, _) | (false, _, None) => {
                Decision::GiveUp(Reason::WorkerNotGone)
            }
            (false, Proof::Gone { .. }, Some(lost)) => self.decide(owners, lost),
        };
        lost_record(
            owners.handle.generation(),
            proof,
            self.attempts.len(),
            decision,
        );
        match decision {
            // The admitted stop goes on without a Worker.
            Decision::Stop(Reason::StopAdmitted) => false,
            Decision::Stop(_) => true,
            Decision::GiveUp(_) => {
                give_up(owners);
                false
            }
            Decision::Replace(target) => {
                // `decide` replaces only a Worker it can place.
                let Some(lost) = lost else {
                    give_up(owners);
                    return false;
                };
                self.replace(owners, shutdown, lost, target)
            }
        }
    }

    /// The action for a Worker proven gone while no stop is admitted.
    fn decide(&self, owners: &Owners, lost: WorkerPlacement) -> Decision {
        let presence = owners.session_record.host_presence();
        // A POSIX Host lock that no process holds also reads as gone for a Host that never
        // started; only a binding the lost Worker settled shows that one ran.
        #[cfg(unix)]
        let never_ran = || {
            owners
                .terminals
                .adoption_bindings(lost)
                .is_ok_and(|bindings| bindings.is_empty())
        };
        #[cfg(not(unix))]
        let never_ran = || {
            let _ = lost;
            false
        };
        let target = match presence {
            Ok(HostPresence::Spawning) => return Decision::Stop(Reason::HostSpawning),
            Ok(HostPresence::Gone) if never_ran() => AdoptHostTarget::NoHost,
            Ok(HostPresence::Gone) => return Decision::Stop(Reason::HostGone),
            Ok(HostPresence::NotStarted) => AdoptHostTarget::NoHost,
            Ok(HostPresence::Running(process)) => AdoptHostTarget::Host {
                process: process.and_then(|(pid, creation)| {
                    Some(AdoptHostProcess {
                        process_id: std::num::NonZeroU32::new(pid)?,
                        creation_time: std::num::NonZeroU64::new(creation)?,
                    })
                }),
                epoch: None,
            },
            Ok(HostPresence::Unknown) | Err(_) => return Decision::GiveUp(Reason::HostUnverified),
        };
        if self.attempts.len() >= CAP_ATTEMPTS {
            return Decision::GiveUp(Reason::CapReached);
        }
        Decision::Replace(target)
    }

    /// Runs replacement attempts one at a time, the first at once and each later one
    /// [`RETRY_SPACING`] after the previous failure, until one serves, the cap or a failure gives
    /// up, the Host is gone, or a stop or an `exit` ends the replacement. Returns whether the
    /// Server should begin its own stop.
    fn replace(
        &mut self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
        lost: WorkerPlacement,
        target: AdoptHostTarget,
    ) -> bool {
        loop {
            if shutdown.ending() {
                return false;
            }
            self.attempts.push_back(Instant::now());
            let failure = match self.attempt(owners, shutdown, lost, target) {
                Attempt::Adopted => {
                    self.unreplaced = None;
                    return false;
                }
                Attempt::Ended => return false,
                Attempt::Failed(failure) => failure,
            };
            self.prune(Instant::now());
            let next = match failure.class {
                Class::HostGone => Next::Stop,
                Class::GiveUp => Next::GiveUp,
                Class::Retry if self.attempts.len() >= CAP_ATTEMPTS => Next::GiveUp,
                Class::Retry => Next::Retry,
            };
            replaced_failed_record(&failure, next);
            match next {
                Next::Stop => return true,
                Next::GiveUp => {
                    give_up(owners);
                    return false;
                }
                Next::Retry => {}
            }
            let resume = Instant::now() + RETRY_SPACING;
            while Instant::now() < resume {
                if shutdown.ending() {
                    return false;
                }
                thread::sleep(POLL);
            }
        }
    }

    /// One replacement attempt: creation, admission and adoption. Once it adopts, the Session's
    /// bindings move to it, the jobs use it and it takes the lost Worker's place in `owners`.
    fn attempt(
        &self,
        owners: &mut Owners,
        shutdown: &SessionShutdown,
        lost: WorkerPlacement,
        target: AdoptHostTarget,
    ) -> Attempt {
        let started = Instant::now();
        let failed = |stage, failure, class, generation| {
            Attempt::Failed(Failure {
                stage,
                failure,
                class,
                generation,
                elapsed: started.elapsed(),
            })
        };
        let mut child = match spawn(owners, lost.worker_id, started + ADMISSION_BUDGET) {
            Ok(child) => child,
            Err(error) => {
                return failed(Stage::Create, io_token(error.kind()), Class::Retry, None);
            }
        };
        let (carrier, handle) = match self.admit(owners, shutdown, &mut child, started) {
            Admission::Admitted(carrier, handle) => (carrier, handle),
            Admission::Ended => {
                end_replacement(&mut child, None);
                return Attempt::Ended;
            }
            Admission::Failed(failure) => {
                let class = if end_replacement(&mut child, None) {
                    Class::Retry
                } else {
                    Class::GiveUp
                };
                return failed(Stage::Admit, failure, class, None);
            }
        };
        let generation = handle.generation();
        let adopted = adopt(owners, shutdown, &handle, lost, target);
        let (sent, outcome) = match adopted {
            Adoption::Ended => {
                end_replacement(&mut child, Some(carrier));
                return Attempt::Ended;
            }
            Adoption::Failed(failure, class) => {
                let class = if end_replacement(&mut child, Some(carrier)) {
                    class
                } else {
                    Class::GiveUp
                };
                return failed(Stage::Adopt, failure, class, Some(generation));
            }
            Adoption::Adopted { sent, outcome } => (sent, outcome),
        };
        // A stop or `exit` admitted while the answer was owed ends the replacement here.
        if shutdown.ending() {
            end_replacement(&mut child, Some(carrier));
            return Attempt::Ended;
        }
        let served = owners
            .terminals
            .serve_replacement(lost, generation, &sent, &outcome.missing);
        if !matches!(served, Ok(Some(_))) {
            let class = if end_replacement(&mut child, Some(carrier)) {
                Class::Retry
            } else {
                Class::GiveUp
            };
            return failed(Stage::Adopt, "unavailable", class, Some(generation));
        }
        let lost_worker = std::mem::replace(&mut owners.worker, child);
        let lost_carrier = std::mem::replace(&mut owners.carrier, carrier);
        owners.handle = handle;
        // The lost Worker's connection thread finished before the Worker was proven gone.
        let _ = lost_carrier.join();
        drop(lost_worker);
        owners.serving.set(owners.worker.id());
        owners
            .terminals
            .terminal_runtime()
            .set(TerminalRuntimeState::Serving);
        adopted_record(generation, &outcome, started.elapsed());
        Attempt::Adopted
    }

    /// Accepts Worker connections until the replacement `child` is admitted with the lost
    /// Worker's id, within [`ADMISSION_BUDGET`] of `started`.
    fn admit(
        &self,
        owners: &Owners,
        shutdown: &SessionShutdown,
        child: &mut RetainedChild,
        started: Instant,
    ) -> Admission {
        let deadline = started + ADMISSION_BUDGET;
        let expected = owners.handle.worker_id();
        let mut carrier = None;
        loop {
            // An admitted replacement is the attempt's from here on, so a stop ends it with its
            // connection's thread.
            if let Some(carrier) = carrier.take() {
                let handle = owners
                    .registry
                    .lock()
                    .ok()
                    .and_then(|registry| registry.adopting_handle());
                match handle {
                    Some(handle) => return Admission::Admitted(carrier, handle),
                    None => return Admission::Failed("unavailable"),
                }
            }
            if shutdown.ending() {
                return Admission::Ended;
            }
            if !matches!(child.try_wait(), Ok(None)) {
                return Admission::Failed("exited");
            }
            if Instant::now() >= deadline {
                return Admission::Failed("timed-out");
            }
            match owners.listener.accept() {
                Ok((stream, _)) => {
                    carrier = super::super::admit_candidate(
                        stream,
                        &owners.registry,
                        &self.cohort,
                        Some(&owners.agents),
                        child.id(),
                        Some(expected),
                        Some(super::wake_on_end(&owners.waker)),
                    );
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => thread::sleep(POLL),
            }
        }
    }

    fn prune(&mut self, now: Instant) {
        while self
            .attempts
            .front()
            .is_some_and(|started| now.saturating_duration_since(*started) >= CAP_WINDOW)
        {
            self.attempts.pop_front();
        }
    }
}

/// Starts the replacement Worker with the Session's Worker id: on Windows through the Session
/// record's replacement transition, which only the recorded Server takes and which keeps the
/// Host's registration.
fn spawn(owners: &Owners, worker_id: WorkerId, deadline: Instant) -> io::Result<RetainedChild> {
    let runtime = owners.session.runtime_directory();
    let worker_id = format!("{:032x}", u128::from_be_bytes(worker_id.as_bytes()));
    let arguments = [
        OsStr::new("--runtime-directory"),
        runtime.as_os_str(),
        OsStr::new("--worker-id"),
        OsStr::new(&worker_id),
    ];
    #[cfg(windows)]
    return orchbee_local_runtime::spawn_session_child(
        &orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?,
        runtime,
        orchbee_local_runtime::SessionChild::ReplacementWorker,
        &owners.worker_executable,
        &arguments,
        deadline,
    );
    #[cfg(not(windows))]
    {
        let _ = deadline;
        orchbee_local_runtime::spawn_retained(&owners.worker_executable, &arguments)
    }
}

/// Sends the replacement its `AdoptHost` pages, its first requests, and reads the last page's
/// answer. A stop or an `exit` ends the replacement before the next page.
fn adopt(
    owners: &Owners,
    shutdown: &SessionShutdown,
    handle: &WorkerRequestHandle,
    lost: WorkerPlacement,
    target: AdoptHostTarget,
) -> Adoption {
    let Ok(bindings) = owners.terminals.adoption_bindings(lost) else {
        return Adoption::Failed("unavailable", Class::Retry);
    };
    // A target that names no Host carries no binding, and a Host target names the epoch its
    // bindings share as they stand now.
    let (target, bindings) = match target {
        AdoptHostTarget::NoHost => (target, Vec::new()),
        AdoptHostTarget::Host { process, .. } => (
            AdoptHostTarget::Host {
                process,
                epoch: shared_epoch(&bindings),
            },
            bindings,
        ),
    };
    let (Ok(adoption_id), Ok(worker_id)) = (
        random_operation(),
        WorkerId::from_bytes(handle.worker_id().as_bytes()),
    ) else {
        return Adoption::Failed("unavailable", Class::Retry);
    };
    let pages: Vec<&[AdoptBinding]> = if bindings.is_empty() {
        vec![&[]]
    } else {
        bindings.chunks(MAXIMUM_ADOPT_HOST_PAGE_BINDINGS).collect()
    };
    let count = pages.len();
    for (index, page) in pages.into_iter().enumerate() {
        if shutdown.ending() {
            return Adoption::Ended;
        }
        let last = index + 1 == count;
        let Ok(index) = u16::try_from(index) else {
            return Adoption::Failed("unavailable", Class::Retry);
        };
        let key = AdoptHostPageKey {
            adoption_id,
            worker_id,
            worker_generation: WorkerGeneration::new(handle.generation()),
            page: AdoptHostPage { index, last },
        };
        let Ok(request) = AdoptHostRequest::new(key, target, page.to_vec()) else {
            return Adoption::Failed("unavailable", Class::Retry);
        };
        let outcome = match handle.request(WorkerOperationRequest::AdoptHost(request)) {
            Ok(WorkerOperationResult::AdoptHost(result)) => result.into_outcome(),
            Ok(_) => return Adoption::Failed("protocol", Class::Retry),
            Err(WorkerRequestError::Declined) => return Adoption::Failed("declined", Class::Retry),
            Err(WorkerRequestError::PostDeliveryOutcomeUnknown(
                WorkerResponseFailure::TimedOut,
            )) => {
                return Adoption::Failed("timed-out", Class::Retry);
            }
            Err(_) => return Adoption::Failed("unavailable", Class::Retry),
        };
        match outcome {
            AdoptHostOutcome::PageAccepted if !last => {}
            AdoptHostOutcome::Adopted {
                imported,
                quarantined,
                missing,
            } if last => {
                return Adoption::Adopted {
                    sent: bindings.iter().map(|binding| binding.terminal_id).collect(),
                    outcome: Adopted {
                        imported,
                        quarantined,
                        missing,
                    },
                };
            }
            AdoptHostOutcome::Failed(failure) => {
                let class = match failure {
                    AdoptHostFailure::HostGone => Class::HostGone,
                    AdoptHostFailure::HostUnverified
                    | AdoptHostFailure::HostMismatch
                    | AdoptHostFailure::HostRefused => Class::GiveUp,
                    AdoptHostFailure::HostUnreachable | AdoptHostFailure::InventoryFailed => {
                        Class::Retry
                    }
                };
                return Adoption::Failed(adopt_failure_token(failure), class);
            }
            AdoptHostOutcome::PageAccepted | AdoptHostOutcome::Adopted { .. } => {
                return Adoption::Failed("protocol", Class::Retry);
            }
        }
    }
    Adoption::Failed("protocol", Class::Retry)
}

/// Proves a lost Worker gone through the handle the Server started it with: up to
/// [`LOSS_EXIT_WAIT`] for its exit, then ended through that handle and up to [`END_WAIT`] more.
/// Its connection's thread, which retires its registration, must finish within the same wait.
fn prove_gone(worker: &mut RetainedChild, carrier: &thread::JoinHandle<()>) -> Proof {
    let waited = Instant::now() + LOSS_EXIT_WAIT;
    let mut exited = None;
    while exited.is_none() && Instant::now() < waited {
        match worker.try_wait() {
            Ok(Some(status)) => exited = Some(status),
            Ok(None) => thread::sleep(POLL),
            Err(_) => break,
        }
    }
    let proof = match exited {
        Some(status) => Proof::Gone {
            disconnected: false,
            status: Some(status),
        },
        None => match worker.terminate_before(Instant::now() + END_WAIT) {
            Ok(status) => Proof::Gone {
                disconnected: true,
                status: Some(status),
            },
            Err(_) => return Proof::NotGone,
        },
    };
    let finished = Instant::now() + END_WAIT;
    while !carrier.is_finished() {
        if Instant::now() >= finished {
            return Proof::NotGone;
        }
        thread::sleep(POLL);
    }
    proof
}

/// Ends a replacement that will not serve through the Server's handle, and waits for its
/// connection's thread. Returns whether it is proven gone.
fn end_replacement(child: &mut RetainedChild, carrier: Option<thread::JoinHandle<()>>) -> bool {
    if child.terminate_before(Instant::now() + END_WAIT).is_err() {
        return false;
    }
    let Some(carrier) = carrier else {
        return true;
    };
    let finished = Instant::now() + END_WAIT;
    while !carrier.is_finished() {
        if Instant::now() >= finished {
            return false;
        }
        thread::sleep(POLL);
    }
    carrier.join().is_ok()
}

fn give_up(owners: &Owners) {
    owners
        .terminals
        .terminal_runtime()
        .set(TerminalRuntimeState::GivenUp);
}

/// The Host epoch every binding names, when they agree.
fn shared_epoch(bindings: &[AdoptBinding]) -> Option<orchbee_capability_node_protocol::HostEpoch> {
    let epoch = bindings.first()?.identity.host_epoch();
    bindings
        .iter()
        .all(|binding| binding.identity.host_epoch() == epoch)
        .then_some(epoch)
}

fn random_operation() -> io::Result<OperationId> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    OperationId::from_bytes(bytes).map_err(io::Error::other)
}

#[derive(Clone, Copy, Debug)]
enum Proof {
    /// `disconnected` when the process still ran after its connection ended, and the Server ended
    /// it.
    Gone {
        disconnected: bool,
        status: Option<ExitStatus>,
    },
    NotGone,
}

#[derive(Clone, Copy, Debug)]
enum Reason {
    StopAdmitted,
    WorkerNotGone,
    HostGone,
    HostSpawning,
    HostUnverified,
    CapReached,
}

#[derive(Clone, Copy, Debug)]
enum Decision {
    Replace(AdoptHostTarget),
    Stop(Reason),
    GiveUp(Reason),
}

enum Attempt {
    Adopted,
    /// A stop or an `exit` ended the replacement; it goes on without a Worker.
    Ended,
    Failed(Failure),
}

enum Admission {
    Admitted(thread::JoinHandle<()>, WorkerRequestHandle),
    Ended,
    Failed(&'static str),
}

enum Adoption {
    Adopted {
        sent: Vec<TerminalId>,
        outcome: Adopted,
    },
    Ended,
    Failed(&'static str, Class),
}

struct Adopted {
    imported: u32,
    quarantined: u32,
    missing: Vec<TerminalId>,
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Create,
    Admit,
    Adopt,
}

/// What a failure allows next, before the cap is counted.
#[derive(Clone, Copy, Debug)]
enum Class {
    Retry,
    HostGone,
    GiveUp,
}

#[derive(Clone, Copy, Debug)]
enum Next {
    Retry,
    Stop,
    GiveUp,
}

struct Failure {
    stage: Stage,
    failure: &'static str,
    class: Class,
    /// The replacement's generation, once it was admitted.
    generation: Option<std::num::NonZeroU64>,
    elapsed: Duration,
}

fn adopt_failure_token(failure: AdoptHostFailure) -> &'static str {
    match failure {
        AdoptHostFailure::HostGone => "host-gone",
        AdoptHostFailure::HostUnreachable => "host-unreachable",
        AdoptHostFailure::HostUnverified => "host-unverified",
        AdoptHostFailure::HostMismatch => "host-mismatch",
        AdoptHostFailure::HostRefused => "host-refused",
        AdoptHostFailure::InventoryFailed => "inventory-failed",
    }
}

fn io_token(kind: io::ErrorKind) -> &'static str {
    match LogValue::from(kind) {
        LogValue::Token(token) => token,
        LogValue::Count(_) | LogValue::Flag(_) => "other",
    }
}

fn lost_record(
    generation: std::num::NonZeroU64,
    proof: Proof,
    attempts: usize,
    decision: Decision,
) {
    let mut fields: Vec<LogField> = Vec::with_capacity(7);
    fields.push(("generation", LogValue::Count(generation.get())));
    let status = match proof {
        Proof::Gone {
            disconnected,
            status,
        } => {
            fields.push((
                "cause",
                LogValue::Token(if disconnected {
                    "disconnected"
                } else {
                    "exited"
                }),
            ));
            status
        }
        Proof::NotGone => {
            fields.push(("cause", LogValue::Token("disconnected")));
            None
        }
    };
    if let Some(status) = status {
        push_status(&mut fields, status);
    }
    fields.push(("attempts", LogValue::Count(attempts as u64)));
    let (action, reason) = match decision {
        Decision::Replace(_) => ("replace", None),
        Decision::Stop(reason) => ("stop", Some(reason)),
        Decision::GiveUp(reason) => ("give-up", Some(reason)),
    };
    fields.push(("action", LogValue::Token(action)));
    if let Some(reason) = reason {
        fields.push(("reason", LogValue::Token(reason_token(reason))));
    }
    orchbee_local_runtime::log_event(LogLevel::Error, WORKER_LOST, &fields);
}

fn adopted_record(generation: std::num::NonZeroU64, outcome: &Adopted, elapsed: Duration) {
    let fields: [LogField; 6] = [
        ("generation", LogValue::Count(generation.get())),
        ("outcome", LogValue::Token("adopted")),
        ("imported", LogValue::Count(u64::from(outcome.imported))),
        (
            "quarantined",
            LogValue::Count(u64::from(outcome.quarantined)),
        ),
        ("missing", LogValue::Count(outcome.missing.len() as u64)),
        ("elapsedMs", LogValue::Count(milliseconds(elapsed))),
    ];
    orchbee_local_runtime::log_event(LogLevel::Info, WORKER_REPLACED, &fields);
}

fn replaced_failed_record(failure: &Failure, next: Next) {
    let mut fields: Vec<LogField> = Vec::with_capacity(6);
    if let Some(generation) = failure.generation {
        fields.push(("generation", LogValue::Count(generation.get())));
    }
    fields.push(("outcome", LogValue::Token("failed")));
    fields.push((
        "stage",
        LogValue::Token(match failure.stage {
            Stage::Create => "create",
            Stage::Admit => "admit",
            Stage::Adopt => "adopt",
        }),
    ));
    fields.push(("failure", LogValue::Token(failure.failure)));
    fields.push((
        "next",
        LogValue::Token(match next {
            Next::Retry => "retry",
            Next::Stop => "stop",
            Next::GiveUp => "give-up",
        }),
    ));
    fields.push(("elapsedMs", LogValue::Count(milliseconds(failure.elapsed))));
    orchbee_local_runtime::log_event(LogLevel::Error, WORKER_REPLACED, &fields);
}

fn reason_token(reason: Reason) -> &'static str {
    match reason {
        Reason::StopAdmitted => "stop-admitted",
        Reason::WorkerNotGone => "worker-not-gone",
        Reason::HostGone => "host-gone",
        Reason::HostSpawning => "host-spawning",
        Reason::HostUnverified => "host-unverified",
        Reason::CapReached => "cap-reached",
    }
}

fn push_status(fields: &mut Vec<LogField>, status: ExitStatus) {
    let field = match status.code() {
        Some(code) => Some(("exitCode", code)),
        #[cfg(unix)]
        None => std::os::unix::process::ExitStatusExt::signal(&status)
            .map(|signal| ("exitSignal", signal)),
        #[cfg(not(unix))]
        None => None,
    };
    if let Some((name, value)) = field {
        fields.push((name, LogValue::Count(u64::from(value as u32))));
    }
}

fn milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
