//! Loads admitted checkpoint values into the coordinator, samples current terminal directory
//! facts, and dispatches prepared restoration launches outside the state lock. It does not
//! define the checkpoint format or own filesystem storage publication.

use super::{LocalTerminalService, create_reference, lease_expiry, placement, random_session_id};
use crate::{
    terminal_coordinator::{Completion, CreateInputs, PendingOperation},
    worker_registry::WorkerRegistry,
};
use orchbee_server::{WorkbenchAuthority, WorkbenchCheckpoint};
use orchbee_workbench_protocol::WorkerId;
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod riders;
pub(super) use riders::{Riders, attach_held_seed};

pub(crate) struct CheckpointRestore {
    /// Each restored launch and, when replay is on, the kept output its Presentation replays.
    pending: Vec<(PendingOperation, riders::Output)>,
    #[cfg(any(windows, target_os = "macos"))]
    resumes: riders::Resumes,
}

#[cfg(any(windows, target_os = "macos"))]
impl CheckpointRestore {
    /// The Presentations whose retained Agent session reference survived startup, in restoration
    /// order. Taking them leaves none behind.
    pub(crate) fn take_agent_resume(&mut self) -> super::agent_resume::AgentResumeCandidates {
        std::mem::take(&mut self.resumes)
    }
}

#[cfg(test)]
mod tests;

impl LocalTerminalService {
    pub(crate) fn checkpoint_revision(
        &self,
    ) -> io::Result<orchbee_workbench_protocol::WorkbenchRevision> {
        self.coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_revision()
    }

    /// Build a bounded configuration from committed state and cached observations. The caller
    /// performs disk publication only after every Session/Worker gate below has been released.
    pub(crate) fn cached_checkpoint(
        &self,
        worker: &WorkerId,
    ) -> io::Result<(orchbee_workbench_protocol::WorkbenchRevision, Vec<u8>)> {
        let riders = Riders::current(self);
        let entries = riders.entries(self);
        let workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?;
        let placement = placement(workers.request_handle().as_ref());
        let mut coordinator = self
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
        let bytes = riders.checkpoint(
            self,
            &mut coordinator,
            worker,
            &directories,
            &targets,
            &entries,
        )?;
        Ok((revision, bytes))
    }

    /// What the binding record says that can change, for the saver to compare between looks.
    pub(crate) fn binding_observation(
        &self,
    ) -> io::Result<crate::session_coordinator::BindingObservation> {
        Ok(self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .binding_observation())
    }

    /// What the binding record keeps of Session Worker `worker`'s terminals, with the observation it
    /// matches, from one coordinator acquisition.
    pub(crate) fn binding_capture(
        &self,
        worker: &WorkerId,
    ) -> io::Result<(
        crate::session_coordinator::BindingObservation,
        crate::binding_record::Capture,
    )> {
        let worker = u128::from_str_radix(worker.as_str(), 16)
            .ok()
            .and_then(|worker| {
                orchbee_capability_node_protocol::WorkerId::from_bytes(worker.to_be_bytes()).ok()
            })
            .ok_or_else(|| io::Error::other("invalid Worker"))?;
        let coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        Ok((
            coordinator.binding_observation(),
            coordinator.binding_capture(worker),
        ))
    }

    /// Captures kept terminal output for the next background save within `budget`, where output
    /// replay is on.
    pub(crate) fn capture_kept_output_within(&self, budget: Duration) {
        riders::capture_background(self, budget);
    }

    /// Settles what a committed save no longer names, such as kept output it dropped.
    pub(crate) fn after_checkpoint_save(&self, bytes: &[u8]) {
        riders::after_save(self, bytes);
    }

    /// Reconstructs and reserves every terminal before any request can launch a Host. Restored
    /// launches take their settings from `configuration`, the Session's startup read that later
    /// becomes its configuration, so they capture the same values as the first later creation.
    pub(crate) fn from_checkpoint(
        workers: Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<WorkbenchCheckpoint>,
        configuration: &orchbee_local_runtime::UserSettingsLoad,
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
        let session = random_session_id()?;
        let recent = checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.recent_agents().to_vec());
        let authority = match checkpoint {
            Some(checkpoint) => checkpoint
                .reconstruct(session, &owner)
                .map_err(io::Error::other)?,
            None => WorkbenchAuthority::new(session).map_err(io::Error::other)?,
        };
        let targets = authority
            .restoration_targets(&owner)
            .map_err(io::Error::other)?;
        let mut service = Self::from_workbench(workers, Some(authority));
        // The Agent catalog owns Recent; the checkpoint only carried it.
        if let Some(recent) = &recent {
            service
                .workers
                .lock()
                .map_err(|_| io::Error::other("Worker registry failed"))?
                .agent_catalog()
                .restore_recent(recent);
        }
        let values = &configuration.settings.values;
        // The settings decide resuming and replay once, from the startup read; with replay off the
        // first save removes the kept output.
        let riders = Riders::startup(values);
        // One read serves every restored launch.
        let capture = super::settings::launch_capture(values);
        let coordinator = service
            .coordinator
            .get_mut()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let expiry = lease_expiry().ok_or_else(|| io::Error::other("restore lease unavailable"))?;
        let mut pending = Vec::with_capacity(targets.len());
        let mut resumes = riders::resumes();
        for target in targets {
            // A Presentation whose directory is unavailable stays unbound in its place, keeping
            // its retained directory and, when resuming is on, its Agent reference until the
            // person retries there or starts elsewhere. Nothing retries on its own.
            if let Some(directory) = target
                .working_directory()
                .filter(|p| !std::path::Path::new(p.as_str()).is_dir())
                .cloned()
            {
                let agent_resume = riders.hold_agent(coordinator, &target);
                coordinator.hold_checkpoint_terminal(&target, directory, agent_resume)?;
                continue;
            }
            riders.restore_agent(coordinator, &target, &mut resumes);
            let reference = create_reference()
                .ok_or_else(|| io::Error::other("restore request identity unavailable"))?;
            let cwd = target
                .working_directory()
                .map(|p| orchbee_client_server_protocol::WorkingDirectory::parse(p.as_str()))
                .transpose()
                .map_err(io::Error::other)?;
            let inputs = CreateInputs::admitted_by(
                Some(reference),
                Some(expiry),
                Some(&handle),
                capture.clone(),
            );
            let output = riders.restored_output(&target);
            pending.push((
                *coordinator.prepare_checkpoint_terminal(&target, inputs, cwd)?,
                output,
            ));
        }
        Ok((
            service,
            CheckpointRestore {
                pending,
                #[cfg(any(windows, target_os = "macos"))]
                resumes,
            },
        ))
    }

    pub(crate) fn launch_checkpoint_terminals(
        &self,
        restore: CheckpointRestore,
        deadline: Instant,
    ) -> io::Result<()> {
        let handle = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?
            .request_handle();
        for (mut terminal, output) in restore.pending {
            // The kept output reaches the Host with the launch, before the program's first byte;
            // a file that cannot replay here is never attached elsewhere.
            riders::attach_seed(self, &mut terminal, output);
            let completion = if Instant::now() >= deadline {
                Completion::Unavailable
            } else {
                handle.as_ref().map_or(Completion::Unavailable, |handle| {
                    self.submit(handle, terminal.command())
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
        }
        Ok(())
    }

    pub(crate) fn checkpoint(&self, worker: &WorkerId, deadline: Instant) -> io::Result<Vec<u8>> {
        let riders = Riders::current(self);
        let (revision, targets) = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_observation_targets()?;
        let presentations = riders.presentations(self)?;
        self.settle_checkpoint_viewer(None)?;
        let mut directories = Vec::new();
        let mut output = riders.final_output(presentations, deadline);
        for target in &targets {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let (directory, pending) = self.sample_checkpoint_directory(target);
            self.settle_checkpoint_viewer(pending)?;
            if let Some(directory) = directory {
                directories.push((target.binding.clone(), directory));
            }
            output.capture(self, target)?;
        }
        let entries = riders.entries(self);
        let workers = self
            .workers
            .lock()
            .map_err(|_| io::Error::other("Worker registry failed"))?;
        let placement = placement(workers.request_handle().as_ref());
        if targets.iter().any(|t| Some(t.target.worker) != placement) || Instant::now() >= deadline
        {
            return Err(io::Error::other("checkpoint observation expired"));
        }
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        if coordinator.checkpoint_observation_targets()? != (revision, targets.clone()) {
            return Err(io::Error::other("checkpoint physical membership changed"));
        }
        riders.checkpoint(
            self,
            &mut coordinator,
            worker,
            &directories,
            &targets,
            &entries,
        )
    }

    /// The final checkpoint once the Worker has exited. Nothing can sample a directory any more, so
    /// each terminal keeps its last sample, fenced to its exact generation as a background save
    /// fences it, and otherwise its restart directory. No placement fence applies: the Worker those
    /// samples name is the one that exited. Unreadable membership keeps only restart directories
    /// rather than leaving the Session unable to stop.
    pub(crate) fn checkpoint_without_worker(&self, worker: &WorkerId) -> io::Result<Vec<u8>> {
        let riders = Riders::current(self);
        let entries = riders.entries(self);
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let targets = coordinator
            .checkpoint_observation_targets()
            .map(|(_, targets)| targets)
            .unwrap_or_default();
        let directories = coordinator.cached_checkpoint_directories(&targets);
        // Output kept from earlier saves still names the exited terminals' files.
        riders.checkpoint(
            self,
            &mut coordinator,
            worker,
            &directories,
            &targets,
            &entries,
        )
    }
}
