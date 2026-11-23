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
    KeeperRefusal, KeeperStatus, ProgramHolding,
};

/// The live Keepers the Active generation of the Session records: the current Keeper's slot, and
/// each live one with its slot, opened on its recorded identity. Both slots name one only while a
/// Keeper update runs or after one was cut short.
#[cfg(windows)]
pub struct RecordedKeepers {
    pub current: crate::KeeperSlot,
    pub live: Vec<(crate::KeeperSlot, crate::ExactProcess)>,
}

/// The live Keepers the Active generation of the Session at `runtime` records. A Keeper runs the
/// Terminal Host's program.
///
/// # Errors
///
/// Returns an error when the record cannot be read, no longer names an Active generation in
/// `runtime`, or names a Keeper that cannot be proven gone.
#[cfg(windows)]
pub fn recorded_keepers(
    root: &crate::DataRoot,
    runtime: &std::path::Path,
) -> std::io::Result<RecordedKeepers> {
    let recorded = crate::durable_sessions::recorded_keepers(root, runtime)?;
    let mut live = Vec::new();
    for slot in crate::KeeperSlot::ALL {
        if let Some(identity) = recorded.slots[slot.index()]
            && let Some(process) =
                crate::ExactProcess::open(identity, crate::BlockerRole::TerminalHost)?
        {
            live.push((slot, process));
        }
    }
    Ok(RecordedKeepers {
        current: recorded.current,
        live,
    })
}

/// Ends each Keeper the generation `record` retains records, checked by its process ID and creation
/// time, and waits until it exited or `deadline` passes: a stop without a Worker ends them once the
/// Hosts are gone, since a Keeper may hold a program's last handles. A Keeper that is gone, or never
/// started, counts as ended.
///
/// # Errors
///
/// Returns an error when the record cannot be read or a Keeper cannot be ended in time.
#[cfg(windows)]
pub fn end_recorded_keeper(
    record: &crate::RetainedSessionRecord,
    deadline: std::time::Instant,
) -> std::io::Result<()> {
    for identity in record.keeper_identities()?.into_iter().flatten() {
        if let Some(keeper) = crate::ExactProcess::open(identity, crate::BlockerRole::TerminalHost)?
        {
            keeper.end_before(deadline)?;
        }
    }
    Ok(())
}

/// Whether release builds run the Keeper: true since the recovery acceptance passed on real Windows
/// (`proposals/platform/host-handover.md`, decision 65). Development builds follow it, so a
/// development Session never runs a Keeper that release builds do not.
const IN_RELEASE_BUILDS: bool = true;

/// Whether this build runs the Keeper. Test builds always do, so tests and acceptance runs drive
/// it; release and development builds do while `IN_RELEASE_BUILDS` is set. It is a constant
/// expression, so a caller may decide at compile time.
pub const fn keeper_live() -> bool {
    match environment_kind() {
        EnvironmentKind::Release | EnvironmentKind::Development => IN_RELEASE_BUILDS,
        EnvironmentKind::Test => true,
    }
}

/// Whether release builds move terminals between Hosts: true since the H3 and H4 acceptance passed
/// on real Windows (`proposals/platform/host-handover.md`, H4 design). Development builds follow it.
const MOVES_IN_RELEASE_BUILDS: bool = true;

/// Whether this build moves terminals from an older Host to its Worker's own. Test builds always
/// do, so tests and acceptance runs drive moves; release and development builds do while
/// `MOVES_IN_RELEASE_BUILDS` is set. Moves need the Keeper, so a build without it moves none.
pub const fn moves_live() -> bool {
    keeper_live()
        && match environment_kind() {
            EnvironmentKind::Release | EnvironmentKind::Development => MOVES_IN_RELEASE_BUILDS,
            EnvironmentKind::Test => true,
        }
}

/// Starts the Session's Keeper in `slot` from `executable`, the Terminal Host's program, as the
/// recorded Worker's registered child in the Host's folder of `runtime`, and waits until its endpoint
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
    slot: crate::KeeperSlot,
    executable: &std::path::Path,
    deadline: std::time::Instant,
) -> std::io::Result<(crate::RetainedChild, (u32, u64))> {
    use crate::ModeCheckedLocalSessionRendezvous as Rendezvous;
    use std::{ffi::OsStr, io, thread, time::Duration};

    let directory = Rendezvous::host_runtime_directory(runtime);
    let endpoint_id = Rendezvous::keeper_endpoint_id(runtime, slot);
    let endpoint = Rendezvous::keeper_endpoint_path(runtime, slot);
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
        crate::SessionChild::Keeper(slot),
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
