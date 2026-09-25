//! Bounded Client-owned acknowledgements. Agent identity is canonical; seen is local.
use crate::ContractError;
use orchbee_agent_observation_protocol::{AgentChangesCursor, AgentSafeInteger, AgentSessionId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAXIMUM_CLIENT_ATTENTION_SESSIONS: usize = 100;
pub const MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES: usize = 64 * 1024;

pub(super) fn validate_sessions(sessions: &[AgentSessionId]) -> Result<(), ContractError> {
    if sessions.is_empty()
        || sessions.len() > MAXIMUM_CLIENT_ATTENTION_SESSIONS
        || sessions.iter().collect::<BTreeSet<_>>().len() != sessions.len()
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientAgentAttentionVersion {
    pub generation: AgentSafeInteger,
    pub sequence: AgentSafeInteger,
    pub state_change_revision: AgentSafeInteger,
    pub seen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientAgentAttentionEntry {
    pub session_id: AgentSessionId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub state: Option<ClientAgentAttentionVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientAgentAttentionSnapshot {
    pub cursor: AgentChangesCursor,
    pub revision: AgentSafeInteger,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub outer_focused: Option<bool>,
    pub entries: Vec<ClientAgentAttentionEntry>,
}

impl ClientAgentAttentionSnapshot {
    pub fn validate(&self) -> Result<(), ContractError> {
        let sessions: Vec<_> = self
            .entries
            .iter()
            .map(|entry| entry.session_id.clone())
            .collect();
        validate_sessions(&sessions)?;
        if self.entries.iter().any(|entry| {
            entry
                .state
                .as_ref()
                .is_some_and(|state| state.state_change_revision.value() > self.revision.value())
        }) {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }

    #[must_use]
    pub fn answers(&self, sessions: &[AgentSessionId]) -> bool {
        self.validate().is_ok()
            && self
                .entries
                .iter()
                .map(|entry| &entry.session_id)
                .eq(sessions)
    }
}
