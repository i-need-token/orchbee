//! What a replacement Worker takes over from the terminal service: the lost generation's settled
//! bindings to adopt, once it adopted their Host their move into its generation together with the
//! moment the Session's jobs start using it, and the rebinding of each terminal it moves to another
//! Host. It sends nothing to a Worker.

use std::{io, num::NonZeroU64};

use orchbee_capability_node_protocol::{AdoptBinding, HostTerminalIdentity, TerminalId};

use super::LocalTerminalService;
use crate::terminal_coordinator::{AdoptionImport, WorkerPlacement};

impl LocalTerminalService {
    /// The bindings a replacement of `lost` is asked to adopt.
    pub(crate) fn adoption_bindings(&self, lost: WorkerPlacement) -> io::Result<Vec<AdoptBinding>> {
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("Session coordinator poisoned"))?;
        Ok(coordinator.adoption_bindings(lost))
    }

    /// Moves the bindings of `lost` that `sent` names into the generation of the replacement that
    /// adopted them, then lets the Session's jobs use that replacement, both under the registry
    /// and coordinator guards every operation takes, so no operation sees the replacement before
    /// its bindings. Returns `None`, changing nothing, once `generation` is no longer the
    /// registered replacement awaiting adoption.
    pub(crate) fn serve_replacement(
        &self,
        lost: WorkerPlacement,
        generation: NonZeroU64,
        sent: &[TerminalId],
        missing: &[TerminalId],
    ) -> io::Result<Option<AdoptionImport>> {
        let mut workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry poisoned"))?;
        let Some(replacement) = workers
            .adopting_handle()
            .filter(|handle| handle.generation() == generation)
            .as_ref()
            .and_then(WorkerPlacement::of)
        else {
            return Ok(None);
        };
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("Session coordinator poisoned"))?;
        let import = coordinator.import_adopted(lost, replacement, sent, missing);
        if !workers.serve_adopted(generation) {
            return Ok(None);
        }
        Ok(Some(import))
    }

    /// Rebinds the binding of `terminal` that `worker` holds from `from` to `to`, the Host terminal
    /// that Worker moved it to; whether one still named `from`.
    pub(crate) fn rebind_moved(
        &self,
        worker: WorkerPlacement,
        terminal: TerminalId,
        from: HostTerminalIdentity,
        to: HostTerminalIdentity,
    ) -> io::Result<bool> {
        Ok(self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("Session coordinator poisoned"))?
            .rebind_moved(worker, terminal, from, to))
    }
}
