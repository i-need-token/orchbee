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
    time::Instant,
};

pub(crate) struct CheckpointRestore {
    /// Each restored launch and, when replay is on, the kept output its Presentation replays.
    pending: Vec<(PendingOperation, Option<RestoredOutput>)>,
    agent_resume: super::agent_resume::AgentResumeCandidates,
}

/// The kept output one restored Presentation replays before its program starts.
struct RestoredOutput {
    presentation_id: orchbee_workbench_protocol::TerminalPresentationId,
    token: u64,
}

impl CheckpointRestore {
    /// The Presentations whose retained Agent session reference survived startup, in restoration
    /// order. Taking them leaves none behind.
    pub(crate) fn take_agent_resume(&mut self) -> super::agent_resume::AgentResumeCandidates {
        std::mem::take(&mut self.agent_resume)
    }
}

#[cfg(test)]
mod tests;

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

    /// What Claude Code's run table names for each published Claude Code run, read while no lock
    /// is held. Capture admits an entry only for a run still published as that exact process, so
    /// any change in between gains nothing. Resuming off, or an unreadable coordinator, reads
    /// nothing.
    fn run_table_entries(&self, resume: bool) -> Vec<crate::session_coordinator::RunTableEntry> {
        if !resume {
            return Vec::new();
        }
        let anchors = match self.coordinator.lock() {
            Ok(coordinator) => coordinator.run_table_anchors(),
            Err(_) => return Vec::new(),
        };
        crate::session_coordinator::read_run_table(&anchors)
    }

    /// Build a bounded configuration from committed state and cached observations. The caller
    /// performs disk publication only after every Session/Worker gate below has been released.
    #[cfg(windows)]
    pub(crate) fn cached_checkpoint(
        &self,
        worker: &WorkerId,
    ) -> io::Result<(orchbee_workbench_protocol::WorkbenchRevision, Vec<u8>)> {
        let resume = self.agent_resume_enabled();
        let replay = self.output_replay_enabled();
        let entries = self.run_table_entries(resume);
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
        let agents = resume.then(|| coordinator.checkpoint_agent_sessions(&targets, &entries));
        let outputs = self.checkpoint_outputs(replay, &targets);
        let bytes = coordinator.checkpoint_with_observations(
            worker,
            &directories,
            agents.as_deref(),
            outputs.as_deref(),
        )?;
        Ok((revision, bytes))
    }

    /// Reconstructs and reserves every terminal before any request can launch a Host. The
    /// Session's preference scope is frozen first, so restored launches capture the same
    /// preferences as any later creation.
    pub(crate) fn from_checkpoint(
        workers: Arc<Mutex<WorkerRegistry>>,
        checkpoint: Option<WorkbenchCheckpoint>,
        preferences: orchbee_local_runtime::UserPreferencesScope,
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
        service.initialize_preferences(preferences)?;
        // The setting decides resuming once, from the store the scope just froze.
        let resume = service.agent_resume_enabled();
        // Output replays only while the setting is on; otherwise the first save removes it.
        let replay = service.output_replay_enabled();
        // One read serves every restored launch, after the scope it reads is frozen.
        let capture = service.launch_capture();
        let coordinator = service
            .coordinator
            .get_mut()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let expiry = lease_expiry().ok_or_else(|| io::Error::other("restore lease unavailable"))?;
        let mut pending = Vec::with_capacity(targets.len());
        let mut agent_resume = super::agent_resume::AgentResumeCandidates::default();
        for target in targets {
            // A Presentation whose directory is unavailable stays unbound in its place, keeping
            // its retained directory and, when resuming is on, its Agent reference until the
            // person retries there or starts elsewhere. Nothing retries on its own.
            if let Some(directory) = target
                .working_directory()
                .filter(|p| !std::path::Path::new(p.as_str()).is_dir())
                .cloned()
            {
                let agent_resume =
                    resume && coordinator.has_pending_agent_session(target.presentation_id());
                if !agent_resume {
                    coordinator.take_pending_agent_session(target.presentation_id());
                }
                coordinator.hold_checkpoint_terminal(&target, directory, agent_resume)?;
                continue;
            }
            if coordinator.has_pending_agent_session(target.presentation_id()) {
                // A reference resumes only where it was recorded.
                match target
                    .restored_directory()
                    .filter(|p| std::path::Path::new(p.as_str()).is_dir())
                {
                    Some(directory) if resume => {
                        agent_resume.push(target.presentation_id().clone(), directory.clone());
                    }
                    _ => {
                        coordinator.take_pending_agent_session(target.presentation_id());
                    }
                }
            }
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
            let output = target
                .restored_output()
                .filter(|_| replay)
                .map(|token| RestoredOutput {
                    presentation_id: target.presentation_id().clone(),
                    token,
                });
            pending.push((
                *coordinator.prepare_checkpoint_terminal(&target, inputs, cwd)?,
                output,
            ));
        }
        Ok((
            service,
            CheckpointRestore {
                pending,
                agent_resume,
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
            if let Some(output) = output
                && let Some(seed) = self.restored_output_seed(&output.presentation_id, output.token)
                && terminal.set_output_seed(seed).is_err()
            {
                eprintln!(
                    "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=seed cause=refused"
                );
            }
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
        let resume = self.agent_resume_enabled();
        let replay = self.output_replay_enabled();
        let (revision, targets) = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?
            .checkpoint_observation_targets()?;
        let presentations = if replay {
            self.coordinator
                .lock()
                .map_err(|_| io::Error::other("coordinator failed"))?
                .checkpoint_presentations()
        } else {
            Vec::new()
        };
        self.settle_checkpoint_viewer(None)?;
        let mut directories = Vec::new();
        let mut output_deadline_noted = false;
        let output_deadline =
            Instant::now() + deadline.saturating_duration_since(Instant::now()) / 2;
        for target in &targets {
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let (directory, pending) = self.sample_checkpoint_directory(target);
            self.settle_checkpoint_viewer(pending)?;
            if let Some(directory) = directory {
                directories.push((target.binding.clone(), directory));
            }
            // Output capture spends at most half the save's budget; past it each terminal keeps
            // its previous file and the save goes on.
            if replay {
                if Instant::now() >= output_deadline {
                    if !output_deadline_noted {
                        output_deadline_noted = true;
                        eprintln!(
                            "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=final-capture cause=deadline"
                        );
                    }
                    continue;
                }
                let presentation = presentations
                    .iter()
                    .find(|(binding, _)| binding == &target.binding)
                    .map(|(_, presentation)| presentation);
                let pending = self.capture_final_output(target, presentation);
                self.settle_checkpoint_viewer(pending)?;
            }
        }
        let entries = self.run_table_entries(resume);
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
        let agents = resume.then(|| coordinator.checkpoint_agent_sessions(&targets, &entries));
        let outputs = self.checkpoint_outputs(replay, &targets);
        coordinator.checkpoint_with_observations(
            worker,
            &directories,
            agents.as_deref(),
            outputs.as_deref(),
        )
    }

    /// The final checkpoint once the Worker has exited. Nothing can sample a directory any more, so
    /// each terminal keeps its last sample, fenced to its exact generation as a background save
    /// fences it, and otherwise its restart directory. No placement fence applies: the Worker those
    /// samples name is the one that exited. Unreadable membership keeps only restart directories
    /// rather than leaving the Session unable to stop.
    pub(crate) fn checkpoint_without_worker(&self, worker: &WorkerId) -> io::Result<Vec<u8>> {
        let resume = self.agent_resume_enabled();
        let replay = self.output_replay_enabled();
        let entries = self.run_table_entries(resume);
        let mut coordinator = self
            .coordinator
            .lock()
            .map_err(|_| io::Error::other("coordinator failed"))?;
        let targets = coordinator
            .checkpoint_observation_targets()
            .map(|(_, targets)| targets)
            .unwrap_or_default();
        let directories = coordinator.cached_checkpoint_directories(&targets);
        let agents = resume.then(|| coordinator.checkpoint_agent_sessions(&targets, &entries));
        // Output kept from earlier saves still names the exited terminals' files.
        let outputs = self.checkpoint_outputs(replay, &targets);
        coordinator.checkpoint_with_observations(
            worker,
            &directories,
            agents.as_deref(),
            outputs.as_deref(),
        )
    }
}
