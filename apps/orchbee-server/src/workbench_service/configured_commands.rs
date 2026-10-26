//! Authenticated command admission coordinates existing execution and Client owners.
use super::{WorkbenchClientConnection, WorkbenchExecution};
use crate::local_terminal_service::configured_commands::descriptor;
use orchbee_local_runtime::{CustomCommandAction, CustomCommandEntry};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControl, ClientViewIntent,
    ClientViewOutcome, ClientViewPhase, ConfiguredCommandAction as Action,
    ConfiguredCommandFailure as Failure, ConfiguredCommandInvocation,
    ConfiguredCommandOutcome as Outcome, ConfiguredCommandPopupSpec, ConfiguredCommandRequest,
    ConfiguredCommandResult, ProtocolVersion, WorkbenchClientViewRequest, WorkbenchRequest,
    WorkbenchResponse,
};
use std::time::Instant;

impl WorkbenchClientConnection<'_> {
    pub(super) fn configured_command(
        &self,
        request: ConfiguredCommandRequest,
        owner: crate::local_terminal_service::PopupOwner,
    ) -> ConfiguredCommandResult {
        let outcome = self
            .configured_outcome(&request, owner)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        ConfiguredCommandResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    fn configured_outcome(
        &self,
        request: &ConfiguredCommandRequest,
        owner: crate::local_terminal_service::PopupOwner,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidIntent)?;
        if !self.service.policy.read {
            return Err(Failure::Denied);
        }
        if self
            .service
            .terminals
            .workbench_session_matches(&request.session_id)
            != Some(true)
        {
            return Err(Failure::SessionMismatch);
        }
        match &request.action {
            Action::List {} => {
                let (revision, entries) = self.service.terminals.configured_commands()?;
                Ok(Outcome::Catalog {
                    revision,
                    entries: entries.iter().map(descriptor).collect(),
                })
            }
            Action::Invoke { invocation } => {
                let command = self
                    .service
                    .terminals
                    .resolve_configured_command(&invocation.command_id)?;
                if !(self.service.policy.create_terminal
                    && self.service.policy.manage_terminal_lifecycle)
                    || matches!(
                        command.action,
                        CustomCommandAction::Pane | CustomCommandAction::Popup
                    ) && !self.service.policy.manage_navigation
                {
                    return Err(Failure::Denied);
                }
                self.check_command_client(invocation)?;
                let prepared = self
                    .service
                    .terminals
                    .prepare_configured_command(invocation)?;
                self.check_command_client(invocation)?;
                if prepared.command.action == CustomCommandAction::Popup {
                    let source = invocation.source.as_ref().ok_or(Failure::StaleSource)?;
                    let entry = descriptor(&CustomCommandEntry {
                        id: invocation.command_id.clone(),
                        definition: prepared.command,
                    });
                    let spec = ConfiguredCommandPopupSpec {
                        command_id: invocation.command_id.clone(),
                        source: source.presentation.clone(),
                        context: Box::new(prepared.context),
                        width: entry.width,
                        height: entry.height,
                    };
                    let command = ClientViewControl {
                        operation_id: request.request_id.clone(),
                        client_id: source.client_id.clone(),
                        expected_view_revision: source.view_revision,
                        minimum_workbench_revision: invocation.expected_revision,
                        intent: ClientViewIntent::CommandPopupOpen {
                            spec: Box::new(spec),
                        },
                    };
                    let nested = WorkbenchClientViewRequest {
                        protocol_version: ProtocolVersion,
                        request_id: request.request_id.clone(),
                        session_id: request.session_id.clone(),
                        action: ClientViewAction::Submit {
                            command: Box::new(command),
                        },
                    };
                    match self.execute(WorkbenchRequest::ClientView(nested)) {
                        WorkbenchExecution::Response(WorkbenchResponse::ClientView(result)) => {
                            Ok(Outcome::PopupControl {
                                command_id: invocation.command_id.clone(),
                                result: Box::new(result),
                            })
                        }
                        // This exchange accepts only the result family selected by its submitted request.
                        _ => Err(Failure::PhysicalUnknown),
                    }
                } else {
                    self.service
                        .terminals
                        .dispatch_configured_command(request, invocation, prepared)
                }
            }
            Action::PopupOpen { .. } | Action::PopupRead { .. } | Action::PopupClose { .. } => {
                if matches!(request.action, Action::PopupOpen { .. })
                    && !(self.service.policy.create_terminal
                        && self.service.policy.manage_terminal_lifecycle
                        && self.service.policy.manage_navigation)
                    || matches!(request.action, Action::PopupClose { .. })
                        && !self.service.policy.manage_terminal_lifecycle
                {
                    return Err(Failure::Denied);
                }
                Ok(self
                    .service
                    .terminals
                    .execute_configured_popup(owner, request.clone())
                    .outcome)
            }
        }
    }
    fn check_command_client(
        &self,
        invocation: &ConfiguredCommandInvocation,
    ) -> Result<(), Failure> {
        let Some(source) = &invocation.source else {
            return Ok(());
        };
        let mut clients = self
            .service
            .client_views
            .try_lock()
            .map_err(|_| Failure::Busy)?;
        let outcome = clients.execute(
            self.generation,
            ClientViewAction::Get {
                client_id: source.client_id.clone(),
            },
            Instant::now(),
        );
        match outcome {
            ClientViewOutcome::Found { client }
                if client.availability == ClientViewAvailability::Current
                    && client.view.phase == ClientViewPhase::Ready
                    && client.view_revision == source.view_revision
                    && client.view.workbench_revision == invocation.expected_revision
                    && client.view.selection.as_ref().is_some_and(|selected| {
                        selected.terminal_context_id == source.presentation.terminal_context_id
                            && selected.tab_id.as_ref() == Some(&source.presentation.tab_id)
                            && selected.panel_id.as_ref() == Some(&source.presentation.panel_id)
                    }) =>
            {
                Ok(())
            }
            // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
            _ => Err(Failure::StaleSource),
        }
    }
}
