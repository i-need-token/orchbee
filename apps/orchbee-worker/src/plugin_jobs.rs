//! Retained plugin and configured process owners, independent of CLI connections or terminal Hosts.
//! Where the Terminal Host runs background programs (`background-programs`), it runs them instead
//! of this Worker, and they outlive the Worker; this owner then keeps only their records.
use orchbee_capability_node_protocol::{
    OperationId, PluginCommandFailure as Failure, PluginCommandIntent, PluginCommandLog,
    PluginJobAction, PluginJobOutcome as Outcome, PluginJobRefusal as Refusal,
};
use orchbee_terminal_protocol::ProgramId;
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod configured;
mod hosted;
mod process;
pub(crate) use hosted::{ProgramHost, Reply as ProgramReply};
const MAX_RUNNING: usize = 32;
const MAX_LOGS: usize = 200;
type Execute = fn(PluginCommandIntent, PluginCommandLog, &AtomicBool) -> PluginCommandLog;
enum State {
    Running {
        cancel: Arc<AtomicBool>,
        join: JoinHandle<PluginCommandLog>,
    },
    /// The Terminal Host runs it as this program.
    Hosted {
        program: ProgramId,
    },
    Complete,
}
struct Record {
    /// The fingerprint of the intent that started it.
    fingerprint: u64,
    log: PluginCommandLog,
    state: State,
}
struct Inner {
    records: Vec<Record>,
    commands: Vec<configured::Record>,
    /// Programs the Host still lists after their exit was collected, disposed of again later.
    disposals: Vec<ProgramId>,
    /// Exits whose output the Host did not answer for, collected again later.
    pending_exits: Vec<orchbee_terminal_protocol::ProgramExited>,
    next_sequence: u64,
    stopping: bool,
    retired: bool,
}
pub(crate) struct PluginJobs {
    inner: Mutex<Inner>,
    execute: Execute,
}
impl PluginJobs {
    pub(crate) fn new() -> Self {
        Self::with_execute(process::run)
    }
    pub(crate) fn with_execute(execute: Execute) -> Self {
        Self {
            inner: Mutex::new(Inner {
                records: Vec::new(),
                commands: Vec::new(),
                disposals: Vec::new(),
                pending_exits: Vec::new(),
                next_sequence: 1,
                stopping: false,
                retired: false,
            }),
            execute,
        }
    }
    #[cfg(test)]
    pub(crate) fn execute(&self, id: OperationId, action: PluginJobAction) -> Outcome {
        self.execute_on(id, action, None)
    }
    /// Serves one plugin-job action. `host` is the Terminal Host that runs background programs for
    /// this Worker, when it has one that does.
    pub(crate) fn execute_on(
        &self,
        id: OperationId,
        action: PluginJobAction,
        host: Option<&dyn ProgramHost>,
    ) -> Outcome {
        if action.validate().is_err() {
            return Outcome::Refused(Refusal::InvalidIntent);
        }
        let Ok(mut inner) = self.inner.lock() else {
            return Outcome::Refused(Refusal::PhysicalUnknown);
        };
        settle(&mut inner);
        if let Some(host) = host {
            hosted::settle(&mut inner, host);
        }
        let start_host = host.filter(|host| host.starts());
        match action {
            PluginJobAction::StartConfigured(intent) => match start_host {
                None => configured::start(&mut inner, id, intent),
                Some(host) => match configured::admission(&inner, id, &intent) {
                    Ok(fingerprint) => {
                        hosted::start_configured(&mut inner, id, &intent, fingerprint, host)
                    }
                    Err(outcome) => outcome,
                },
            },
            PluginJobAction::Start(intent) => self.start(&mut inner, id, intent, start_host),
            PluginJobAction::Get { job_id } => Outcome::Logs {
                logs: inner
                    .records
                    .iter()
                    .filter(|r| r.log.job_id == job_id)
                    .map(|r| r.log.clone())
                    .collect(),
                next_before: None,
            },
            PluginJobAction::List {
                plugin_id,
                before,
                limit,
            } => {
                let mut matching = inner.records.iter().rev().filter(|r| {
                    plugin_id.as_ref().is_none_or(|id| *id == r.log.plugin_id)
                        && before.is_none_or(|seq| r.log.sequence < seq)
                });
                let logs = matching
                    .by_ref()
                    .take(usize::from(limit))
                    .map(|r| r.log.clone())
                    .collect::<Vec<_>>();
                let next_before = if matching.next().is_some() {
                    logs.last().map(|l| l.sequence)
                } else {
                    None
                };
                Outcome::Logs { logs, next_before }
            }
            PluginJobAction::Cancel { job_id } => {
                match inner.records.iter().find(|r| r.log.job_id == job_id) {
                    None => Outcome::Refused(Refusal::NotFound),
                    Some(Record {
                        state: State::Running { cancel, .. },
                        ..
                    }) => {
                        cancel.store(true, Ordering::Relaxed);
                        Outcome::Cancellation {
                            already_complete: false,
                        }
                    }
                    Some(Record {
                        state: State::Hosted { program },
                        ..
                    }) => hosted::cancel(host, *program),
                    Some(_) => Outcome::Cancellation {
                        already_complete: true,
                    },
                }
            }
            // A retired owner cannot vouch that its processes ended, so it answers no count.
            PluginJobAction::Running if inner.retired => Outcome::Refused(Refusal::PhysicalUnknown),
            PluginJobAction::Running => Outcome::Running {
                plugin_jobs: count(running_plugin_jobs(&inner)),
                configured_commands: count(running_commands(&inner)),
                host_programs: count(hosted_programs(&inner)),
            },
        }
    }
    fn start(
        &self,
        inner: &mut Inner,
        id: OperationId,
        intent: PluginCommandIntent,
        host: Option<&dyn ProgramHost>,
    ) -> Outcome {
        if inner.commands.iter().any(|record| record.id == id) {
            return Outcome::Refused(Refusal::Conflict);
        }
        let fingerprint = fingerprint(&intent);
        if let Some(record) = inner.records.iter().find(|r| r.log.job_id == id) {
            return if record.fingerprint == fingerprint {
                Outcome::Started {
                    log: Box::new(record.log.clone()),
                    existing: true,
                }
            } else {
                Outcome::Refused(Refusal::Conflict)
            };
        }
        if inner.retired {
            return Outcome::Refused(Refusal::PhysicalUnknown);
        }
        if inner.stopping || inner.next_sequence >= 9_007_199_254_740_991 {
            return Outcome::Refused(Refusal::Unavailable);
        }
        if running(inner) >= MAX_RUNNING {
            return Outcome::Refused(Refusal::Busy);
        }
        if let Some(host) = host
            && let Some(outcome) = hosted::start_plugin(inner, id, &intent, fingerprint, host)
        {
            return outcome;
        }
        let log = new_log(inner, id, &intent);
        let (admit, admitted) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_log = log.clone();
        let execute = self.execute;
        let join = match thread::Builder::new()
            .name("orchbee-plugin-command".into())
            .spawn(move || {
                if admitted.recv().is_err() {
                    return finish_failure(worker_log, Failure::Cancelled);
                }
                execute(intent, worker_log, &worker_cancel)
            }) {
            Ok(join) => join,
            Err(_) => return Outcome::Refused(Refusal::Unavailable),
        };
        crate::plugin_jobs::admit(
            inner,
            fingerprint,
            log.clone(),
            State::Running { cancel, join },
        );
        if admit.send(()).is_err() {
            inner.retired = true;
        }
        Outcome::Started {
            log: Box::new(log),
            existing: false,
        }
    }
    /// Rebuilds the records of the programs `host` already runs for the Worker this one replaces,
    /// and collects those that exited. A program a list that could not be read whole left out is
    /// disposed of, unrecorded, when it exits.
    pub(crate) fn adopt(&self, host: &dyn ProgramHost) {
        if let Ok(mut inner) = self.inner.lock() {
            hosted::adopt(&mut inner, host);
        }
    }
    /// Closes admission and signals every running owner without waiting for any of them. The
    /// programs the Host runs outlive this Worker and are left running.
    pub(crate) fn request_stop(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        request_stop(&mut inner);
    }
    /// Requests stop, then observes every owner's completion before the deadline. `Ok` means
    /// every owner has ended and been joined; `TimedOut` leaves the stop requested and pending.
    pub(crate) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        loop {
            {
                let mut inner = self
                    .inner
                    .lock()
                    .map_err(|_| io::Error::other("Plugin process owner poisoned"))?;
                settle(&mut inner);
                if !request_stop(&mut inner) {
                    return if inner.retired {
                        Err(io::Error::other("Plugin process cleanup unknown"))
                    } else {
                        Ok(())
                    };
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
}
/// The unfinished log a new plugin job starts with, taking the next sequence.
fn new_log(inner: &Inner, id: OperationId, intent: &PluginCommandIntent) -> PluginCommandLog {
    PluginCommandLog {
        job_id: id,
        sequence: inner.next_sequence,
        plugin_id: intent.plugin_id.clone(),
        command_id: intent.command_id.clone(),
        command: intent.argv.clone(),
        started_ms: now_ms(),
        finished_ms: None,
        exit_code: None,
        failure: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        stdout_truncated: false,
        stderr_truncated: false,
    }
}
/// Retains a new plugin job's record, making room by dropping the oldest completed one, and
/// advances the sequence past it.
fn admit(inner: &mut Inner, fingerprint: u64, log: PluginCommandLog, state: State) {
    if inner.records.len() == MAX_LOGS {
        let index = inner
            .records
            .iter()
            .position(|r| matches!(r.state, State::Complete))
            .expect("fewer running records than retained logs");
        inner.records.remove(index);
    }
    inner.next_sequence = inner.next_sequence.max(log.sequence + 1);
    inner.records.push(Record {
        fingerprint,
        log,
        state,
    });
}
fn fingerprint(intent: &PluginCommandIntent) -> u64 {
    intent
        .argv
        .iter()
        .fold(
            Fingerprint::new()
                .field(intent.plugin_id.as_bytes())
                .field(intent.command_id.as_bytes())
                .field(intent.directory.as_bytes())
                .field(&(intent.argv.len() as u64).to_le_bytes()),
            |print, argument| print.field(argument.as_bytes()),
        )
        .variables(&intent.environment)
        .get()
}
/// Closes admission and signals each running owner; reports whether any owner is still running.
fn request_stop(inner: &mut Inner) -> bool {
    inner.stopping = true;
    let mut running = false;
    for record in &inner.records {
        if let State::Running { cancel, .. } = &record.state {
            cancel.store(true, Ordering::Relaxed);
            running = true;
        }
    }
    configured::cancel(&inner.commands) || running
}
fn settle(inner: &mut Inner) {
    configured::settle(inner);
    for record in &mut inner.records {
        if !matches!(&record.state,State::Running{join,..} if join.is_finished()) {
            continue;
        }
        let State::Running { join, .. } = std::mem::replace(&mut record.state, State::Complete)
        else {
            unreachable!()
        };
        let log = match join.join() {
            Ok(log)
                if log.validate().is_ok()
                    && log.job_id == record.log.job_id
                    && log.sequence == record.log.sequence
                    && log.finished_ms.is_some() =>
            {
                log
            }
            _ => finish_failure(record.log.clone(), Failure::PhysicalUnknown),
        };
        inner.retired |= log.failure == Some(Failure::PhysicalUnknown);
        record.log = log;
    }
}
impl Drop for PluginJobs {
    fn drop(&mut self) {
        // The bounded `stop_before` owns completion. Drop only requests stop and joins
        // owners that have already finished; an unfinished owner is never waited for here.
        let inner = self.inner.get_mut().unwrap_or_else(|e| e.into_inner());
        settle(inner);
        request_stop(inner);
    }
}
/// Every running program, the Worker's own and those its Host runs, which admission bounds.
fn running(inner: &Inner) -> usize {
    running_plugin_jobs(inner) + running_commands(inner) + hosted_programs(inner)
}
fn running_plugin_jobs(inner: &Inner) -> usize {
    inner
        .records
        .iter()
        .filter(|r| matches!(r.state, State::Running { .. }))
        .count()
}
fn running_commands(inner: &Inner) -> usize {
    inner.commands.iter().filter(|r| r.running()).count()
}
fn hosted_programs(inner: &Inner) -> usize {
    let plugin_jobs = inner
        .records
        .iter()
        .filter(|r| matches!(r.state, State::Hosted { .. }))
        .count();
    plugin_jobs + inner.commands.iter().filter(|r| r.hosted()).count()
}
/// Admission keeps each count within [`MAX_RUNNING`]; a larger one still reads as running.
fn count(running: usize) -> u8 {
    u8::try_from(running).unwrap_or(u8::MAX)
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(9_007_199_254_740_991) as u64
}
fn finish_failure(mut log: PluginCommandLog, reason: Failure) -> PluginCommandLog {
    log.finished_ms = Some(now_ms().max(log.started_ms));
    log.exit_code = None;
    log.failure = Some(reason);
    log
}
/// A fingerprint of an intent's fields, FNV-1a over each field's length and bytes, so a start
/// repeated under the same operation id is recognized, also by a replacement Worker.
struct Fingerprint(u64);
impl Fingerprint {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn field(mut self, bytes: &[u8]) -> Self {
        let length = (bytes.len() as u64).to_le_bytes();
        for byte in length.iter().chain(bytes) {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self
    }
    fn variables(self, variables: &[(String, String)]) -> Self {
        let counted = self.field(&(variables.len() as u64).to_le_bytes());
        variables.iter().fold(counted, |print, (name, value)| {
            print.field(name.as_bytes()).field(value.as_bytes())
        })
    }
    const fn get(&self) -> u64 {
        self.0
    }
}
#[cfg(test)]
mod tests;
