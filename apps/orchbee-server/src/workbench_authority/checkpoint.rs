//! Private retained configuration and provider restart intent, without physical runtime authority.

use std::{collections::BTreeSet, fmt};

use orchbee_workbench_protocol::{
    ContractError, LaunchProfileEntry, LaunchProfileId, LaunchProfileName, LaunchSelection,
    MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES, MAXIMUM_LAUNCH_PROFILE_ENTRIES, PanelNode, Presentation,
    ProtocolVersion, SiteId, Snapshot, TerminalContext, WorkbenchDirectoryPath,
    WorkbenchProtocolDocument, WorkbenchRevision, WorkerId, serialize_document,
};
use serde::{Deserialize, Serialize};

use super::{WorkbenchAuthority, catalog_serialized_size, identity};
use crate::workbench_projection::WorkbenchProjectionStore;

mod agent_sessions;
mod navigation;
use navigation::StoredNavigation;
mod managed_names;
pub use managed_names::ManagedAgentRestart;
mod directories;
mod restore;
pub use restore::{PreparedWorkbenchRestoreBinding, WorkbenchRestoreError, WorkbenchRestoreTarget};

pub const MAXIMUM_WORKBENCH_CHECKPOINT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchCheckpointError {
    RetiredAuthority,
    ForeignWorker,
    EncodedTooLarge {
        actual: usize,
        maximum: usize,
    },
    HistoricalMigrationTooLarge {
        version: u32,
        actual: usize,
        maximum: usize,
    },
    InvalidDocument,
    UnsupportedVersion(u32),
    InvalidProjection(ContractError),
    InvalidIdentity,
    InvalidCatalog,
    TerminalBinding,
    InvalidAgentSession,
    InvalidTerminalDirectory,
    InvalidManagedName,
    InvalidSplitRatio,
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
            Self::HistoricalMigrationTooLarge {
                version,
                actual,
                maximum,
            } => write!(
                formatter,
                "checkpoint version {version} migration requires {actual} Snapshot bytes; maximum is {maximum}; automatic restoration refused and committed checkpoints retained"
            ),
            Self::InvalidDocument => formatter.write_str("invalid Workbench checkpoint document"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported Workbench checkpoint version {version}"
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
            Self::InvalidManagedName => {
                formatter.write_str("invalid checkpoint managed Agent name")
            }
            Self::InvalidSplitRatio => {
                formatter.write_str("invalid checkpoint split ratio version")
            }
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
    terminal_contexts: Vec<TerminalContext>,
    profiles: Vec<StoredProfile>,
    #[serde(
        default,
        deserialize_with = "agent_sessions::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    agent_sessions: Option<Vec<agent_sessions::StoredAgentSession>>,
    #[serde(
        default,
        deserialize_with = "directories::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    terminal_directories: Option<Vec<directories::StoredDirectory>>,
    #[serde(
        default,
        deserialize_with = "managed_names::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    managed_names: Option<Vec<ManagedAgentRestart>>,
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
        self.checkpoint_with_agents(local_worker, &[])
    }

    /// Physical evidence is supplied under the Site gate, never reconstructed from saved data.
    pub fn checkpoint_with_agents(
        &self,
        local_worker: &WorkerId,
        facts: &[super::TerminalAgentBinding],
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        self.checkpoint_with_terminal_directories(local_worker, facts, &[])
    }

    /// The runtime supplies directory observations after exact physical-generation fencing.
    pub fn checkpoint_with_terminal_directories(
        &self,
        local_worker: &WorkerId,
        facts: &[super::TerminalAgentBinding],
        observed: &[(
            orchbee_workbench_protocol::PresentationBinding,
            WorkbenchDirectoryPath,
        )],
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        self.checkpoint_with_managed_agents(local_worker, facts, observed, &[])
    }

    /// Names supplies only completed claims already fenced against these physical facts.
    pub fn checkpoint_with_managed_agents(
        &self,
        local_worker: &WorkerId,
        facts: &[super::TerminalAgentBinding],
        observed: &[(
            orchbee_workbench_protocol::PresentationBinding,
            WorkbenchDirectoryPath,
        )],
        managed: &[ManagedAgentRestart],
    ) -> Result<Vec<u8>, WorkbenchCheckpointError> {
        if self.retired {
            return Err(WorkbenchCheckpointError::RetiredAuthority);
        }
        validate_catalog(&self.catalog)?;
        // Serialization alone does not validate the shared graph. Validate before recursion.
        let projection = validate_projection(self.snapshot().clone())?;
        let agent_sessions = agent_sessions::capture(self, local_worker, facts)?;
        managed_names::validate(managed, &agent_sessions)?;
        let terminal_directories = directories::capture(self, local_worker, observed)?;
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
                    .strip_prefix(self.site_id().as_str())
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
            version: 7,
            last_identity,
            navigation,
            terminal_contexts: snapshot.terminal_contexts,
            profiles,
            agent_sessions: Some(agent_sessions),
            terminal_directories: Some(terminal_directories),
            managed_names: Some(managed.to_vec()),
        };
        let encoded = serde_json::to_vec(&checkpoint)
            .map_err(|_error| WorkbenchCheckpointError::InvalidDocument)?;
        enforce_encoded_limit(&encoded)?;
        Ok(encoded)
    }

    /// The caller supplies a fresh boot Site and its admitted local Worker.
    pub fn from_checkpoint(
        encoded: &[u8],
        site_id: SiteId,
        local_worker: &WorkerId,
    ) -> Result<Self, WorkbenchCheckpointError> {
        WorkbenchCheckpoint::parse(encoded)?.reconstruct(site_id, local_worker)
    }
}

/// Fully validated retained configuration, admitted before creating runtime processes.
/// Its private records contain no live Site, Worker, terminal or controller authority.
pub struct WorkbenchCheckpoint {
    checkpoint: Checkpoint,
}

impl WorkbenchCheckpoint {
    /// Immutable intent is consumed by native startup planning before reconstruction.
    pub fn managed_names(&self) -> &[ManagedAgentRestart] {
        self.checkpoint.managed_names.as_deref().unwrap_or_default()
    }
    pub fn parse(encoded: &[u8]) -> Result<Self, WorkbenchCheckpointError> {
        enforce_encoded_limit(encoded)?;
        let checkpoint: Checkpoint = serde_json::from_slice(encoded)
            .map_err(|_error| WorkbenchCheckpointError::InvalidDocument)?;
        if !matches!(checkpoint.version, 1..=7) {
            return Err(WorkbenchCheckpointError::UnsupportedVersion(
                checkpoint.version,
            ));
        }
        if (checkpoint.version >= 2) != checkpoint.agent_sessions.is_some() {
            return Err(WorkbenchCheckpointError::InvalidAgentSession);
        }
        if (checkpoint.version >= 3) != checkpoint.terminal_directories.is_some() {
            return Err(WorkbenchCheckpointError::InvalidTerminalDirectory);
        }
        if (checkpoint.version >= 4) != checkpoint.managed_names.is_some() {
            return Err(WorkbenchCheckpointError::InvalidManagedName);
        }
        if checkpoint.version < 5
            && checkpoint.terminal_contexts.iter().any(|context| {
                context
                    .tabs
                    .iter()
                    .any(|tab| has_nondefault_ratio(&tab.root))
            })
        {
            return Err(WorkbenchCheckpointError::InvalidSplitRatio);
        }
        // These values only validate the shared graph shape. No authority is constructed,
        // retained or published; actual boot identities arrive during reconstruction.
        let validation_site = SiteId::parse("00000000000000000000000000000001")
            .expect("canonical nonzero validation identity");
        let validation_worker = WorkerId::parse("00000000000000000000000000000001")
            .expect("canonical nonzero validation identity");
        let _validated = checkpoint_parts(&checkpoint, &validation_site, &validation_worker)?;
        Ok(Self { checkpoint })
    }

    pub fn reconstruct(
        self,
        site_id: SiteId,
        local_worker: &WorkerId,
    ) -> Result<WorkbenchAuthority, WorkbenchCheckpointError> {
        let parts = checkpoint_parts(&self.checkpoint, &site_id, local_worker)?;
        let mut authority = WorkbenchAuthority::new(site_id)
            .map_err(WorkbenchCheckpointError::InvalidProjection)?;
        authority.projection = parts.projection;
        authority.last_identity = self.checkpoint.last_identity.0;
        authority.catalog = parts.catalog;
        authority.pending_agent_sessions = self
            .checkpoint
            .agent_sessions
            .unwrap_or_default()
            .into_iter()
            .map(|row| (row.presentation_id, row.session))
            .collect();
        authority.restart_directories = self
            .checkpoint
            .terminal_directories
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.presentation_id, r.path))
            .collect();
        Ok(authority)
    }
}

fn has_nondefault_ratio(node: &PanelNode) -> bool {
    match node {
        PanelNode::Panel { .. } => false,
        PanelNode::Split {
            children, ratio, ..
        } => {
            !ratio.is_half()
                || children
                    .iter()
                    .any(|child| has_nondefault_ratio(&child.node))
        }
    }
}

struct CheckpointParts {
    projection: WorkbenchProjectionStore,
    catalog: Vec<LaunchProfileEntry>,
}

fn checkpoint_parts(
    checkpoint: &Checkpoint,
    site_id: &SiteId,
    local_worker: &WorkerId,
) -> Result<CheckpointParts, WorkbenchCheckpointError> {
    if checkpoint.profiles.len() > MAXIMUM_LAUNCH_PROFILE_ENTRIES {
        return Err(WorkbenchCheckpointError::InvalidCatalog);
    }
    let catalog = checkpoint
        .profiles
        .iter()
        .map(|profile| {
            Ok(LaunchProfileEntry {
                profile_id: LaunchProfileId::parse(&format!(
                    "{}:{}",
                    site_id.as_str(),
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
        site_id: site_id.clone(),
        navigation: checkpoint
            .navigation
            .restore(checkpoint.version, local_worker)?,
        terminal_contexts: checkpoint.terminal_contexts.clone(),
    };
    let projection = validate_projection(snapshot).map_err(|error| match error {
        WorkbenchCheckpointError::InvalidProjection(ContractError::EncodedValueTooLarge {
            actual,
            maximum,
        }) if checkpoint.version <= 5 => WorkbenchCheckpointError::HistoricalMigrationTooLarge {
            version: checkpoint.version,
            actual,
            maximum,
        },
        other => other,
    })?;
    validate_identities(
        projection.snapshot(),
        &checkpoint.profiles,
        checkpoint.last_identity,
    )?;
    agent_sessions::validate(
        checkpoint.agent_sessions.as_deref().unwrap_or_default(),
        projection.snapshot(),
    )?;
    managed_names::validate(
        checkpoint.managed_names.as_deref().unwrap_or_default(),
        checkpoint.agent_sessions.as_deref().unwrap_or_default(),
    )?;
    directories::validate(
        checkpoint
            .terminal_directories
            .as_deref()
            .unwrap_or_default(),
        projection.snapshot(),
    )?;
    Ok(CheckpointParts {
        projection,
        catalog,
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
