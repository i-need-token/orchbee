//! Retained plugin and configured process owners, independent of CLI connections or terminal Hosts.
use orchbee_capability_node_protocol::{
    OperationId, PluginCommandFailure as Failure, PluginCommandIntent, PluginCommandLog,
    PluginJobAction, PluginJobOutcome as Outcome, PluginJobRefusal as Refusal,
};
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
mod process;
const MAX_RUNNING: usize = 32;
const MAX_LOGS: usize = 200;
type Execute = fn(PluginCommandIntent, PluginCommandLog, &AtomicBool) -> PluginCommandLog;
enum State {
    Running {
        cancel: Arc<AtomicBool>,
        join: JoinHandle<PluginCommandLog>,
    },
    Complete,
}
struct Record {
    intent: PluginCommandIntent,
    log: PluginCommandLog,
    state: State,
}
struct Inner {
    records: Vec<Record>,
    commands: Vec<configured::Record>,
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
    fn with_execute(execute: Execute) -> Self {
        Self {
            inner: Mutex::new(Inner {
                records: Vec::new(),
                commands: Vec::new(),
                next_sequence: 1,
                stopping: false,
                retired: false,
            }),
            execute,
        }
    }
    pub(crate) fn execute(&self, id: OperationId, action: PluginJobAction) -> Outcome {
        if action.validate().is_err() {
            return Outcome::Refused(Refusal::InvalidIntent);
        }
        let Ok(mut inner) = self.inner.lock() else {
            return Outcome::Refused(Refusal::PhysicalUnknown);
        };
        settle(&mut inner);
        match action {
            PluginJobAction::StartConfigured(intent) => configured::start(&mut inner, id, intent),
            PluginJobAction::Start(intent) => self.start(&mut inner, id, intent),
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
                    Some(_) => Outcome::Cancellation {
                        already_complete: true,
                    },
                }
            }
        }
    }
    fn start(&self, inner: &mut Inner, id: OperationId, intent: PluginCommandIntent) -> Outcome {
        if inner.commands.iter().any(|record| record.id == id) {
            return Outcome::Refused(Refusal::Conflict);
        }
        if let Some(record) = inner.records.iter().find(|r| r.log.job_id == id) {
            return if record.intent == intent {
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
        let log = PluginCommandLog {
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
        };
        let (admit, admitted) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_log = log.clone();
        let worker_intent = intent.clone();
        let execute = self.execute;
        let join = match thread::Builder::new()
            .name("orchbee-plugin-command".into())
            .spawn(move || {
                if admitted.recv().is_err() {
                    return finish_failure(worker_log, Failure::Cancelled);
                }
                execute(worker_intent, worker_log, &worker_cancel)
            }) {
            Ok(join) => join,
            Err(_) => return Outcome::Refused(Refusal::Unavailable),
        };
        if inner.records.len() == MAX_LOGS {
            let index = inner
                .records
                .iter()
                .position(|r| matches!(r.state, State::Complete))
                .expect("at most32 running records");
            inner.records.remove(index);
        }
        inner.next_sequence += 1;
        inner.records.push(Record {
            intent,
            log: log.clone(),
            state: State::Running { cancel, join },
        });
        if admit.send(()).is_err() {
            inner.retired = true;
        }
        Outcome::Started {
            log: Box::new(log),
            existing: false,
        }
    }
    pub(crate) fn stop_before(&self, deadline: Instant) -> io::Result<()> {
        loop {
            {
                let mut inner = self
                    .inner
                    .lock()
                    .map_err(|_| io::Error::other("Plugin process owner poisoned"))?;
                inner.stopping = true;
                settle(&mut inner);
                let mut running = false;
                for record in &inner.records {
                    if let State::Running { cancel, .. } = &record.state {
                        cancel.store(true, Ordering::Relaxed);
                        running = true;
                    }
                }
                running |= configured::cancel(&inner.commands);
                if !running {
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
        let inner = self.inner.get_mut().unwrap_or_else(|e| e.into_inner());
        configured::cancel(&inner.commands);
        for record in &inner.records {
            if let State::Running { cancel, .. } = &record.state {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        for record in &mut inner.records {
            if let State::Running { join, .. } =
                std::mem::replace(&mut record.state, State::Complete)
            {
                let _settled = join.join();
            }
        }
        configured::join(&mut inner.commands);
    }
}
fn running(inner: &Inner) -> usize {
    inner
        .records
        .iter()
        .filter(|r| matches!(r.state, State::Running { .. }))
        .count()
        + inner.commands.iter().filter(|r| r.running()).count()
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
#[cfg(test)]
mod tests;
