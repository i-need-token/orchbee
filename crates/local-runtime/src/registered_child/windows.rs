//! Direct creators publish durable ownership while their child's initial thread is suspended.
use crate::{
    DataRoot, RetainedChild,
    durable_sessions::{Directory, Lifecycle},
    private_filesystem as permissions, process_observation,
    process_registration::{Creator, ProcessRole},
};
use std::{
    ffi::OsStr,
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use super::SessionChild;

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
    let _runtime = permissions::open_tree(runtime)?;
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
    let role = match child {
        SessionChild::Worker => ProcessRole::Worker,
        SessionChild::Host => ProcessRole::Host,
    };
    spawn_registered(&directory, scope, role, executable, arguments, deadline)
}

pub(crate) fn spawn_registered(
    directory: &Directory,
    scope: &str,
    role: ProcessRole,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
) -> io::Result<RetainedChild> {
    let initial = directory.read(scope)?.ok_or(io::ErrorKind::NotFound)?;
    if initial.lifecycle != Lifecycle::Active {
        return Err(io::ErrorKind::InvalidData.into());
    }
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
    if record.lifecycle != Lifecycle::Active || record.activation_id != initial.activation_id {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let creator = match role {
        ProcessRole::Server => Creator::Activator,
        ProcessRole::Worker | ProcessRole::Host => {
            Creator::Process(process_observation::current_identity()?)
        }
    };
    record
        .processes
        .reserve(role, creator)
        .map_err(|_| io::ErrorKind::InvalidData)?;
    directory.publish(scope, &record)?;
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    let mut child = RetainedChild::spawn_suspended(executable, arguments)?;
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
        return match child.terminate_before(Instant::now() + Duration::from_secs(5)) {
            Ok(_) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; suspended child cleanup unconfirmed: {cleanup}"
            ))),
        };
    }
    Ok(child)
}
