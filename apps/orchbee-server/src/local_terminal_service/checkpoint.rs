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
mod resume;

impl LocalTerminalService {
    /// Reconstructs and reserves every terminal before any request can launch a Host.
    pub(crate) fn from_checkpoint(
        workers: Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<WorkbenchCheckpoint>,
        resume_agents: bool,
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
        let mut authority = match checkpoint {
            Some(checkpoint) => checkpoint
                .reconstruct(site, &owner)
                .map_err(io::Error::other)?,
            None => WorkbenchAuthority::new(site).map_err(io::Error::other)?,
        };
        let plans = resume::prepare(&mut authority, &owner, resume_agents)?;
        let mut service = Self::from_workbench(workers, Some(authority));
        let coordinator = service
            .coordinator
            .get_mut()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let expiry = lease_expiry().ok_or_else(|| io::Error::other("restore lease unavailable"))?;
        let mut pending = Vec::with_capacity(plans.len());
        for (target, launch) in plans {
            let reference = create_reference()
                .ok_or_else(|| io::Error::other("restore request identity unavailable"))?;
            let cwd = target
                .working_directory()
                .filter(|p| std::path::Path::new(p.as_str()).is_dir())
                .map(|p| orchbee_client_server_protocol::WorkingDirectory::parse(p.as_str()))
                .transpose()
                .map_err(io::Error::other)?;
            pending
                .push(*coordinator.prepare_checkpoint_terminal(
                    &target, reference, worker, expiry, launch, cwd,
                )?);
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

    pub(crate) fn checkpoint(&self, worker: &WorkerId, deadline: Instant) -> io::Result<Vec<u8>> {
        let (revision, targets) = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_observation_targets()?;
        self.settle_checkpoint_viewer(None)?;
        let mut directories = Vec::new();
        for target in &targets {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let (directory, pending) = self.sample_checkpoint_directory(target);
            self.settle_checkpoint_viewer(pending)?;
            if let Some(directory) = directory {
                directories.push((target.binding.clone(), directory));
            }
        }
        let workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?;
        let placement = placement(workers.request_handle().as_ref());
        if targets.iter().any(|t| Some(t.target.worker) != placement) || Instant::now() >= deadline
        {
            return Err(io::Error::other("checkpoint observation expired"));
        }
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        if coordinator.checkpoint_observation_targets()? != (revision, targets) {
            return Err(io::Error::other("checkpoint physical membership changed"));
        }
        coordinator.checkpoint_with_agent_sessions(worker, &directories)
    }
}
