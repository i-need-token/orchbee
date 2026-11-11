//! Pairs prepared Tab or Workspace launch membership with an exact terminal authorization,
//! preserving committed launch metadata through settlement. It does not perform Worker I/O or
//! redirect a completion to another prepared creation.

use super::{SessionCoordinator, presentation_binding};
use crate::terminal_coordinator::{Completion, CreateInputs, PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::{CapabilityNodeMessage, TerminalOperationAuthorization};
use orchbee_client_server_protocol::{
    CreateAcceptedState, CreateOutcome, CreateRejectedReason, CreateUnavailableReason, Geometry,
    RequestId, ServerMessage, StartMode, TerminalCreateRequest, WorkerConstraint, WorkingDirectory,
};
use orchbee_server::{
    PendingWorkbenchTabCreate, WorkbenchAuthority, WorkbenchGrant, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
};
use orchbee_workbench_protocol::{
    PresentationBinding, ProtocolVersion, TerminalPresentationId, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchRejectionReason, WorkbenchTerminalFailureReason,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkbenchTerminalUnavailableReason,
};

pub(crate) enum PreparedTerminalTabCreate {
    Ready(WorkbenchCommandResult),
    Dispatch(PendingTerminalTabCreate),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompletedTerminalTabCreate {
    pub(crate) result: WorkbenchCommandResult,
    /// The Agent start the Server delivers into the started terminal.
    pub(crate) agent_start: Option<super::AgentStartLaunch>,
}

pub(crate) struct PendingTerminalTabCreate {
    workbench: PendingWorkbenchTabCreate,
    terminal: Box<PendingOperation>,
    reference: RequestId,
    worker: WorkerPlacement,
    workspace_cwd: Option<WorkingDirectory>,
    launch_cwd: Option<WorkingDirectory>,
    environment: orchbee_client_server_protocol::LaunchEnvironment,
    launch: Option<orchbee_client_server_protocol::LaunchSelection>,
    plugin_environment: Option<orchbee_client_server_protocol::PluginTerminalEnvironment>,
    agent_start: Option<orchbee_workbench_protocol::AgentStartSelector>,
}

impl PendingTerminalTabCreate {
    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.terminal.command()
    }

    /// The Tab this creation committed and the revision that committed it.
    pub(crate) fn created(
        &self,
    ) -> (
        &orchbee_workbench_protocol::WorkbenchTabId,
        orchbee_workbench_protocol::WorkbenchRevision,
    ) {
        let metadata = self.workbench.metadata();
        (metadata.tab_id(), metadata.revision())
    }

    fn association_matches(&self) -> bool {
        let metadata = self.workbench.metadata();
        let authorization = pending_authorization(&self.terminal);
        self.workspace_cwd.is_some()
            && self.terminal.create_reference() == &self.reference
            && authorization.and_then(presentation_binding).as_ref() == Some(metadata.binding())
            && pending_cwd_matches(self.terminal.command(), self.launch_cwd.as_ref())
            && pending_environment_matches(self.terminal.command(), &self.environment)
            && pending_launch_matches(self.terminal.command(), self.launch.as_ref())
            && pending_plugin_environment_matches(
                self.terminal.command(),
                self.plugin_environment.as_ref(),
            )
            && authorization.is_some_and(|authorization| {
                authorization.worker_id() == self.worker.worker_id
                    && authorization.worker_generation() == self.worker.generation
            })
    }

    #[cfg(test)]
    pub(crate) fn swap_workbench_token_for_test(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.workbench, &mut other.workbench);
    }

    #[cfg(test)]
    pub(crate) fn replace_reference_for_test(&mut self, reference: RequestId) {
        self.reference = reference;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PostcommitCoordinationError;

impl std::fmt::Display for PostcommitCoordinationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("committed terminal tab coordination failed")
    }
}

impl std::error::Error for PostcommitCoordinationError {}

impl SessionCoordinator {
    pub(crate) fn prepare_terminal_tab_create(
        &mut self,
        command: WorkbenchCommand,
        inputs: CreateInputs,
    ) -> PreparedTerminalTabCreate {
        self.prepare_terminal_creation(command, inputs, None)
    }

    /// The caller holds registry then Session gates and has validated the directory
    /// observation against this exact live Worker generation before entering.
    pub(crate) fn prepare_workspace_launch(
        &mut self,
        command: WorkbenchCommand,
        inputs: CreateInputs,
    ) -> PreparedTerminalTabCreate {
        let Some(Ok(worker_id)) = inputs.worker().map(|worker| {
            orchbee_workbench_protocol::WorkerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(worker.worker_id.as_bytes())
            ))
        }) else {
            return PreparedTerminalTabCreate::Ready(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            ));
        };
        self.prepare_terminal_creation(command, inputs, Some(worker_id))
    }

    fn prepare_terminal_creation(
        &mut self,
        command: WorkbenchCommand,
        inputs: CreateInputs,
        workspace_worker: Option<orchbee_workbench_protocol::WorkerId>,
    ) -> PreparedTerminalTabCreate {
        let (candidate, expiry, worker, capture) = inputs.into_parts();
        let capture = &capture;
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return PreparedTerminalTabCreate::Ready(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        };
        // The selection is part of the committed command's identity; it is
        // extracted before admission so the same value reaches the Worker
        // verbatim. A decoded selection already passed the closed lexical
        // contract, so conversion failure is an invariant break. A `profile`
        // selector resolves inside admission under the coordinator lock; the
        // frozen stored selection then substitutes for the explicit launch.
        let explicit_launch = match &command.action {
            WorkbenchAction::TabCreate { launch, .. }
            | WorkbenchAction::WorkspaceLaunch { launch, .. } => launch.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        };
        let launch = match explicit_launch
            .map(|selection| {
                orchbee_client_server_protocol::LaunchSelection::new(
                    selection.executable().to_owned(),
                    selection.arguments().to_vec(),
                )
            })
            .transpose()
        {
            Ok(launch) => launch,
            Err(_) => {
                return PreparedTerminalTabCreate::Ready(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                ));
            }
        };
        let options = match &command.action {
            WorkbenchAction::TabCreate { options, .. }
            | WorkbenchAction::WorkspaceLaunch { options, .. } => options.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => Default::default(),
        };
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(workbench.session_id().clone());
        let begin = if let Some(worker_id) = workspace_worker {
            workbench.begin_workspace_launch(
                Some(&grant.with_directory_association()),
                command,
                worker_id,
            )
        } else {
            workbench.begin_terminal_tab_create(Some(&grant), command)
        };
        let admission = match begin {
            WorkbenchTabCreateBegin::Ready(result) => {
                return PreparedTerminalTabCreate::Ready(result);
            }
            WorkbenchTabCreateBegin::Admitted(admission) => admission,
        };
        let plugin_use = admission.plugin_use().cloned();
        let (launch, profile_provenance) = match admission.profile_use() {
            Some(profile_use) => {
                let launch = match orchbee_client_server_protocol::LaunchSelection::new(
                    profile_use.launch.executable().to_owned(),
                    profile_use.launch.arguments().to_vec(),
                ) {
                    Ok(launch) => Some(launch),
                    Err(_) => {
                        return PreparedTerminalTabCreate::Ready(
                            admission.retire_composition_failure(),
                        );
                    }
                };
                let provenance = match orchbee_client_server_protocol::LaunchProfileProvenance::new(
                    profile_use.profile_id.as_str().to_owned(),
                    profile_use.catalog_revision.value(),
                ) {
                    Ok(provenance) => Some(provenance),
                    Err(_) => {
                        return PreparedTerminalTabCreate::Ready(
                            admission.retire_composition_failure(),
                        );
                    }
                };
                (launch, provenance)
            }
            None => (launch, None),
        };
        let launch = plugin_use
            .as_ref()
            .map(|resolved| resolved.launch.clone())
            .or(launch);
        let plugin_environment = plugin_use
            .as_ref()
            .map(|resolved| resolved.environment.clone());
        let (worker_constraint, workspace_cwd) = {
            let (worker_id, path) = admission.staged_metadata().workspace_launch();
            let Ok(worker_id) = orchbee_client_server_protocol::WorkerId::parse(worker_id.as_str())
            else {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            };
            let Ok(cwd) = WorkingDirectory::parse(path.as_str()) else {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            };
            (Some(WorkerConstraint::Exact { worker_id }), Some(cwd))
        };
        let launch_cwd = match options
            .cwd
            .as_ref()
            .map(|cwd| WorkingDirectory::parse(cwd.as_str()))
            .transpose()
        {
            Ok(cwd) => cwd
                .or_else(|| {
                    plugin_use
                        .as_ref()
                        .map(|resolved| resolved.directory.clone())
                })
                .or_else(|| workspace_cwd.clone()),
            Err(_) => {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            }
        };
        let environment = match orchbee_client_server_protocol::LaunchEnvironment::new(
            options.environment.entries().to_vec(),
        ) {
            Ok(environment) => environment,
            Err(_) => {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            }
        };
        let Some(worker) = worker else {
            return PreparedTerminalTabCreate::Ready(admission.runtime_unavailable(
                WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker,
            ));
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = &worker_constraint
            && !placement_matches_worker(worker, worker_id)
        {
            return PreparedTerminalTabCreate::Ready(admission.runtime_unavailable(
                WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker,
            ));
        }
        let (Some(reference), Some(expiry)) = (candidate, expiry) else {
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let Some(geometry) = client_geometry(admission.staged_metadata().geometry()) else {
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let request = match TerminalCreateRequest::new(
            reference.clone(),
            worker_constraint,
            launch_cwd.clone(),
            StartMode::Immediate { geometry },
            launch.clone(),
        )
        .with_environment(environment.clone())
        .with_plugin_environment(plugin_environment.clone())
        {
            Ok(request) => request,
            Err(_) => {
                return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
            }
        };
        let mut terminal = match terminals.plan_create(request, Some(worker), Some(expiry), capture)
        {
            Ok(plan) => plan,
            Err(result) => {
                return PreparedTerminalTabCreate::Ready(
                    match terminal_preflight_reason(&result) {
                        Ok(reason) => admission.runtime_unavailable(reason),
                        Err(reason) => admission.preflight_rejected(reason),
                    },
                );
            }
        };
        terminal.set_profile_provenance(profile_provenance);
        if let Some(selector) = &options.agent_start
            && super::agent_start::agent_start_command(selector)
                .and_then(|start| terminal.set_agent_start(start))
                .is_err()
        {
            drop(terminal);
            return PreparedTerminalTabCreate::Ready(admission.retire_composition_failure());
        }
        let authorization = terminal.authorization();
        let Some(binding) = presentation_binding(authorization) else {
            drop(terminal);
            return PreparedTerminalTabCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy),
            );
        };
        let workbench_plan =
            match admission.finish(binding.clone(), terminal.launch_program().cloned()) {
                WorkbenchTabCreatePreparation::Ready(result) => {
                    return PreparedTerminalTabCreate::Ready(result);
                }
                WorkbenchTabCreatePreparation::Prepared(plan) => plan,
            };
        let metadata = workbench_plan.metadata().clone();
        if terminal.create_reference() != &reference
            || terminal.authorization() != authorization
            || pending_authorization_from_message(terminal.command()) != Some(authorization)
            || !pending_cwd_matches(terminal.command(), launch_cwd.as_ref())
            || !pending_environment_matches(terminal.command(), &environment)
            || !pending_launch_matches(terminal.command(), launch.as_ref())
            || !pending_plugin_environment_matches(terminal.command(), plugin_environment.as_ref())
            || metadata.binding() != &binding
            || authorization.worker_id() != worker.worker_id
            || authorization.worker_generation() != worker.generation
            || workspace_cwd.is_none()
        {
            drop(workbench_plan);
            drop(terminal);
            workbench.retire_terminal_membership();
            return PreparedTerminalTabCreate::Ready(unavailable_by_id(
                metadata.request_id().clone(),
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        }

        // Both borrowed plans are complete. Publication below contains only infallible owner moves.
        let terminal = terminal.commit();
        let workbench = workbench_plan.commit().into_pending();
        if let Some(agent) = options.agent_start {
            self.record_agent_start(agent);
        }
        PreparedTerminalTabCreate::Dispatch(PendingTerminalTabCreate {
            workbench,
            terminal,
            reference,
            worker,
            workspace_cwd,
            launch_cwd,
            environment,
            launch,
            plugin_environment,
            agent_start: options.agent_start,
        })
    }

    pub(crate) fn complete_terminal_tab_create(
        &mut self,
        pending: PendingTerminalTabCreate,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<CompletedTerminalTabCreate, PostcommitCoordinationError> {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return Err(PostcommitCoordinationError);
        };
        if !pending.association_matches() {
            return Err(PostcommitCoordinationError);
        }
        let metadata = pending.workbench.metadata().clone();
        let PendingTerminalTabCreate {
            workbench: workbench_pending,
            terminal,
            reference,
            worker: _,
            workspace_cwd: _,
            launch_cwd: _,
            environment: _,
            launch,
            plugin_environment: _,
            agent_start,
        } = pending;
        let agent_start = super::agent_start::launch_of(
            agent_start,
            terminal.command(),
            metadata.presentation_id(),
            metadata.binding(),
        );
        let terminal = terminals
            .prepare_completion(terminal, completion, current_worker)
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        let terminal_outcome = terminal_outcome(&reference, metadata.geometry(), terminal.result())
            .ok_or(PostcommitCoordinationError)?;
        let settlement = workbench
            .prepare_terminal_tab_settlement(workbench_pending, terminal_outcome.clone())
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        let agent_start = agent_start
            .filter(|_| matches!(terminal_outcome, WorkbenchTerminalOutcome::Started { .. }));

        terminal.commit();
        let result = settlement.commit();
        record_launch_failure(
            workbench,
            &terminal_outcome,
            metadata.presentation_id(),
            metadata.binding(),
            launch.as_ref(),
        );
        Ok(CompletedTerminalTabCreate {
            result,
            agent_start,
        })
    }
}

/// Publishes a definite launch failure on the created Presentation; other outcomes publish nothing.
pub(super) fn record_launch_failure(
    workbench: &mut WorkbenchAuthority,
    outcome: &WorkbenchTerminalOutcome,
    presentation_id: &TerminalPresentationId,
    binding: &PresentationBinding,
    launch: Option<&orchbee_client_server_protocol::LaunchSelection>,
) {
    if matches!(outcome, WorkbenchTerminalOutcome::Failed { .. }) {
        workbench.record_terminal_launch_failure(
            presentation_id,
            binding,
            launch.map(orchbee_client_server_protocol::LaunchSelection::executable),
        );
    }
}

pub(super) fn pending_cwd_matches(
    message: &CapabilityNodeMessage,
    expected: Option<&WorkingDirectory>,
) -> bool {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = message else {
        return false;
    };
    command.cwd().map(|cwd| cwd.as_str()) == expected.map(WorkingDirectory::as_str)
}

pub(super) fn pending_launch_matches(
    message: &CapabilityNodeMessage,
    expected: Option<&orchbee_client_server_protocol::LaunchSelection>,
) -> bool {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = message else {
        return false;
    };
    command
        .launch()
        .map(|launch| (launch.executable(), launch.arguments()))
        == expected.map(|selection| (selection.executable(), selection.arguments()))
}

pub(super) fn placement_matches_worker(
    placement: WorkerPlacement,
    expected: &orchbee_client_server_protocol::WorkerId,
) -> bool {
    expected.as_str()
        == format!(
            "{:032x}",
            u128::from_be_bytes(placement.worker_id.as_bytes())
        )
}

pub(super) fn pending_authorization(
    pending: &PendingOperation,
) -> Option<TerminalOperationAuthorization> {
    pending_authorization_from_message(pending.command())
}

pub(super) fn pending_authorization_from_message(
    message: &CapabilityNodeMessage,
) -> Option<TerminalOperationAuthorization> {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = message else {
        return None;
    };
    Some(command.authorization())
}

pub(super) fn terminal_preflight_reason(
    result: &ServerMessage,
) -> Result<WorkbenchTabCreateRuntimeUnavailableReason, WorkbenchRejectionReason> {
    if matches!(result, ServerMessage::TerminalCreateResult(result) if result.outcome() == CreateOutcome::Rejected { reason: CreateRejectedReason::LaunchRequestTooLarge })
    {
        return Err(WorkbenchRejectionReason::LaunchRequestTooLarge);
    }
    if matches!(
        result,
        ServerMessage::TerminalCreateResult(result)
            if result.outcome() == CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::NoCompatibleWorker,
            }
    ) {
        Ok(WorkbenchTabCreateRuntimeUnavailableReason::NoCompatibleWorker)
    } else {
        Ok(WorkbenchTabCreateRuntimeUnavailableReason::ServerBusy)
    }
}

pub(super) fn terminal_outcome(
    reference: &RequestId,
    expected_geometry: WorkbenchTerminalGeometry,
    result: &ServerMessage,
) -> Option<WorkbenchTerminalOutcome> {
    let ServerMessage::TerminalCreateResult(result) = result else {
        return None;
    };
    if result.request_id() != reference {
        return None;
    }
    Some(match result.outcome() {
        CreateOutcome::Accepted {
            state: CreateAcceptedState::Started { geometry },
        } => {
            let geometry = workbench_geometry(geometry)?;
            if geometry != expected_geometry {
                return None;
            }
            WorkbenchTerminalOutcome::Started { geometry }
        }
        CreateOutcome::Unavailable { reason } => WorkbenchTerminalOutcome::Unavailable {
            reason: match reason {
                CreateUnavailableReason::ServerBusy => {
                    WorkbenchTerminalUnavailableReason::ServerBusy
                }
                CreateUnavailableReason::NoCompatibleWorker
                | CreateUnavailableReason::TerminalRuntimeUnavailable => {
                    WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable
                }
            },
        },
        CreateOutcome::Failed { .. } => WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        },
        CreateOutcome::OutcomeUnknown => WorkbenchTerminalOutcome::OutcomeUnknown {},
        CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        }
        | CreateOutcome::Rejected { .. } => return None,
    })
}

pub(super) fn client_geometry(geometry: WorkbenchTerminalGeometry) -> Option<Geometry> {
    Geometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}

fn workbench_geometry(geometry: Geometry) -> Option<WorkbenchTerminalGeometry> {
    WorkbenchTerminalGeometry::new(
        geometry.columns(),
        geometry.rows(),
        geometry.cell_pixel_width(),
        geometry.cell_pixel_height(),
    )
    .ok()
}

fn unavailable(
    command: &WorkbenchCommand,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    unavailable_by_id(command.request_id.clone(), reason)
}

pub(super) fn unavailable_by_id(
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id,
        outcome: WorkbenchCommandOutcome::Unavailable { reason },
    }
}

/// Frozen per-launch environment must survive every owner conversion unchanged.
pub(super) fn pending_environment_matches(
    command: &CapabilityNodeMessage,
    environment: &orchbee_client_server_protocol::LaunchEnvironment,
) -> bool {
    match command {
        CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) => {
            launch.environment().entries() == environment.entries()
        }
        // This coordinator path handles only authorized Create/Start operations; other OBCN families use their dedicated paths.
        _ => false,
    }
}

/// Protected display metadata is part of the same frozen execution intent.
pub(super) fn pending_plugin_environment_matches(
    command: &CapabilityNodeMessage,
    environment: Option<&orchbee_client_server_protocol::PluginTerminalEnvironment>,
) -> bool {
    match command {
        CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) => {
            launch.plugin_environment().map(|value| value.entries())
                == environment.map(|value| value.entries())
        }
        // This coordinator path handles only authorized Create/Start operations; other OBCN families use their dedicated paths.
        _ => false,
    }
}
