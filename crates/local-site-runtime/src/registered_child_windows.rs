//! Direct creators publish durable ownership while their child's initial thread is suspended.
use crate::{
    RetainedChild,
    durable_sessions_windows::{Directory, Lifecycle},
    process_observation_windows,
    process_registration::{Creator, ProcessRole},
    windows_permissions as permissions,
};
use std::{
    ffi::OsStr,
    io,
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// A managed process registers only the directly created child owned by its role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsSiteChild {
    Worker,
    Host,
}

pub fn spawn_windows_site_child(
    local: &Path,
    runtime: &Path,
    child: WindowsSiteChild,
    executable: &Path,
    arguments: &[&OsStr],
    deadline: Instant,
) -> io::Result<RetainedChild> {
    let _runtime = permissions::open_tree(runtime)?;
    let scope = runtime
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    if !crate::WindowsNamedSiteCatalog::supports_scope(scope)? {
        return RetainedChild::spawn(executable, arguments);
    }
    let base = runtime.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let directory = Directory::open(local, base)?.ok_or(io::ErrorKind::NotFound)?;
    let role = match child {
        WindowsSiteChild::Worker => ProcessRole::Worker,
        WindowsSiteChild::Host => ProcessRole::Host,
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
            Creator::Process(process_observation_windows::current_identity()?)
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
