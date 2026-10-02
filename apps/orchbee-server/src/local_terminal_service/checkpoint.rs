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

pub(crate) struct CheckpointRestore {
    pending: Vec<PendingOperation>,
    managed: Vec<(
        orchbee_server::ManagedAgentRestart,
        orchbee_workbench_protocol::PanelAgentSession,
    )>,
}

impl LocalTerminalService {
    #[cfg(windows)]
    pub(crate) fn checkpoint_revision(
        &self,
    ) -> io::Result<orchbee_workbench_protocol::WorkbenchRevision> {
        self.coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_revision()
    }

    /// Build a bounded configuration from committed state and cached observations. The caller
    /// performs disk publication only after every Site/Worker gate below has been released.
    #[cfg(windows)]
    pub(crate) fn cached_checkpoint(
        &self,
        worker: &WorkerId,
        agents: &crate::agent_service::AgentService,
    ) -> io::Result<(orchbee_workbench_protocol::WorkbenchRevision, Vec<u8>)> {
        let workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?;
        let placement = placement(workers.request_handle().as_ref());
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let (revision, targets) = coordinator.checkpoint_observation_targets()?;
        if targets
            .iter()
            .any(|target| Some(target.target.worker) != placement)
        {
            return Err(io::Error::other("checkpoint Worker changed"));
        }
        let directories = coordinator.cached_checkpoint_directories(&targets);
        let bytes = coordinator.checkpoint_with_agent_sessions(worker, agents, &directories)?;
        Ok((revision, bytes))
    }

    /// Reconstructs and reserves every terminal before any request can launch a Host.
    pub(crate) fn from_checkpoint(
        workers: Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<WorkbenchCheckpoint>,
        resume_agents: bool,
    ) -> io::Result<(Self, CheckpointRestore)> {
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
        let saved_names = checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.managed_names().to_vec())
            .unwrap_or_default();
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
        let mut managed = Vec::new();
        for (target, launch) in plans {
            if launch.is_some()
                && let Some(claim) = saved_names
                    .iter()
                    .find(|claim| &claim.presentation_id == target.presentation_id())
                && let Some(session) = target.agent_session()
            {
                managed.push((claim.clone(), session.clone()));
            }
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
        Ok((service, CheckpointRestore { pending, managed }))
    }

    pub(crate) fn launch_checkpoint_terminals(
        &self,
        restore: CheckpointRestore,
        agents: &crate::agent_service::AgentService,
        deadline: Instant,
    ) -> io::Result<()> {
        let handle = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle();
        for terminal in restore.pending {
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
        if !restore.managed.is_empty() {
            super::agent_drain_read::agent_drain_finish(
                &self.workers,
                &self.coordinator,
                agents,
                deadline,
                std::time::Duration::from_millis(25),
                |coordinator, _| {
                    let facts = coordinator
                        .terminal_name_facts()
                        .map_err(|_| crate::agent_service::PreparedUnavailable)?;
                    agents.restore_managed_names(&restore.managed, &facts)
                },
            )
            .map_err(|_| io::Error::other("managed Agent name restoration unavailable"))?;
        }
        self.standalone_changed.notify_all();
        Ok(())
    }

    pub(crate) fn checkpoint(
        &self,
        worker: &WorkerId,
        agents: &crate::agent_service::AgentService,
        deadline: Instant,
    ) -> io::Result<Vec<u8>> {
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
        coordinator.checkpoint_with_agent_sessions(worker, agents, &directories)
    }
}
