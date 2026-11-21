//! The processes of a Server restart that keeps terminals, as the Session record names them in its
//! successor entry. The `orchbee` that requests the restart creates the successor Server and
//! records it; the successor creates its Worker and records it, then commits; the requester clears
//! what a failed attempt left. Each creation follows the rule every Session child follows: the
//! child is created suspended, its identity is published under the record's writer lock, and only
//! then does it run, so a child that was never recorded never runs. Each step holds that lock for
//! its whole attempt, and whoever removes the entry first ends what it names by its recorded
//! identity, the Worker first.

use crate::{
    DataRoot, ExactProcess, JobPlacement, ModeCheckedLocalSessionRendezvous as Rendezvous,
    RetainedChild,
    durable_sessions::{self as durable, Directory, Lifecycle, Record, RecordWriter, Successor},
    process_observation,
    process_registration::{ProcessIdentity, ProcessRole},
};
use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// How long a step waits for a child it created and could not record to end.
const CLEANUP: Duration = Duration::from_secs(5);

/// Why a successor Server ended before the Server it replaces answered `prepared`, as its exit code
/// tells the `orchbee` that requested the restart. Both come from one build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartRefusal {
    /// The running Server did not answer.
    NotServing,
    /// The running Server cannot hand its terminals over.
    Unsupported,
    /// The running Server is busy with another restart, a stop, or a Worker replacement.
    Busy,
    /// A background program runs in the Session.
    BackgroundPrograms,
    /// A connected Client cannot reconnect to the successor by itself, or cannot join it.
    Clients,
    /// The running Server has no serving Worker.
    WorkerUnavailable,
    /// A Terminal Host cannot be adopted.
    Host,
    /// The Session's records cannot be used for the restart.
    Records,
    /// The Session record no longer names this restart's successor.
    NotRecorded,
}

impl RestartRefusal {
    const CODES: [(Self, u8, &'static str); 9] = [
        (Self::NotServing, 64, "not-serving"),
        (Self::Unsupported, 65, "unsupported"),
        (Self::Busy, 66, "busy"),
        (Self::BackgroundPrograms, 67, "background-programs"),
        (Self::Clients, 68, "clients"),
        (Self::WorkerUnavailable, 69, "worker-unavailable"),
        (Self::Host, 70, "host"),
        (Self::Records, 71, "records"),
        (Self::NotRecorded, 72, "not-recorded"),
    ];

    /// The successor Server's exit code.
    pub fn code(self) -> u8 {
        Self::CODES
            .iter()
            .find_map(|(refusal, code, _)| (*refusal == self).then_some(*code))
            .unwrap_or(1)
    }

    /// The name diagnostics record.
    pub fn name(self) -> &'static str {
        Self::CODES
            .iter()
            .find_map(|(refusal, _, name)| (*refusal == self).then_some(*name))
            .unwrap_or("unknown")
    }

    /// The refusal a successor's exit code names, if it names one.
    pub fn from_code(code: u32) -> Option<Self> {
        Self::CODES
            .iter()
            .find_map(|(refusal, known, _)| (u32::from(*known) == code).then_some(*refusal))
    }
}

/// What clearing a restart's successor entry found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartCleared {
    /// The entry for the ticket was there: what it named was ended and the entry removed.
    /// `worker_recorded` says whether it named the successor's Worker, which the successor records
    /// once its Check passed and before the replaced Server can hold the Session.
    Cleared { worker_recorded: bool },
    /// No entry names the ticket and the record names another generation than the one the
    /// restart reserved: the replaced Server took the Session back, or never saw the entry.
    Absent,
    /// The record names the generation the restart reserved: the successor committed.
    Committed,
}

/// What one restart reserves in the Session record, chosen at random by its requester: the ticket
/// every `handover` step carries, and the activation identifier its successor commits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartReservation {
    pub ticket: [u8; 16],
    pub activation_id: [u8; 16],
}

/// Creates the successor Server of a restart suspended outside the creator's Job where the Job
/// lets it leave, names it in the Session record of `scope` as the successor `reservation` names,
/// and resumes it. The record must be Active, use `runtime`, and name its Server. An earlier entry
/// whose Server still runs is a restart in progress and refuses with `AlreadyExists`; one whose
/// Server is gone is cleared first, its Worker ended by its recorded identity.
pub(crate) fn spawn_server(
    directory: &Directory,
    scope: &str,
    runtime: &Path,
    reservation: RestartReservation,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
) -> io::Result<(RetainedChild, JobPlacement)> {
    let RestartReservation {
        ticket,
        activation_id,
    } = reservation;
    let (_writer, mut record) = writer_and_record(directory, scope, Some(runtime), deadline)?;
    if let Some(entry) = Successor::of(&record) {
        if ExactProcess::successor_server(&entry)?.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        ExactProcess::end_successor(&entry, deadline)?;
        durable::remove_successor(&mut record, entry.ticket());
    }
    let (mut child, placement) = RetainedChild::spawn_suspended_outside_job(executable, arguments)?;
    let released = (|| -> io::Result<()> {
        let identity = child.identity()?;
        durable::reserve_successor(&mut record, ticket, identity, activation_id)?;
        directory.publish(scope, &record)?;
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        child.resume()
    })();
    if let Err(error) = released {
        // This is the newly created exact object. Only once it is proven ended may the entry that
        // may name it go; an entry left behind names a Server that never ran, which the replaced
        // Server clears as abandoned.
        if child.terminate_before(Instant::now() + CLEANUP).is_ok()
            && durable::remove_successor(&mut record, ticket)
        {
            let _ = directory.publish(scope, &record);
        }
        return Err(error);
    }
    Ok((child, placement))
}

/// Clears what the restart `reservation` names left in the Session record of the Session `session`
/// admitted, for the requester that holds the activation lock `activation`: a remaining entry for
/// its ticket has every live process it names ended by its recorded identity, the Worker first, and
/// is then removed.
pub fn clear_restart(
    root: &DataRoot,
    session: &Rendezvous,
    activation: &crate::ModeCheckedSessionLock,
    reservation: RestartReservation,
    deadline: Instant,
) -> io::Result<RestartCleared> {
    let RestartReservation {
        ticket,
        activation_id,
    } = reservation;
    session.require_lock(activation, crate::LockKind::Activation)?;
    let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
    let scope = scope_of(session.runtime_directory())?;
    let _writer = writer_before(&directory, scope, deadline)?;
    let mut record = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
    if record.activation_id == Some(activation_id) {
        return Ok(RestartCleared::Committed);
    }
    let Some(entry) = Successor::of(&record).filter(|entry| entry.ticket() == ticket) else {
        return Ok(RestartCleared::Absent);
    };
    ExactProcess::end_successor(&entry, deadline)?;
    durable::remove_successor(&mut record, ticket);
    directory.publish(scope, &record)?;
    Ok(RestartCleared::Cleared {
        worker_recorded: entry.worker_recorded(),
    })
}

/// A restart's successor Server holding its entry in the Session record: the entry names this
/// process and the ticket every `handover` step carries.
pub struct SuccessorRecord {
    directory: Directory,
    scope: String,
    runtime: PathBuf,
    ticket: [u8; 16],
    /// This process, as the entry names it.
    server: ProcessIdentity,
    /// The Server the record named when this successor opened it, which the commit replaces.
    replaced: ProcessIdentity,
    /// The Worker this successor recorded, once it did.
    worker: Option<ProcessIdentity>,
}

impl SuccessorRecord {
    /// The entry for `ticket` in the record of the Session whose runtime `session` admitted. The
    /// record must be Active, use that runtime and name its Server; the entry must name this
    /// process as the successor and no Worker yet.
    pub fn open(root: &DataRoot, session: &Rendezvous, ticket: [u8; 16]) -> io::Result<Self> {
        let runtime = session.runtime_directory();
        let scope = scope_of(runtime)?;
        let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
        let record = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        let server = process_observation::current_identity()?;
        let named = Successor::of(&record).is_some_and(|entry| {
            entry.ticket() == ticket && entry.server() == server && !entry.worker_recorded()
        });
        if !named || record.lifecycle != Lifecycle::Active || !record.uses_runtime(runtime) {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let replaced = record
            .processes
            .registration(ProcessRole::Server)
            .identity()
            .ok_or(io::ErrorKind::InvalidData)?;
        Ok(Self {
            directory,
            scope: scope.to_owned(),
            runtime: runtime.to_owned(),
            ticket,
            server,
            replaced,
            worker: None,
        })
    }

    /// The process ID of the Server this restart replaces, as the record named it.
    pub fn replaced_process_id(&self) -> u32 {
        self.replaced.pid()
    }

    /// The Server this restart replaces, opened on its recorded identity, or `None` once it is
    /// gone.
    pub fn open_replaced(&self) -> io::Result<Option<ExactProcess>> {
        ExactProcess::open(self.replaced)
    }

    /// Creates this successor's Worker suspended, names it in the entry, and only then resumes it.
    /// A Worker this step cannot name is ended and never runs.
    pub fn spawn_worker(
        &mut self,
        executable: &Path,
        arguments: &[&OsStr],
        deadline: Instant,
    ) -> io::Result<RetainedChild> {
        if self.worker.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let (_writer, mut record) =
            writer_and_record(&self.directory, &self.scope, Some(&self.runtime), deadline)?;
        let mut child = RetainedChild::spawn_suspended(executable, arguments)?;
        let released = (|| -> io::Result<ProcessIdentity> {
            let identity = child.identity()?;
            durable::record_successor_worker(&mut record, self.ticket, self.server, identity)?;
            self.directory.publish(&self.scope, &record)?;
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            child.resume()?;
            Ok(identity)
        })();
        match released {
            Ok(identity) => {
                self.worker = Some(identity);
                Ok(child)
            }
            Err(error) => Err(match child.terminate_before(Instant::now() + CLEANUP) {
                Ok(_) => error,
                Err(cleanup) => io::Error::other(format!(
                    "{error}; suspended Worker cleanup unconfirmed: {cleanup}"
                )),
            }),
        }
    }

    /// The commit point of the restart: one publish of the record under its writer lock, while
    /// the entry still names this ticket, this Server and its Worker and the record still names
    /// the replaced Server, that makes this Server and its Worker the Session's. A publish that
    /// reports a failure is read back, since the replace may have happened.
    pub fn commit(&self, deadline: Instant) -> io::Result<()> {
        let worker = self.worker.ok_or(io::ErrorKind::InvalidInput)?;
        let (_writer, mut record) =
            writer_and_record(&self.directory, &self.scope, Some(&self.runtime), deadline)?;
        durable::commit_successor(&mut record, self.ticket, self.server, worker, self.replaced)?;
        match self.directory.publish(&self.scope, &record) {
            Ok(()) => Ok(()),
            Err(error) => match self.directory.read(&self.scope) {
                Ok(Some(current))
                    if current.activation_id == record.activation_id
                        && current.processes == record.processes =>
                {
                    Ok(())
                }
                _ => Err(error),
            },
        }
    }
}

/// The record's writer lock, waited for until `deadline`, and the Active record it guards, which
/// uses `runtime` when one is given.
fn writer_and_record(
    directory: &Directory,
    scope: &str,
    runtime: Option<&Path>,
    deadline: Instant,
) -> io::Result<(RecordWriter, Record)> {
    let writer = writer_before(directory, scope, deadline)?;
    let record = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
    if record.lifecycle != Lifecycle::Active
        || runtime.is_some_and(|runtime| !record.uses_runtime(runtime))
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok((writer, record))
}

fn writer_before(
    directory: &Directory,
    scope: &str,
    deadline: Instant,
) -> io::Result<RecordWriter> {
    loop {
        match directory.try_writer(scope) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            writer => return writer,
        }
    }
}

fn scope_of(runtime: &Path) -> io::Result<&str> {
    runtime
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
