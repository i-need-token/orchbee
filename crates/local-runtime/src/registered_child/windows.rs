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
        Ok((writer, record))
    })()
    .map_err(|error| at_step(SessionChildStep::Registration, error))?;
    let mut child = RetainedChild::spawn_suspended(executable, arguments)
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
    Ok(child)
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
