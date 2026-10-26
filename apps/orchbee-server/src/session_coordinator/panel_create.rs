use super::tab_create::{
    client_geometry, pending_authorization, pending_authorization_from_message,
    pending_cwd_matches, pending_environment_matches, pending_launch_matches,
    pending_plugin_environment_matches, placement_matches_worker, record_launch_failure,
    terminal_outcome, terminal_preflight_reason, unavailable_by_id,
};
use super::{PostcommitCoordinationError, SessionCoordinator, presentation_binding};
use crate::terminal_coordinator::{Completion, PendingOperation, WorkerPlacement};
use orchbee_capability_node_protocol::{CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds};
use orchbee_client_server_protocol::{
    RequestId, StartMode, TerminalCreateRequest, WorkerConstraint, WorkingDirectory,
};
use orchbee_server::{
    PendingWorkbenchPanelCreate, WorkbenchGrant, WorkbenchPanelCreateBegin,
    WorkbenchPanelCreatePreparation, WorkbenchPanelCreateSettlementTokenMismatch,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason,
};

pub(crate) enum PreparedTerminalPanelCreate {
    Ready(WorkbenchCommandResult),
    Dispatch(PendingTerminalPanelCreate),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompletedTerminalPanelCreate {
    pub(crate) result: WorkbenchCommandResult,
}

pub(crate) struct PendingTerminalPanelCreate {
    workbench: PendingWorkbenchPanelCreate,
    terminal: Box<PendingOperation>,
    reference: RequestId,
    worker: WorkerPlacement,
    launch_cwd: Option<WorkingDirectory>,
    environment: Box<orchbee_client_server_protocol::LaunchEnvironment>,
    launch: Option<orchbee_client_server_protocol::LaunchSelection>,
    plugin_environment: Option<orchbee_client_server_protocol::PluginTerminalEnvironment>,
}

impl PendingTerminalPanelCreate {
    pub(crate) fn command(&self) -> &CapabilityNodeMessage {
        self.terminal.command()
    }

    fn association_matches(&self) -> bool {
        let metadata = self.workbench.metadata();
        let authorization = pending_authorization(&self.terminal);
        self.terminal.create_reference() == &self.reference
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
}

impl SessionCoordinator {
    pub(crate) fn prepare_terminal_panel_create(
        &mut self,
        command: WorkbenchCommand,
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        worker: Option<WorkerPlacement>,
    ) -> PreparedTerminalPanelCreate {
        let Self {
            terminals,
            workbench,
            ..
        } = self;
        let Some(workbench) = workbench.as_mut() else {
            return PreparedTerminalPanelCreate::Ready(unavailable(
                &command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        };
        // The selection is part of the committed command's identity; it is
        // extracted before admission so the same value reaches the Worker
        // verbatim. A decoded selection already passed the closed lexical
        // contract, so conversion failure is an invariant break.
        let launch = match &command.action {
            WorkbenchAction::PanelSplit { launch, .. }
            | WorkbenchAction::PanelCreateTerminal { launch, .. } => launch.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        };
        let launch = match launch
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
                return PreparedTerminalPanelCreate::Ready(unavailable(
                    &command,
                    WorkbenchCommandUnavailableReason::ServiceUnavailable,
                ));
            }
        };
        let options = match &command.action {
            WorkbenchAction::PanelSplit { options, .. }
            | WorkbenchAction::PanelCreateTerminal { options, .. } => options.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => Default::default(),
        };
        let grant =
            WorkbenchGrant::manage_navigation_and_terminal_create(workbench.session_id().clone());
        let admission = match workbench.begin_terminal_panel_create(Some(&grant), command) {
            WorkbenchPanelCreateBegin::Ready(result) => {
                return PreparedTerminalPanelCreate::Ready(result);
            }
            WorkbenchPanelCreateBegin::Admitted(admission) => admission,
        };
        // A `profile` selector resolved inside admission under the coordinator
        // lock; its frozen stored selection substitutes for the explicit
        // launch and its provenance rides alongside, immutable across the gap.
        let plugin_use = admission.plugin_use().cloned();
        let (launch, profile_provenance) = match admission.profile_use() {
            Some(profile_use) => {
                let launch = match orchbee_client_server_protocol::LaunchSelection::new(
                    profile_use.launch.executable().to_owned(),
                    profile_use.launch.arguments().to_vec(),
                ) {
                    Ok(launch) => Some(launch),
                    Err(_) => {
                        return PreparedTerminalPanelCreate::Ready(
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
                        return PreparedTerminalPanelCreate::Ready(
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
                return PreparedTerminalPanelCreate::Ready(admission.retire_composition_failure());
            };
            let Ok(cwd) = WorkingDirectory::parse(path.as_str()) else {
                return PreparedTerminalPanelCreate::Ready(admission.retire_composition_failure());
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
                return PreparedTerminalPanelCreate::Ready(admission.retire_composition_failure());
            }
        };
        let environment = match orchbee_client_server_protocol::LaunchEnvironment::new(
            options.environment.entries().to_vec(),
        ) {
            Ok(environment) => environment,
            Err(_) => {
                return PreparedTerminalPanelCreate::Ready(admission.retire_composition_failure());
            }
        };
        let Some(worker) = worker else {
            return PreparedTerminalPanelCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchCommandUnavailableReason::NoCompatibleWorker),
            );
        };
        if let Some(WorkerConstraint::Exact { worker_id }) = &worker_constraint
            && !placement_matches_worker(worker, worker_id)
        {
            return PreparedTerminalPanelCreate::Ready(
                admission
                    .runtime_unavailable(WorkbenchCommandUnavailableReason::NoCompatibleWorker),
            );
        }
        let (Some(reference), Some(expiry)) = (candidate, expiry) else {
            return PreparedTerminalPanelCreate::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
            );
        };
        let Some(geometry) = client_geometry(admission.staged_metadata().geometry()) else {
            return PreparedTerminalPanelCreate::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
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
                return PreparedTerminalPanelCreate::Ready(admission.retire_composition_failure());
            }
        };
        let mut terminal = match terminals.plan_create(request, Some(worker), Some(expiry)) {
            Ok(plan) => plan,
            Err(result) => {
                let reason = terminal_preflight_reason(&result);
                return PreparedTerminalPanelCreate::Ready(
                    admission.runtime_unavailable(reason.into()),
                );
            }
        };
        terminal.set_profile_provenance(profile_provenance);
        let authorization = terminal.authorization();
        let Some(binding) = presentation_binding(authorization) else {
            drop(terminal);
            return PreparedTerminalPanelCreate::Ready(
                admission.runtime_unavailable(WorkbenchCommandUnavailableReason::ServerBusy),
            );
        };
        let workbench_plan = match admission.finish(binding.clone()) {
            WorkbenchPanelCreatePreparation::Ready(result) => {
                return PreparedTerminalPanelCreate::Ready(result);
            }
            WorkbenchPanelCreatePreparation::Prepared(plan) => plan,
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
            return PreparedTerminalPanelCreate::Ready(unavailable_by_id(
                metadata.request_id().clone(),
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            ));
        }
        // Every fallible association check precedes the two infallible owner moves.
        let terminal = terminal.commit();
        let pending_workbench = workbench_plan.commit_into_pending();
        PreparedTerminalPanelCreate::Dispatch(PendingTerminalPanelCreate {
            workbench: pending_workbench,
            terminal,
            reference,
            worker,
            launch_cwd,
            environment: Box::new(environment),
            launch,
            plugin_environment,
        })
    }

    pub(crate) fn complete_terminal_panel_create(
        &mut self,
        pending: PendingTerminalPanelCreate,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<CompletedTerminalPanelCreate, PostcommitCoordinationError> {
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
        let PendingTerminalPanelCreate {
            workbench: workbench_pending,
            terminal,
            reference,
            worker: _,
            launch_cwd: _,
            environment: _,
            launch,
            plugin_environment: _,
        } = pending;
        let terminal = terminals
            .prepare_completion(terminal, completion, current_worker)
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        let terminal_outcome = terminal_outcome(&reference, metadata.geometry(), terminal.result())
            .ok_or(PostcommitCoordinationError)?;
        let settlement = workbench
            .prepare_terminal_panel_create_settlement(workbench_pending, terminal_outcome.clone())
            .map_err(|_mismatch: WorkbenchPanelCreateSettlementTokenMismatch| {
                PostcommitCoordinationError
            })?;
        terminal.commit();
        let result = settlement.commit();
        record_launch_failure(
            workbench,
            &terminal_outcome,
            metadata.presentation_id(),
            metadata.binding(),
            launch.as_ref(),
        );
        Ok(CompletedTerminalPanelCreate { result })
    }
}

fn unavailable(
    command: &WorkbenchCommand,
    reason: WorkbenchCommandUnavailableReason,
) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome: WorkbenchCommandOutcome::Unavailable { reason },
    }
}
