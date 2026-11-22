//! The Terminal Host's Keeper as a Session child: the one switch that its start, its offer to each
//! Host, and the recovery of a lost Host's programs consult, so that release builds turn them on
//! together; its start as the recorded Worker's registered child; and the Worker's connection to
//! it. The Keeper keeps a Host's programs running when that Host ends
//! (`apps/terminal-host/docs/runtime-model.md`, Keeper).
use crate::{EnvironmentKind, environment_kind};

#[cfg(windows)]
mod client;
#[cfg(windows)]
pub use client::{
    HeldTerminal, Holding, HoldingState, KEEPER_REPLY_BOUND, KeeperClient, KeeperProcess,
    KeeperRefusal, KeeperStatus,
};

/// The live Keeper the Active generation of the Session at `runtime` records, opened on its
/// recorded identity, or `None` when the record names none or it is gone. The Keeper runs the
/// Terminal Host's program.
///
/// # Errors
///
/// Returns an error when the record cannot be read, no longer names an Active generation in
/// `runtime`, or names a Keeper that cannot be proven gone.
#[cfg(windows)]
pub fn recorded_keeper(
    root: &crate::DataRoot,
    runtime: &std::path::Path,
) -> std::io::Result<Option<crate::ExactProcess>> {
    match crate::durable_sessions::recorded_keeper(root, runtime)? {
        Some(identity) => crate::ExactProcess::open(identity, crate::BlockerRole::TerminalHost),
        None => Ok(None),
    }
}

/// Ends the Keeper the generation `record` retains records, checked by its process ID and creation
/// time, and waits until it exited or `deadline` passes: a stop without a Worker ends it once the
/// Host is gone, since a Keeper may hold a program's last handles. A Keeper that is gone, or never
/// started, counts as ended.
///
/// # Errors
///
/// Returns an error when the record cannot be read or the Keeper cannot be ended in time.
#[cfg(windows)]
pub fn end_recorded_keeper(
    record: &crate::RetainedSessionRecord,
    deadline: std::time::Instant,
) -> std::io::Result<()> {
    let Some(identity) = record.keeper_identity()? else {
        return Ok(());
    };
    match crate::ExactProcess::open(identity, crate::BlockerRole::TerminalHost)? {
        Some(keeper) => keeper.end_before(deadline),
        None => Ok(()),
    }
}

/// Whether release builds run the Keeper, true once the recovery acceptance passed on real Windows
/// (`proposals/platform/host-handover.md`, decision 65). Development builds follow it, so a
/// development Session never runs a Keeper that release builds do not.
const IN_RELEASE_BUILDS: bool = false;

/// Whether this build runs the Keeper. Test builds always do, so tests and acceptance runs drive
/// it; release and development builds do once `IN_RELEASE_BUILDS` is set. It is a constant
/// expression, so a caller may decide at compile time.
pub const fn keeper_live() -> bool {
    match environment_kind() {
        EnvironmentKind::Release | EnvironmentKind::Development => IN_RELEASE_BUILDS,
        EnvironmentKind::Test => true,
    }
}

/// Starts the Session's Keeper from `executable`, the Terminal Host's program, as the recorded
/// Worker's registered child in the Host's folder of `runtime`, and waits until its endpoint
/// listens or `deadline` passes, and answers it with its exact identity, its process ID and
/// creation time. A Keeper that exits first, or does not listen in time, fails; one that did not
/// listen in time is ended first, so no Keeper outlives a failed start.
///
/// # Errors
///
/// Returns an error when the registration, the creation, or the wait fails.
#[cfg(windows)]
pub fn start_keeper(
    root: &crate::DataRoot,
    runtime: &std::path::Path,
    executable: &std::path::Path,
    deadline: std::time::Instant,
) -> std::io::Result<(crate::RetainedChild, (u32, u64))> {
    use crate::ModeCheckedLocalSessionRendezvous as Rendezvous;
    use std::{ffi::OsStr, io, thread, time::Duration};

    let directory = Rendezvous::host_runtime_directory(runtime);
    let endpoint_id = Rendezvous::keeper_endpoint_id(runtime);
    let endpoint = Rendezvous::keeper_endpoint_path(runtime);
    let arguments = [
        OsStr::new("keep"),
        OsStr::new("--runtime-directory"),
        directory.as_os_str(),
        OsStr::new("--endpoint-id"),
        OsStr::new(&endpoint_id),
    ];
    let mut child = crate::spawn_session_child(
        root,
        runtime,
        crate::SessionChild::Keeper,
        executable,
        &arguments,
        deadline,
    )?;
    loop {
        if child.try_wait()?.is_some() {
            return Err(io::Error::other("the Keeper exited before it listened"));
        }
        if orchbee_local_ipc::endpoint_exists(&endpoint)? {
            let identity = child.exact_identity()?;
            return Ok((child, identity));
        }
        if std::time::Instant::now() >= deadline {
            // A Keeper holds nothing before a Host attaches it, so ending it ends no program.
            let _ended = child.terminate_before(std::time::Instant::now() + Duration::from_secs(2));
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "the Keeper did not listen in time",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}
