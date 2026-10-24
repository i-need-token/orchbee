//! Completed managed-name restart intent; never a retained process or session identity.
use super::{WorkbenchCheckpointError as Error, agent_sessions::StoredAgentSession};
use orchbee_agent_observation_protocol::{
    AgentName, AgentProviderId, AgentReportProvider, MAXIMUM_NAME_BINDINGS,
};
use orchbee_workbench_protocol::TerminalPresentationId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedAgentRestart {
    pub presentation_id: TerminalPresentationId,
    pub name: AgentName,
    pub provider_id: AgentProviderId,
}

pub(super) fn validate(
    names: &[ManagedAgentRestart],
    sessions: &[StoredAgentSession],
) -> Result<(), Error> {
    if names.len() > MAXIMUM_NAME_BINDINGS {
        return Err(Error::InvalidManagedName);
    }
    let mut claimed_names = BTreeSet::new();
    let mut presentations = BTreeSet::new();
    for name in names {
        if !claimed_names.insert(&name.name) || !presentations.insert(&name.presentation_id) {
            return Err(Error::InvalidManagedName);
        }
        let provider = sessions
            .iter()
            .find(|session| session.presentation_id == name.presentation_id)
            .and_then(|session| AgentReportProvider::lookup(&session.session.agent));
        if provider
            .is_none_or(|provider| provider.native_provider_id() != name.provider_id.as_str())
        {
            return Err(Error::InvalidManagedName);
        }
    }
    Ok(())
}
