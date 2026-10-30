//! Configured jobs have their own receipt identity; they never enter plugin history.
use super::{Inner, MAX_LOGS, MAX_RUNNING, Outcome, Refusal, running};
use orchbee_capability_node_protocol::{ConfiguredCommandIntent, OperationId};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

// Physical Windows process creation can exceed 500 ms under load.
// Leave room for a confirmed launch within the carrier's 20-second response budget.
const SPAWN_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Record {
    pub(super) id: OperationId,
    accepted: bool,
    intent: ConfiguredCommandIntent,
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<bool>>,
}
impl Record {
    pub(super) fn running(&self) -> bool {
        self.join.is_some()
    }
}
pub(super) fn start(
    inner: &mut Inner,
    id: OperationId,
    intent: ConfiguredCommandIntent,
) -> Outcome {
    start_with(inner, id, intent, SPAWN_CONFIRMATION_TIMEOUT, run)
}

fn start_with(
    inner: &mut Inner,
    id: OperationId,
    intent: ConfiguredCommandIntent,
    timeout: Duration,
    execute: impl FnOnce(ConfiguredCommandIntent, &AtomicBool, mpsc::Sender<Result<(), bool>>) -> bool
    + Send
    + 'static,
) -> Outcome {
    if let Some(record) = inner.commands.iter().find(|r| r.id == id) {
        return if record.intent == intent {
            if record.accepted {
                Outcome::ConfiguredStarted {
                    job_id: id,
                    existing: true,
                }
            } else {
                Outcome::Refused(if inner.retired {
                    Refusal::PhysicalUnknown
                } else {
                    Refusal::Unavailable
                })
            }
        } else {
            Outcome::Refused(Refusal::Conflict)
        };
    }
    // Operation identities cannot alias across either kind of process.
    if inner.records.iter().any(|r| r.log.job_id == id) {
        return Outcome::Refused(Refusal::Conflict);
    }
    if inner.retired {
        return Outcome::Refused(Refusal::PhysicalUnknown);
    }
    if inner.stopping {
        return Outcome::Refused(Refusal::Unavailable);
    }
    if running(inner) >= MAX_RUNNING {
        return Outcome::Refused(Refusal::Busy);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let command = intent.clone();
    let (admit, admitted) = mpsc::channel();
    let (spawned, observe_spawn) = mpsc::channel();
    #[cfg(test)]
    let requested = std::time::Instant::now();
    let join = match thread::Builder::new()
        .name("orchbee-configured-command".into())
        .spawn(move || {
            if admitted.recv().is_err() {
                return true;
            }
            #[cfg(test)]
            eprintln!("Configured owner entered after {:?}", requested.elapsed());
            execute(command, &worker_cancel, spawned)
        }) {
        Ok(join) => join,
        Err(_) => return Outcome::Refused(Refusal::Unavailable),
    };
    if inner.commands.len() == MAX_LOGS {
        let index = inner
            .commands
            .iter()
            .position(|r| !r.running())
            .expect("bounded running jobs");
        inner.commands.remove(index);
    }
    inner.commands.push(Record {
        id,
        accepted: false,
        intent,
        cancel,
        join: Some(join),
    });
    if admit.send(()).is_err() {
        inner.retired = true;
    }
    let observed = observe_spawn.recv_timeout(timeout);
    #[cfg(test)]
    if !matches!(observed, Ok(Ok(()))) {
        eprintln!("Configured command admission failed: {observed:?}");
    }
    match observed {
        Ok(Ok(())) => {
            inner
                .commands
                .last_mut()
                .expect("retained command")
                .accepted = true;
            Outcome::ConfiguredStarted {
                job_id: id,
                existing: false,
            }
        }
        Ok(Err(false)) => Outcome::Refused(Refusal::Unavailable),
        _ => {
            inner.retired = true;
            Outcome::Refused(Refusal::PhysicalUnknown)
        }
    }
}

fn run(
    intent: ConfiguredCommandIntent,
    cancel: &AtomicBool,
    spawned: mpsc::Sender<Result<(), bool>>,
) -> bool {
    #[cfg(test)]
    let started = std::time::Instant::now();
    use orchbee_local_runtime::CommandChild;
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    // The execution machine chooses its command processor, not a source cwd/PATH.
    let shell = std::env::var_os("ComSpec")
        .filter(|path| std::path::Path::new(path).is_absolute())
        .or_else(|| {
            std::env::var_os("SystemRoot").map(|root| {
                std::path::PathBuf::from(root)
                    .join("System32/cmd.exe")
                    .into_os_string()
            })
        });
    let Some(shell) = shell else {
        let _delivered = spawned.send(Err(false));
        return true;
    };
    let mut command = Command::new(shell);
    command
        .args(["/d", "/c"])
        .raw_arg(&intent.command)
        .current_dir(&intent.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    super::process::clear_context(&mut command);
    command.envs(intent.environment);
    let mut child = match CommandChild::spawn(command) {
        Ok(child) => child,
        Err(failure) => {
            #[cfg(test)]
            eprintln!(
                "Configured command spawn failed (created={}): {}",
                failure.created, failure.error
            );
            let _delivered = spawned.send(Err(failure.created));
            return !failure.created;
        }
    };
    #[cfg(test)]
    eprintln!(
        "Configured Job created in {:?}, PID {}",
        started.elapsed(),
        child.id()
    );
    let _delivered = spawned.send(Ok(()));
    loop {
        if cancel.load(Ordering::Relaxed) {
            return child.settle().is_ok();
        }
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => thread::sleep(std::time::Duration::from_millis(10)),
            Err(_) => {
                let _cleanup = child.settle();
                return false;
            }
        }
    }
}
pub(super) fn settle(inner: &mut Inner) {
    for record in &mut inner.commands {
        if record.join.as_ref().is_some_and(JoinHandle::is_finished) {
            let join = record.join.take().expect("finished job");
            inner.retired |= !join.join().unwrap_or(false);
        }
    }
}
pub(super) fn cancel(records: &[Record]) -> bool {
    let mut running = false;
    for record in records {
        if record.running() {
            record.cancel.store(true, Ordering::Relaxed);
            running = true;
        }
    }
    running
}

#[cfg(test)]
mod tests;
