//! Transient logical-terminal metadata, committed atomically under the Site owner gate.
use super::WorkbenchAuthority;
use orchbee_workbench_protocol::{
    MAXIMUM_METADATA_TOKEN_KEYS, MAXIMUM_PANEL_METADATA_SOURCES,
    MetadataReportDisposition as Disposition, MetadataSource, MetadataTokenKey, MetadataTokenValue,
    MetadataTokens, PanelMetadataAction as Action, PanelMetadataFailure as Failure,
    PanelMetadataField as Field, PanelMetadataOutcome as Outcome, PanelMetadataPatch as Patch,
    PanelMetadataPresentation as Presentation, PanelMetadataRequest, PanelMetadataResult,
    PanelMetadataRow, PanelNode, PresentationBinding, ProtocolVersion, TerminalId,
    WorkbenchPanelId, WorkbenchRevision, WorkerId,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

type BindingKey = (WorkerId, TerminalId);
const MAXIMUM_RESOURCES: usize = 4096;
const MAXIMUM_BYTES: usize = 16 * 1024 * 1024;
fn key(binding: &PresentationBinding) -> BindingKey {
    (binding.worker_id.clone(), binding.terminal_id.clone())
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    value: MetadataTokenValue,
    expires: Option<Instant>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Source {
    sequence: Option<WorkbenchRevision>,
    presentation: Option<Presentation>,
    expires: Option<Instant>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Resource {
    tokens: BTreeMap<MetadataTokenKey, Token>,
    sources: BTreeMap<MetadataSource, Source>,
}
impl Resource {
    fn empty(&self) -> bool {
        self.tokens.is_empty() && self.sources.is_empty()
    }
    fn bytes(&self) -> usize {
        128 + self
            .tokens
            .iter()
            .map(|(k, v)| 128 + k.as_str().len() + v.value.as_str().len())
            .sum::<usize>()
            + self
                .sources
                .iter()
                .map(|(id, s)| {
                    128 + id.as_str().len()
                        + s.presentation.as_ref().map_or(0, |p| {
                            256 + p.agent.as_ref().map_or(0, |v| v.as_str().len())
                                + p.applies_to_source.as_ref().map_or(0, |v| v.as_str().len())
                                + p.title
                                    .iter()
                                    .chain(p.display_agent.iter())
                                    .chain(p.state_labels.values())
                                    .map(|f| 128 + f.value.as_str().len())
                                    .sum::<usize>()
                        })
                })
                .sum::<usize>()
    }
    fn expire(&mut self, now: Instant) {
        self.tokens
            .retain(|_, v| v.expires.is_none_or(|end| end > now));
        self.sources.retain(|_, v| {
            if v.expires.is_some_and(|end| end <= now) {
                v.presentation = None;
                v.expires = None;
            }
            v.sequence.is_some() || v.presentation.is_some()
        });
    }
    fn due(&self, now: Instant) -> bool {
        self.tokens
            .values()
            .any(|v| v.expires.is_some_and(|end| end <= now))
            || self
                .sources
                .values()
                .any(|v| v.expires.is_some_and(|end| end <= now))
    }
    fn tokens(&self) -> MetadataTokens {
        MetadataTokens::new(
            self.tokens
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect(),
        )
        .expect("admitted token count")
    }
}
#[derive(Default)]
pub(super) struct PanelMetadata {
    resources: BTreeMap<BindingKey, Resource>,
    revision: u64,
    bytes: usize,
}
impl PanelMetadata {
    fn maintain(&mut self, live: &BTreeSet<BindingKey>, now: Instant) -> Result<(), Failure> {
        if !self
            .resources
            .iter()
            .any(|(id, r)| !live.contains(id) || r.due(now))
        {
            return Ok(());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        self.resources.retain(|id, r| {
            if !live.contains(id) {
                return false;
            }
            r.expire(now);
            !r.empty()
        });
        self.revision = revision;
        self.bytes = self.resources.values().map(Resource::bytes).sum();
        Ok(())
    }
    fn report(
        &mut self,
        binding: &BindingKey,
        source: &MetadataSource,
        patch: &Patch,
        sequence: Option<WorkbenchRevision>,
        ttl: Option<u32>,
        now: Instant,
    ) -> Result<Disposition, Failure> {
        let prior = self.resources.get(binding);
        if sequence.is_some_and(|seq| {
            prior
                .and_then(|r| r.sources.get(source))
                .and_then(|s| s.sequence)
                .is_some_and(|last| seq <= last)
        }) {
            return Ok(Disposition::Stale);
        }
        let expires = ttl
            .map(|ttl| {
                now.checked_add(Duration::from_millis(u64::from(ttl)))
                    .ok_or(Failure::DeadlineOverflow)
            })
            .transpose()?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        let mut next = prior.cloned().unwrap_or_default();
        if let Some(tokens) = &patch.tokens {
            for (k, v) in tokens.as_map() {
                match v {
                    Some(v) => {
                        next.tokens.insert(
                            k.clone(),
                            Token {
                                value: v.clone(),
                                expires,
                            },
                        );
                    }
                    None => {
                        next.tokens.remove(k);
                    }
                }
            }
        }
        if next.tokens.len() > MAXIMUM_METADATA_TOKEN_KEYS {
            return Err(Failure::TokenLimit);
        }
        let mut updated = next.sources.get(source).cloned().unwrap_or_default();
        if let Some(seq) = sequence {
            updated.sequence = Some(seq);
        }
        if patch.sets_presentation() || patch.clears_presentation() {
            let field = |v: &MetadataTokenValue| Field {
                value: v.clone(),
                order: WorkbenchRevision::new(revision),
            };
            let empty = || Presentation {
                source: source.clone(),
                agent: patch.agent.clone(),
                applies_to_source: patch.applies_to_source.clone(),
                title: None,
                display_agent: None,
                state_labels: BTreeMap::new(),
            };
            let mut presentation = if patch.clears_presentation() {
                updated.presentation.take().unwrap_or_else(empty)
            } else {
                empty()
            };
            if patch.clear_title {
                presentation.title = None;
            }
            if patch.clear_display_agent {
                presentation.display_agent = None;
            }
            if patch.clear_state_labels {
                presentation.state_labels.clear();
            }
            if let Some(agent) = &patch.agent {
                presentation.agent = Some(agent.clone());
            }
            if let Some(applies) = &patch.applies_to_source {
                presentation.applies_to_source = Some(applies.clone());
            }
            if let Some(value) = &patch.title {
                presentation.title = Some(field(value));
            }
            if let Some(value) = &patch.display_agent {
                presentation.display_agent = Some(field(value));
            }
            for (state, value) in &patch.state_labels {
                presentation.state_labels.insert(*state, field(value));
            }
            if patch.sets_presentation() || ttl.is_some() {
                updated.expires = expires;
            }
            updated.presentation = (!presentation.state_labels.is_empty()
                || presentation.title.is_some()
                || presentation.display_agent.is_some())
            .then_some(presentation);
            if updated.presentation.is_none() {
                updated.expires = None;
            }
        }
        if updated.sequence.is_some() || updated.presentation.is_some() {
            next.sources.insert(source.clone(), updated);
        } else {
            next.sources.remove(source);
        }
        if next.sources.len() > MAXIMUM_PANEL_METADATA_SOURCES {
            return Err(Failure::SourceLimit);
        }
        if prior == Some(&next) || (prior.is_none() && next.empty()) {
            return Ok(Disposition::Applied);
        }
        if prior.is_none() && !next.empty() && self.resources.len() >= MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let retained = if next.empty() { 0 } else { next.bytes() };
        let bytes = self
            .bytes
            .checked_sub(prior.map_or(0, Resource::bytes))
            .and_then(|n| n.checked_add(retained))
            .filter(|n| *n <= MAXIMUM_BYTES)
            .ok_or(Failure::ResourceLimit)?;
        if next.empty() {
            self.resources.remove(binding);
        } else {
            self.resources.insert(binding.clone(), next);
        }
        self.bytes = bytes;
        self.revision = revision;
        Ok(Disposition::Applied)
    }
    fn row(&self, id: WorkbenchPanelId, binding: Option<PresentationBinding>) -> PanelMetadataRow {
        let resource = binding.as_ref().and_then(|b| self.resources.get(&key(b)));
        PanelMetadataRow {
            panel_id: id,
            binding,
            tokens: resource.map_or_else(MetadataTokens::default, Resource::tokens),
            presentations: resource.map_or_else(Vec::new, |r| {
                r.sources
                    .values()
                    .filter_map(|s| s.presentation.clone())
                    .collect()
            }),
        }
    }
}
fn membership(
    node: &PanelNode,
    panels: &mut BTreeMap<WorkbenchPanelId, Option<PresentationBinding>>,
    live: &mut BTreeSet<BindingKey>,
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => {
            panels.insert(panel_id.clone(), primary_presentation.binding.clone());
            for p in std::iter::once(primary_presentation).chain(child_presentations) {
                if let Some(binding) = &p.binding {
                    live.insert(key(binding));
                }
            }
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter() {
                membership(&child.node, panels, live);
            }
        }
    }
}
impl WorkbenchAuthority {
    /// The caller holds the Site gate; no I/O, canonical topology mutation or persistence occurs.
    pub fn panel_metadata(
        &mut self,
        request: PanelMetadataRequest,
        now: Instant,
    ) -> PanelMetadataResult {
        let outcome = self
            .panel_metadata_outcome(&request, now)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PanelMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn panel_metadata_outcome(
        &mut self,
        request: &PanelMetadataRequest,
        now: Instant,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidRequest)?;
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if request.site_id != *self.site_id() {
            return Err(Failure::SiteMismatch);
        }
        let mut panels = BTreeMap::new();
        let mut live = BTreeSet::new();
        for context in &self.snapshot().terminal_contexts {
            for tab in &context.tabs {
                membership(&tab.root, &mut panels, &mut live);
            }
        }
        self.panel_metadata.maintain(&live, now)?;
        let workbench_revision = self.snapshot().revision;
        match &request.action {
            Action::Read { panel_ids } => {
                if panel_ids.iter().any(|id| !panels.contains_key(id)) {
                    return Err(Failure::NotFound);
                }
                Ok(Outcome::Read {
                    workbench_revision,
                    metadata_revision: WorkbenchRevision::new(self.panel_metadata.revision),
                    panels: panel_ids
                        .iter()
                        .map(|id| self.panel_metadata.row(id.clone(), panels[id].clone()))
                        .collect(),
                })
            }
            Action::Report {
                panel_id,
                expected_binding,
                source,
                patch,
                sequence,
                ttl_milliseconds,
            } => {
                let binding = panels
                    .get(panel_id)
                    .ok_or(Failure::NotFound)?
                    .as_ref()
                    .ok_or(Failure::Unbound)?;
                if binding != expected_binding {
                    return Err(Failure::BindingChanged);
                }
                let disposition = self.panel_metadata.report(
                    &key(binding),
                    source,
                    patch,
                    *sequence,
                    *ttl_milliseconds,
                    now,
                )?;
                Ok(Outcome::Reported {
                    workbench_revision,
                    metadata_revision: WorkbenchRevision::new(self.panel_metadata.revision),
                    panel_id: panel_id.clone(),
                    binding: binding.clone(),
                    disposition,
                })
            }
        }
    }
}
#[cfg(test)]
mod tests;
