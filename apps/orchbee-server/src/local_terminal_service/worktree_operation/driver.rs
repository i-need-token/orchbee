use super::LocalTerminalService;
use orchbee_capability_node_protocol::{
    AbsoluteCwd, GitReference, WorktreeCheckoutSelector, WorktreeMutation,
};
use orchbee_server::WorktreeOperationPlan;
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchRequestId, WorktreeCheckoutFacts, WorktreeGitEffect,
    WorktreeIntent, WorktreeOperationFailure as Failure, WorktreeOperationPhase as Phase,
    WorktreeOperationStatus as Status, WorktreeReference, WorktreeSelector,
    WorktreeWorkspaceDisposition,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
mod git;
mod workspace;

pub(super) fn run(
    service: &LocalTerminalService,
    plan: &mut WorktreeOperationPlan,
    stop: &AtomicBool,
) -> Status {
    let mut driver = Driver {
        service,
        plan,
        stop,
        deadline: Instant::now() + Duration::from_secs(120),
    };
    match driver.execute() {
        Ok(status) => status,
        Err(reason) => {
            let resumable = !matches!(
                reason,
                Failure::TransportUnknown {}
                    | Failure::PhysicalUnknown {}
                    | Failure::SiteStopping {}
                    | Failure::Internal {}
                    | Failure::SourceChanged {}
            );
            Status::Failed {
                reason,
                progress: driver.plan.progress.clone(),
                resumable,
            }
        }
    }
}
struct Driver<'a> {
    service: &'a LocalTerminalService,
    plan: &'a mut WorktreeOperationPlan,
    stop: &'a AtomicBool,
    deadline: Instant,
}
impl Driver<'_> {
    fn execute(&mut self) -> Result<Status, Failure> {
        loop {
            self.check()?;
            match self.plan.phase {
                Phase::ObservingSource => {
                    let source = self.checkout_job(WorktreeMutation::InspectSource)?;
                    self.associate_source(source.clone())?;
                    if matches!(self.plan.intent, WorktreeIntent::Create { .. }) {
                        let (branch, _, path) = self.create_operands(Some(&source))?;
                        let _checked_branch = branch;
                        self.plan.progress.path = Some(path);
                        self.advance(Phase::Creating)?;
                    } else {
                        self.advance(Phase::Opening)?;
                    }
                }
                Phase::Creating => {
                    self.recheck_source()?;
                    let (branch, base, path) = self.create_operands(None)?;
                    let checkout = self.checkout_job(WorktreeMutation::Create {
                        path: absolute(&path)?,
                        branch,
                        base,
                    })?;
                    self.plan.progress.git_effect = WorktreeGitEffect::Changed;
                    self.observed(checkout);
                    self.advance(Phase::Attaching)?;
                }
                Phase::Opening => {
                    self.recheck_source()?;
                    let selector = match &self.plan.intent {
                        WorktreeIntent::Open { selector, .. } => match selector {
                            WorktreeSelector::Path { path } => {
                                WorktreeCheckoutSelector::Path(absolute(path)?)
                            }
                            WorktreeSelector::Branch { branch } => {
                                WorktreeCheckoutSelector::Branch(reference(branch.as_str())?)
                            }
                        },
                        // A known partial Create resumes by observing the original path, never by rerunning add.
                        WorktreeIntent::Create { .. } => WorktreeCheckoutSelector::Path(absolute(
                            self.plan
                                .progress
                                .path
                                .as_ref()
                                .ok_or(Failure::Internal {})?,
                        )?),
                        WorktreeIntent::Remove { .. } => return Err(Failure::Internal {}),
                    };
                    let checkout = self.checkout_job(WorktreeMutation::Open(selector))?;
                    if let Some(previous) = &self.plan.progress.checkout
                        && previous.common_directory != checkout.common_directory
                    {
                        return Err(Failure::SourceChanged {});
                    }
                    if matches!(self.plan.intent, WorktreeIntent::Create { .. })
                        && checkout.branch.as_ref().map(WorktreeReference::as_str)
                            != Some(self.create_operands(None)?.0.as_str())
                    {
                        return Err(Failure::SourceChanged {});
                    }
                    self.observed(checkout);
                    self.advance(Phase::Attaching)?;
                }
                Phase::Attaching => return self.attach(),
                Phase::InspectingRemove => {
                    let path = self
                        .plan
                        .progress
                        .path
                        .clone()
                        .ok_or(Failure::Internal {})?;
                    let checkout = self.checkout_job(WorktreeMutation::InspectRemove {
                        path: absolute(&path)?,
                    })?;
                    if let Some(previous) = &self.plan.progress.checkout
                        && previous.common_directory != checkout.common_directory
                    {
                        return Err(Failure::SourceChanged {});
                    }
                    self.observed(checkout);
                    let WorktreeIntent::Remove { force, .. } = self.plan.intent else {
                        return Err(Failure::Internal {});
                    };
                    if !force
                        && self
                            .plan
                            .progress
                            .checkout
                            .as_ref()
                            .is_some_and(|checkout| checkout.dirty)
                    {
                        return Err(Failure::Git {
                            stage: orchbee_workbench_protocol::WorktreeGitStage::DirtyCheck,
                            error: orchbee_workbench_protocol::WorktreeGitError::Dirty,
                            status: None,
                            diagnostics: vec![],
                        });
                    }
                    if matches!(
                        self.plan.progress.workspace,
                        WorktreeWorkspaceDisposition::Closed { .. }
                    ) {
                        self.advance(Phase::Removing)?;
                    } else {
                        self.begin_close()?;
                        self.advance(Phase::Closing)?;
                    }
                }
                Phase::Closing => {
                    self.wait_close()?;
                    self.advance(Phase::Removing)?;
                }
                Phase::Removing => {
                    self.require_exclusive_checkout()?;
                    let checkout = self
                        .plan
                        .progress
                        .checkout
                        .clone()
                        .ok_or(Failure::Internal {})?;
                    let WorktreeIntent::Remove {
                        force,
                        workspace_id,
                        ..
                    } = &self.plan.intent
                    else {
                        return Err(Failure::Internal {});
                    };
                    let workspace_id = workspace_id.clone();
                    let mutation = WorktreeMutation::Remove {
                        path: absolute(&checkout.path)?,
                        expected_common_directory: absolute(&checkout.common_directory)?,
                        force: *force,
                    };
                    match self.job(mutation)? {
                        orchbee_capability_node_protocol::WorktreeMutationCompletion::Removed {
                            path,
                        } => {
                            return Ok(Status::Removed {
                                path: WorkbenchDirectoryPath::parse(path.as_str())
                                    .map_err(|_| Failure::Internal {})?,
                                workspace_id,
                            });
                        }
                        _ => return Err(Failure::Internal {}),
                    }
                }
            }
        }
    }
    fn check(&self) -> Result<(), Failure> {
        if self.stop.load(Ordering::Acquire) {
            return Err(Failure::SiteStopping {});
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::CloseIncomplete {});
        }
        Ok(())
    }
    fn observed(&mut self, checkout: WorktreeCheckoutFacts) {
        self.plan.progress.path = Some(checkout.path.clone());
        self.plan.progress.checkout = Some(checkout);
    }
    fn advance(&mut self, phase: Phase) -> Result<(), Failure> {
        self.plan.phase = phase;
        let status = Status::Pending {
            phase,
            progress: self.plan.progress.clone(),
        };
        let mut coordinator = self
            .service
            .coordinator
            .lock()
            .map_err(|_| Failure::Internal {})?;
        if !coordinator
            .worktree_authority()
            .ok_or(Failure::Internal {})?
            .publish_worktree_operation(self.plan, status)
        {
            return Err(Failure::Internal {});
        }
        Ok(())
    }
    fn create_operands(
        &self,
        source: Option<&WorktreeCheckoutFacts>,
    ) -> Result<(GitReference, GitReference, WorkbenchDirectoryPath), Failure> {
        let WorktreeIntent::Create {
            branch, base, path, ..
        } = &self.plan.intent
        else {
            return Err(Failure::Internal {});
        };
        let branch = branch.as_ref().map_or_else(
            || format!("worktree/{}", self.plan.operation_id.as_str()),
            |branch| branch.as_str().to_owned(),
        );
        let path = path
            .clone()
            .or_else(|| self.plan.progress.path.clone())
            .or_else(|| {
                let root = source?.repository_root.as_str().trim_end_matches('/');
                let (parent, name) = root.rsplit_once('/')?;
                let slug: String = branch
                    .chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                            ch
                        } else {
                            '-'
                        }
                    })
                    .collect();
                WorkbenchDirectoryPath::parse(&format!("{parent}/{name}-worktrees/{slug}")).ok()
            })
            .ok_or(Failure::Internal {})?;
        Ok((
            reference(&branch)?,
            reference(base.as_ref().map_or("HEAD", WorktreeReference::as_str))?,
            path,
        ))
    }
}
fn absolute(path: &WorkbenchDirectoryPath) -> Result<AbsoluteCwd, Failure> {
    AbsoluteCwd::new(path.as_str()).map_err(|_| Failure::Internal {})
}
fn reference(value: &str) -> Result<GitReference, Failure> {
    GitReference::new(value).map_err(|_| Failure::Internal {})
}
fn identity() -> Result<WorkbenchRequestId, Failure> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Internal {})?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::Internal {})
}
