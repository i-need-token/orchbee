//! Stage every fallible report change before the Names owner releases its alias.
use super::{PanelAgents, Resource, TerminalAgentBinding, WorkbenchAuthority, key, retained_bytes};
use orchbee_workbench_protocol::{PanelAgentFailure as Failure, PanelAgentSession};

pub struct PreparedManagedAgentClear<'a> {
    owner: &'a mut PanelAgents,
    next: Option<Resource>,
    bytes: usize,
    revision: u64,
}
impl PreparedManagedAgentClear<'_> {
    pub fn commit(self) {
        if let Some(resource) = self.next {
            // Retain an empty physical resource: an explicit clear must not fall back to restart intent.
            self.owner
                .resources
                .insert(key(&resource.physical.binding), resource);
            self.owner.bytes = self.bytes;
            self.owner.revision = self.revision;
        }
    }
}
impl WorkbenchAuthority {
    pub fn terminal_restart_session(
        &self,
        physical: &TerminalAgentBinding,
    ) -> Option<PanelAgentSession> {
        self.panel_agent.checkpoint_session(physical).flatten()
    }

    pub fn prepare_managed_agent_clear(
        &mut self,
        physical: &TerminalAgentBinding,
        owned: &PanelAgentSession,
    ) -> Result<PreparedManagedAgentClear<'_>, Failure> {
        if self.retired {
            return Err(Failure::BindingChanged);
        }
        let current = self.panel_agent.resources.get(&key(&physical.binding));
        let next = current.filter(|r| &r.physical == physical).and_then(|r| {
            r.reports
                .clear_owned_restart_session(owned)
                .map(|reports| (r, reports))
        });
        let mut bytes = self.panel_agent.bytes;
        let mut revision = self.panel_agent.revision;
        let resource = if let Some((prior, reports)) = next {
            let next_bytes = retained_bytes(&reports)?;
            bytes = bytes
                .checked_sub(prior.bytes)
                .and_then(|n| n.checked_add(next_bytes))
                .filter(|n| *n <= super::MAXIMUM_BYTES)
                .ok_or(Failure::ResourceLimit)?;
            revision = revision.checked_add(1).ok_or(Failure::RevisionExhausted)?;
            Some(Resource {
                physical: physical.clone(),
                reports,
                explicit_revision: revision,
                bytes: next_bytes,
            })
        } else {
            None
        };
        Ok(PreparedManagedAgentClear {
            owner: &mut self.panel_agent,
            next: resource,
            bytes,
            revision,
        })
    }
}
