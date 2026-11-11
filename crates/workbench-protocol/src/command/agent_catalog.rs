//! Reads the Session's startable Agent catalog: whether its machine can start Agents, the state of
//! the installed-Agent lookup its Worker runs, the providers started most recently, and one row per
//! startable provider with the name and command a Client shows. The read changes no Workbench
//! state; `refresh` only asks the Server for a new lookup.
use crate::{
    AgentStartSelector, ContractError, ProtocolVersion, SessionId, WorkbenchRequestId,
    WorkbenchRevision,
};
use orchbee_agent_observation_protocol::STARTABLE_AGENT_PROVIDER_IDS;
use serde::{Deserialize, Serialize};

/// Thirty-six rows of display text and the Recent list fit with room for the catalog to grow to
/// its 64-row bound.
pub const MAXIMUM_ENCODED_AGENT_CATALOG_BYTES: usize = 32 * 1024;
/// The most providers `recent` holds.
pub const MAXIMUM_RECENT_AGENTS: usize = 8;
/// The most rows a catalog may hold, whatever the catalog id set grows to.
pub const MAXIMUM_AGENT_CATALOG_ROWS: usize = 64;
const MAXIMUM_PRODUCT_NAME_BYTES: usize = 64;
const MAXIMUM_COMMAND_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AgentCatalogAction {
    /// Reads the catalog as it stands.
    Current {},
    /// Asks for a new lookup and answers at once with the catalog as it stands.
    Refresh {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCatalogRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub action: AgentCatalogAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCatalogLookupStatus {
    /// A lookup is in flight, including the first.
    Running,
    Complete,
    /// The last lookup missed its deadline, or its Worker refused it or was lost.
    Failed,
    /// No lookup runs on this machine's platform.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCatalogLookup {
    pub status: AgentCatalogLookupStatus,
    /// Unix milliseconds of the last completed lookup; `null` while none has completed.
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub checked_at: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCatalogEntryState {
    Found,
    /// The lookup missed it, or a start reported it missing since.
    NotFound,
    /// Its command is generic, such as `agent`, so finding it would not say the provider is
    /// installed.
    CantCheck,
    /// No lookup has completed yet.
    Checking,
    /// Its own query failed, or the lookup failed or is unsupported.
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCatalogEntry {
    pub provider_id: AgentStartSelector,
    /// The provider's product name, never translated.
    pub product_name: String,
    /// The line a start runs: the command, then its fixed arguments, joined by one space. Display
    /// only; no Client sends it back.
    pub command: String,
    pub state: AgentCatalogEntryState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchAgentCatalog {
    /// Starts at 1 and advances on any change of the other fields.
    pub revision: WorkbenchRevision,
    /// Whether this Session's machine can start Agents at all.
    pub startable: bool,
    pub lookup: AgentCatalogLookup,
    /// The providers started most recently, newest first, one each.
    pub recent: Vec<AgentStartSelector>,
    /// One row per startable provider, in catalog order.
    pub agents: Vec<AgentCatalogEntry>,
}

impl WorkbenchAgentCatalog {
    pub fn validate(&self) -> Result<(), ContractError> {
        let recent_unique = self
            .recent
            .iter()
            .enumerate()
            .all(|(index, provider)| !self.recent[..index].contains(provider));
        let checked_at = self
            .lookup
            .checked_at
            .is_none_or(|at| (1..=super::MAXIMUM_BINARY64_SAFE_INTEGER).contains(&at));
        let catalog_order = self.agents.len() <= MAXIMUM_AGENT_CATALOG_ROWS
            && self.agents.len() == STARTABLE_AGENT_PROVIDER_IDS.len()
            && self
                .agents
                .iter()
                .zip(STARTABLE_AGENT_PROVIDER_IDS)
                .all(|(row, id)| row.provider_id.agent().id() == *id);
        let rows = self
            .agents
            .iter()
            .all(|row| product_name(&row.product_name) && command(&row.command));
        if self.revision.value() == 0
            || !checked_at
            || self.recent.len() > MAXIMUM_RECENT_AGENTS
            || !recent_unique
            || !catalog_order
            || !rows
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

fn product_name(text: &str) -> bool {
    (1..=MAXIMUM_PRODUCT_NAME_BYTES).contains(&text.len()) && !text.chars().any(char::is_control)
}

fn command(text: &str) -> bool {
    (1..=MAXIMUM_COMMAND_BYTES).contains(&text.len())
        && text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCatalogFailure {
    Denied,
    /// The Session has no Worker yet, or the Server is stopping.
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentCatalogOutcome {
    Current { catalog: WorkbenchAgentCatalog },
    Failed { reason: AgentCatalogFailure },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentCatalogResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub outcome: AgentCatalogOutcome,
}

impl AgentCatalogResult {
    pub fn answers(&self, request: &AgentCatalogRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            AgentCatalogOutcome::Current { catalog } => catalog.validate(),
            AgentCatalogOutcome::Failed { .. } => Ok(()),
        }
    }
}
