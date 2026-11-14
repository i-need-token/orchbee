//! Private retained configuration, terminal directories, Agent session references and the Recent
//! list, without physical runtime authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use orchbee_workbench_protocol::{
    AgentStartSelector, ContractError, LaunchProfileEntry, LaunchProfileId, LaunchProfileName,
    LaunchSelection, MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES, MAXIMUM_LAUNCH_PROFILE_ENTRIES,
    MAXIMUM_RECENT_AGENTS, PanelNode, Presentation, PresentationBinding, ProtocolVersion,
    SessionId, Snapshot, TerminalContext, TerminalPresentationId, WorkbenchDirectoryPath,
    WorkbenchProtocolDocument, WorkbenchRevision, WorkerId, serialize_document,
};
use serde::{Deserialize, Serialize};

use super::{WorkbenchAuthority, catalog_serialized_size, identity};
use crate::workbench_projection::WorkbenchProjectionStore;

mod contexts;
use contexts::StoredContext;
mod navigation;
use navigation::StoredNavigation;
mod agent_sessions;
pub use agent_sessions::{AgentResumeProvider, RetainedAgentSession};
mod directories;
mod lineage;
pub(super) use lineage::LayoutLineage;
mod restore;
pub use restore::{PreparedWorkbenchRestoreBinding, WorkbenchRestoreError, WorkbenchRestoreTarget};

pub const MAXIMUM_WORKBENCH_CHECKPOINT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchCheckpointError {
    RetiredAuthority,
    ForeignWorker,
    EncodedTooLarge { actual: usize, maximum: usize },
    InvalidDocument,
    UnsupportedVersion(u32),
    InvalidProjection(ContractError),
    InvalidIdentity,
    InvalidCatalog,
    TerminalBinding,
    InvalidTerminalDirectory,
    InvalidAgentSession,
}

impl fmt::Display for WorkbenchCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RetiredAuthority => formatter.write_str("Workbench authority is retired"),
            Self::ForeignWorker => formatter.write_str("checkpoint contains a foreign Worker"),
            Self::EncodedTooLarge { actual, maximum } => write!(
                formatter,
                "checkpoint uses {actual} bytes; maximum is {maximum}"
            ),
            Self::InvalidDocument => formatter.write_str("invalid Workbench checkpoint document"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported Workbench checkpoint version {version}; expected version 1"
            ),
            Self::InvalidProjection(error) => error.fmt(formatter),
            Self::InvalidIdentity => {
                formatter.write_str("checkpoint identity allocation is invalid")
            }
            Self::InvalidCatalog => {
                formatter.write_str("checkpoint launch profile catalog is invalid")
            }
            Self::TerminalBinding => {
                formatter.write_str("checkpoint cannot retain terminal bindings")
            }
            Self::InvalidTerminalDirectory => {
                formatter.write_str("invalid checkpoint terminal directory")
            }
            Self::InvalidAgentSession => formatter.write_str("invalid checkpoint Agent session"),
        }
    }
}

impl std::error::Error for WorkbenchCheckpointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidProjection(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct Counter(u128);

impl TryFrom<String> for Counter {
    type Error = WorkbenchCheckpointError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Counter> for String {
    fn from(value: Counter) -> Self {
        identity(value.0)
    }
}

impl Counter {
    fn parse(value: &str) -> Result<Self, WorkbenchCheckpointError> {
        if value.len() != 32
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(WorkbenchCheckpointError::InvalidIdentity);
        }
        let counter = u128::from_str_radix(value, 16)
            .map_err(|_error| WorkbenchCheckpointError::InvalidIdentity)?;
        if counter == 0 {
            return Err(WorkbenchCheckpointError::InvalidIdentity);
        }
        Ok(Self(counter))
    }
}

#[derive(Serialize, Deserialize)]
enum Schema {
    #[serde(rename = "orchbee.workbench.checkpoint")]
    Workbench,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    schema: Schema,
    version: u32,
    last_identity: Counter,
    navigation: StoredNavigation,
    terminal_contexts: Vec<StoredContext>,
    profiles: Vec<StoredProfile>,
    terminal_directories: Vec<directories::StoredDirectory>,
    /// Always written; an omitted list reads as no references, while a present list is strict.
    #[serde(default)]
    agent_sessions: Vec<agent_sessions::StoredAgentSession>,
    /// Always written; an omitted list reads as no lineage, while a present list is strict.
    #[serde(default)]
    layout_lineage: Vec<lineage::StoredLineage>,
    /// The Session's Recent list, newest first. Always written; an omitted list reads as empty,
    /// while a present list is strict.
    #[serde(default)]
    recent_agents: Vec<AgentStartSelector>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredProfile {
    counter: Counter,
    name: LaunchProfileName,
    launch: LaunchSelection,
}

impl WorkbenchAuthority {
    /// Captures committed configuration without changing this authority or authorizing effects.
    pub fn checkpoint(&self, local_worker: &WorkerId) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        self.checkpoint_with_observations(local_worker, &[], None)
    }

    /// The runtime supplies directory and live Agent session observations after exact
    /// physical-generation fencing. `agent_sessions = None` means recording is off: no Agent
    /// session row is written, not even a pending restart reference.
    pub fn checkpoint_with_observations(
        &self,
        local_worker: &WorkerId,
        directories: &[(PresentationBinding, WorkbenchDirectoryPath)],
        agent_sessions: Option<&[(PresentationBinding, RetainedAgentSession)]>,
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        self.checkpoint_with_terminal_output(local_worker, directories, agent_sessions, None)
    }

    /// As [`Self::checkpoint_with_observations`], also naming each terminal's kept output token.
    /// `outputs = None` means replay is off: no row names output, not even a held one's.
    pub fn checkpoint_with_terminal_output(
        &self,
        local_worker: &WorkerId,
        directories: &[(PresentationBinding, WorkbenchDirectoryPath)],
        agent_sessions: Option<&[(PresentationBinding, RetainedAgentSession)]>,
        outputs: Option<&[(PresentationBinding, u64)]>,
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        self.checkpoint_with_recent_agents(local_worker, directories, agent_sessions, outputs, &[])
    }

    /// As [`Self::checkpoint_with_terminal_output`], also saving the Session's Recent list, newest
    /// first. The Agent catalog owns that list; this authority only carries it to disk.
    pub fn checkpoint_with_recent_agents(
        &self,
        local_worker: &WorkerId,
        directories: &[(PresentationBinding, WorkbenchDirectoryPath)],
        agent_sessions: Option<&[(PresentationBinding, RetainedAgentSession)]>,
        outputs: Option<&[(PresentationBinding, u64)]>,
        recent_agents: &[AgentStartSelector],
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        if self.retired {
            return Err(WorkbenchCheckpointError::RetiredAuthority);
        }
        validate_catalog(&self.catalog)?;
        validate_recent_agents(recent_agents)?;
        // Serialization alone does not validate the shared graph. Validate before recursion.
        let projection = validate_projection(self.snapshot().clone())?;
        let terminal_directories = directories::capture(self, local_worker, directories, outputs)?;
        let agent_sessions =
            agent_sessions::capture(self, local_worker, agent_sessions, &terminal_directories)?;
        let mut snapshot = projection.snapshot().clone();
        for context in &mut snapshot.terminal_contexts {
            for tab in &mut context.tabs {
                remove_local_bindings(&mut tab.root, local_worker)?;
            }
        }
        let profiles = self
            .catalog
            .iter()
            .map(|entry| {
                let counter = entry
                    .profile_id
                    .as_str()
                    .strip_prefix(self.session_id().as_str())
                    .and_then(|suffix| suffix.strip_prefix(':'))
                    .ok_or(WorkbenchCheckpointError::InvalidIdentity)?;
                Ok(StoredProfile {
                    counter: Counter::parse(counter)?,
                    name: entry.name.clone(),
                    launch: entry.launch.clone(),
                })
            })
            .collect::<Result<Vec<_>, WorkbenchCheckpointError>>()?;
        let last_identity = Counter::parse(&identity(self.last_identity))?;
        validate_identities(&snapshot, &profiles, last_identity)?;
        let navigation = StoredNavigation::capture(snapshot.navigation, local_worker)?;
        let checkpoint = Checkpoint {
            schema: Schema::Workbench,
            version: 1,
            last_identity,
            navigation,
            terminal_contexts: snapshot
                .terminal_contexts
                .into_iter()
                .map(StoredContext::from)
                .collect(),
            profiles,
            terminal_directories,
            agent_sessions,
            layout_lineage: self.layout_lineage.capture(projection.snapshot()),
            recent_agents: recent_agents.to_vec(),
        };
        let encoded = serde_json::to_vec(&checkpoint)
            .map_err(|_error| WorkbenchCheckpointError::InvalidDocument)?;
        enforce_encoded_limit(&encoded)?;
        Ok(encoded)
    }

    /// The caller supplies a fresh boot Session and its admitted local Worker.
    pub fn from_checkpoint(
        encoded: &[u8],
        session_id: SessionId,
        local_worker: &WorkerId,
    ) -> Result<Self, WorkbenchCheckpointError> {
        WorkbenchCheckpoint::parse(encoded)?.reconstruct(session_id, local_worker)
    }
}

/// Fully validated retained configuration, admitted before creating runtime processes.
/// Its private records contain no live Session, Worker, terminal or controller authority.
pub struct WorkbenchCheckpoint {
    checkpoint: Checkpoint,
}

impl WorkbenchCheckpoint {
    pub fn parse(encoded: &[u8]) -> Result<Self, WorkbenchCheckpointError> {
        enforce_encoded_limit(encoded)?;
        let checkpoint: Checkpoint = serde_json::from_slice(encoded)
            .map_err(|_error| WorkbenchCheckpointError::InvalidDocument)?;
        if checkpoint.version != 1 {
            return Err(WorkbenchCheckpointError::UnsupportedVersion(
                checkpoint.version,
            ));
        }
        // These values only validate the shared graph shape. No authority is constructed,
        // retained or published; actual boot identities arrive during reconstruction.
        let validation_session = validation_session();
        let validation_worker = WorkerId::parse("00000000000000000000000000000001")
            .expect("canonical nonzero validation identity");
        let _validated = checkpoint_parts(&checkpoint, &validation_session, &validation_worker)?;
        Ok(Self { checkpoint })
    }

    pub fn reconstruct(
        self,
        session_id: SessionId,
        local_worker: &WorkerId,
    ) -> Result<WorkbenchAuthority, WorkbenchCheckpointError> {
        let parts = checkpoint_parts(&self.checkpoint, &session_id, local_worker)?;
        let mut authority = WorkbenchAuthority::new(session_id)
            .map_err(WorkbenchCheckpointError::InvalidProjection)?;
        authority.projection = parts.projection;
        authority.last_identity = self.checkpoint.last_identity.0;
        authority.catalog = parts.catalog;
        authority.restart_outputs = self
            .checkpoint
            .terminal_directories
            .iter()
            .filter_map(|r| Some((r.presentation_id.clone(), r.output?.0)))
            .collect();
        authority.restart_directories = self
            .checkpoint
            .terminal_directories
            .into_iter()
            .map(|r| (r.presentation_id, r.path))
            .collect();
        authority.restart_agent_sessions = parts.agent_sessions;
        authority.layout_lineage = parts.layout_lineage;
        Ok(authority)
    }
}

impl WorkbenchCheckpoint {
    /// The Session's saved Recent list, newest first; empty when the checkpoint has none.
    #[must_use]
    pub fn recent_agents(&self) -> &[AgentStartSelector] {
        &self.checkpoint.recent_agents
    }

    /// Every kept terminal output token this checkpoint names; any other kept file is stale.
    #[must_use]
    pub fn terminal_output_tokens(&self) -> Vec<u64> {
        self.checkpoint
            .terminal_directories
            .iter()
            .filter_map(|row| row.output.map(|token| token.0))
            .collect()
    }
}

fn validation_session() -> SessionId {
    SessionId::parse("00000000000000000000000000000001")
        .expect("canonical nonzero validation identity")
}

struct CheckpointParts {
    projection: WorkbenchProjectionStore,
    catalog: Vec<LaunchProfileEntry>,
    agent_sessions: BTreeMap<TerminalPresentationId, RetainedAgentSession>,
    layout_lineage: LayoutLineage,
}

fn checkpoint_parts(
    checkpoint: &Checkpoint,
    session_id: &SessionId,
    local_worker: &WorkerId,
) -> Result<CheckpointParts, WorkbenchCheckpointError> {
    if checkpoint.profiles.len() > MAXIMUM_LAUNCH_PROFILE_ENTRIES {
        return Err(WorkbenchCheckpointError::InvalidCatalog);
    }
    validate_recent_agents(&checkpoint.recent_agents)?;
    let catalog = checkpoint
        .profiles
        .iter()
        .map(|profile| {
            Ok(LaunchProfileEntry {
                profile_id: LaunchProfileId::parse(&format!(
                    "{}:{}",
                    session_id.as_str(),
                    identity(profile.counter.0)
                ))
                .map_err(|_error| WorkbenchCheckpointError::InvalidIdentity)?,
                name: profile.name.clone(),
                launch: profile.launch.clone(),
            })
        })
        .collect::<Result<Vec<_>, WorkbenchCheckpointError>>()?;
    validate_catalog(&catalog)?;
    let snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(0),
        session_id: session_id.clone(),
        navigation: checkpoint.navigation.restore(local_worker)?,
        terminal_contexts: checkpoint
            .terminal_contexts
            .iter()
            .cloned()
            .map(TerminalContext::from)
            .collect(),
    };
    let projection = validate_projection(snapshot)?;
    validate_identities(
        projection.snapshot(),
        &checkpoint.profiles,
        checkpoint.last_identity,
    )?;
    directories::validate(&checkpoint.terminal_directories, projection.snapshot())?;
    let agent_sessions = agent_sessions::validate(
        &checkpoint.agent_sessions,
        projection.snapshot(),
        &checkpoint.terminal_directories,
    )?;
    let layout_lineage = LayoutLineage::restore(
        &checkpoint.layout_lineage,
        projection.snapshot(),
        checkpoint.last_identity.0,
    )?;
    Ok(CheckpointParts {
        projection,
        catalog,
        agent_sessions,
        layout_lineage,
    })
}
fn enforce_encoded_limit(encoded: &[u8]) -> Result<(), WorkbenchCheckpointError> {
    if encoded.len() > MAXIMUM_WORKBENCH_CHECKPOINT_BYTES {
        return Err(WorkbenchCheckpointError::EncodedTooLarge {
            actual: encoded.len(),
            maximum: MAXIMUM_WORKBENCH_CHECKPOINT_BYTES,
        });
    }
    Ok(())
}

fn validate_projection(
    snapshot: Snapshot,
) -> Result<WorkbenchProjectionStore, WorkbenchCheckpointError> {
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot))
        .map_err(WorkbenchCheckpointError::InvalidProjection)?;
    WorkbenchProjectionStore::load(&encoded).map_err(WorkbenchCheckpointError::InvalidProjection)
}

/// At most [`MAXIMUM_RECENT_AGENTS`] providers, each once.
fn validate_recent_agents(recent: &[AgentStartSelector]) -> Result<(), WorkbenchCheckpointError> {
    let unique = recent
        .iter()
        .enumerate()
        .all(|(index, agent)| !recent[..index].contains(agent));
    if recent.len() > MAXIMUM_RECENT_AGENTS || !unique {
        return Err(WorkbenchCheckpointError::InvalidDocument);
    }
    Ok(())
}

fn validate_catalog(catalog: &[LaunchProfileEntry]) -> Result<(), WorkbenchCheckpointError> {
    if catalog.len() > MAXIMUM_LAUNCH_PROFILE_ENTRIES
        || catalog_serialized_size(catalog)
            .is_none_or(|size| size > MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES)
    {
        return Err(WorkbenchCheckpointError::InvalidCatalog);
    }
    Ok(())
}

fn remove_local_bindings(
    node: &mut PanelNode,
    local_worker: &WorkerId,
) -> Result<(), WorkbenchCheckpointError> {
    match node {
        PanelNode::Split { children, .. } => {
            for child in children.iter_mut() {
                remove_local_bindings(&mut child.node, local_worker)?;
            }
        }
        PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } => {
            for presentation in std::iter::once(primary_presentation).chain(child_presentations) {
                if presentation
                    .binding
                    .as_ref()
                    .is_some_and(|binding| binding.worker_id != *local_worker)
                {
                    return Err(WorkbenchCheckpointError::ForeignWorker);
                }
                presentation.binding = None;
                presentation.launch_failure = None;
                presentation.launch_program = None;
                presentation.agent_not_resumed = None;
                presentation.agent_start_failure = None;
            }
        }
    }
    Ok(())
}

struct Identities {
    maximum: Counter,
    seen: BTreeSet<u128>,
}

impl Identities {
    fn insert(&mut self, value: &str) -> Result<(), WorkbenchCheckpointError> {
        let value = Counter::parse(value)?;
        if value.0 > self.maximum.0 || !self.seen.insert(value.0) {
            return Err(WorkbenchCheckpointError::InvalidIdentity);
        }
        Ok(())
    }

    fn presentation(
        &mut self,
        presentation: &Presentation,
    ) -> Result<(), WorkbenchCheckpointError> {
        if presentation.binding.is_some() {
            return Err(WorkbenchCheckpointError::TerminalBinding);
        }
        self.insert(presentation.presentation_id.as_str())
    }

    fn panel(&mut self, node: &PanelNode) -> Result<(), WorkbenchCheckpointError> {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => {
                self.insert(panel_id.as_str())?;
                self.presentation(primary_presentation)?;
                for presentation in child_presentations {
                    self.presentation(presentation)?;
                }
            }
            PanelNode::Split {
                split_id, children, ..
            } => {
                self.insert(split_id.as_str())?;
                for child in children.iter() {
                    self.insert(child.slot_id.as_str())?;
                    self.panel(&child.node)?;
                }
            }
        }
        Ok(())
    }
}

fn validate_identities(
    snapshot: &Snapshot,
    profiles: &[StoredProfile],
    last_identity: Counter,
) -> Result<(), WorkbenchCheckpointError> {
    let mut ids = Identities {
        maximum: last_identity,
        seen: BTreeSet::new(),
    };
    for project in &snapshot.navigation.projects {
        ids.insert(project.project_id.as_str())?;
    }
    for directory in &snapshot.navigation.directories {
        ids.insert(directory.directory_id.as_str())?;
    }
    for workspace in &snapshot.navigation.workspaces {
        ids.insert(workspace.workspace_id.as_str())?;
    }
    for context in &snapshot.terminal_contexts {
        ids.insert(context.terminal_context_id.as_str())?;
        for group in &context.groups {
            ids.insert(group.group_id.as_str())?;
        }
        for tab in &context.tabs {
            ids.insert(tab.tab_id.as_str())?;
            ids.panel(&tab.root)?;
        }
    }
    for profile in profiles {
        ids.insert(&identity(profile.counter.0))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
