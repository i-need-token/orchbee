//! Restart intent is keyed by retained Presentation identity, never old process identities.
use super::{WorkbenchAuthority, WorkbenchCheckpointError as Error};
use crate::workbench_authority::{
    TerminalAgentBinding, report_membership::key, terminal_membership::visit_panel_presentations,
};
use orchbee_agent_observation_protocol::{
    AgentProviderSessionReference as Reference, AgentProviderSessionReferenceKind as Kind,
    AgentReportSourcePolicy, agent_report_source_policy, normalize_agent_report_label,
};
use orchbee_workbench_protocol::{
    PanelAgentSession, PresentationBinding, Snapshot, TerminalPresentationId, WorkerId,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Instant};

const MAXIMUM_RECORDS: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredAgentSession {
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) session: PanelAgentSession,
}

pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<StoredAgentSession>>, D::Error> {
    Vec::<StoredAgentSession>::deserialize(deserializer).map(Some)
}

fn valid(session: &PanelAgentSession) -> bool {
    if normalize_agent_report_label(&session.agent).as_deref() != Some(&session.agent)
        || agent_report_source_policy(&session.source, &session.agent)
            == AgentReportSourcePolicy::Custom
    {
        return false;
    }
    match session.reference.kind {
        Kind::Id => Reference::id(&session.reference.value).is_some(),
        Kind::Path => {
            matches!(session.agent.as_str(), "pi" | "omp")
                && Reference::path(&session.reference.value, |p| {
                    std::path::Path::new(p).is_absolute()
                })
                .is_some()
        }
    }
}

fn members(snapshot: &Snapshot) -> BTreeMap<TerminalPresentationId, Option<PresentationBinding>> {
    let mut members = BTreeMap::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            visit_panel_presentations(&tab.root, &mut |_, p| {
                members.insert(p.presentation_id.clone(), p.binding.clone());
            });
        }
    }
    members
}

pub(super) fn validate(rows: &[StoredAgentSession], snapshot: &Snapshot) -> Result<(), Error> {
    if rows.len() > MAXIMUM_RECORDS {
        return Err(Error::InvalidAgentSession);
    }
    let mut members = members(snapshot);
    for row in rows {
        if members.remove(&row.presentation_id).is_none() || !valid(&row.session) {
            return Err(Error::InvalidAgentSession);
        }
    }
    Ok(())
}

pub(super) fn capture(
    authority: &WorkbenchAuthority,
    worker: &WorkerId,
    facts: &[TerminalAgentBinding],
) -> Result<Vec<StoredAgentSession>, Error> {
    if facts.len() > MAXIMUM_RECORDS {
        return Err(Error::InvalidAgentSession);
    }
    let members = members(authority.snapshot());
    let mut physical = BTreeMap::new();
    for fact in facts {
        if &fact.binding.worker_id != worker
            || !members.values().any(|b| b.as_ref() == Some(&fact.binding))
            || physical.insert(key(&fact.binding), fact).is_some()
        {
            return Err(Error::InvalidAgentSession);
        }
    }
    let mut rows = Vec::new();
    for (presentation_id, binding) in members {
        let current = binding
            .as_ref()
            .and_then(|b| physical.get(&key(b)))
            .and_then(|fact| authority.panel_agent.checkpoint_session(fact));
        let session = current.unwrap_or_else(|| {
            authority
                .pending_agent_sessions
                .get(&presentation_id)
                .cloned()
        });
        if let Some(session) = session {
            if !valid(&session) {
                return Err(Error::InvalidAgentSession);
            }
            rows.push(StoredAgentSession {
                presentation_id,
                session,
            });
        }
    }
    if rows.len() > MAXIMUM_RECORDS {
        return Err(Error::InvalidAgentSession);
    }
    Ok(rows)
}

impl WorkbenchAuthority {
    /// The runtime resolves physical generations only for retained restart references.
    /// Unrelated terminal counts must not impose the canonical Agent read limit on clean stop.
    pub fn agent_restart_bindings(&self) -> Result<Vec<PresentationBinding>, Error> {
        let mut bindings = BTreeMap::new();
        for (id, binding) in members(self.snapshot()) {
            if let Some(binding) = binding
                && (self.pending_agent_sessions.contains_key(&id)
                    || self.panel_agent.has_restart_session(&binding))
            {
                bindings.insert(key(&binding), binding);
            }
        }
        if bindings.len() > MAXIMUM_RECORDS {
            return Err(Error::InvalidAgentSession);
        }
        Ok(bindings.into_values().collect())
    }

    /// Called after new physical creation, before the restored Site is published.
    /// Existing report owners supersede restart intent, including an explicitly cleared reference.
    pub fn seed_restored_agent_sessions(
        &mut self,
        facts: &[TerminalAgentBinding],
        now: Instant,
    ) -> Result<usize, orchbee_workbench_protocol::PanelAgentFailure> {
        use orchbee_workbench_protocol::PanelAgentFailure as Failure;
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if facts.len() > MAXIMUM_RECORDS {
            return Err(Failure::ResourceLimit);
        }
        let members = members(self.snapshot());
        let mut physical = BTreeMap::new();
        for fact in facts {
            if !members.values().any(|b| b.as_ref() == Some(&fact.binding))
                || physical.insert(key(&fact.binding), fact).is_some()
            {
                return Err(Failure::BindingChanged);
            }
        }
        let mut consumed = Vec::new();
        let mut seeds = Vec::new();
        for (id, session) in &self.pending_agent_sessions {
            if !members.contains_key(id) {
                consumed.push(id.clone());
                continue;
            }
            let Some(binding) = members.get(id).and_then(Option::as_ref) else {
                continue;
            };
            let Some(fact) = physical.get(&key(binding)) else {
                continue;
            };
            seeds.push(((*fact).clone(), session.clone()));
            consumed.push(id.clone());
        }
        self.panel_agent.seed_sessions(seeds, now)?;
        let count = consumed.len();
        for id in consumed {
            self.pending_agent_sessions.remove(&id);
        }
        Ok(count)
    }
}
