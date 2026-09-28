use super::{
    LocalTerminalService, create_reference, lease_expiry, placement, random_site_id, submit,
};
use crate::{
    terminal_coordinator::{Completion, PendingOperation},
    worker_registry::WorkerRegistry,
};
use orchbee_server::{WorkbenchAuthority, WorkbenchCheckpoint};
use orchbee_workbench_protocol::WorkerId;
use std::{
    io,
    sync::{Arc, Mutex},
    time::Instant,
};

impl LocalTerminalService {
    /// Reconstructs and reserves every terminal before any request can launch a Host.
    pub(crate) fn from_checkpoint(
        workers: Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<WorkbenchCheckpoint>,
    ) -> io::Result<(Self, Vec<PendingOperation>)> {
        let handle = workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle()
            .ok_or_else(|| io::Error::other("Worker unavailable"))?;
        let worker = placement(Some(&handle)).ok_or_else(|| io::Error::other("invalid Worker"))?;
        let owner = WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(worker.worker_id.as_bytes())
        ))
        .map_err(io::Error::other)?;
        let site = random_site_id()?;
        let authority = match checkpoint {
            Some(checkpoint) => checkpoint
                .reconstruct(site, &owner)
                .map_err(io::Error::other)?,
            None => WorkbenchAuthority::new(site).map_err(io::Error::other)?,
        };
        let targets = authority
            .restoration_targets(&owner)
            .map_err(io::Error::other)?;
        let mut service = Self::from_workbench(workers, Some(authority));
        let coordinator = service
            .coordinator
            .get_mut()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let expiry = lease_expiry().ok_or_else(|| io::Error::other("restore lease unavailable"))?;
        let mut pending = Vec::with_capacity(targets.len());
        for target in targets {
            let reference = create_reference()
                .ok_or_else(|| io::Error::other("restore request identity unavailable"))?;
            pending.push(
                *coordinator.prepare_checkpoint_terminal(&target, reference, worker, expiry)?,
            );
        }
        Ok((service, pending))
    }

    pub(crate) fn launch_checkpoint_terminals(
        &self,
        pending: Vec<PendingOperation>,
        deadline: Instant,
    ) -> io::Result<()> {
        let handle = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle();
        for terminal in pending {
            let completion = if Instant::now() >= deadline {
                Completion::Unavailable
            } else {
                handle.as_ref().map_or(Completion::Unavailable, |handle| {
                    submit(handle, terminal.command())
                })
            };
            let workers = self
                .workers
                .lock()
                .map_err(|_| io::Error::other("Worker registry failed"))?;
            let mut coordinator = self
                .coordinator
                .lock()
                .map_err(|_| io::Error::other("coordinator failed"))?;
            coordinator
                .complete(
                    Box::new(terminal),
                    completion,
                    placement(workers.request_handle().as_ref()),
                )
                .map_err(|_| io::Error::other("restore settlement failed"))?;
            coordinator.seed_checkpoint_agent_sessions()?;
        }
        self.standalone_changed.notify_all();
        Ok(())
    }

    pub(crate) fn checkpoint(&self, worker: &WorkerId) -> io::Result<Vec<u8>> {
        self.coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_with_agent_sessions(worker)
    }
}
