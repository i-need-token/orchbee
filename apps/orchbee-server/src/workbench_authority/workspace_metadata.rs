//! One transient owner for Workspace token values, per-source sequence fences and expiry.
use super::WorkbenchAuthority;
use orchbee_workbench_protocol::{
    MetadataReportDisposition as Disposition, MetadataSource, MetadataTokenKey, MetadataTokenPatch,
    MetadataTokenValue, MetadataTokens, ProtocolVersion, WorkbenchRevision, WorkbenchWorkspaceId,
    WorkspaceMetadataAction as Action, WorkspaceMetadataFailure as Failure,
    WorkspaceMetadataOutcome as Outcome, WorkspaceMetadataRequest, WorkspaceMetadataResult,
    WorkspaceMetadataRow,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

const MAXIMUM_RESOURCES: usize = 4096;
const MAXIMUM_RETAINED_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_SEQUENCE_SOURCES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    value: MetadataTokenValue,
    expires_at: Option<Instant>,
}
#[derive(Clone, Default, Debug, Eq, PartialEq)]
struct Resource {
    tokens: BTreeMap<MetadataTokenKey, Token>,
    sequences: BTreeMap<MetadataSource, WorkbenchRevision>,
}
impl Resource {
    fn bytes(&self) -> usize {
        128 + self
            .tokens
            .iter()
            .map(|(key, value)| 128 + key.as_str().len() + value.value.as_str().len())
            .sum::<usize>()
            + self
                .sequences
                .keys()
                .map(|source| 96 + source.as_str().len())
                .sum::<usize>()
    }
    fn empty(&self) -> bool {
        self.tokens.is_empty() && self.sequences.is_empty()
    }
    fn values(&self) -> MetadataTokens {
        MetadataTokens::new(
            self.tokens
                .iter()
                .map(|(key, token)| (key.clone(), token.value.clone()))
                .collect(),
        )
        .expect("admitted token count")
    }
}
#[derive(Default)]
pub(super) struct WorkspaceMetadata {
    resources: BTreeMap<WorkbenchWorkspaceId, Resource>,
    revision: u64,
    bytes: usize,
}
impl WorkspaceMetadata {
    fn maintain(
        &mut self,
        live: &BTreeSet<WorkbenchWorkspaceId>,
        now: Instant,
    ) -> Result<Vec<(WorkbenchWorkspaceId, MetadataTokens)>, Failure> {
        let changed = self.resources.iter().any(|(id, resource)| {
            !live.contains(id)
                || resource
                    .tokens
                    .values()
                    .any(|token| token.expires_at.is_some_and(|deadline| deadline <= now))
        });
        if !changed {
            return Ok(Vec::new());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        let mut expired = Vec::new();
        self.resources.retain(|id, resource| {
            if !live.contains(id) {
                return false;
            }
            let before = resource.tokens.len();
            resource
                .tokens
                .retain(|_, token| token.expires_at.is_none_or(|deadline| deadline > now));
            if before != resource.tokens.len() {
                expired.push((id.clone(), resource.values()));
            }
            !resource.empty()
        });
        self.bytes = self.resources.values().map(Resource::bytes).sum();
        self.revision = revision;
        Ok(expired)
    }
    fn report(
        &mut self,
        id: &WorkbenchWorkspaceId,
        source: &MetadataSource,
        patch: &MetadataTokenPatch,
        sequence: Option<WorkbenchRevision>,
        ttl: Option<u32>,
        now: Instant,
    ) -> Result<Disposition, Failure> {
        let prior = self.resources.get(id);
        if sequence.is_some_and(|seq| {
            prior
                .and_then(|resource| resource.sequences.get(source))
                .is_some_and(|last| seq <= *last)
        }) {
            return Ok(Disposition::Stale);
        }
        let expires_at = ttl
            .map(|ttl| {
                now.checked_add(Duration::from_millis(u64::from(ttl)))
                    .ok_or(Failure::DeadlineOverflow)
            })
            .transpose()?;
        let mut next = prior.cloned().unwrap_or_default();
        for (key, value) in patch.as_map() {
            if let Some(value) = value {
                next.tokens.insert(
                    key.clone(),
                    Token {
                        value: value.clone(),
                        expires_at,
                    },
                );
            } else {
                next.tokens.remove(key);
            }
        }
        if next.tokens.len() > orchbee_workbench_protocol::MAXIMUM_METADATA_TOKEN_KEYS {
            return Err(Failure::TokenLimit);
        }
        if let Some(sequence) = sequence {
            if !next.sequences.contains_key(source)
                && next.sequences.len() >= MAXIMUM_SEQUENCE_SOURCES
            {
                return Err(Failure::SequenceSourceLimit);
            }
            next.sequences.insert(source.clone(), sequence);
        }
        if prior == Some(&next) || (prior.is_none() && next.empty()) {
            return Ok(Disposition::Applied);
        }
        if prior.is_none() && !next.empty() && self.resources.len() >= MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let new_bytes = if next.empty() { 0 } else { next.bytes() };
        let bytes = self
            .bytes
            .checked_sub(prior.map_or(0, Resource::bytes))
            .and_then(|bytes| bytes.checked_add(new_bytes))
            .ok_or(Failure::ResourceLimit)?;
        if bytes > MAXIMUM_RETAINED_BYTES {
            return Err(Failure::ResourceLimit);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        if next.empty() {
            self.resources.remove(id);
        } else {
            self.resources.insert(id.clone(), next);
        }
        self.bytes = bytes;
        self.revision = revision;
        Ok(Disposition::Applied)
    }
}

impl WorkbenchAuthority {
    /// Expiry commits are observed even when subscribers are the only active readers.
    pub fn maintain_workspace_metadata(&mut self, now: Instant) -> Result<(), Failure> {
        if self.retired {
            return Err(Failure::Unavailable);
        }
        let live = self.workspace_metadata_ids();
        self.expire_workspace_metadata(&live, now)
    }
    fn workspace_metadata_ids(&self) -> BTreeSet<WorkbenchWorkspaceId> {
        self.snapshot()
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect()
    }
    fn publish_workspace_tokens(&self, id: WorkbenchWorkspaceId, tokens: MetadataTokens) {
        self.event_journal().publish(
            self.snapshot().revision,
            Some(id.clone()),
            orchbee_workbench_protocol::SemanticEventKind::WorkspaceMetadataUpdated,
            orchbee_workbench_protocol::SemanticEventData::WorkspaceMetadata {
                workspace_id: id,
                tokens,
            },
        );
    }
    fn expire_workspace_metadata(
        &mut self,
        live: &BTreeSet<WorkbenchWorkspaceId>,
        now: Instant,
    ) -> Result<(), Failure> {
        for (id, tokens) in self.workspace_metadata.maintain(live, now)? {
            self.publish_workspace_tokens(id, tokens);
        }
        Ok(())
    }
    /// Caller holds the Session gate and supplies its monotonic observation time.
    pub fn workspace_metadata(
        &mut self,
        request: WorkspaceMetadataRequest,
        now: Instant,
    ) -> WorkspaceMetadataResult {
        let outcome = self
            .workspace_metadata_outcome(&request, now)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        WorkspaceMetadataResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    fn workspace_metadata_outcome(
        &mut self,
        request: &WorkspaceMetadataRequest,
        now: Instant,
    ) -> Result<Outcome, Failure> {
        request.validate().map_err(|_| Failure::InvalidRequest)?;
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if request.session_id != *self.session_id() {
            return Err(Failure::SessionMismatch);
        }
        let live = self.workspace_metadata_ids();
        self.expire_workspace_metadata(&live, now)?;
        let workbench_revision = self.snapshot().revision;
        match &request.action {
            Action::Read { workspace_ids } => {
                if workspace_ids.iter().any(|id| !live.contains(id)) {
                    return Err(Failure::NotFound);
                }
                Ok(Outcome::Read {
                    workbench_revision,
                    metadata_revision: WorkbenchRevision::new(self.workspace_metadata.revision),
                    workspaces: workspace_ids
                        .iter()
                        .map(|id| WorkspaceMetadataRow {
                            workspace_id: id.clone(),
                            tokens: self
                                .workspace_metadata
                                .resources
                                .get(id)
                                .map_or_else(MetadataTokens::default, Resource::values),
                        })
                        .collect(),
                })
            }
            Action::Report {
                workspace_id,
                source,
                tokens,
                sequence,
                ttl_milliseconds,
            } => {
                if !live.contains(workspace_id) {
                    return Err(Failure::NotFound);
                }
                let before = self
                    .workspace_metadata
                    .resources
                    .get(workspace_id)
                    .map_or_else(MetadataTokens::default, Resource::values);
                let disposition = self.workspace_metadata.report(
                    workspace_id,
                    source,
                    tokens,
                    *sequence,
                    *ttl_milliseconds,
                    now,
                )?;
                let after = self
                    .workspace_metadata
                    .resources
                    .get(workspace_id)
                    .map_or_else(MetadataTokens::default, Resource::values);
                if before != after {
                    self.publish_workspace_tokens(workspace_id.clone(), after);
                }
                Ok(Outcome::Reported {
                    workbench_revision,
                    metadata_revision: WorkbenchRevision::new(self.workspace_metadata.revision),
                    workspace_id: workspace_id.clone(),
                    disposition,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests;
