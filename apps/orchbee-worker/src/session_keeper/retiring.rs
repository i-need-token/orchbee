//! The Keeper's update support and retirement on Windows: a Keeper an update replaced or rolled
//! back, or one an adoption found beside the current Keeper, is retired. Each Host detaches it
//! before it ends, so no Host reports it lost, and its slot in the Session record is then freed.

use std::{
    error::Error,
    io,
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::Instant,
};

use orchbee_local_runtime::{ExactProcess, KeeperProcess, KeeperSlot};
use orchbee_terminal_protocol::{HandoverControl, HandoverProcess};

use super::{
    END_BOUND, Owner, RECORD_BUDGET, Serving, SessionKeeper, data_root, request, unexpected,
};
use crate::host_connection::HostBrokerConnection;

/// A Keeper this Worker retires: one an update replaced or rolled back, or one an adoption found
/// beside the current Keeper. Each Host detaches it, then it is ended and its slot freed.
pub(crate) struct RetiringKeeper {
    owner: Owner,
    process: KeeperProcess,
    slot: KeeperSlot,
    /// The Hosts that detached it.
    detached: Vec<(u32, u64)>,
    /// Whether a failed detach was recorded, once per retirement.
    recorded: bool,
}

impl SessionKeeper {
    /// The Keeper's process ID and creation time.
    pub(crate) const fn identity(&self) -> (u32, u64) {
        (self.process.pid, self.process.created)
    }

    /// Makes this Keeper's slot the Session record's current Keeper's, which commits an update.
    ///
    /// # Errors
    ///
    /// Returns an error when the record cannot be written or no longer names this Keeper.
    pub(crate) fn make_current(&self, runtime: &Path) -> io::Result<()> {
        orchbee_local_runtime::make_session_keeper_current(
            &data_root()?,
            runtime,
            self.slot,
            self.identity(),
            Instant::now() + RECORD_BUDGET,
        )
    }

    /// The live Keeper an adoption found in the other slot, which the caller retires.
    pub(crate) fn take_other(&mut self) -> Option<RetiringKeeper> {
        self.other.take()
    }

    /// Whether the Keeper runs `executable`: one this Worker started does; an adopted one runs
    /// the program its full image path names. `None` when that path cannot be read.
    pub(crate) fn runs(&self, executable: &Path) -> Option<bool> {
        let Owner::Adopted(process) = &self.owner else {
            return Some(true);
        };
        let image = process.image_path().ok()?;
        Some(canonical(&image)? == canonical(executable)?)
    }

    /// This Keeper as one to retire, closing the Worker's connection to it.
    pub(crate) fn retire(self) -> RetiringKeeper {
        RetiringKeeper {
            owner: self.owner,
            process: self.process,
            slot: self.slot,
            detached: Vec::new(),
            recorded: false,
        }
    }
}

impl RetiringKeeper {
    pub(super) fn adopted(slot: KeeperSlot, process: ExactProcess) -> io::Result<Self> {
        let (pid, created) = process.identity();
        Ok(Self {
            owner: Owner::Adopted(process.endable()?),
            process: KeeperProcess { pid, created },
            slot,
            detached: Vec::new(),
            recorded: false,
        })
    }

    /// Has each of `hosts`, a Host connection with its process, that has not yet detached this
    /// Keeper detach it between Server requests; once every one did, ends the Keeper, which then
    /// holds nothing, and frees its slot in the Session record at `runtime`. Returns whether it
    /// ended. A failed detach is tried again at the next call and recorded once.
    pub(crate) fn retire<'a>(
        &mut self,
        runtime: &Path,
        hosts: impl IntoIterator<Item = (&'a HostBrokerConnection, (u32, u64))>,
    ) -> bool {
        let keeper = HandoverProcess {
            pid: match NonZeroU32::new(self.process.pid) {
                Some(pid) => pid,
                None => return false,
            },
            creation_time: self.process.created,
        };
        let mut pending = false;
        for (host, process) in hosts {
            if self.detached.contains(&process) {
                continue;
            }
            match detach(host, keeper) {
                Ok(()) => self.detached.push(process),
                Err(error) => {
                    pending = true;
                    if !std::mem::replace(&mut self.recorded, true) {
                        crate::diagnostic_events::keeper_failed("detach", error.as_ref());
                    }
                }
            }
        }
        if pending {
            return false;
        }
        self.end(runtime);
        true
    }

    /// Ends the Keeper and frees its slot. The caller ends it only once no Host holds anything in
    /// it, or every Host is gone.
    pub(crate) fn end(&mut self, runtime: &Path) {
        let deadline = Instant::now() + END_BOUND;
        self.owner.end(deadline);
        let released = data_root().and_then(|root| {
            orchbee_local_runtime::release_session_keeper(
                &root,
                runtime,
                self.slot,
                (self.process.pid, self.process.created),
                Instant::now() + RECORD_BUDGET,
            )
        });
        if let Err(error) = released {
            crate::diagnostic_events::keeper_failed("release", &error);
        }
    }

    /// Ends the Keeper without touching the Session record, for a stop that ends every Host.
    pub(crate) fn end_with_session(&mut self, deadline: Instant) {
        self.owner.end(deadline);
    }
}

/// Has `host` detach `keeper` between Server requests.
fn detach(host: &HostBrokerConnection, keeper: HandoverProcess) -> Result<(), Box<dyn Error>> {
    match request(
        host,
        Serving::BetweenRequests,
        &HandoverControl::DetachKeeper(keeper),
    )? {
        HandoverControl::KeeperDetached => Ok(()),
        _ => Err(unexpected().into()),
    }
}

/// `path` with every link resolved, for comparing two programs.
fn canonical(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}
