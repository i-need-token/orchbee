//! Owns canonical Session Workbench state, revision admission, identity allocation, and the
//! bounded retained request ledger with prepared projection commits. It decides logical
//! membership and publication without performing Worker effects or choosing Client-local
//! focus.

mod layout_create;
use std::collections::BTreeMap;

use orchbee_workbench_protocol::{
    ContractError, Directory, LaunchProfileCatalogOutcome, LaunchProfileCatalogRevision,
    LaunchProfileEntry, LaunchProfileId, LaunchProfileSelector,
    MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES, MAXIMUM_LAUNCH_PROFILE_ENTRIES, MAXIMUM_PROJECTS,
    MAXIMUM_TERMINAL_CONTEXTS, Navigation, Project, ProtocolVersion, SessionId, Snapshot,
    TerminalContext, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchDirectoryId,
    WorkbenchLabel, WorkbenchLaunchProfileCatalogResult, WorkbenchProjectId,
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

mod checkpoint;
pub use checkpoint::{
    AgentResumeProvider, LayoutHistorySummary, LayoutReopenError,
    MAXIMUM_WORKBENCH_CHECKPOINT_BYTES, OpenLayout, PreparedWorkbenchRestoreBinding,
    ReopenedLayout, RetainedAgentSession, RetainedLayout, WorkbenchCheckpoint,
    WorkbenchCheckpointError, WorkbenchRestoreError, WorkbenchRestoreTarget,
};
mod agent_reports;
mod panel_agent;
mod report_membership;
pub use panel_agent::{
    TerminalAgentBinding, TerminalAgentExplainContext, TerminalAgentNameOwner,
    TerminalAgentProcessObservation, TerminalAgentProcessOutcome, TerminalAgentScreenObservation,
};
mod panel_metadata;
mod plugin_panes;
mod popup_source;
mod split_ratios;
pub use plugin_panes::PluginPaneLaunch;
mod tab_duplicate;
pub use tab_duplicate::TabDuplicatePlan;
mod tab_set_close;
pub use tab_set_close::{
    TabSetCloseAdmission, TabSetCloseBegin, TabSetCloseMemberPlan, TabSetCloseMemberState,
    TabSetClosePlan,
};
mod terminal_membership;
mod workspace_directory;
mod workspace_group_close;
mod workspace_metadata;
mod workspace_order;
mod workspace_organization;
pub use workspace_organization::idle_dissolve::{
    WorkbenchProjectDissolveAdmission, WorkbenchProjectDissolveBegin,
};
mod worktree_membership;
mod worktree_operation;
pub use workspace_group_close::{WorkspaceGroupCloseMemberPlan, WorkspaceGroupClosePlan};
mod worktree_query;
pub use terminal_membership::{
    WorkbenchTerminalInvariantError, WorkbenchTerminalMembership, WorkbenchTerminalTab,
};
pub use worktree_operation::{
    PreparedWorktreeRebind, WorktreeOperationPlan, WorktreeRebindError, WorktreeSourceTarget,
};
mod terminal_launch_failure;
mod terminal_tab_create;
mod workspace_directory_label;
pub use terminal_tab_create::{
    CommittedWorkbenchTabCreate, CommittedWorkbenchTabCreateMetadata, PendingWorkbenchTabCreate,
    PreparedWorkbenchTabCreate, PreparedWorkbenchTabSettlement, StagedWorkbenchTabCreateMetadata,
    WorkbenchSettlementTokenMismatch, WorkbenchTabCreateAdmission, WorkbenchTabCreateBegin,
    WorkbenchTabCreatePreparation, WorkbenchTabCreateRuntimeUnavailableReason,
};
pub use workspace_directory_label::{same_directory, without_trailing_separator};
mod terminal_panel_create;
pub use terminal_panel_create::{
    CommittedWorkbenchPanelCreateMetadata, PendingWorkbenchPanelCreate,
    PreparedWorkbenchPanelCreate, PreparedWorkbenchPanelCreateSettlement,
    StagedWorkbenchPanelCreateMetadata, WorkbenchPanelCreateAdmission, WorkbenchPanelCreateBegin,
    WorkbenchPanelCreatePreparation, WorkbenchPanelCreateSettlementTokenMismatch,
};
mod terminal_binding_selection;
pub use terminal_binding_selection::WorkbenchBindingExposure;
mod terminal_tab_move;
pub use terminal_tab_move::{
    PreparedWorkbenchTabMove, WorkbenchTabMoveAdmission, WorkbenchTabMoveBegin,
    WorkbenchTabMovePreparation,
};
mod terminal_panel_placement;
pub use terminal_panel_placement::{
    PreparedWorkbenchPanelPlacement, WorkbenchPanelPlacementAdmission,
    WorkbenchPanelPlacementBegin, WorkbenchPanelPlacementPreparation,
};
mod terminal_panel_relocate;
pub use terminal_panel_relocate::{
    PreparedWorkbenchPanelRelocate, WorkbenchPanelRelocateAdmission, WorkbenchPanelRelocateBegin,
    WorkbenchPanelRelocatePreparation,
};
mod terminal_tab_close;
pub use terminal_tab_close::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch, PreparedWorkbenchTabClose, PreparedWorkbenchTabClosePublication,
    PreparedWorkbenchTabCloseResume, WorkbenchTabCloseAdmission, WorkbenchTabCloseBegin,
    WorkbenchTabCloseMemberRuntime, WorkbenchTabClosePreparation,
    WorkbenchTabClosePrimaryAdmission, WorkbenchTabClosePublicationError,
    WorkbenchTabCloseResolution, WorkbenchTabCloseResumeAdmission, WorkbenchTabCloseResumeBegin,
    WorkbenchTabCloseResumePreparation, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchTabCloseStepTarget,
    WorkbenchTabCloseTarget, WorkbenchWorkspaceCloseAdmission, WorkbenchWorkspaceCloseBegin,
    WorkbenchWorkspaceCloseResumeAdmission, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation,
};

const MAXIMUM_LEDGER_ENTRIES: usize = 4096;
const MAXIMUM_LEDGER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct WorkbenchGrant {
    session_id: SessionId,
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
    LaunchProfileCatalogRead(orchbee_workbench_protocol::WorkbenchLaunchProfileCatalogRead),
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
            WorkbenchRequest::LaunchProfileCatalogRead(intent) => Ok(Self(
                ReadOrNavigationRequest::LaunchProfileCatalogRead(intent),
            )),
            WorkbenchRequest::Command(command) => {
                let Some(action) = PureWorkbenchAction::from_action(&command.action) else {
                    return Err(WorkbenchRequest::Command(command));
                };
                Ok(Self(ReadOrNavigationRequest::Command { command, action }))
            }
            request @ (WorkbenchRequest::AgentView(_)
            | WorkbenchRequest::TabCloseRead(_)
            | WorkbenchRequest::TabCloseResume(_)
            | WorkbenchRequest::WorkspaceCloseRead(_)
            | WorkbenchRequest::WorkspaceCloseResume(_)
            | WorkbenchRequest::WorktreeListRead(_)
            | WorkbenchRequest::WorktreeOperation(_)
            | WorkbenchRequest::WorktreeQuery(_)
            | WorkbenchRequest::WorkspaceGroupClose(_)
            | WorkbenchRequest::TabSetClose(_)
            | WorkbenchRequest::TabDuplicate(_)
            | WorkbenchRequest::LayoutHistory(_)
            | WorkbenchRequest::SessionStop(_)
            | WorkbenchRequest::Configuration(_)
            | WorkbenchRequest::CloseOperationRead(_)
            | WorkbenchRequest::Settings(_)
            | WorkbenchRequest::TerminalEditor(_)
            | WorkbenchRequest::Plugin(_)
            | WorkbenchRequest::AgentIntegration(_)
            | WorkbenchRequest::WindowTitle(_)
            | WorkbenchRequest::Notification(_)
            | WorkbenchRequest::AgentManifests(_)
            | WorkbenchRequest::PanelAgent(_)
            | WorkbenchRequest::PanelMetadata(_)
            | WorkbenchRequest::WorkspaceMetadata(_)
            | WorkbenchRequest::ClientView(_)
            | WorkbenchRequest::WorkspaceGit(_)
            | WorkbenchRequest::CurrentDirectory(_)
            | WorkbenchRequest::AgentCatalog(_)
            | WorkbenchRequest::ConfiguredCommand(_)
            | WorkbenchRequest::Events(_)
            | WorkbenchRequest::PaneGraphics(_)) => Err(request),
        }
    }
}

impl WorkbenchGrant {
    #[must_use]
    pub fn read_only(session_id: SessionId) -> Self {
        Self {
            session_id,
            mutate_navigation: false,
            create_terminal: false,
            close_terminal: false,
            associate_directory: false,
        }
    }
    #[must_use]
    pub fn manage_navigation(session_id: SessionId) -> Self {
        Self {
            session_id,
            mutate_navigation: true,
            create_terminal: false,
            close_terminal: false,
            associate_directory: false,
        }
    }

    #[must_use]
    pub fn manage_navigation_and_terminal_create(session_id: SessionId) -> Self {
        Self {
            session_id,
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

/// The committed point-in-time membership slice a Worktree List request
/// resolves and later re-verifies: session identity, revision, and the registered
/// directory record (which carries its owning Worker identity).
#[derive(Clone, Debug)]
pub struct WorktreeListTarget {
    pub session_id: SessionId,
    pub revision: WorkbenchRevision,
    pub directory: Directory,
}

#[derive(Debug)]
pub enum WorktreeListResolution {
    Found(WorktreeListTarget),
    Missing,
    Unavailable,
}

struct SettledRequest {
    command: WorkbenchCommand,
    result: WorkbenchCommandResult,
}

enum RetainedRequest {
    Settled(Box<SettledRequest>),
    PendingTabCreate(Box<terminal_tab_create::PendingTabCreateEntry>),
    PendingPanelCreate(Box<terminal_panel_create::PendingPanelCreateEntry>),
    TabClose(Box<terminal_tab_close::CloseRecord>),
}

impl RetainedRequest {
    fn command(&self) -> &WorkbenchCommand {
        match self {
            Self::Settled(entry) => &entry.command,
            Self::PendingTabCreate(entry) => &entry.command,
            Self::PendingPanelCreate(entry) => &entry.command,
            Self::TabClose(entry) => &entry.command,
        }
    }

    fn replay_result(&self) -> &WorkbenchCommandResult {
        match self {
            Self::Settled(entry) => &entry.result,
            Self::PendingTabCreate(entry) => &entry.pending_result,
            Self::PendingPanelCreate(entry) => &entry.pending_result,
            Self::TabClose(entry) => &entry.result,
        }
    }
}

pub struct WorkbenchAuthority {
    projection: WorkbenchProjectionStore,
    last_identity: u128,
    ledger: BTreeMap<WorkbenchRequestId, RetainedRequest>,
    ledger_bytes: usize,
    /// The per-Session ephemeral Launch Profile catalog: private Server state
    /// that never enters the public snapshot, journaled only through the same
    /// request ledger as every other command.
    catalog: Vec<LaunchProfileEntry>,
    /// Catalog-private revision, advanced only by committed create/delete —
    /// navigation mutations never touch it.
    catalog_revision: u64,
    retired: bool,
    worktrees: worktree_operation::WorktreeOperations,
    workspace_groups: workspace_group_close::WorkspaceGroupCloses,
    tab_sets: tab_set_close::TabSetCloses,
    tab_duplicates: tab_duplicate::TabDuplicates,
    workspace_metadata: workspace_metadata::WorkspaceMetadata,
    panel_metadata: panel_metadata::PanelMetadata,
    panel_agent: panel_agent::PanelAgents,
    plugin_panes: plugin_panes::Membership,
    // Restart intent, consumed only after a fresh physical binding is admitted.
    restart_directories: BTreeMap<
        orchbee_workbench_protocol::TerminalPresentationId,
        orchbee_workbench_protocol::WorkbenchDirectoryPath,
    >,
    // Kept recent output each restored Presentation replays when it starts; a capture carries an
    // entry only while its Presentation is still unbound.
    restart_outputs: BTreeMap<orchbee_workbench_protocol::TerminalPresentationId, u64>,
    // Pending Agent resume intent: the driver takes an entry when it attempts it, and a capture
    // carries an entry nobody has taken. Taking one never changes the Workbench revision.
    restart_agent_sessions:
        BTreeMap<orchbee_workbench_protocol::TerminalPresentationId, RetainedAgentSession>,
    // The saved identity each reopened Workspace and Tab stands in for.
    layout_lineage: checkpoint::LayoutLineage,
}

impl WorkbenchAuthority {
    pub fn new(session_id: SessionId) -> Result<Self, ContractError> {
        let snapshot = Snapshot {
            protocol_version: ProtocolVersion,
            revision: WorkbenchRevision::new(0),
            session_id,
            navigation: Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: Vec::new(),
                workspace_order: Vec::new(),
                projects: Vec::new(),
            },
            terminal_contexts: Vec::new(),
        };
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot))?;
        Ok(Self {
            projection: WorkbenchProjectionStore::load(&encoded)?,
            last_identity: 1,
            ledger: BTreeMap::new(),
            ledger_bytes: 0,
            catalog: Vec::new(),
            catalog_revision: 0,
            retired: false,
            worktrees: worktree_operation::WorktreeOperations::default(),
            workspace_groups: workspace_group_close::WorkspaceGroupCloses::default(),
            tab_sets: tab_set_close::TabSetCloses::default(),
            tab_duplicates: tab_duplicate::TabDuplicates::default(),
            workspace_metadata: workspace_metadata::WorkspaceMetadata::default(),
            panel_metadata: panel_metadata::PanelMetadata::default(),
            panel_agent: panel_agent::PanelAgents::default(),
            plugin_panes: plugin_panes::Membership::default(),
            restart_directories: BTreeMap::new(),
            restart_outputs: BTreeMap::new(),
            restart_agent_sessions: BTreeMap::new(),
            layout_lineage: checkpoint::LayoutLineage::default(),
        })
    }

    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.projection.snapshot().session_id
    }

    /// The committed projection head. Callers already hold the coordinator
    /// lock, so the returned snapshot is a consistent join with any retained
    /// terminal facts resolved under the same acquisition.
    #[must_use]
    pub fn snapshot(&self) -> &Snapshot {
        self.projection.snapshot()
    }
    /// Shared ephemeral visual owner; production consumers hold the canonical owner before this lock.
    pub fn graphics_owner(&self) -> std::sync::Arc<std::sync::Mutex<crate::PaneGraphicsAuthority>> {
        self.projection.graphics()
    }
    pub fn event_journal(&self) -> crate::SessionEventJournal {
        self.projection.events()
    }

    /// Resolves the stable Directory identity for the Worktree
    /// List path against the projection snapshot; `Unavailable` means the
    /// authority itself is retired, `Missing` means absent membership.
    pub fn worktree_list_target(
        &self,
        directory_id: &WorkbenchDirectoryId,
    ) -> WorktreeListResolution {
        if self.retired {
            return WorktreeListResolution::Unavailable;
        }
        let snapshot = self.projection.snapshot();
        let Some(directory) = snapshot.navigation.directory(directory_id) else {
            return WorktreeListResolution::Missing;
        };
        WorktreeListResolution::Found(WorktreeListTarget {
            session_id: snapshot.session_id.clone(),
            revision: snapshot.revision,
            directory: directory.clone(),
        })
    }

    pub fn execute(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        request: WorkbenchReadOrNavigationRequest,
    ) -> WorkbenchResponse {
        match request.0 {
            ReadOrNavigationRequest::SnapshotRead(intent) => {
                let authorized = grant.is_some_and(|grant| grant.session_id == *self.session_id());
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
                let authorized = grant.is_some_and(|grant| grant.session_id == *self.session_id());
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
            ReadOrNavigationRequest::LaunchProfileCatalogRead(intent) => {
                let authorized = grant.is_some_and(|grant| grant.session_id == *self.session_id());
                let outcome = if !authorized {
                    LaunchProfileCatalogOutcome::Denied {}
                } else if self.retired {
                    LaunchProfileCatalogOutcome::Unavailable {
                        reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                    }
                } else {
                    match self.catalog_outcome(intent.selector.as_ref()) {
                        Ok(outcome) => outcome,
                        Err(()) => LaunchProfileCatalogOutcome::Unavailable {
                            reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                        },
                    }
                };
                WorkbenchResponse::LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult {
                    protocol_version: ProtocolVersion,
                    request_id: intent.request_id,
                    outcome,
                })
            }
            ReadOrNavigationRequest::Command { command, action } => {
                let authorized = grant.is_some_and(|grant| {
                    grant.session_id == *self.session_id() && grant.mutate_navigation
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

    /// Resolves the catalog read under the same lock that owns the catalog:
    /// `None` lists every entry, `Some` resolves exactly one profile by
    /// id-first selector semantics. The returned outcome is preflighted
    /// through the typed response serializer before emission, so an
    /// encodable-but-oversize catalog degrades to `Unavailable` instead of
    /// escaping the frame bound.
    fn catalog_outcome(
        &self,
        selector: Option<&LaunchProfileSelector>,
    ) -> Result<LaunchProfileCatalogOutcome, ()> {
        let outcome = match selector {
            None => LaunchProfileCatalogOutcome::Catalog {
                session_id: self.session_id().clone(),
                catalog_revision: LaunchProfileCatalogRevision::new(self.catalog_revision),
                entries: self.catalog.clone(),
            },
            Some(selector) => match resolve_profile_selector(&self.catalog, selector) {
                ProfileSelectorResolution::Found(entry) => LaunchProfileCatalogOutcome::Profile {
                    session_id: self.session_id().clone(),
                    catalog_revision: LaunchProfileCatalogRevision::new(self.catalog_revision),
                    profile: entry.clone(),
                },
                ProfileSelectorResolution::NotFound => LaunchProfileCatalogOutcome::NotFound {},
                ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                    LaunchProfileCatalogOutcome::Ambiguous { candidate_ids }
                }
            },
        };
        self.catalog_result_fits(&outcome)
            .then_some(outcome)
            .ok_or(())
    }

    /// Serializes a candidate catalog outcome through the full typed response
    /// envelope — the same serialize-before-emit proof `publication_batch_fits`
    /// performs — so the dedicated variant cap is verified, never assumed.
    fn catalog_result_fits(&self, outcome: &LaunchProfileCatalogOutcome) -> bool {
        let Ok(request_id) = WorkbenchRequestId::parse("00000000000000000000000000000001") else {
            return false;
        };
        serialize_workbench_response(&WorkbenchResponse::LaunchProfileCatalog(
            WorkbenchLaunchProfileCatalogResult {
                protocol_version: ProtocolVersion,
                request_id,
                outcome: outcome.clone(),
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
            Err(PlanFailure::Rejected(reason)) => self.settled(self.rejected(reason)),
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
            catalog_next: plan.catalog_next,
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
        self.retain_outcome(command, self.rejected(reason))
    }

    /// Journals a zero-mutation settled outcome — a typed resolution failure
    /// such as `LaunchProfileUnavailable` — so an identical request replay
    /// returns the same retained result instead of re-resolving.
    pub(crate) fn retain_outcome(
        &mut self,
        command: WorkbenchCommand,
        outcome: WorkbenchCommandOutcome,
    ) -> WorkbenchCommandResult {
        match self.prepare_command_commit(command, self.settled(outcome)) {
            Ok(prepared) => prepared.commit(),
            Err(result) => *result,
        }
    }

    fn settled(&self, outcome: WorkbenchCommandOutcome) -> CommitPlan {
        CommitPlan {
            projection: None,
            last_identity: None,
            catalog_next: None,
            outcome,
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
        if command.session_id != current.session_id {
            return reject(WorkbenchRejectionReason::SessionMismatch);
        }
        if command.expected_revision != current.revision {
            return reject(WorkbenchRejectionReason::RevisionMismatch);
        }
        let mut candidate = current.clone();
        let mut last_identity = self.last_identity;
        let mut catalog_next = None;
        let outcome =
            match action {
                PureWorkbenchAction::TabCreateLayout {
                    terminal_context_id,
                    label,
                    automatic_label,
                    layout,
                } => {
                    if self.open_close_fences_context(&terminal_context_id) {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    let context = candidate
                        .terminal_contexts
                        .iter_mut()
                        .find(|context| context.terminal_context_id == terminal_context_id)
                        .ok_or(PlanFailure::Rejected(
                            WorkbenchRejectionReason::TerminalContextUnavailable,
                        ))?;
                    let tab_id = layout_create::materialize(
                        context,
                        &layout,
                        &label,
                        automatic_label,
                        &mut last_identity,
                    )?;
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::TabLayoutCreated {
                        revision,
                        terminal_context_id,
                        tab_id,
                    }
                }
                PureWorkbenchAction::ProjectCreate { label } => {
                    if candidate.navigation.projects.len() >= MAXIMUM_PROJECTS {
                        return reject(WorkbenchRejectionReason::ResourceLimit);
                    }
                    last_identity = allocate(last_identity, 1)?;
                    let project_id = WorkbenchProjectId::parse(&identity(last_identity))
                        .map_err(|_error| PlanFailure::Internal)?;
                    let revision = self.next_revision()?;
                    if !candidate.navigation.project_order.is_empty() {
                        candidate.navigation.project_order.push(project_id.clone());
                    }
                    candidate.navigation.projects.push(Project {
                        project_id: project_id.clone(),
                        label: label.as_str().to_owned(),
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
                    workspace_organization::admit_project_member(
                        &candidate.navigation,
                        project_id.as_ref(),
                    )?;
                    if candidate.terminal_contexts.len() >= MAXIMUM_TERMINAL_CONTEXTS {
                        return reject(WorkbenchRejectionReason::ResourceLimit);
                    }
                    last_identity = allocate(last_identity, 2)?;
                    let workspace_id = WorkbenchWorkspaceId::parse(&identity(last_identity - 1))
                        .map_err(|_error| PlanFailure::Internal)?;
                    let terminal_context_id =
                        WorkbenchTerminalContextId::parse(&identity(last_identity))
                            .map_err(|_error| PlanFailure::Internal)?;
                    let revision = self.next_revision()?;
                    candidate.navigation.workspaces.push(Workspace {
                        terminal_directory: None,
                        automatic_label: false,
                        project_id,
                        workspace_id: workspace_id.clone(),
                        primary_directory_id: None,
                        terminal_context_id: terminal_context_id.clone(),
                        label: label.as_str().to_owned(),
                    });
                    workspace_organization::append_workspace_order(
                        &mut candidate.navigation,
                        &workspace_id,
                    );
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
                    workspace_id,
                    label,
                } => {
                    let workspace = candidate
                        .navigation
                        .workspaces
                        .iter_mut()
                        .find(|workspace| workspace.workspace_id == workspace_id)
                        .ok_or(PlanFailure::Rejected(
                            WorkbenchRejectionReason::WorkspaceUnavailable,
                        ))?;
                    // A rename fixes an automatic label, so equal text still changes its mode.
                    if workspace.label == label.as_str() && !workspace.automatic_label {
                        return Ok(self.unchanged());
                    }
                    let revision = self.next_revision()?;
                    workspace.label = label.as_str().to_owned();
                    workspace.automatic_label = false;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::Renamed { revision }
                }
                PureWorkbenchAction::WorkspaceSetProject {
                    workspace_id,
                    project_id,
                } => {
                    let workspace = candidate.navigation.workspace(&workspace_id).ok_or(
                        PlanFailure::Rejected(WorkbenchRejectionReason::WorkspaceUnavailable),
                    )?;
                    if workspace.project_id == project_id {
                        return Ok(self.unchanged());
                    }
                    if self.workspace_organization_busy(&workspace_id) {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    workspace_organization::admit_project_member(
                        &candidate.navigation,
                        project_id.as_ref(),
                    )?;
                    candidate
                        .navigation
                        .workspaces
                        .iter_mut()
                        .find(|workspace| workspace.workspace_id == workspace_id)
                        .ok_or(PlanFailure::Internal)?
                        .project_id = project_id.clone();
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::WorkspaceProjectChanged {
                        revision,
                        workspace_id,
                        project_id,
                    }
                }
                PureWorkbenchAction::WorkspaceCreateProject {
                    workspace_id,
                    label,
                } => {
                    if candidate.navigation.workspace(&workspace_id).is_none() {
                        return reject(WorkbenchRejectionReason::WorkspaceUnavailable);
                    }
                    if self.workspace_organization_busy(&workspace_id) {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    if candidate.navigation.projects.len() >= MAXIMUM_PROJECTS {
                        return reject(WorkbenchRejectionReason::ResourceLimit);
                    }
                    let project_id = workspace_organization::PreparedPlacement::Create {
                        label,
                        source: Some(workspace_id.clone()),
                    }
                    .apply(&mut candidate.navigation, &mut last_identity)?;
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::WorkspaceProjectChanged {
                        revision,
                        workspace_id,
                        project_id,
                    }
                }
                PureWorkbenchAction::ProjectDissolve { project_id } => {
                    if candidate.navigation.project(&project_id).is_none() {
                        return reject(WorkbenchRejectionReason::ProjectUnavailable);
                    }
                    if candidate
                        .navigation
                        .workspaces_in_project(Some(&project_id))
                        .any(|workspace| self.workspace_organization_busy(&workspace.workspace_id))
                    {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    candidate
                        .navigation
                        .projects
                        .retain(|project| project.project_id != project_id);
                    candidate
                        .navigation
                        .project_order
                        .retain(|id| id != &project_id);
                    for workspace in &mut candidate.navigation.workspaces {
                        if workspace.project_id.as_ref() == Some(&project_id) {
                            workspace.project_id = None;
                        }
                    }
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::ProjectDissolved {
                        revision,
                        project_id,
                    }
                }
                action @ (PureWorkbenchAction::WorkspaceMove { .. }
                | PureWorkbenchAction::WorkspaceMoveBlock { .. }) => {
                    let Some(order) = workspace_order::reorder(&candidate.navigation, &action)?
                    else {
                        return Ok(self.unchanged());
                    };
                    let revision = self.next_revision()?;
                    candidate.navigation.workspace_order = order.ids().to_vec();
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::WorkspacesReordered {
                        revision,
                        workspace_order: order,
                    }
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
                    if !tab.automatic_label && tab.label == label.as_str() {
                        return Ok(self.unchanged());
                    }
                    let revision = self.next_revision()?;
                    tab.label = label.as_str().to_owned();
                    tab.automatic_label = false;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::Renamed { revision }
                }
                PureWorkbenchAction::NavigationConfigure { operation } => {
                    if let orchbee_workbench_protocol::NavigationConfiguration::MoveWorkspace {
                        workspace_id,
                        ..
                    } = &operation
                        && self.workspace_organization_busy(workspace_id)
                    {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    let Some(navigation) = orchbee_workbench_protocol::configure_navigation(
                        &candidate.navigation,
                        &operation,
                    )
                    .map_err(PlanFailure::Rejected)?
                    else {
                        return Ok(self.unchanged());
                    };
                    candidate.navigation = navigation;
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::NavigationConfigured { revision }
                }
                PureWorkbenchAction::TabsConfigure {
                    terminal_context_id,
                    operation,
                } => {
                    if self.open_close_covers_context(&terminal_context_id) {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    let context = candidate
                        .terminal_contexts
                        .iter_mut()
                        .find(|context| context.terminal_context_id == terminal_context_id)
                        .ok_or(PlanFailure::Rejected(
                            WorkbenchRejectionReason::TerminalContextUnavailable,
                        ))?;
                    let group_id = if operation.creates_group() {
                        last_identity = allocate(last_identity, 1)?;
                        Some(
                            orchbee_workbench_protocol::WorkbenchTabGroupId::parse(&identity(
                                last_identity,
                            ))
                            .map_err(|_| PlanFailure::Internal)?,
                        )
                    } else {
                        None
                    };
                    let Some(configured) = orchbee_workbench_protocol::configure_tabs(
                        context,
                        &operation,
                        group_id.as_ref(),
                    )
                    .map_err(PlanFailure::Rejected)?
                    else {
                        return Ok(self.unchanged());
                    };
                    *context = configured;
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::TabsConfigured { revision, group_id }
                }
                PureWorkbenchAction::PanelRename {
                    terminal_context_id,
                    tab_id,
                    panel_id,
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
                    let presentation = panel_primary_mut(&mut tab.root, &panel_id).ok_or(
                        PlanFailure::Rejected(WorkbenchRejectionReason::PanelUnavailable),
                    )?;
                    if presentation.custom_label.as_ref() == label.custom_label() {
                        return Ok(self.unchanged());
                    }
                    let revision = self.next_revision()?;
                    presentation.custom_label = label.custom_label().cloned();
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::Renamed { revision }
                }
                PureWorkbenchAction::PanelInputSet {
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    right_click,
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
                    let orchbee_workbench_protocol::PanelNode::Panel {
                        right_click: policy,
                        ..
                    } = panel_mut(&mut tab.root, &panel_id).ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::PanelUnavailable,
                    ))?
                    else {
                        return Err(PlanFailure::Internal);
                    };
                    if *policy == right_click {
                        return Ok(self.unchanged());
                    }
                    let revision = self.next_revision()?;
                    *policy = right_click;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::PanelInputConfigured { revision }
                }
                PureWorkbenchAction::SplitRatioSet {
                    terminal_context_id,
                    tab_id,
                    split,
                    previous_ratio,
                    ratio,
                } => {
                    if self.open_close_fences_context(&terminal_context_id) {
                        return Ok(self.settled(unavailable(
                            WorkbenchCommandUnavailableReason::TerminalBusy,
                        )));
                    }
                    if !split_ratios::set(
                        &mut candidate,
                        &terminal_context_id,
                        &tab_id,
                        &split,
                        previous_ratio,
                        ratio,
                    )? {
                        return Ok(self.unchanged());
                    }
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    WorkbenchCommandOutcome::SplitRatioConfigured { revision }
                }
                PureWorkbenchAction::LaunchProfileCreate { name, launch } => {
                    if self.catalog.len() >= MAXIMUM_LAUNCH_PROFILE_ENTRIES {
                        return reject(WorkbenchRejectionReason::ResourceLimit);
                    }
                    let catalog_revision =
                        self.catalog_revision
                            .checked_add(1)
                            .ok_or(PlanFailure::Rejected(
                                WorkbenchRejectionReason::RevisionExhausted,
                            ))?;
                    last_identity = allocate(last_identity, 1)?;
                    let profile_id = LaunchProfileId::parse(&format!(
                        "{}:{}",
                        current.session_id.as_str(),
                        identity(last_identity)
                    ))
                    .map_err(|_error| PlanFailure::Internal)?;
                    let mut entries = self.catalog.clone();
                    entries.push(LaunchProfileEntry {
                        profile_id: profile_id.clone(),
                        name,
                        launch,
                    });
                    if catalog_serialized_size(&entries)
                        .is_none_or(|size| size > MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES)
                    {
                        return reject(WorkbenchRejectionReason::ResourceLimit);
                    }
                    let revision = self.next_revision()?;
                    candidate.revision = revision;
                    catalog_next = Some((entries, catalog_revision));
                    WorkbenchCommandOutcome::LaunchProfileCreated {
                        revision,
                        catalog_revision: LaunchProfileCatalogRevision::new(catalog_revision),
                        profile_id,
                    }
                }
                PureWorkbenchAction::LaunchProfileDelete { selector } => {
                    match resolve_profile_selector(&self.catalog, &selector) {
                        ProfileSelectorResolution::NotFound => {
                            return Ok(
                                self.settled(WorkbenchCommandOutcome::LaunchProfileUnavailable {})
                            );
                        }
                        ProfileSelectorResolution::Ambiguous(candidate_ids) => {
                            return Ok(self.settled(
                                WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids },
                            ));
                        }
                        ProfileSelectorResolution::Found(entry) => {
                            let catalog_revision = self.catalog_revision.checked_add(1).ok_or(
                                PlanFailure::Rejected(WorkbenchRejectionReason::RevisionExhausted),
                            )?;
                            let profile_id = entry.profile_id.clone();
                            let mut entries = self.catalog.clone();
                            entries.retain(|item| item.profile_id != profile_id);
                            let revision = self.next_revision()?;
                            candidate.revision = revision;
                            catalog_next = Some((entries, catalog_revision));
                            WorkbenchCommandOutcome::LaunchProfileDeleted {
                                revision,
                                catalog_revision: LaunchProfileCatalogRevision::new(
                                    catalog_revision,
                                ),
                                profile_id,
                            }
                        }
                    }
                }
            };
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => {
                    PlanFailure::Rejected(WorkbenchRejectionReason::ResourceLimit)
                }
                ContractError::InvalidDocument | ContractError::WrongDocumentKind => {
                    PlanFailure::Internal
                }
            },
        )?;
        let projection = self
            .projection
            .prepare_advance(&encoded)
            .map_err(|_error| PlanFailure::Internal)?;
        Ok(CommitPlan {
            projection: Some(projection),
            last_identity: Some(last_identity),
            catalog_next,
            outcome,
        })
    }

    fn unchanged(&self) -> CommitPlan {
        self.settled(WorkbenchCommandOutcome::Unchanged {
            revision: self.projection.snapshot().revision,
        })
    }
}

struct CommitPlan {
    projection: Option<PreparedProjectionAdvance>,
    last_identity: Option<u128>,
    /// The private catalog replacement committed atomically with the rest of
    /// the plan: present only for committed profile create/delete.
    catalog_next: Option<(Vec<LaunchProfileEntry>, u64)>,
    outcome: WorkbenchCommandOutcome,
}

struct PreparedCommandCommit<'a> {
    authority: &'a mut WorkbenchAuthority,
    projection: Option<PreparedProjectionAdvance>,
    last_identity: Option<u128>,
    catalog_next: Option<(Vec<LaunchProfileEntry>, u64)>,
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
        if let Some((catalog, catalog_revision)) = self.catalog_next {
            self.authority.catalog = catalog;
            self.authority.catalog_revision = catalog_revision;
        }
        self.authority.ledger_bytes = self.next_bytes;
        self.authority
            .ledger
            .insert(self.request_id, RetainedRequest::Settled(self.retained));
        self.response
    }
}
fn panel_mut<'a>(
    node: &'a mut orchbee_workbench_protocol::PanelNode,
    panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
) -> Option<&'a mut orchbee_workbench_protocol::PanelNode> {
    use orchbee_workbench_protocol::PanelNode;
    match node {
        PanelNode::Panel {
            panel_id: candidate,
            ..
        } if candidate == panel_id => Some(node),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .find_map(|slot| panel_mut(&mut slot.node, panel_id)),
    }
}
fn panel_primary_mut<'a>(
    node: &'a mut orchbee_workbench_protocol::PanelNode,
    panel_id: &orchbee_workbench_protocol::WorkbenchPanelId,
) -> Option<&'a mut orchbee_workbench_protocol::Presentation> {
    match panel_mut(node, panel_id)? {
        orchbee_workbench_protocol::PanelNode::Panel {
            primary_presentation,
            ..
        } => Some(primary_presentation),
        orchbee_workbench_protocol::PanelNode::Split { .. } => None,
    }
}

enum PlanFailure {
    Rejected(WorkbenchRejectionReason),
    Internal,
}

enum PureWorkbenchAction {
    NavigationConfigure {
        operation: orchbee_workbench_protocol::NavigationConfiguration,
    },
    TabsConfigure {
        terminal_context_id: WorkbenchTerminalContextId,
        operation: orchbee_workbench_protocol::TabConfiguration,
    },
    TabCreateLayout {
        terminal_context_id: WorkbenchTerminalContextId,
        label: WorkbenchLabel,
        automatic_label: bool,
        layout: orchbee_workbench_protocol::WorkbenchLayoutTemplate,
    },
    WorkspaceMove {
        workspace_id: WorkbenchWorkspaceId,
        insert_index: orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex,
    },
    WorkspaceMoveBlock {
        workspace_ids: orchbee_workbench_protocol::WorkbenchWorkspaceList,
        before_workspace_id: Option<WorkbenchWorkspaceId>,
    },
    ProjectCreate {
        label: WorkbenchLabel,
    },
    ProjectRename {
        project_id: WorkbenchProjectId,
        label: WorkbenchLabel,
    },
    WorkspaceCreate {
        project_id: Option<WorkbenchProjectId>,
        label: WorkbenchLabel,
    },
    WorkspaceRename {
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    WorkspaceSetProject {
        workspace_id: WorkbenchWorkspaceId,
        project_id: Option<WorkbenchProjectId>,
    },
    WorkspaceCreateProject {
        workspace_id: WorkbenchWorkspaceId,
        label: WorkbenchLabel,
    },
    ProjectDissolve {
        project_id: WorkbenchProjectId,
    },
    TabRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        label: WorkbenchLabel,
    },
    PanelRename {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
        label: orchbee_workbench_protocol::PanelLabelUpdate,
    },
    PanelInputSet {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: orchbee_workbench_protocol::WorkbenchPanelId,
        right_click: orchbee_workbench_protocol::PanelRightClickTarget,
    },
    SplitRatioSet {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        split: orchbee_workbench_protocol::ClientSplitIdentity,
        previous_ratio: orchbee_workbench_protocol::WorkbenchSplitRatio,
        ratio: orchbee_workbench_protocol::WorkbenchSplitRatio,
    },
    LaunchProfileCreate {
        name: orchbee_workbench_protocol::LaunchProfileName,
        launch: orchbee_workbench_protocol::LaunchSelection,
    },
    LaunchProfileDelete {
        selector: LaunchProfileSelector,
    },
}

impl PureWorkbenchAction {
    fn from_action(action: &WorkbenchAction) -> Option<Self> {
        Some(match action {
            WorkbenchAction::NavigationConfigure { operation } => Self::NavigationConfigure {
                operation: operation.clone(),
            },
            WorkbenchAction::TabsConfigure {
                terminal_context_id,
                operation,
            } => Self::TabsConfigure {
                terminal_context_id: terminal_context_id.clone(),
                operation: operation.clone(),
            },
            WorkbenchAction::TabCreateLayout {
                terminal_context_id,
                label,
                automatic_label,
                layout,
            } => Self::TabCreateLayout {
                terminal_context_id: terminal_context_id.clone(),
                label: label.clone(),
                automatic_label: *automatic_label,
                layout: layout.clone(),
            },
            WorkbenchAction::WorkspaceMove {
                workspace_id,
                insert_index,
            } => Self::WorkspaceMove {
                workspace_id: workspace_id.clone(),
                insert_index: *insert_index,
            },
            WorkbenchAction::WorkspaceMoveBlock {
                workspace_ids,
                before_workspace_id,
            } => Self::WorkspaceMoveBlock {
                workspace_ids: workspace_ids.clone(),
                before_workspace_id: before_workspace_id.clone(),
            },
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
                workspace_id,
                label,
            } => Self::WorkspaceRename {
                workspace_id: workspace_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::WorkspaceSetProject {
                workspace_id,
                project_id,
            } => Self::WorkspaceSetProject {
                workspace_id: workspace_id.clone(),
                project_id: project_id.clone(),
            },
            WorkbenchAction::WorkspaceCreateProject {
                workspace_id,
                label,
            } => Self::WorkspaceCreateProject {
                workspace_id: workspace_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::ProjectDissolve { project_id } => Self::ProjectDissolve {
                project_id: project_id.clone(),
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
            WorkbenchAction::PanelRename {
                terminal_context_id,
                tab_id,
                panel_id,
                label,
            } => Self::PanelRename {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                label: label.clone(),
            },
            WorkbenchAction::PanelInputSet {
                terminal_context_id,
                tab_id,
                panel_id,
                right_click,
            } => Self::PanelInputSet {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                right_click: *right_click,
            },
            WorkbenchAction::SplitRatioSet {
                terminal_context_id,
                tab_id,
                split,
                previous_ratio,
                ratio,
            } => Self::SplitRatioSet {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                split: split.clone(),
                previous_ratio: *previous_ratio,
                ratio: *ratio,
            },
            WorkbenchAction::LaunchProfileCreate { name, launch } => Self::LaunchProfileCreate {
                name: name.clone(),
                launch: launch.clone(),
            },
            WorkbenchAction::LaunchProfileDelete { selector } => Self::LaunchProfileDelete {
                selector: selector.clone(),
            },
            WorkbenchAction::WorkspaceLaunch { .. }
            | WorkbenchAction::TabCreate { .. }
            | WorkbenchAction::PanelSplit { .. }
            | WorkbenchAction::PanelCreateTerminal { .. }
            | WorkbenchAction::PanelRestoreTerminal { .. }
            | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
            | WorkbenchAction::TabMove { .. }
            | WorkbenchAction::TabMoveIndex { .. }
            | WorkbenchAction::TabClose { .. }
            | WorkbenchAction::TabCloseIfIdle { .. }
            | WorkbenchAction::PanelClose { .. }
            | WorkbenchAction::PanelCloseIfIdle { .. }
            | WorkbenchAction::PanelMove { .. }
            | WorkbenchAction::PanelSwap { .. }
            | WorkbenchAction::PanelRelocate { .. }
            | WorkbenchAction::WorkspaceClose { .. }
            | WorkbenchAction::WorkspaceCloseIfIdle { .. }
            | WorkbenchAction::ProjectDissolveIfIdle { .. } => return None,
        })
    }
}

/// The selector contract shared by get/delete/use resolutions: an exact
/// profile id match wins first; otherwise the name's cardinality decides
/// not-found, unique, or ambiguous — and ambiguity carries only the bounded
/// candidate id list captured at this same resolution.
pub(crate) enum ProfileSelectorResolution<'a> {
    Found(&'a LaunchProfileEntry),
    NotFound,
    Ambiguous(Vec<LaunchProfileId>),
}

pub(crate) fn resolve_profile_selector<'a>(
    catalog: &'a [LaunchProfileEntry],
    selector: &LaunchProfileSelector,
) -> ProfileSelectorResolution<'a> {
    if let Some(entry) = catalog
        .iter()
        .find(|entry| entry.profile_id.as_str() == selector.as_str())
    {
        return ProfileSelectorResolution::Found(entry);
    }
    let mut named = catalog
        .iter()
        .filter(|entry| entry.name.as_str() == selector.as_str());
    let Some(entry) = named.next() else {
        return ProfileSelectorResolution::NotFound;
    };
    if named.next().is_none() {
        return ProfileSelectorResolution::Found(entry);
    }
    ProfileSelectorResolution::Ambiguous(
        catalog
            .iter()
            .filter(|entry| entry.name.as_str() == selector.as_str())
            .map(|entry| entry.profile_id.clone())
            .collect(),
    )
}

/// Exact serialized bytes of the catalog document, escaping included — the
/// `ledger_size` pattern applied to the private catalog candidate before any
/// live mutation.
fn catalog_serialized_size(entries: &[LaunchProfileEntry]) -> Option<usize> {
    serde_json::to_vec(entries)
        .ok()
        .map(|encoded| encoded.len())
}

/// The launch facts frozen at prepare when a command resolves a profile:
/// the entry's stored selection plus the provenance retained for later
/// publication — captured under the coordinator lock, immutable across the
/// asynchronous gap, and independent of any later catalog mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchProfileUse {
    pub launch: orchbee_workbench_protocol::LaunchSelection,
    pub profile_id: LaunchProfileId,
    pub catalog_revision: LaunchProfileCatalogRevision,
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
