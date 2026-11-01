//! Coordinates source association, Workspace attachment, exclusive checkout admission, and
//! retained Workspace close progress for the Worktree driver. It does not execute Git or
//! decide physical terminal cleanup from layout state alone.

use super::{Driver, identity};
use crate::local_terminal_service::{create_reference, lease_expiry, placement};
use crate::terminal_coordinator::CreateInputs;
use orchbee_server::{WorkbenchGrant, WorkbenchReadOrNavigationRequest};
use orchbee_workbench_protocol::{
    LaunchOptions, ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchLabel, WorkbenchRequest, WorkbenchResponse, WorkbenchTabCloseStatus,
    WorkbenchTerminalOutcome, WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseResume, WorkspacePlacement, WorktreeIntent,
    WorktreeOperationFailure as Failure, WorktreeOperationStatus as Status,
    WorktreeWorkspaceDisposition,
};
use std::{
    thread,
    time::{Duration, Instant},
};
impl Driver<'_> {
    pub(super) fn associate_source(
        &self,
        checkout: orchbee_workbench_protocol::WorktreeCheckoutFacts,
    ) -> Result<(), Failure> {
        let workers = self
            .service
            .workers
            .lock()
            .map_err(|_| Failure::Internal {})?;
        let handle = workers
            .request_handle()
            .ok_or(Failure::WorkerUnavailable {})?;
        if handle.generation() != self.plan.worker_generation
            || format!(
                "{:032x}",
                u128::from_be_bytes(handle.worker_id().as_bytes())
            ) != self.plan.source.worker_id.as_str()
        {
            return Err(Failure::SourceChanged {});
        }
        let mut coordinator = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?;
        if !coordinator
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .associate_worktree_source(self.plan, checkout)
        {
            return Err(Failure::SourceChanged {});
        }
        Ok(())
    }

    pub(super) fn recheck_source(&self) -> Result<(), Failure> {
        let _handle = self.worker()?;
        let mut coordinator = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?;
        if !coordinator
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .worktree_source_matches(self.plan)
        {
            return Err(Failure::SourceChanged {});
        }
        Ok(())
    }
    pub(super) fn attach(&mut self) -> Result<Status, Failure> {
        let checkout = self
            .plan
            .progress
            .checkout
            .clone()
            .ok_or(Failure::Internal {})?;
        let (label, geometry) = match &self.plan.intent {
            WorktreeIntent::Create {
                label, geometry, ..
            }
            | WorktreeIntent::Open {
                label, geometry, ..
            } => (label.clone(), *geometry),
            WorktreeIntent::Remove { .. } => return Err(Failure::Internal {}),
        };
        let request_id = identity()?;
        let candidate = create_reference();
        let expiry = lease_expiry();
        let capture = self.service.launch_capture();
        let (prepared, handle, workspace_id) = {
            let workers = self
                .service
                .workers
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let handle = workers
                .request_handle()
                .ok_or(Failure::WorkerUnavailable {})?;
            let inputs = CreateInputs::admitted_by(candidate, expiry, Some(&handle), capture);
            if inputs.worker().is_none() {
                return Err(Failure::WorkerUnavailable {});
            }
            if handle.generation() != self.plan.worker_generation
                || format!(
                    "{:032x}",
                    u128::from_be_bytes(handle.worker_id().as_bytes())
                ) != self.plan.source.worker_id.as_str()
            {
                return Err(Failure::WorkerUnavailable {});
            }
            let mut coordinator = self
                .service
                .coordinator
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let authority = coordinator
                .worktree_authority()
                .ok_or(Failure::Internal {})?;
            if !authority.worktree_source_matches(self.plan) {
                return Err(Failure::SourceChanged {});
            }
            let existing =
                authority.checkout_workspaces(&checkout.path, &self.plan.source.worker_id);
            if let [workspace_id] = existing.as_slice() {
                if !authority.worktree_workspace_available(workspace_id) {
                    return Err(Failure::CheckoutInUse {});
                }
                if let Some(label) = label {
                    let command = WorkbenchCommand {
                        protocol_version: ProtocolVersion,
                        request_id,
                        session_id: authority.session_id().clone(),
                        expected_revision: authority.snapshot().revision,
                        action: WorkbenchAction::WorkspaceRename {
                            workspace_id: workspace_id.clone(),
                            label,
                        },
                    };
                    let request = WorkbenchReadOrNavigationRequest::try_from(
                        WorkbenchRequest::Command(command),
                    )
                    .map_err(|_| Failure::Internal {})?;
                    let grant = WorkbenchGrant::manage_navigation(authority.session_id().clone());
                    if !matches!(authority.execute(Some(&grant), request), WorkbenchResponse::Command(result)
                        if matches!(result.outcome, WorkbenchCommandOutcome::Renamed { .. } | WorkbenchCommandOutcome::Unchanged { .. }))
                    {
                        return Err(Failure::AssociationFailed {});
                    }
                }
                if !authority.associate_worktree(self.plan, workspace_id, checkout.clone()) {
                    return Err(Failure::AssociationFailed {});
                }
                return Ok(Status::Opened {
                    checkout,
                    workspace_id: workspace_id.clone(),
                    reused: true,
                    terminal_outcome: None,
                });
            }
            if !existing.is_empty() {
                return Err(Failure::CheckoutInUse {});
            }
            let label = label
                .or_else(|| {
                    checkout
                        .branch
                        .as_ref()
                        .and_then(|branch| WorkbenchLabel::parse(branch.as_str()).ok())
                })
                .unwrap_or_else(|| WorkbenchLabel::parse("Worktree").expect("fixed label"));
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: authority.session_id().clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::WorkspaceLaunch {
                    automatic_label: false,
                    placement: match &self.plan.source_workspace_id {
                        Some(workspace_id) => WorkspacePlacement::Alongside {
                            workspace_id: workspace_id.clone(),
                            project_label: project_label(&checkout),
                        },
                        // A Directory source groups only into an explicitly requested
                        // Project; without one the new Workspace stays ungrouped.
                        None => match &self.plan.project_id {
                            Some(project_id) => WorkspacePlacement::Project {
                                project_id: project_id.clone(),
                            },
                            None => WorkspacePlacement::Ungrouped {},
                        },
                    },
                    label,
                    geometry,
                    launch: None,
                    profile: None,
                    options: Box::new(LaunchOptions {
                        plugin_pane: None,
                        cwd: Some(checkout.path.clone()),
                        ..LaunchOptions::default()
                    }),
                },
            };
            let prepared = coordinator.prepare_workspace_launch(command, inputs);
            let authority = coordinator
                .worktree_authority()
                .ok_or(Failure::Internal {})?;
            let committed =
                authority.checkout_workspaces(&checkout.path, &self.plan.source.worker_id);
            let [workspace_id] = committed.as_slice() else {
                return Err(Failure::AssociationFailed {});
            };
            if !authority.associate_worktree(self.plan, workspace_id, checkout.clone()) {
                return Err(Failure::AssociationFailed {});
            }
            self.plan.progress.workspace = WorktreeWorkspaceDisposition::Open {
                workspace_id: workspace_id.clone(),
            };
            if !authority.publish_worktree_operation(
                self.plan,
                Status::Pending {
                    phase: self.plan.phase,
                    progress: self.plan.progress.clone(),
                },
            ) {
                return Err(Failure::Internal {});
            }
            (prepared, handle, workspace_id.clone())
        };
        // Workspace identity/association is committed before terminal launch crosses IPC.
        let terminal_outcome = match self
            .service
            .dispatch_terminal_tab_create(prepared, Some(handle))
        {
            Ok(result) => match result.outcome {
                WorkbenchCommandOutcome::WorkspaceLaunched {
                    terminal_outcome, ..
                } => terminal_outcome,
                // This step consumes its specific mutation receipt; other command results cannot confirm the requested postcondition.
                _ => WorkbenchTerminalOutcome::OutcomeUnknown {},
            },
            Err(_) => WorkbenchTerminalOutcome::OutcomeUnknown {},
        };
        Ok(Status::Opened {
            checkout,
            workspace_id,
            reused: false,
            terminal_outcome: Some(terminal_outcome),
        })
    }
    pub(super) fn require_exclusive_checkout(&self) -> Result<(), Failure> {
        let _worker = self.worker()?;
        let checkout = self
            .plan
            .progress
            .checkout
            .as_ref()
            .ok_or(Failure::Internal {})?;
        let mut coordinator = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?;
        let authority = coordinator
            .worktree_authority()
            .ok_or(Failure::Internal {})?;
        let current = authority.checkout_workspaces(&checkout.path, &self.plan.source.worker_id);
        let closed = matches!(
            self.plan.progress.workspace,
            WorktreeWorkspaceDisposition::Closed { .. }
        );
        let WorktreeIntent::Remove { workspace_id, .. } = &self.plan.intent else {
            return Err(Failure::Internal {});
        };
        if current
            .iter()
            .any(|workspace| closed || workspace != workspace_id)
        {
            return Err(Failure::CheckoutInUse {});
        }
        if let Some(previous) = authority.worktree_checkout(workspace_id)
            && previous.common_directory != checkout.common_directory
        {
            return Err(Failure::SourceChanged {});
        }
        Ok(())
    }
    pub(super) fn begin_close(&mut self) -> Result<(), Failure> {
        self.require_exclusive_checkout()?;
        let WorktreeIntent::Remove { workspace_id, .. } = &self.plan.intent else {
            return Err(Failure::Internal {});
        };
        let request_id = identity()?;
        let result = {
            let workers = self
                .service
                .workers
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let handle = workers
                .request_handle()
                .ok_or(Failure::WorkerUnavailable {})?;
            if handle.generation() != self.plan.worker_generation {
                return Err(Failure::WorkerUnavailable {});
            }
            let mut coordinator = self
                .service
                .coordinator
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let authority = coordinator
                .worktree_authority()
                .ok_or(Failure::Internal {})?;
            if !authority.worktree_source_matches(self.plan) {
                return Err(Failure::SourceChanged {});
            }
            let command = WorkbenchCommand {
                protocol_version: ProtocolVersion,
                request_id,
                session_id: authority.session_id().clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::WorkspaceClose {
                    workspace_id: workspace_id.clone(),
                },
            };
            let result = coordinator
                .execute_workspace_close(
                    command,
                    placement(Some(&handle)),
                    self.service.close_maintenance.available(),
                    Instant::now(),
                )
                .ok_or(Failure::CloseIncomplete {})?;
            if let WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                close_request_id, ..
            } = &result.outcome
                && !coordinator
                    .worktree_authority()
                    .ok_or(Failure::Internal {})?
                    .hold_worktree_close(self.plan, close_request_id)
            {
                return Err(Failure::Internal {});
            }
            result
        };
        self.service.close_maintenance.notify();
        let WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            close_request_id, ..
        } = result.outcome
        else {
            return Err(Failure::CloseIncomplete {});
        };
        self.plan.progress.workspace = WorktreeWorkspaceDisposition::Closing {
            workspace_id: workspace_id.clone(),
            close_request_id,
        };
        Ok(())
    }
    pub(super) fn wait_close(&mut self, quiesce: bool) -> Result<(), Failure> {
        if matches!(
            self.plan.progress.workspace,
            WorktreeWorkspaceDisposition::Closed { .. }
        ) {
            return Ok(());
        }
        let WorktreeWorkspaceDisposition::Closing {
            workspace_id,
            close_request_id,
        } = self.plan.progress.workspace.clone()
        else {
            return Err(Failure::Internal {});
        };
        let session_id = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .session_id()
            .clone();
        let mut may_resume = self.plan.attempt > 1;
        loop {
            self.check()?;
            let result = self
                .service
                .read_workspace_close(WorkbenchWorkspaceCloseRead {
                    protocol_version: ProtocolVersion,
                    request_id: identity()?,
                    session_id: session_id.clone(),
                    workspace_id: workspace_id.clone(),
                })
                .ok_or(Failure::CloseIncomplete {})?;
            let WorkbenchWorkspaceCloseReadOutcome::Available {
                close_request_id: observed,
                attempt_generation,
                status,
                ..
            } = result.outcome
            else {
                return Err(Failure::CloseIncomplete {});
            };
            if observed != close_request_id {
                return Err(Failure::CloseIncomplete {});
            }
            if quiesce
                && self
                    .service
                    .coordinator
                    .lock()
                    .map_err(|_| Failure::Internal {})?
                    .worktree_authority()
                    .ok_or(Failure::Internal {})?
                    .worktree_close_quiesced(self.plan, &close_request_id)
            {
                return Ok(());
            }
            match status {
                WorkbenchTabCloseStatus::Closed { .. } => {
                    self.plan.progress.workspace = WorktreeWorkspaceDisposition::Closed {
                        workspace_id,
                        close_request_id,
                    };
                    return Ok(());
                }
                // Only an active Close is progress; a parked one waits for resume.
                WorkbenchTabCloseStatus::Closing { .. }
                | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                    reason:
                        orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::Preparing
                        | orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::TargetCleanupPending,
                    resumable: false,
                } => thread::sleep(Duration::from_millis(50)),
                WorkbenchTabCloseStatus::CleanupBlocked {
                    resumable: true, ..
                }
                | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                    resumable: true, ..
                } if may_resume => {
                    may_resume = false;
                    let _result =
                        self.service
                            .resume_workspace_close(WorkbenchWorkspaceCloseResume {
                                protocol_version: ProtocolVersion,
                                request_id: identity()?,
                                session_id: session_id.clone(),
                                workspace_id: workspace_id.clone(),
                                close_request_id: close_request_id.clone(),
                                expected_attempt_generation: attempt_generation,
                            });
                }
                // This Close continuation requires the corresponding published receipt; other child states cannot prove final disposal.
                _ => return Err(Failure::CloseIncomplete {}),
            }
        }
    }
}

fn project_label(checkout: &orchbee_workbench_protocol::WorktreeCheckoutFacts) -> WorkbenchLabel {
    std::path::Path::new(checkout.repository_root.as_str())
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| WorkbenchLabel::parse(name).ok())
        .unwrap_or_else(|| WorkbenchLabel::parse("Git").expect("fixed repository label"))
}
