//! The Session gate composes current physical membership/report ownership with the one Names leaf.
use super::SessionCoordinator;
use crate::{
    agent::native_observation::{Binding, CarrierIdentity},
    agent_service::{AgentService, TerminalNameFact},
};
use orchbee_server::{TerminalAgentBinding, WorkbenchTerminalInvariantError};
use std::{
    io,
    sync::{Arc, Weak},
};

impl SessionCoordinator {
    pub(crate) fn bind_agent_names(&mut self, agents: &Arc<AgentService>) -> io::Result<()> {
        if let Some(existing) = &self.agents {
            if !Weak::ptr_eq(existing, &Arc::downgrade(agents)) {
                return Err(io::Error::other("Names owner already bound"));
            }
        } else {
            self.agents = Some(Arc::downgrade(agents));
        }
        Ok(())
    }
    pub(crate) fn terminal_name_facts(
        &self,
    ) -> Result<Vec<TerminalNameFact>, WorkbenchTerminalInvariantError> {
        self.terminal_name_facts_for(None)
    }
    fn terminal_name_facts_for(
        &self,
        target: Option<&orchbee_workbench_protocol::PresentationBinding>,
    ) -> Result<Vec<TerminalNameFact>, WorkbenchTerminalInvariantError> {
        let Some(workbench) = &self.workbench else {
            return Ok(vec![]);
        };
        self.agent_registration_snapshot()?
            .into_iter()
            .filter(|fact| target.is_none_or(|target| fact.binding() == target))
            .map(|fact| {
                let physical = TerminalAgentBinding {
                    binding: fact.binding().clone(),
                    worker_generation: fact.worker_generation(),
                    identity: fact.identity(),
                };
                Ok(TerminalNameFact {
                    binding: Binding {
                        presentation: fact.presentation_id().clone(),
                        carrier: CarrierIdentity {
                            worker_id: fact.worker_id(),
                            generation: fact.worker_generation(),
                        },
                        host: fact.identity(),
                    },
                    owner: workbench.terminal_agent_name_owner(&physical),
                })
            })
            .collect()
    }
    pub(super) fn observe_name_owner(
        &self,
        target: &orchbee_workbench_protocol::PresentationBinding,
        released: bool,
    ) {
        let Some(agents) = self.agents.as_ref().and_then(Weak::upgrade) else {
            return;
        };
        // Select the affected physical target before reading reports or locking the
        // Names leaf. A process update never folds every terminal's name owner.
        let Ok(facts) = self.terminal_name_facts_for(Some(target)) else {
            return;
        };
        for mut fact in facts {
            fact.owner.clear |= released;
            agents.observe_terminal_name(&fact);
        }
    }
}
