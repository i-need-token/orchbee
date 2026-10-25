use super::{SiteCoordinator, presentation_binding};
use crate::terminal_coordinator::{PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::{
    Geometry, RequestId, StartMode, TerminalCreateRequest, WorkerConstraint, WorkerId,
    WorkingDirectory,
};
use orchbee_server::WorkbenchRestoreTarget;
use std::io;

impl SiteCoordinator {
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
                worker: WorkerPlacement {
                    worker_id: fact.worker_id(),
                    generation: fact.worker_generation(),
                },
            };
            if self.target_lifecycle(target) == super::TargetLifecycle::Unfenced {
                targets.push(super::ProcessObservationTarget {
                    site_id: workbench.site_id().clone(),
                    binding: fact.binding().clone(),
                    target,
                });
            }
        }
        Ok((workbench.snapshot().revision, targets))
    }

    pub(crate) fn checkpoint_with_agent_sessions(
        &self,
        worker: &orchbee_workbench_protocol::WorkerId,
        agents: &crate::agent_service::AgentService,
        directories: &[(
            orchbee_workbench_protocol::PresentationBinding,
            orchbee_workbench_protocol::WorkbenchDirectoryPath,
        )],
    ) -> io::Result<Vec<u8>> {
        let facts = self.checkpoint_agent_bindings()?;
        let workbench = self
            .workbench
            .as_ref()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?;
        let claims = agents
            .managed_name_claims()
            .map_err(|_| io::Error::other("managed Names unavailable"))?;
        let registrations = self
            .agent_registration_for_bindings(
                &facts.iter().map(|f| f.binding.clone()).collect::<Vec<_>>(),
            )
            .map_err(|_| io::Error::other("checkpoint name membership unavailable"))?;
        let mut managed = Vec::new();
        for claim in claims {
            let Some(registration) = registrations.iter().find(|fact| {
                fact.presentation_id() == &claim.binding.presentation
                    && fact.identity() == claim.binding.host
                    && fact.worker_id() == claim.binding.carrier.worker_id
                    && fact.worker_generation() == claim.binding.carrier.generation
            }) else {
                continue;
            };
            let Some(physical) = facts
                .iter()
                .find(|fact| fact.binding == *registration.binding())
            else {
                continue;
            };
            if workbench
                .terminal_restart_session(physical)
                .and_then(|session| {
                    orchbee_agent_observation_protocol::AgentReportProvider::lookup(&session.agent)
                })
                .is_some_and(|provider| provider.native_provider_id() == claim.provider.as_str())
            {
                managed.push(orchbee_server::ManagedAgentRestart {
                    presentation_id: claim.binding.presentation,
                    name: claim.name,
                    provider_id: claim.provider,
                });
            }
        }
        workbench
            .checkpoint_with_managed_agents(worker, &facts, directories, &managed)
            .map_err(io::Error::other)
    }

    pub(crate) fn seed_checkpoint_agent_sessions(&mut self) -> io::Result<()> {
        let facts = self.checkpoint_agent_bindings()?;
        self.workbench
            .as_mut()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?
            .seed_restored_agent_sessions(&facts, std::time::Instant::now())
            .map_err(|reason| {
                io::Error::other(format!("Agent restart reference unavailable: {reason:?}"))
            })?;
        Ok(())
    }

    fn checkpoint_agent_bindings(&self) -> io::Result<Vec<orchbee_server::TerminalAgentBinding>> {
        let bindings = self
            .workbench
            .as_ref()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?
            .agent_restart_bindings()
            .map_err(io::Error::other)?;
        Ok(self
            .agent_registration_for_bindings(&bindings)
            .map_err(|_| io::Error::other("checkpoint membership unavailable"))?
            .into_iter()
            .map(|fact| orchbee_server::TerminalAgentBinding {
                binding: fact.binding().clone(),
                worker_generation: fact.worker_generation(),
                identity: fact.identity(),
            })
            .collect())
    }

    /// Only used while this entire coordinator is unpublished; no launch IPC is performed here.
    pub(crate) fn prepare_checkpoint_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        reference: RequestId,
        worker: WorkerPlacement,
        expiry: LeaseExpiresAtUnixMilliseconds,
        launch: Option<orchbee_client_server_protocol::LaunchSelection>,
        cwd: Option<WorkingDirectory>,
    ) -> io::Result<Box<PendingOperation>> {
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
                    launch,
                ),
                Some(worker),
                Some(expiry),
            )
            .map_err(|_| io::Error::other("restored terminal reservation unavailable"))?;
        let binding = presentation_binding(terminal.authorization())
            .ok_or_else(|| io::Error::other("restored terminal binding unavailable"))?;
        let rebind = workbench
            .prepare_restored_terminal(target, binding)
            .map_err(io::Error::other)?;
        let pending = terminal.commit();
        let _revision = rebind.commit();
        Ok(pending)
    }
}
