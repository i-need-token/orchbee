//! One complete read model joins canonical sessions to exact physical Agent incarnations.
//! It owns neither source, Client acknowledgement state, nor any terminal effect.
use crate::{machine_agents, terminal_session::NativeContextLocation};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentNameTarget, AgentObservationFreshness, AgentReportProvider,
    AgentSemanticStatus, AgentSessionSummary,
};
use orchbee_workbench_protocol::{PanelAgentState, TerminalAgentStateSnapshot};
use serde_json::{Value, json};

pub(crate) mod cursor;
pub(crate) mod read;
pub(crate) const MAXIMUM_ROWS: usize = 4096;
pub(crate) const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) enum Identity {
    Canonical(AgentSessionSummary),
    Terminal(TerminalAgentStateSnapshot),
    Both {
        canonical: AgentSessionSummary,
        terminal: TerminalAgentStateSnapshot,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Placement {
    pub(crate) panel: String,
    pub(crate) presentation: String,
    pub(crate) terminal: String,
    context: String,
    workspace: Option<String>,
    tab: String,
    order: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) identity: Identity,
    pub(crate) placement: Option<Placement>,
    memberships: Vec<Placement>,
}

pub(crate) fn provider(value: &str) -> Option<&'static str> {
    AgentReportProvider::lookup(&value.trim().to_ascii_lowercase())
        .map(AgentReportProvider::native_provider_id)
}

fn canonical_state(status: &AgentSemanticStatus) -> &'static str {
    match status {
        AgentSemanticStatus::NeedsAttention { .. } => "blocked",
        AgentSemanticStatus::Failed {}
        | AgentSemanticStatus::Canceled {}
        | AgentSemanticStatus::Interrupted {} => "error",
        AgentSemanticStatus::Succeeded {} => "done",
        AgentSemanticStatus::Working { .. } => "working",
        AgentSemanticStatus::Idle {} => "idle",
        AgentSemanticStatus::Unknown { .. } => "unknown",
        AgentSemanticStatus::Ended {} => "ended",
    }
}

impl Row {
    pub(crate) fn in_panel(&self, id: &str) -> bool {
        self.memberships.iter().any(|p| p.panel == id)
    }
    pub(crate) fn in_presentation(&self, id: &str) -> bool {
        self.memberships.iter().any(|p| p.presentation == id)
    }
    pub(crate) fn canonical(&self) -> Option<&AgentSessionSummary> {
        match &self.identity {
            Identity::Canonical(row) | Identity::Both { canonical: row, .. } => Some(row),
            Identity::Terminal(_) => None,
        }
    }
    pub(crate) fn terminal(&self) -> Option<&TerminalAgentStateSnapshot> {
        match &self.identity {
            Identity::Terminal(row) | Identity::Both { terminal: row, .. } => Some(row),
            Identity::Canonical(_) => None,
        }
    }
    pub(crate) fn source(&self) -> &'static str {
        match &self.identity {
            Identity::Canonical(_) => "canonical",
            Identity::Terminal(_) => "terminal",
            Identity::Both { .. } => "both",
        }
    }
    pub(crate) fn provider(&self) -> &str {
        let raw = self
            .canonical()
            .map(|c| c.provider_id.as_str())
            .or_else(|| self.terminal().and_then(|t| t.agent.as_deref()))
            .expect("every projection row has a real identity");
        provider(raw).unwrap_or(raw)
    }
    pub(crate) fn status(&self) -> &'static str {
        let canonical = self.canonical();
        if let Some(c) = canonical
            && (self.terminal().is_none()
                || (c.observation.freshness == AgentObservationFreshness::Fresh
                    && !matches!(c.status, AgentSemanticStatus::Unknown { .. })))
        {
            return canonical_state(&c.status);
        }
        self.terminal()
            .map_or("unknown", |t| terminal_state(t.state))
    }
    /// Prefer the full physical incarnation on a merged row. Existing effect owners
    /// revalidate it; a projection never grants a lease on a later terminal operation.
    pub(crate) fn target(&self) -> AgentNameTarget {
        match self.terminal() {
            Some(t) => AgentNameTarget::Terminal {
                binding: t.binding.clone(),
            },
            None => AgentNameTarget::Session {
                session_id: self
                    .canonical()
                    .expect("canonical identity")
                    .session_id
                    .clone(),
            },
        }
    }
    pub(crate) fn document(&self, ordinal: usize, cwd: Option<&str>) -> Value {
        let placement = self.placement.as_ref().map(|p| {
            json!({
                "panelId":p.panel,"presentationId":p.presentation,"terminalId":p.terminal,
                "terminalContextId":p.context,"workspaceId":p.workspace,"tabId":p.tab,
            })
        });
        let terminal = self.terminal().map(|t| {
            json!({
                "binding":t.binding,"provider":t.agent,"state":t.state,
                "sequence":t.sequence,
            })
        });
        json!({"ordinal":ordinal,"provider":self.provider(),"status":self.status(),
            "source":self.source(),"canonical":self.canonical(),"terminal":terminal,
            "placement":placement,"cwd":cwd})
    }
}

fn compatible(c: &AgentSessionSummary, t: &TerminalAgentStateSnapshot) -> bool {
    c.lifecycle == AgentLifecycle::Live
        && provider(c.provider_id.as_str())
            .is_some_and(|p| t.agent.as_deref().and_then(provider) == Some(p))
        && c.presentation_attachments.iter().any(|a| {
            a.navigable
                && a.presentation_id == t.binding.presentation_id
                && a.generation == t.binding.generation
        })
}

/// Merge only one-to-one components. Ambiguous edges leave every identity visible.
pub(crate) fn project(
    canonical: Vec<AgentSessionSummary>,
    checkpoint: &machine_agents::StateCheckpoint,
) -> Result<Vec<Row>, ()> {
    let placements = &checkpoint.placements;
    let physical: Vec<_> = placements
        .iter()
        .filter_map(|p| {
            checkpoint
                .observed
                .get(&p.member)
                .and_then(Option::as_ref)
                .filter(|s| s.agent.is_some())
                .cloned()
        })
        .collect();
    let placement = |id: &str| {
        placements
            .iter()
            .enumerate()
            .find(|(_, p)| p.member.as_str() == id)
            .map(|(order, p)| {
                let (context, workspace) = match &p.context.location {
                    NativeContextLocation::Standalone(context) => (context.to_string(), None),
                    NativeContextLocation::Workspace {
                        workspace_id,
                        terminal_context_id,
                    } => (
                        terminal_context_id.to_string(),
                        Some(workspace_id.to_string()),
                    ),
                };
                Placement {
                    panel: p.panel.to_string(),
                    presentation: p.member.to_string(),
                    terminal: p.terminal.terminal_id.to_string(),
                    context,
                    workspace,
                    tab: p.tab.to_string(),
                    order,
                }
            })
    };
    let edges: Vec<Vec<usize>> = canonical
        .iter()
        .map(|c| {
            physical
                .iter()
                .enumerate()
                .filter_map(|(i, t)| compatible(c, t).then_some(i))
                .collect()
        })
        .collect();
    let mut degrees = vec![0; physical.len()];
    for indices in &edges {
        for &i in indices {
            degrees[i] += 1;
        }
    }
    let mut merged = vec![None; physical.len()];
    let mut remaining = Vec::new();
    for (c, indices) in canonical.into_iter().zip(edges) {
        if let [i] = indices.as_slice()
            && degrees[*i] == 1
        {
            merged[*i] = Some(c);
        } else {
            let p = (c.lifecycle == AgentLifecycle::Live)
                .then(|| {
                    c.presentation_attachments
                        .iter()
                        .filter(|a| {
                            a.navigable
                                && checkpoint.observed.iter().any(|(id, state)| {
                                    id.as_str() == a.presentation_id.as_str()
                                        && state
                                            .as_ref()
                                            .is_some_and(|s| s.binding.generation == a.generation)
                                })
                        })
                        .filter_map(|a| placement(a.presentation_id.as_str()))
                        .min_by_key(|p| p.order)
                })
                .flatten();
            remaining.push(Row {
                identity: Identity::Canonical(c),
                placement: p,
                memberships: Vec::new(),
            });
        }
    }
    let mut rows: Vec<_> = physical
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, t)| {
            let p = placement(t.binding.presentation_id.as_str());
            let identity = match merged[i].take() {
                Some(canonical) => Identity::Both {
                    canonical,
                    terminal: t,
                },
                None => Identity::Terminal(t),
            };
            Row {
                identity,
                placement: p,
                memberships: Vec::new(),
            }
        })
        .collect();
    rows.append(&mut remaining);
    if rows.len() > MAXIMUM_ROWS {
        return Err(());
    }
    for row in &mut rows {
        let mut members = row
            .canonical()
            .map(|c| {
                c.presentation_attachments
                    .iter()
                    .filter(|a| c.lifecycle == AgentLifecycle::Live && a.navigable)
                    .filter(|a| {
                        checkpoint.observed.iter().any(|(id, state)| {
                            id.as_str() == a.presentation_id.as_str()
                                && state
                                    .as_ref()
                                    .is_some_and(|s| s.binding.generation == a.generation)
                        })
                    })
                    .filter_map(|a| placement(a.presentation_id.as_str()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(p) = &row.placement
            && !members.iter().any(|m| m.presentation == p.presentation)
        {
            members.push(p.clone());
        }
        members.sort_by_key(|p| p.order);
        row.memberships = members;
    }
    rows.sort_by(|a, b| {
        a.placement
            .as_ref()
            .map(|p| p.order)
            .unwrap_or(usize::MAX)
            .cmp(&b.placement.as_ref().map(|p| p.order).unwrap_or(usize::MAX))
            .then_with(|| a.terminal().is_none().cmp(&b.terminal().is_none()))
            .then_with(|| {
                a.canonical()
                    .map(|c| c.session_id.as_str())
                    .unwrap_or("")
                    .encode_utf16()
                    .cmp(
                        b.canonical()
                            .map(|c| c.session_id.as_str())
                            .unwrap_or("")
                            .encode_utf16(),
                    )
            })
    });
    Ok(rows)
}

pub(crate) fn terminal_state(state: PanelAgentState) -> &'static str {
    match state {
        PanelAgentState::Done => "done",
        PanelAgentState::Error => "error",
        PanelAgentState::Idle => "idle",
        PanelAgentState::Blocked => "blocked",
        PanelAgentState::Working => "working",
        PanelAgentState::Unknown => "unknown",
    }
}

#[cfg(test)]
pub(crate) mod tests;
