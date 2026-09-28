//! Explicit terminal report transitions, distinct from canonical Agent observations.
//! Each transition prepares a candidate and metadata intent; the Site owner admits and commits it.

mod authority;
mod process;
mod session;
#[cfg(test)]
mod tests;

use orchbee_agent_observation_protocol::{
    AgentProviderSessionReference as Reference, AgentProviderSessionReferenceKind as ReferenceKind,
    AgentReportProvider as Provider, AgentReportSequence as Sequence,
    AgentReportSourcePolicy as SourcePolicy, AgentSessionStartSource as Start,
    agent_report_allows_session_replacement, agent_report_reserves_native_state,
    agent_report_source_policy, normalize_agent_report_label, windows_agent_resume_argv,
};
use serde::Serialize;
use std::collections::BTreeMap;

use orchbee_workbench_protocol::{PanelAgentDisposition as Disposition, PanelAgentState as Status};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Author {
    source: String,
    agent: String,
}
impl Author {
    pub(super) fn new(source: &str, agent: &str) -> Option<Self> {
        Some(Self {
            source: source.to_owned(),
            agent: normalize_agent_report_label(agent)?,
        })
    }
    fn policy(&self) -> SourcePolicy {
        agent_report_source_policy(&self.source, &self.agent)
    }
    fn known(&self) -> Option<Provider> {
        Provider::lookup(&self.agent)
    }
    fn full(&self) -> bool {
        self.policy() == SourcePolicy::FullLifecycle
    }
    fn replacement(&self, start: Option<Start>) -> bool {
        agent_report_allows_session_replacement(&self.source, &self.agent, start)
    }
    fn reference(&self, reference: Option<Reference>) -> Option<Reference> {
        reference.filter(|r| {
            self.policy() != SourcePolicy::Custom
                && (r.kind() == ReferenceKind::Id || matches!(self.agent.as_str(), "pi" | "omp"))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Session {
    author: Author,
    reference: Reference,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Authority {
    author: Author,
    status: Status,
    message: Option<String>,
    at: u64,
    reference: Option<Reference>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
enum SuppressionReason {
    Cleared,
    Exited,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Suppression {
    agent: String,
    reference: Option<Reference>,
    at: u64,
    reason: SuppressionReason,
    replacement: Option<Reference>,
    pending: Option<(Authority, Sequence)>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct StaleSession {
    agent: String,
    reference: Reference,
}

fn serialize_provider<S: serde::Serializer>(
    provider: &Option<Provider>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    provider.map(Provider::report_label).serialize(serializer)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Process {
    pub(super) observed_at: u64,
    #[serde(serialize_with = "serialize_provider")]
    pub(super) agent: Option<Provider>,
    pub(super) status: Status,
    pub(super) visible_blocker: bool,
    pub(super) exited: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) enum Event {
    State {
        author: Author,
        status: Status,
        message: Option<String>,
        reference: Option<Reference>,
        sequence: Option<Sequence>,
    },
    Session {
        author: Author,
        reference: Option<Reference>,
        sequence: Option<Sequence>,
        start: Option<Start>,
    },
    Clear {
        source: Option<String>,
        sequence: Option<Sequence>,
    },
    Release {
        author: Author,
        sequence: Option<Sequence>,
    },
    Process(Process),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Effective {
    pub(super) agent: Option<String>,
    pub(super) status: Status,
    pub(super) authority_source: Option<String>,
    pub(super) message: Option<String>,
    pub(super) session: Option<Session>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MetadataExit {
    pub(super) agent: Provider,
    pub(super) observed_at: u64,
}
pub(super) struct Transition {
    pub(super) next: Reports,
    pub(super) disposition: Disposition,
    pub(super) changed: bool,
    pub(super) released: bool,
    pub(super) metadata_exit: Option<MetadataExit>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub(super) struct Reports {
    authority: Option<Authority>,
    persisted: Option<Session>,
    sequences: BTreeMap<String, Sequence>,
    suppressed: BTreeMap<String, Suppression>,
    stale: BTreeMap<String, Vec<StaleSession>>,
    #[serde(serialize_with = "serialize_provider")]
    detected: Option<Provider>,
    fallback: Status,
    visible_blocker: bool,
    observed_at: Option<u64>,
    recent_exit: Option<(String, u64)>,
}
impl Reports {
    pub(super) fn transition(&self, event: Event, at: u64) -> Transition {
        let mut next = self.clone();
        let mut released = false;
        let mut metadata_exit = None;
        let disposition = match event {
            Event::State {
                author,
                status,
                message,
                reference,
                sequence,
            } => {
                let reference = author.reference(reference);
                if agent_report_reserves_native_state(&author.source, &author.agent) {
                    next.report_session(author, reference, sequence, None, at)
                } else {
                    next.report_state(author, status, message, reference, sequence, at)
                }
            }
            Event::Session {
                author,
                reference,
                sequence,
                start,
            } => {
                let reference = author.reference(reference);
                next.report_session(author, reference, sequence, start, at)
            }
            Event::Clear { source, sequence } => next.clear(source.as_deref(), sequence, at),
            Event::Release { author, sequence } => {
                if author.policy() != SourcePolicy::Custom {
                    Disposition::Ignored
                } else {
                    let result = next.release(&author, sequence, at);
                    released = result.1;
                    result.0
                }
            }
            Event::Process(process) => {
                let result = next.observe(process, at);
                released = result.1;
                if process.exited
                    && let Some(agent) = process.agent
                {
                    metadata_exit = Some(MetadataExit {
                        agent,
                        observed_at: process.observed_at,
                    });
                }
                result.0
            }
        };
        let changed = next != *self;
        Transition {
            next,
            disposition,
            changed,
            released,
            metadata_exit,
        }
    }
    pub(super) fn effective(&self) -> Effective {
        let authority = self
            .authority
            .as_ref()
            .filter(|a| self.authority_effective(a));
        let blocker = !self.live_full()
            && self.visible_blocker
            && self.authority.as_ref().is_some_and(|a| {
                self.observed_at.is_some_and(|at| at >= a.at)
                    && a.status != Status::Blocked
                    && a.author.known() == self.detected
            });
        Effective {
            agent: authority.map(|a| a.author.agent.clone()).or_else(|| {
                self.recent_exit
                    .is_none()
                    .then_some(self.detected)
                    .flatten()
                    .map(|p| p.report_label().to_owned())
            }),
            status: if blocker {
                Status::Blocked
            } else {
                authority.map_or(self.fallback, |a| a.status)
            },
            authority_source: authority.map(|a| a.author.source.clone()),
            message: authority.and_then(|a| a.message.clone()),
            session: self.current_session(),
        }
    }
    fn current_session(&self) -> Option<Session> {
        self.authority
            .as_ref()
            .and_then(|a| {
                a.reference.as_ref().map(|reference| Session {
                    author: a.author.clone(),
                    reference: reference.clone(),
                })
            })
            .or_else(|| self.persisted.clone())
    }
    fn process_present(&self, author: &Author) -> bool {
        author.known().is_some() && author.known() == self.detected && self.recent_exit.is_none()
    }
    fn authority_effective(&self, authority: &Authority) -> bool {
        !authority.author.full() || self.process_present(&authority.author)
    }
    fn live_full(&self) -> bool {
        self.authority
            .as_ref()
            .is_some_and(|a| a.author.full() && self.authority_effective(a))
    }
    fn owner_conflicts(&self, author: &Author) -> bool {
        self.current_session()
            .is_some_and(|session| session.author != *author)
    }
    fn known_conflicts(&self, author: &Author) -> bool {
        self.detected
            .zip(author.known())
            .is_some_and(|(a, b)| a != b)
    }
    fn foreground_confirms(&self, author: &Author, reference: &Reference) -> bool {
        self.detected.is_some()
            && self.detected == author.known()
            && windows_agent_resume_argv(&author.source, &author.agent, reference).is_some()
    }
    fn accept_sequence(&mut self, source: &str, sequence: Option<Sequence>) -> bool {
        match sequence {
            None => !self.sequences.contains_key(source),
            Some(seq) if self.sequences.get(source).is_none_or(|last| seq > *last) => {
                self.sequences.insert(source.to_owned(), seq);
                true
            }
            Some(_) => false,
        }
    }
    fn conflicting_reference(
        &self,
        author: &Author,
        reference: &Reference,
        start: Option<Start>,
    ) -> Option<Reference> {
        self.current_session()
            .filter(|s| {
                s.author == *author
                    && s.reference.kind() == ReferenceKind::Id
                    && reference.kind() == ReferenceKind::Id
                    && s.reference != *reference
                    && !author.replacement(start)
            })
            .map(|s| s.reference)
    }
    fn remember_stale(&mut self, author: &Author, reference: Reference) {
        let entry = StaleSession {
            agent: author.agent.clone(),
            reference,
        };
        let stale = self.stale.entry(author.source.clone()).or_default();
        if !stale.contains(&entry) {
            stale.push(entry);
        }
    }
    fn forget_stale(&mut self, author: &Author, reference: &Reference) {
        if let Some(stale) = self.stale.get_mut(&author.source) {
            stale.retain(|s| s.agent != author.agent || s.reference != *reference);
            if stale.is_empty() {
                self.stale.remove(&author.source);
            }
        }
    }
    fn suppress(
        &mut self,
        author: &Author,
        reference: Option<Reference>,
        reason: SuppressionReason,
        at: u64,
    ) {
        self.suppressed.insert(
            author.source.clone(),
            Suppression {
                agent: author.agent.clone(),
                reference,
                reason,
                at,
                replacement: None,
                pending: None,
            },
        );
    }
    fn suppress_current(&mut self, at: u64) {
        if let Some(authority) = self.authority.clone().filter(|a| a.author.full()) {
            self.suppress(
                &authority.author,
                authority.reference,
                SuppressionReason::Cleared,
                at,
            );
        }
    }
}

impl Reports {
    pub(super) fn view(&self) -> orchbee_workbench_protocol::PanelAgentView {
        use orchbee_workbench_protocol::{PanelAgentReference, PanelAgentSession, PanelAgentView};
        let effective = self.effective();
        PanelAgentView {
            agent: effective.agent,
            state: effective.status,
            authority_source: effective.authority_source,
            message: effective.message,
            session: effective.session.map(|s| PanelAgentSession {
                source: s.author.source,
                agent: s.author.agent,
                reference: PanelAgentReference {
                    kind: s.reference.kind(),
                    value: s.reference.value().to_owned(),
                },
            }),
        }
    }
    pub(super) fn admitted_bytes(
        &self,
    ) -> Result<usize, orchbee_workbench_protocol::PanelAgentFailure> {
        use orchbee_workbench_protocol::PanelAgentFailure as Failure;
        let mut sources: std::collections::BTreeSet<&str> = self
            .sequences
            .keys()
            .chain(self.suppressed.keys())
            .chain(self.stale.keys())
            .map(String::as_str)
            .collect();
        if let Some(a) = &self.authority {
            sources.insert(&a.author.source);
        }
        if let Some(s) = &self.persisted {
            sources.insert(&s.author.source);
        }
        if sources.len() > 128 {
            return Err(Failure::SourceLimit);
        }
        if self.stale.values().map(Vec::len).sum::<usize>() > 256 {
            return Err(Failure::HistoryLimit);
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| Failure::ResourceLimit)?
            .len();
        if bytes > 512 * 1024 {
            return Err(Failure::ResourceLimit);
        }
        Ok(bytes)
    }
}

impl Reports {
    pub(super) fn recent_exit_agent(&self) -> Option<Provider> {
        self.recent_exit
            .as_ref()
            .and_then(|(agent, _)| Provider::lookup(agent))
    }
}
