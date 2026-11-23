//! Direct creators publish durable ownership while their child's initial thread is suspended.
use crate::{
    DataRoot, RetainedChild,
    durable_sessions::{Directory, Lifecycle},
    private_filesystem as permissions, process_observation,
    process_registration::{Creator, ProcessRole},
};
use std::{
    ffi::OsStr,
    fmt, io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use super::{SessionChild, SessionChildStep};

/// Registers `child` in the Active record of the Session whose runtime folder is `runtime`, in
/// `root`'s `sessions/` folder. A record whose generation uses another runtime folder refuses.
pub fn spawn_session_child(
    root: &DataRoot,
    runtime: &Path,
    child: SessionChild,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
) -> io::Result<RetainedChild> {
    let (_runtime, directory, scope) = (|| -> io::Result<_> {
        let runtime_pin = permissions::open_tree(runtime)?;
        let scope = runtime
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(io::ErrorKind::InvalidInput)?;
        let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
        if !directory
            .read(scope)?
            .ok_or(io::ErrorKind::NotFound)?
            .uses_runtime(runtime)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok((runtime_pin, directory, scope))
    })()
    .map_err(|error| at_step(SessionChildStep::Registration, error))?;
    let reservation = match child {
        SessionChild::Worker => Reservation::Fresh(ProcessRole::Worker),
        SessionChild::ReplacementWorker => Reservation::ReplacementWorker,
        SessionChild::Host(slot) => Reservation::Fresh(ProcessRole::Host(slot)),
        SessionChild::RecoveryHost(slot) => Reservation::RecoveryHost(slot),
        SessionChild::Keeper(slot) => Reservation::Keeper(slot),
    };
    spawn_reserved_with(
        &directory,
        scope,
        reservation,
        executable,
        arguments,
        deadline,
        |path, words| RetainedChild::spawn_suspended(path, words).map(|child| (child, ())),
    )
    .map(|(child, ())| child)
}

/// Writes `epoch`, the one the Host in `slot` answered with, into the Active record of the Session
/// whose runtime folder is `runtime`, while that slot still names `process` (its ID and creation
/// time). The Worker that started or adopted the Host writes it once connected, so the Server can
/// pair the Host with the terminals whose identities carry the epoch.
pub fn record_session_host_epoch(
    root: &DataRoot,
    runtime: &Path,
    slot: crate::HostSlot,
    process: (u32, u64),
    epoch: [u8; 16],
    deadline: Instant,
) -> io::Result<()> {
    let identity = crate::process_registration::ProcessIdentity::new(process.0, process.1)
        .ok_or(io::ErrorKind::InvalidInput)?;
    edit_active_record(root, runtime, deadline, |processes| {
        if processes.registration(ProcessRole::Host(slot)).epoch() == Some(epoch) {
            return Ok(false);
        }
        processes
            .record_host_epoch(slot, identity, epoch)
            .map_err(|_| io::ErrorKind::InvalidData)?;
        Ok(true)
    })
}

/// Frees `slot` of the Active record of the Session whose runtime folder is `runtime`, while that
/// slot still names `process`: the Worker drained that Host and saw it end, so the Session keeps no
/// Host there to adopt or recover, and a later Host may start in the slot.
pub fn release_session_host(
    root: &DataRoot,
    runtime: &Path,
    slot: crate::HostSlot,
    process: (u32, u64),
    deadline: Instant,
) -> io::Result<()> {
    let identity = crate::process_registration::ProcessIdentity::new(process.0, process.1)
        .ok_or(io::ErrorKind::InvalidInput)?;
    edit_active_record(root, runtime, deadline, |processes| {
        processes
            .release_host(slot, identity)
            .map_err(|_| io::ErrorKind::InvalidData)?;
        Ok(true)
    })
}

/// Makes `slot` of the Active record of the Session whose runtime folder is `runtime` the current
/// Keeper's, while it still names `process`: the Worker commits a Keeper update, or adopts the one
/// live Keeper an update left in the other slot.
pub fn make_session_keeper_current(
    root: &DataRoot,
    runtime: &Path,
    slot: crate::KeeperSlot,
    process: (u32, u64),
    deadline: Instant,
) -> io::Result<()> {
    let identity = crate::process_registration::ProcessIdentity::new(process.0, process.1)
        .ok_or(io::ErrorKind::InvalidInput)?;
    edit_active_record(root, runtime, deadline, |processes| {
        if processes.current_keeper() == slot {
            return Ok(false);
        }
        processes
            .make_keeper_current(slot, identity)
            .map_err(|_| io::ErrorKind::InvalidData)?;
        Ok(true)
    })
}

/// Frees Keeper `slot` of the Active record of the Session whose runtime folder is `runtime`, while
/// it still names `process` and is not the current Keeper's: the Worker ended the Keeper an update
/// replaced or rolled back once no Host held anything in it.
pub fn release_session_keeper(
    root: &DataRoot,
    runtime: &Path,
    slot: crate::KeeperSlot,
    process: (u32, u64),
    deadline: Instant,
) -> io::Result<()> {
    let identity = crate::process_registration::ProcessIdentity::new(process.0, process.1)
        .ok_or(io::ErrorKind::InvalidInput)?;
    edit_active_record(root, runtime, deadline, |processes| {
        processes
            .release_keeper(slot, identity)
            .map_err(|_| io::ErrorKind::InvalidData)?;
        Ok(true)
    })
}

/// Applies `edit` to the process registrations of the Active record whose runtime folder is
/// `runtime`, under the record's writer lock taken before `deadline`, and publishes the record
/// when `edit` changed it.
fn edit_active_record(
    root: &DataRoot,
    runtime: &Path,
    deadline: Instant,
    edit: impl FnOnce(&mut crate::process_registration::SessionProcesses) -> io::Result<bool>,
) -> io::Result<()> {
    let _runtime = permissions::open_tree(runtime)?;
    let scope = runtime
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let directory = Directory::open(root)?.ok_or(io::ErrorKind::NotFound)?;
    let _writer = loop {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match directory.try_writer(scope) {
            Ok(writer) => break writer,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(10)),
            ),
            Err(error) => return Err(error),
        }
    };
    let mut record = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
    if record.lifecycle != Lifecycle::Active || !record.uses_runtime(runtime) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    if edit(&mut record.processes)? {
        directory.publish(scope, &record)?;
    }
    Ok(())
}

/// How a registered child's slot is reserved before its creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reservation {
    /// A slot never started in this generation.
    Fresh(ProcessRole),
    /// The Worker slot, returned to spawning for a replacement of a Worker proven gone.
    ReplacementWorker,
    /// A Host slot, returned to spawning for a recovery Host once the Host recorded there is gone.
    RecoveryHost(crate::HostSlot),
    /// A Keeper slot that names no live Keeper.
    Keeper(crate::KeeperSlot),
}

impl Reservation {
    fn role(self) -> ProcessRole {
        match self {
            Self::Fresh(role) => role,
            Self::ReplacementWorker => ProcessRole::Worker,
            Self::RecoveryHost(slot) => ProcessRole::Host(slot),
            Self::Keeper(slot) => ProcessRole::Keeper(slot),
        }
    }
}

pub(crate) fn spawn_registered(
    directory: &Directory,
    scope: &str,
    role: ProcessRole,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
) -> io::Result<RetainedChild> {
    spawn_registered_with(
        directory,
        scope,
        role,
        executable,
        arguments,
        deadline,
        |path, words| RetainedChild::spawn_suspended(path, words).map(|child| (child, ())),
    )
    .map(|(child, ())| child)
}

/// [`spawn_registered`] whose suspended process `create` makes, answering what `create` reports
/// beside the child.
pub(crate) fn spawn_registered_with<T>(
    directory: &Directory,
    scope: &str,
    role: ProcessRole,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
    create: impl FnOnce(&Path, &[&OsStr]) -> io::Result<(RetainedChild, T)>,
) -> io::Result<(RetainedChild, T)> {
    spawn_reserved_with(
        directory,
        scope,
        Reservation::Fresh(role),
        executable,
        arguments,
        deadline,
        create,
    )
}

fn spawn_reserved_with<T>(
    directory: &Directory,
    scope: &str,
    reservation: Reservation,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
    create: impl FnOnce(&Path, &[&OsStr]) -> io::Result<(RetainedChild, T)>,
) -> io::Result<(RetainedChild, T)> {
    let role = reservation.role();
    // The writer lock stays held until this attempt returns. Cold recovery reads a Spawning slot
    // under the same lock as an attempt that ended without an identity, so releasing it before the
    // identity write and resume, or the failure cleanup, would let recovery start a new generation
    // beside a child this attempt may still resume.
    let (_writer, mut record) = (|| -> io::Result<_> {
        let initial = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if initial.lifecycle != Lifecycle::Active {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let writer = loop {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            match directory.try_writer(scope) {
                Ok(writer) => break writer,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(10)),
                ),
                Err(error) => return Err(error),
            }
        };
        let mut record = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
        if record.lifecycle != Lifecycle::Active || record.activation_id != initial.activation_id {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let creator = match role {
            ProcessRole::Server => Creator::Activator,
            ProcessRole::Worker | ProcessRole::Host(_) | ProcessRole::Keeper(_) => {
                Creator::Process(process_observation::current_identity()?)
            }
        };
        match reservation {
            Reservation::Fresh(role) => record.processes.reserve(role, creator),
            Reservation::ReplacementWorker => record.processes.reserve_replacement_worker(creator),
            // Under the writer lock, so the Host observed gone stays the recorded one.
            Reservation::RecoveryHost(slot)
                if process_observation::role_absent(&record.processes, ProcessRole::Host(slot)) =>
            {
                record.processes.reserve_recovery_host(slot, creator)
            }
            Reservation::RecoveryHost(_) => return Err(io::ErrorKind::ResourceBusy.into()),
            Reservation::Keeper(slot)
                if process_observation::role_absent(
                    &record.processes,
                    ProcessRole::Keeper(slot),
                ) =>
            {
                record.processes.reserve_keeper(slot, creator)
            }
            Reservation::Keeper(_) => return Err(io::ErrorKind::ResourceBusy.into()),
        }
        .map_err(|_| io::ErrorKind::InvalidData)?;
        directory.publish(scope, &record)?;
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok((writer, record))
    })()
    .map_err(|error| at_step(SessionChildStep::Registration, error))?;
    let (mut child, created) = create(executable, arguments)
        .map_err(|error| at_step(SessionChildStep::Creation, error))?;
    let released = (|| -> io::Result<()> {
        let identity = child.identity()?;
        record
            .processes
            .identify(role, identity)
            .map_err(|_| io::ErrorKind::InvalidData)?;
        directory.publish(scope, &record)?;
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        child.resume()
    })();
    if let Err(error) = released {
        // This is the newly created exact object, never a PID-discovered old owner.
        return Err(at_step(
            SessionChildStep::Release,
            match child.terminate_before(Instant::now() + Duration::from_secs(5)) {
                Ok(_) => error,
                Err(cleanup) => io::Error::other(format!(
                    "{error}; suspended child cleanup unconfirmed: {cleanup}"
                )),
            },
        ));
    }
    Ok((child, created))
}

pub(super) fn step_of(error: &io::Error) -> Option<SessionChildStep> {
    error
        .get_ref()?
        .downcast_ref::<StepFailure>()
        .map(|failure| failure.step)
}

/// A failed step and the operation's own error, which stays this error's source.
#[derive(Debug)]
struct StepFailure {
    step: SessionChildStep,
    source: io::Error,
}

impl fmt::Display for StepFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for StepFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Names `step` on `error`, keeping its kind.
fn at_step(step: SessionChildStep, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        StepFailure {
            step,
            source: error,
        },
    )
}
