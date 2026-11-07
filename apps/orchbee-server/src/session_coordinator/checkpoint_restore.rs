//! Joins checkpoint membership and retained launch state to prepare exact terminal restoration
//! requests and checkpoint directory facts. It does not read storage, launch a terminal, or
//! adopt a physical terminal from another incarnation.

use super::{SessionCoordinator, presentation_binding};
use crate::terminal_coordinator::{CreateInputs, PendingOperation};
use orchbee_client_server_protocol::{
    Geometry, StartMode, TerminalCreateRequest, WorkerConstraint, WorkerId, WorkingDirectory,
};
use orchbee_server::WorkbenchRestoreTarget;
use std::io;

impl SessionCoordinator {
    #[cfg(windows)]
    pub(crate) fn checkpoint_revision(
        &self,
    ) -> io::Result<orchbee_workbench_protocol::WorkbenchRevision> {
        self.workbench
            .as_ref()
            .map(|workbench| workbench.snapshot().revision)
            .ok_or_else(|| io::Error::other("Workbench unavailable"))
    }

    #[cfg(windows)]
    pub(crate) fn checkpoint_observation_targets(
        &self,
    ) -> io::Result<(
        orchbee_workbench_protocol::WorkbenchRevision,
        Vec<super::ProcessObservationTarget>,
    )> {
        use crate::terminal_coordinator::BindingViewerTarget;
        let workbench = self
            .workbench
            .as_ref()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?;
        let bindings = workbench.terminal_restart_bindings();
        let facts = self
            .agent_registration_for_bindings(&bindings)
            .map_err(|_| io::Error::other("checkpoint membership unavailable"))?;
        let mut targets = Vec::new();
        for fact in facts {
            let terminal_id = u128::from_str_radix(fact.binding().terminal_id.as_str(), 16)
                .ok()
                .and_then(|id| {
                    orchbee_capability_node_protocol::TerminalId::from_bytes(id.to_be_bytes()).ok()
                })
                .ok_or_else(|| io::Error::other("checkpoint terminal identity invalid"))?;
            let target = BindingViewerTarget {
                terminal_id,
                identity: fact.identity(),
                worker: crate::terminal_coordinator::WorkerPlacement {
                    worker_id: fact.worker_id(),
                    generation: fact.worker_generation(),
                },
            };
            if self.target_lifecycle(target) == super::TargetLifecycle::Unfenced {
                targets.push(super::ProcessObservationTarget {
                    session_id: workbench.session_id().clone(),
                    binding: fact.binding().clone(),
                    target,
                });
            }
        }
        Ok((workbench.snapshot().revision, targets))
    }

    #[cfg(windows)]
    pub(crate) fn checkpoint_with_observations(
        &self,
        worker: &orchbee_workbench_protocol::WorkerId,
        directories: &[(
            orchbee_workbench_protocol::PresentationBinding,
            orchbee_workbench_protocol::WorkbenchDirectoryPath,
        )],
        agent_sessions: Option<
            &[(
                orchbee_workbench_protocol::PresentationBinding,
                orchbee_server::RetainedAgentSession,
            )],
        >,
        outputs: Option<&[(orchbee_workbench_protocol::PresentationBinding, u64)]>,
    ) -> io::Result<Vec<u8>> {
        self.workbench
            .as_ref()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?
            .checkpoint_with_terminal_output(worker, directories, agent_sessions, outputs)
            .map_err(io::Error::other)
    }

    /// Every bound checkpoint member and the Presentation it shows.
    #[cfg(windows)]
    pub(crate) fn checkpoint_presentations(
        &self,
    ) -> Vec<(
        orchbee_workbench_protocol::PresentationBinding,
        orchbee_workbench_protocol::TerminalPresentationId,
    )> {
        self.workbench
            .as_ref()
            .map(orchbee_server::WorkbenchAuthority::terminal_restart_presentations)
            .unwrap_or_default()
    }

    /// The kept output token a held, still unbound Presentation would replay.
    pub(crate) fn restart_output(
        &self,
        presentation_id: &orchbee_workbench_protocol::TerminalPresentationId,
    ) -> Option<u64> {
        self.workbench.as_ref()?.restart_output(presentation_id)
    }

    /// Keeps `target` unbound in its place because the directory it would start in is
    /// unavailable. Only used while this entire coordinator is unpublished.
    #[cfg(windows)]
    pub(crate) fn hold_checkpoint_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        directory: orchbee_workbench_protocol::WorkbenchDirectoryPath,
        agent_resume: bool,
    ) -> io::Result<()> {
        self.workbench
            .as_mut()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?
            .hold_restored_terminal(target, directory, agent_resume)
            .map(drop)
            .map_err(io::Error::other)
    }

    /// Only used while this entire coordinator is unpublished; no launch IPC is performed here.
    pub(crate) fn prepare_checkpoint_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        inputs: CreateInputs,
        cwd: Option<WorkingDirectory>,
    ) -> io::Result<Box<PendingOperation>> {
        self.prepare_restore_binding(target, inputs, cwd)
            .map(|(pending, _binding, _revision)| pending)
    }

    /// Reserves one restored terminal and binds `target` to it, committing both owners under this
    /// one gate. Startup restore and a held restore share it, so both launch the same request.
    pub(crate) fn prepare_restore_binding(
        &mut self,
        target: &WorkbenchRestoreTarget,
        inputs: CreateInputs,
        cwd: Option<WorkingDirectory>,
    ) -> io::Result<(
        Box<PendingOperation>,
        orchbee_workbench_protocol::PresentationBinding,
        orchbee_workbench_protocol::WorkbenchRevision,
    )> {
        let (Some(reference), Some(expiry), Some(worker), capture) = inputs.into_parts() else {
            return Err(io::Error::other("restore admission inputs unavailable"));
        };
        let workbench = self
            .workbench
            .as_mut()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?;
        let worker_id = WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(worker.worker_id.as_bytes())
        ))
        .map_err(io::Error::other)?;
        let terminal = self
            .terminals
            .plan_create(
                TerminalCreateRequest::new(
                    reference.clone(),
                    Some(WorkerConstraint::Exact { worker_id }),
                    cwd,
                    StartMode::Immediate {
                        geometry: Geometry::new(80, 24, 8, 16).map_err(io::Error::other)?,
                    },
                    // A checkpoint retains no launch selection: every restored terminal starts the
                    // captured default program.
                    None,
                ),
                Some(worker),
                Some(expiry),
                &capture,
            )
            .map_err(|_| io::Error::other("restored terminal reservation unavailable"))?;
        let binding = presentation_binding(terminal.authorization())
            .ok_or_else(|| io::Error::other("restored terminal binding unavailable"))?;
        let rebind = workbench
            .prepare_restored_terminal(target, binding.clone(), terminal.launch_program().cloned())
            .map_err(io::Error::other)?;
        let pending = terminal.commit();
        let revision = rebind.commit();
        Ok((pending, binding, revision))
    }
}
