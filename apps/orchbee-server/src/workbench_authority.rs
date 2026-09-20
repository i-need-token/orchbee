use std::collections::BTreeMap;

use orchbee_workbench_protocol::{
    ContractError, MAXIMUM_PROJECTS, MAXIMUM_TERMINAL_CONTEXTS, MAXIMUM_WORKSPACES_PER_PROJECT,
    Navigation, Project, ProtocolVersion, SiteId, Snapshot, TerminalContext, WorkbenchAction,
    WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchLabel, WorkbenchProjectId,
    WorkbenchProtocolDocument, WorkbenchPublicationDocument, WorkbenchPublicationOutcome,
    WorkbenchPublicationResult, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    WorkbenchSnapshotUnavailableReason, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, Workspace, serialize_document, serialize_workbench_request,
    serialize_workbench_response,
};

use crate::workbench_projection::{
    PreparedProjectionAdvance, WorkbenchProjectionStore, WorkbenchPublication,
};

mod terminal_membership;
mod workspace_directory;
pub use terminal_membership::{
    PreparedStandaloneIntent, StandaloneIntentError, StandaloneIntentMetadata,
    WorkbenchTerminalInvariantError, WorkbenchTerminalMembership, WorkbenchTerminalTab,
};
mod terminal_tab_create;
pub use terminal_tab_create::{
    CommittedWorkbenchTabCreate, CommittedWorkbenchTabCreateMetadata, PendingWorkbenchTabCreate,
    PreparedWorkbenchTabCreate, PreparedWorkbenchTabSettlement, StagedWorkbenchTabCreateMetadata,
    WorkbenchSettlementTokenMismatch, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
};
mod terminal_panel_split;
pub use terminal_panel_split::{
    CommittedWorkbenchPanelSplitMetadata, PendingWorkbenchPanelSplit, PreparedWorkbenchPanelSplit,
    PreparedWorkbenchPanelSplitSettlement, StagedWorkbenchPanelSplitMetadata,
    WorkbenchPanelSplitAdmission, WorkbenchPanelSplitBegin, WorkbenchPanelSplitPreparation,
    WorkbenchPanelSplitSettlementTokenMismatch,
};
mod terminal_binding_selection;
pub use terminal_binding_selection::WorkbenchBindingExposure;
mod terminal_tab_move;
pub use terminal_tab_move::{
    PreparedWorkbenchTabMove, WorkbenchTabMoveAdmission, WorkbenchTabMoveBegin,
    WorkbenchTabMovePreparation,
};
mod terminal_panel_move;
pub use terminal_panel_move::{
    PreparedWorkbenchPanelMove, WorkbenchPanelMoveAdmission, WorkbenchPanelMoveBegin,
    WorkbenchPanelMovePreparation,
};
mod terminal_tab_close;
pub use terminal_tab_close::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch, PreparedWorkbenchTabClose, PreparedWorkbenchTabClosePublication,
    PreparedWorkbenchTabCloseResume, WorkbenchTabCloseAdmission, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabClosePrimaryAdmission,
    WorkbenchTabClosePublicationError, WorkbenchTabCloseResolution,
    WorkbenchTabCloseResumeAdmission, WorkbenchTabCloseResumeBegin,
    WorkbenchTabCloseResumePreparation, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTabCloseStepTarget,
    WorkbenchTabCloseTarget,
};

const MAXIMUM_LEDGER_ENTRIES: usize = 4096;
const MAXIMUM_LEDGER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct WorkbenchGrant {
    site_id: SiteId,
    mutate_navigation: bool,
    create_terminal: bool,
    close_terminal: bool,
    associate_directory: bool,
}

pub struct WorkbenchReadOrNavigationRequest(ReadOrNavigationRequest);

#[allow(clippy::large_enum_variant)]
enum ReadOrNavigationRequest {
    SnapshotRead(orchbee_workbench_protocol::SnapshotReadIntent),
    PublicationRead(orchbee_workbench_protocol::PublicationReadIntent),
    Command {
        command: WorkbenchCommand,
        action: PureWorkbenchAction,
    },
}

impl TryFrom<WorkbenchRequest> for WorkbenchReadOrNavigationRequest {
    type Error = WorkbenchRequest;

    fn try_from(request: WorkbenchRequest) -> Result<Self, Self::Error> {
        match request {
            WorkbenchRequest::SnapshotRead(intent) => {
                Ok(Self(ReadOrNavigationRequest::SnapshotRead(intent)))
            }
            WorkbenchRequest::PublicationRead(intent) => {
                Ok(Self(ReadOrNavigationRequest::PublicationRead(intent)))
            }
            WorkbenchRequest::Command(command) => {
                let Some(action) = PureWorkbenchAction::from_action(&command.action) else {
                    return Err(WorkbenchRequest::Command(command));
                };
                Ok(Self(ReadOrNavigationRequest::Command { command, action }))
            }
            request @ (WorkbenchRequest::TabCloseRead(_) | WorkbenchRequest::TabCloseResume(_)) => {
                Err(request)
            }
        }
    }
}

impl WorkbenchGrant {
    #[must_use]
    pub fn read_only(site_id: SiteId) -> Self {
        Self {
            site_id,
            mutate_navigation: false,
            create_terminal: false,
            close_terminal: false,
            associate_directory: false,
        }
    }
    #[must_use]
    pub fn manage_navigation(site_id: SiteId) -> Self {
        Self {
            site_id,
            mutate_navigation: true,
            create_terminal: false,
            close_terminal: false,
            associate_directory: false,
        }
    }

    #[must_use]
    pub fn manage_navigation_and_terminal_create(site_id: SiteId) -> Self {
        Self {
            site_id,
            mutate_navigation: true,
            create_terminal: true,
            close_terminal: false,
            associate_directory: false,
        }
    }

    /// Adds the independent terminal lifecycle grant, including for read-only Clients.
    #[must_use]
    pub const fn with_terminal_lifecycle(mut self) -> Self {
        self.close_terminal = true;
        self
    }

    /// Adds directory association; navigation authority remains independently required.
    #[must_use]
    pub const fn with_directory_association(mut self) -> Self {
        self.associate_directory = true;
        self
    }
}

struct SettledRequest {
    command: WorkbenchCommand,
    result: WorkbenchCommandResult,
}

enum RetainedRequest {
    Settled(Box<SettledRequest>),
    PendingTabCreate(Box<terminal_tab_create::PendingTabCreateEntry>),
    PendingPanelSplit(Box<terminal_panel_split::PendingPanelSplitEntry>),
    TabClose(Box<terminal_tab_close::CloseRecord>),
}

impl RetainedRequest {
    fn command(&self) -> &WorkbenchCommand {
        match self {
            Self::Settled(entry) => &entry.command,
            Self::PendingTabCreate(entry) => &entry.command,
            Self::PendingPanelSplit(entry) => &entry.command,
            Self::TabClose(entry) => &entry.command,
        }
    }

    fn replay_result(&self) -> &WorkbenchCommandResult {
        match self {
            Self::Settled(entry) => &entry.result,
            Self::PendingTabCreate(entry) => &entry.pending_result,
            Self::PendingPanelSplit(entry) => &entry.pending_result,
            Self::TabClose(entry) => &entry.result,
        }
    }
}

pub struct WorkbenchAuthority {
    projection: WorkbenchProjectionStore,
    last_identity: u128,
    ledger: BTreeMap<WorkbenchRequestId, RetainedRequest>,
    ledger_bytes: usize,
    retired: bool,
}

impl WorkbenchAuthority {
    pub fn new(site_id: SiteId) -> Result<Self, ContractError> {
        let context_id = WorkbenchTerminalContextId::parse("00000000000000000000000000000001")
            .map_err(|_error| ContractError::InvalidDocument)?;
        let snapshot = Snapshot {
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(0),
            site_id,
            navigation: Navigation {
                projects: Vec::new(),
                standalone_terminal_context_id: context_id.clone(),
            },
            terminal_contexts: vec![empty_context(context_id)],
        };
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot))?;
        Ok(Self {
            projection: WorkbenchProjectionStore::load(&encoded)?,
            last_identity: 1,
            ledger: BTreeMap::new(),
            ledger_bytes: 0,
            retired: false,
        })
    }

    #[must_use]
    pub fn site_id(&self) -> &SiteId {
        &self.projection.snapshot().site_id
    }

    /// The committed projection head. Callers already hold the coordinator
    /// lock, so the returned snapshot is a consistent join with any retained
    /// terminal facts resolved under the same acquisition.
    #[must_use]
    pub fn snapshot(&self) -> &Snapshot {
        self.projection.snapshot()
    }

    pub fn execute(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchReadOrNavigationRequest,
    ) -> WorkbenchResponse {
        match request.0 {
            ReadOrNavigationRequest::SnapshotRead(intent) => {
                let authorized = grant.is_some_and(|grant| grant.site_id == *self.site_id());
                let outcome = if !authorized {
                    WorkbenchSnapshotOutcome::Denied {}
                } else if self.retired {
                    WorkbenchSnapshotOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                } else {
                    WorkbenchSnapshotOutcome::Available {
                        snapshot: self.projection.snapshot().clone(),
                    }
                };
                WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id,
                    outcome,
                })
            }
            ReadOrNavigationRequest::PublicationRead(intent) => {
                let authorized = grant.is_some_and(|grant| grant.site_id == *self.site_id());
                let outcome = if !authorized {
                    WorkbenchPublicationOutcome::Denied {}
                } else if self.retired {
                    WorkbenchPublicationOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                } else {
                    self.publication_outcome(intent.known_revision)
                };
                WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id,
                    outcome,
                })
            }
            ReadOrNavigationRequest::Command { command, action } => {
                let authorized = grant.is_some_and(|grant| {
                    grant.site_id == *self.site_id() && grant.mutate_navigation
                });
                let command_result = if authorized {
                    self.command(command, action)
                } else {
                    result(&command, WorkbenchCommandOutcome::Denied {})
                };
                WorkbenchResponse::Command(command_result)
            }
        }
    }

    /// Builds the committed-publication replay for `known_revision`. A batch that cannot fit the
    /// snapshot-result envelope degrades to a single current Snapshot; that Snapshot already passed
    /// its own encoded bound when it was committed.
    fn publication_outcome(
        &self,
        known_revision: WorkbenchRevision,
    ) -> WorkbenchPublicationOutcome {
        let batch = self.projection.publications_since(known_revision);
        if batch.is_empty() {
            return WorkbenchPublicationOutcome::Current {
                revision: self.projection.snapshot().revision,
            };
        }
        let documents = batch
            .iter()
            .map(|publication| match publication {
                WorkbenchPublication::Delta(delta) => {
                    WorkbenchPublicationDocument::Delta(delta.clone())
                }
                WorkbenchPublication::Snapshot(snapshot) => {
                    WorkbenchPublicationDocument::Snapshot(snapshot.clone())
                }
            })
            .collect::<Vec<_>>();
        if self.publication_batch_fits(&documents) {
            return WorkbenchPublicationOutcome::Published {
                publications: documents,
            };
        }
        let snapshot = vec![WorkbenchPublicationDocument::Snapshot(
            self.projection.snapshot().clone(),
        )];
        if self.publication_batch_fits(&snapshot) {
            WorkbenchPublicationOutcome::Published {
                publications: snapshot,
            }
        } else {
            WorkbenchPublicationOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            }
        }
    }

    /// Serializes a candidate `published` outcome to prove the encoded envelope and every nested
    /// document fit their bounds before the outcome is emitted.
    fn publication_batch_fits(&self, documents: &[WorkbenchPublicationDocument]) -> bool {
        let Ok(request_id) = WorkbenchRequestId::parse("00000000000000000000000000000001") else {
            return false;
        };
        serialize_workbench_response(&WorkbenchResponse::Publication(
            WorkbenchPublicationResult {
                protocol_version: ProtocolVersion,
                request_id,
                outcome: WorkbenchPublicationOutcome::Published {
                    publications: documents.to_vec(),
                },
            },
        ))
        .is_ok()
    }

    fn command(
        &mut self,
        command: WorkbenchCommand,
        action: PureWorkbenchAction,
    ) -> WorkbenchCommandResult {
        if let Some(result) = self.command_preflight(&command) {
            return result;
        }
        let plan = match self.plan(&command, action) {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => CommitPlan {
                projection: None,
                last_identity: None,
                outcome: self.rejected(reason),
            },
            Err(PlanFailure::Internal) => return self.retire(&command),
        };
        match self.prepare_command_commit(command, plan) {
            Ok(prepared) => prepared.commit(),
            Err(result) => *result,
        }
    }

    fn command_preflight(&self, command: &WorkbenchCommand) -> Option<WorkbenchCommandResult> {
        if self.retired {
            return Some(result(
                command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        }
        if let Some(settled) = self.ledger.get(&command.request_id) {
            return Some(if settled.command() == command {
                settled.replay_result().clone()
            } else {
                result(
                    command,
                    self.rejected(WorkbenchRejectionReason::RequestIdentityConflict),
                )
            });
        }
        if self.ledger.len() >= MAXIMUM_LEDGER_ENTRIES {
            return Some(result(
                command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        None
    }

    fn prepare_command_commit(
        &mut self,
        command: WorkbenchCommand,
        plan: CommitPlan,
    ) -> Result<PreparedCommandCommit<'_>, Box<WorkbenchCommandResult>> {
        let response = result(&command, plan.outcome);
        let Some(entry_bytes) = ledger_size(&command, &response) else {
            return Err(Box::new(self.retire(&command)));
        };
        let Some(next_bytes) = self.ledger_bytes.checked_add(entry_bytes) else {
            return Err(Box::new(self.retire(&command)));
        };
        if next_bytes > MAXIMUM_LEDGER_BYTES {
            return Err(Box::new(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            )));
        }
        let request_id = command.request_id.clone();
        let retained = Box::new(SettledRequest {
            command,
            result: response.clone(),
        });
        Ok(PreparedCommandCommit {
            authority: self,
            projection: plan.projection,
            last_identity: plan.last_identity,
            next_bytes,
            request_id,
            retained,
            response,
        })
    }

    fn retain_rejection(
        &mut self,
        command: WorkbenchCommand,
        reason: WorkbenchRejectionReason,
    ) -> WorkbenchCommandResult {
        let plan = CommitPlan {
            projection: None,
            last_identity: None,
            outcome: self.rejected(reason),
        };
        match self.prepare_command_commit(command, plan) {
            Ok(prepared) => prepared.commit(),
            Err(result) => *result,
        }
    }

    fn rejected(&self, reason: WorkbenchRejectionReason) -> WorkbenchCommandOutcome {
        WorkbenchCommandOutcome::Rejected {
            revision: self.projection.snapshot().revision,
            reason,
        }
    }

    fn retire(&mut self, command: &WorkbenchCommand) -> WorkbenchCommandResult {
        self.retired = true;
        result(
            command,
            unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
        )
    }

    fn plan(
        &self,
        command: &WorkbenchCommand,
        action: PureWorkbenchAction,
    ) -> Result<CommitPlan, PlanFailure> {
        let current = self.projection.snapshot();
        if command.site_id != current.site_id {
            return reject(WorkbenchRejectionReason::SiteMismatch);
        }
        if command.expected_revision != current.revision {
            return reject(WorkbenchRejectionReason::RevisionMismatch);
        }
        let mut candidate = current.clone();
        let mut last_identity = self.last_identity;
        let outcome = match action {
            PureWorkbenchAction::ProjectCreate { label } => {
                if candidate.navigation.projects.len() >= MAXIMUM_PROJECTS {
                    return reject(WorkbenchRejectionReason::ResourceLimit);
                }
                last_identity = allocate(last_identity, 1)?;
                let project_id = WorkbenchProjectId::parse(&identity(last_identity))
                    .map_err(|_error| PlanFailure::Internal)?;
                let revision = self.next_revision()?;
                candidate.navigation.projects.push(Project {
                    project_id: project_id.clone(),
                    label: label.as_str().to_owned(),
                    directories: Vec::new(),
                    workspaces: Vec::new(),
                });
                candidate.revision = revision;
                WorkbenchCommandOutcome::ProjectCreated {
                    revision,
                    project_id,
                }
            }
            PureWorkbenchAction::ProjectRename { project_id, label } => {
                let project = candidate
                    .navigation
                    .projects
                    .iter_mut()
                    .find(|project| project.project_id == project_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ProjectUnavailable,
                    ))?;
                if project.label == label.as_str() {
                    return Ok(self.unchanged());
                }
                let revision = self.next_revision()?;
                project.label = label.as_str().to_owned();
                candidate.revision = revision;
                WorkbenchCommandOutcome::Renamed { revision }
            }
            PureWorkbenchAction::WorkspaceCreate { project_id, label } => {
                let project = candidate
                    .navigation
                    .projects
                    .iter_mut()
                    .find(|project| project.project_id == project_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ProjectUnavailable,
                    ))?;
                if project.workspaces.len() >= MAXIMUM_WORKSPACES_PER_PROJECT
                    || candidate.terminal_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS
                {
                    return reject(WorkbenchRejectionReason::ResourceLimit);
                }
                last_identity = allocate(last_identity, 2)?;
                let workspace_id = WorkbenchWorkspaceId::parse(&identity(last_identity - 1))
                    .map_err(|_error| PlanFailure::Internal)?;
                let terminal_context_id =
                    WorkbenchTerminalContextId::parse(&identity(last_identity))
                        .map_err(|_error| PlanFailure::Internal)?;
                let revision = self.next_revision()?;
                project.workspaces.push(Workspace {
                    workspace_id: workspace_id.clone(),
                    primary_directory_id: None,
                    terminal_context_id: terminal_context_id.clone(),
                    label: label.as_str().to_owned(),
                });
                candidate
                    .terminal_contexts
                    .push(empty_context(terminal_context_id.clone()));
                candidate.revision = revision;
                WorkbenchCommandOutcome::WorkspaceCreated {
                    revision,
                    workspace_id,
                    terminal_context_id,
                }
            }
            PureWorkbenchAction::WorkspaceRename {
                project_id,
                workspace_id,
                label,
            } => {
                let project = candidate
                    .navigation
                    .projects
                    .iter_mut()
                    .find(|project| project.project_id == project_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::ProjectUnavailable,
                    ))?;
                let workspace = project
                    .workspaces
                    .iter_mut()
                    .find(|workspace| workspace.workspace_id == workspace_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::WorkspaceUnavailable,
                    ))?;
                if workspace.label == label.as_str() {
                    return Ok(self.unchanged());
                }
                let revision = self.next_revision()?;
                workspace.label = label.as_str().to_owned();
                candidate.revision = revision;
                WorkbenchCommandOutcome::Renamed { revision }
            }
            PureWorkbenchAction::TabRename {
                terminal_context_id,
                tab_id,
                label,
            } => {
                let context = candidate
                    .terminal_contexts
                    .iter_mut()
                    .find(|context| context.terminal_context_id == terminal_context_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::TerminalContextUnavailable,
                    ))?;
                let tab = context
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.tab_id == tab_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::TabUnavailable,
                    ))?;
                if tab.label == label.as_str() {
                    return Ok(self.unchanged());
                }
                let revision = self.next_revision()?;
                tab.label = label.as_str().to_owned();
                candidate.revision = revision;
                WorkbenchCommandOutcome::Renamed { revision }
            }
        };
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => {
                    PlanFailure::Rejected(WorkbenchRejectionReason::ResourceLimit)
                }
                _ => PlanFailure::Internal,
            },
        )?;
        let projection = self
            .projection
            .prepare_advance(&encoded)
            .map_err(|_error| PlanFailure::Internal)?;
        Ok(CommitPlan {
            projection: Some(projection),
            last_identity: Some(last_identity),
            outcome,
        })
    }

    fn unchanged(&self) -> CommitPlan {
        CommitPlan {
            projection: None,
            last_identity: None,
            outcome: WorkbenchCommandOutcome::Unchanged {
                revision: self.projection.snapshot().revision,
            },
        }
    }
}

struct CommitPlan {
    projection: Option<PreparedProjectionAdvance>,
    last_identity: Option<u128>,
    outcome: WorkbenchCommandOutcome,
}

struct PreparedCommandCommit<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: Option<PreparedProjectionAdvance>,
    last_identity: Option<u128>,
    next_bytes: usize,
    request_id: WorkbenchRequestId,
    retained: Box<SettledRequest>,
    response: WorkbenchCommandResult,
}

impl PreparedCommandCommit<'_> {
    fn commit(self) -> WorkbenchCommandResult {
        if let Some(projection) = self.projection {
            self.authority.projection.commit_prepared(projection);
        }
        if let Some(last_identity) = self.last_identity {
            self.authority.last_identity = last_identity;
        }
        self.authority.ledger_bytes = self.next_bytes;
        self.authority
            .ledger
            .insert(self.request_id, RetainedRequest::Settled(self.retained));
        self.response
    }
}
enum PlanFailure {
    Rejected(WorkbenchRejectionReason),
    Internal,
}

enum PureWorkbenchAction {
    ProjectCreate {
        label: WorkbenchLabel,
    },
    ProjectRename {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    WorkspaceCreate {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    WorkspaceRename {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    TabRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        label: WorkbenchLabel,
    },
}

impl PureWorkbenchAction {
    fn from_action(action: &WorkbenchAction) -> Option<Self> {
        Some(match action {
            WorkbenchAction::ProjectCreate { label } => Self::ProjectCreate {
                label: label.clone(),
            },
            WorkbenchAction::ProjectRename { project_id, label } => Self::ProjectRename {
                project_id: project_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::WorkspaceCreate { project_id, label } => Self::WorkspaceCreate {
                project_id: project_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::WorkspaceRename {
                project_id,
                workspace_id,
                label,
            } => Self::WorkspaceRename {
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::TabRename {
                terminal_context_id,
                tab_id,
                label,
            } => Self::TabRename {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::TabCreate { .. }
            | WorkbenchAction::PanelSplit { .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
            | WorkbenchAction::TabMove { .. }
            | WorkbenchAction::TabClose { .. }
            | WorkbenchAction::PanelClose { .. }
            | WorkbenchAction::PanelMove { .. } => return None,
        })
    }
}

fn reject<T>(reason: WorkbenchRejectionReason) -> Result<T, PlanFailure> {
    Err(PlanFailure::Rejected(reason))
}
fn allocate(last: u128, count: u128) -> Result<u128, PlanFailure> {
    last.checked_add(count).ok_or(PlanFailure::Rejected(
        WorkbenchRejectionReason::IdentityExhausted,
    ))
}
fn identity(value: u128) -> String {
    format!("{value:032x}")
}
fn empty_context(terminal_context_id: WorkbenchTerminalContextId) -> TerminalContext {
    TerminalContext {
        terminal_context_id,
        groups: Vec::new(),
        tabs: Vec::new(),
    }
}
fn result(command: &WorkbenchCommand, outcome: WorkbenchCommandOutcome) -> WorkbenchCommandResult {
    WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome,
    }
}
fn unavailable(reason: WorkbenchCommandUnavailableReason) -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::Unavailable { reason }
}
fn ledger_size(command: &WorkbenchCommand, result: &WorkbenchCommandResult) -> Option<usize> {
    let request = serialize_workbench_request(&WorkbenchRequest::Command(command.clone())).ok()?;
    let response =
        serialize_workbench_response(&WorkbenchResponse::Command(result.clone())).ok()?;
    request.len().checked_add(response.len())
}

#[cfg(test)]
mod tests;
