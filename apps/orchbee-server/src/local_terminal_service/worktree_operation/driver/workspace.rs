use super::{Driver, identity};
use crate::local_terminal_service::{create_reference, lease_expiry, placement};
use orchbee_server::{WorkbenchGrant, WorkbenchReadOrNavigationRequest};
use orchbee_workbench_protocol::{
    LaunchOptions, ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchLabel, WorkbenchRequest, WorkbenchResponse, WorkbenchTabCloseStatus,
    WorkbenchTerminalOutcome, WorkbenchWorkspaceCloseRead, WorkbenchWorkspaceCloseReadOutcome,
    WorkbenchWorkspaceCloseResume, WorktreeIntent, WorktreeOperationFailure as Failure,
    WorktreeOperationStatus as Status, WorktreeWorkspaceDisposition,
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
        let (prepared, handle, workspace_id) = {
            let workers = self
                .service
                .workers
                .lock()
                .map_err(|_| Failure::Internal {})?;
            let handle = workers
                .request_handle()
                .ok_or(Failure::WorkerUnavailable {})?;
            let worker = placement(Some(&handle)).ok_or(Failure::WorkerUnavailable {})?;
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
            if let [(project_id, workspace_id)] = existing.as_slice() {
                if *project_id != self.plan.project_id
                    || !authority.worktree_workspace_available(project_id, workspace_id)
                {
                    return Err(Failure::CheckoutInUse {});
                }
                if let Some(label) = label {
                    let command = WorkbenchCommand {
                        protocol_version: ProtocolVersion,
                        request_id,
                        site_id: authority.site_id().clone(),
                        expected_revision: authority.snapshot().revision,
                        action: WorkbenchAction::WorkspaceRename {
                            project_id: project_id.clone(),
                            workspace_id: workspace_id.clone(),
                            label,
                        },
                    };
                    let request = WorkbenchReadOrNavigationRequest::try_from(
                        WorkbenchRequest::Command(command),
                    )
                    .map_err(|_| Failure::Internal {})?;
                    let grant = WorkbenchGrant::manage_navigation(authority.site_id().clone());
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
                site_id: authority.site_id().clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::WorkspaceLaunch {
                    project_id: self.plan.project_id.clone(),
                    label,
                    geometry,
                    launch: None,
                    profile: None,
                    options: Box::new(LaunchOptions {
                        cwd: Some(checkout.path.clone()),
                        ..LaunchOptions::default()
                    }),
                },
            };
            let prepared = coordinator.prepare_workspace_launch(command, candidate, expiry, worker);
            let authority = coordinator
                .worktree_authority()
                .ok_or(Failure::Internal {})?;
            let committed =
                authority.checkout_workspaces(&checkout.path, &self.plan.source.worker_id);
            let [(project_id, workspace_id)] = committed.as_slice() else {
                return Err(Failure::AssociationFailed {});
            };
            if *project_id != self.plan.project_id
                || !authority.associate_worktree(self.plan, workspace_id, checkout.clone())
            {
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
        if current.iter().any(|(project, workspace)| {
            closed || *project != self.plan.project_id || workspace != workspace_id
        }) {
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
                site_id: authority.site_id().clone(),
                expected_revision: authority.snapshot().revision,
                action: WorkbenchAction::WorkspaceClose {
                    project_id: self.plan.project_id.clone(),
                    workspace_id: workspace_id.clone(),
                },
            };
            coordinator
                .execute_workspace_close(
                    command,
                    placement(Some(&handle)),
                    self.service.close_maintenance.available(),
                    Instant::now(),
                )
                .ok_or(Failure::CloseIncomplete {})?
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
    pub(super) fn wait_close(&mut self) -> Result<(), Failure> {
        let WorktreeWorkspaceDisposition::Closing {
            workspace_id,
            close_request_id,
        } = self.plan.progress.workspace.clone()
        else {
            return Err(Failure::Internal {});
        };
        let site_id = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .site_id()
            .clone();
        let mut may_resume = self.plan.attempt > 1;
        loop {
            self.check()?;
            let result = self
                .service
                .read_workspace_close(WorkbenchWorkspaceCloseRead {
                    protocol_version: ProtocolVersion,
                    request_id: identity()?,
                    site_id: site_id.clone(),
                    project_id: self.plan.project_id.clone(),
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
            match status {
                WorkbenchTabCloseStatus::Closed { .. } => {
                    self.plan.progress.workspace = WorktreeWorkspaceDisposition::Closed {
                        workspace_id,
                        close_request_id,
                    };
                    return Ok(());
                }
                WorkbenchTabCloseStatus::Closing { .. } => thread::sleep(Duration::from_millis(50)),
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
                                site_id: site_id.clone(),
                                project_id: self.plan.project_id.clone(),
                                workspace_id: workspace_id.clone(),
                                close_request_id: close_request_id.clone(),
                                expected_attempt_generation: attempt_generation,
                            });
                }
                _ => return Err(Failure::CloseIncomplete {}),
            }
        }
    }
}
