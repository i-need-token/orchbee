//! Configured jobs have their own receipt identity; they never enter plugin history. The Worker
//! runs one itself, or its Terminal Host runs it as a background program.
use super::{Fingerprint, Inner, MAX_LOGS, MAX_RUNNING, Outcome, Refusal, running};
use orchbee_capability_node_protocol::{ConfiguredCommandIntent, OperationId};
use orchbee_local_runtime::{CommandChild, CommandSpawnFailure};
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
mod shell;

// Physical Windows process creation can exceed 500 ms under load.
// Leave room for a confirmed launch within the carrier's 20-second response budget.
const SPAWN_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Record {
    pub(super) id: OperationId,
    accepted: bool,
    /// The fingerprint of the intent that started it.
    fingerprint: u64,
    owner: Owner,
}
enum Owner {
    Worker {
        cancel: Arc<AtomicBool>,
        join: Option<JoinHandle<bool>>,
    },
    /// The Terminal Host runs it, until the Host reports its exit.
    Host { running: bool },
}
impl Record {
    /// Whether the Worker itself still runs it.
    pub(super) fn running(&self) -> bool {
        matches!(&self.owner, Owner::Worker { join: Some(_), .. })
    }
    /// Whether the Terminal Host still runs it.
    pub(super) fn hosted(&self) -> bool {
        matches!(self.owner, Owner::Host { running: true })
    }
}
pub(super) fn start(
    inner: &mut Inner,
    id: OperationId,
    intent: ConfiguredCommandIntent,
) -> Outcome {
    start_with(inner, id, intent, SPAWN_CONFIRMATION_TIMEOUT, run)
}

/// The fingerprint of a start this Worker admits as a new command, or the answer to one it does
/// not: a repeat of an earlier start, an identity in use, or no room to start one.
pub(super) fn admission(
    inner: &Inner,
    id: OperationId,
    intent: &ConfiguredCommandIntent,
) -> Result<u64, Outcome> {
    let fingerprint = Fingerprint::new()
        .field(intent.command_id.as_bytes())
        .field(intent.directory.as_bytes())
        .field(intent.command.as_bytes())
        .variables(&intent.environment)
        .get();
    if let Some(record) = inner.commands.iter().find(|r| r.id == id) {
        return Err(if record.fingerprint != fingerprint {
            Outcome::Refused(Refusal::Conflict)
        } else if record.accepted {
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
        });
    }
    // Operation identities cannot alias across either kind of process.
    if inner.records.iter().any(|r| r.log.job_id == id) {
        return Err(Outcome::Refused(Refusal::Conflict));
    }
    if inner.retired {
        return Err(Outcome::Refused(Refusal::PhysicalUnknown));
    }
    if inner.stopping {
        return Err(Outcome::Refused(Refusal::Unavailable));
    }
    if running(inner) >= MAX_RUNNING {
        return Err(Outcome::Refused(Refusal::Busy));
    }
    Ok(fingerprint)
}

/// Retains a command's record, making room by dropping the oldest finished one.
fn retain(inner: &mut Inner, record: Record) {
    if inner.commands.len() == MAX_LOGS {
        let index = inner
            .commands
            .iter()
            .position(|r| !r.running() && !r.hosted())
            .expect("bounded running jobs");
        inner.commands.remove(index);
    }
    inner.commands.push(record);
}

/// Records a command the Terminal Host runs: started now, or one a replacement Worker found.
pub(super) fn admit_hosted(inner: &mut Inner, id: OperationId, fingerprint: u64) {
    retain(
        inner,
        Record {
            id,
            accepted: true,
            fingerprint,
            owner: Owner::Host { running: true },
        },
    );
}

/// Records that the Host's program ended, when it ran a configured command.
pub(super) fn finish_hosted(inner: &mut Inner, program: orchbee_terminal_protocol::ProgramId) {
    for record in &mut inner.commands {
        if record.id.as_bytes() == program
            && let Owner::Host { running } = &mut record.owner
        {
            *running = false;
        }
    }
}

/// The absolute path of the command processor a configured command runs in, and its arguments
/// before the command text, on Windows and macOS, where a Terminal Host runs background programs.
pub(super) fn command_processor() -> Option<(String, Vec<String>)> {
    shell::processor()
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
    let fingerprint = match admission(inner, id, &intent) {
        Ok(fingerprint) => fingerprint,
        Err(outcome) => return outcome,
    };
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
    retain(
        inner,
        Record {
            id,
            accepted: false,
            fingerprint,
            owner: Owner::Worker {
                cancel,
                join: Some(join),
            },
        },
    );
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
    // The execution machine chooses its command processor, not a source cwd/PATH.
    let Some(mut command) = shell::command(&intent.command) else {
        let _delivered = spawned.send(Err(false));
        return true;
    };
    command.current_dir(&intent.directory);
    super::process::clear_context(&mut command);
    command.envs(intent.environment);
    let mut child = match spawn_configured(command) {
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
        "Configured command owner created in {:?}, PID {}",
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

/// Starts the configured command as the leader of its own process group, with the null device as
/// each standard stream. On macOS the native adapter starts it, so it inherits no descriptor but
/// those three, however many threads of this process create descriptors meanwhile.
#[cfg(target_os = "macos")]
fn spawn_configured(command: Command) -> Result<CommandChild, CommandSpawnFailure> {
    use orchbee_local_runtime::SpawnEnvironment;
    use std::{fs::OpenOptions, io, os::fd::OwnedFd};
    let not_created = |error: io::Error| CommandSpawnFailure {
        error,
        created: false,
    };
    // Opened as the standard library opens `Stdio::null()`: input to read, each output to write.
    let input = OpenOptions::new()
        .read(true)
        .open("/dev/null")
        .map_err(not_created)?;
    let output = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .map_err(not_created)?;
    let errors = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .map_err(not_created)?;
    CommandChild::spawn_with_streams(
        command,
        SpawnEnvironment::Inherited,
        [
            OwnedFd::from(input),
            OwnedFd::from(output),
            OwnedFd::from(errors),
        ],
    )
}

/// Elsewhere the standard library's spawn starts the same command with the same null streams.
#[cfg(not(target_os = "macos"))]
fn spawn_configured(mut command: Command) -> Result<CommandChild, CommandSpawnFailure> {
    use std::process::Stdio;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    CommandChild::spawn(command)
}

pub(super) fn settle(inner: &mut Inner) {
    for record in &mut inner.commands {
        if let Owner::Worker { join, .. } = &mut record.owner
            && join.as_ref().is_some_and(JoinHandle::is_finished)
        {
            let join = join.take().expect("finished job");
            inner.retired |= !join.join().unwrap_or(false);
        }
    }
}
/// Signals each command the Worker itself runs; those the Host runs outlive the Worker.
pub(super) fn cancel(records: &[Record]) -> bool {
    let mut running = false;
    for record in records {
        if let Owner::Worker {
            cancel,
            join: Some(_),
        } = &record.owner
        {
            cancel.store(true, Ordering::Relaxed);
            running = true;
        }
    }
    running
}

#[cfg(test)]
mod tests;
