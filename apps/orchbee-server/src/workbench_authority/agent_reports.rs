//! Explicit custom reports combine with trusted process observations under the Session owner.
//! Sources share one ordering and release policy and never grant provider identity or lifecycle proof.

#[cfg(test)]
mod tests;

use orchbee_agent_observation_protocol::{
    AgentReportProvider as Provider, AgentReportSequence as Sequence, normalize_agent_report_label,
};
use orchbee_workbench_protocol::{PanelAgentDisposition as Disposition, PanelAgentState as Status};
use serde::Serialize;
use std::collections::BTreeMap;

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
    fn known(&self) -> Option<Provider> {
        Provider::lookup(&self.agent)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Authority {
    author: Author,
    status: Status,
    message: Option<String>,
    at: u64,
}
fn serialize_provider<S: serde::Serializer>(
    provider: &Option<Provider>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    provider.map(Provider::report_label).serialize(serializer)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Fallback {
    pub(super) status: Status,
    pub(super) visible_blocker: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Process {
    pub(super) observed_at: u64,
    #[serde(serialize_with = "serialize_provider")]
    pub(super) agent: Option<Provider>,
    #[serde(flatten)]
    pub(super) fallback: Option<Fallback>,
    pub(super) exited: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) enum Event {
    State {
        author: Author,
        status: Status,
        message: Option<String>,
        sequence: Option<Sequence>,
    },
    Session,
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
    sequences: BTreeMap<String, Sequence>,
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
                sequence,
            } => next.report_state(author, status, message, sequence, at),
            Event::Session => Disposition::Ignored,
            Event::Clear { source, sequence } => next.clear(source.as_deref(), sequence),
            Event::Release { author, sequence } => {
                let result = next.release(&author, sequence);
                released = result.1;
                result.0
            }
            Event::Process(process) => {
                let result = next.observe(process);
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
        Transition {
            changed: next != *self,
            next,
            disposition,
            released,
            metadata_exit,
        }
    }
    fn report_state(
        &mut self,
        author: Author,
        status: Status,
        message: Option<String>,
        sequence: Option<Sequence>,
        at: u64,
    ) -> Disposition {
        if self
            .recent_exit
            .as_ref()
            .is_some_and(|(agent, _)| author.known().is_some_and(|p| p.report_label() == agent))
            || self
                .detected
                .zip(author.known())
                .is_some_and(|(a, b)| a != b)
            || !self.accept_sequence(&author.source, sequence)
        {
            return Disposition::Ignored;
        }
        self.authority = Some(Authority {
            author,
            status,
            message,
            at,
        });
        Disposition::Applied
    }
    fn clear(&mut self, source: Option<&str>, sequence: Option<Sequence>) -> Disposition {
        let Some(authority) = self
            .authority
            .as_ref()
            .filter(|a| source.is_none_or(|s| a.author.source == s))
        else {
            return Disposition::Ignored;
        };
        let source = authority.author.source.clone();
        if !self.accept_sequence(&source, sequence) {
            return Disposition::Ignored;
        }
        self.authority = None;
        Disposition::Applied
    }
    fn release(&mut self, author: &Author, sequence: Option<Sequence>) -> (Disposition, bool) {
        if self.authority.as_ref().is_some_and(|a| a.author != *author)
            || self.effective().agent.as_deref() != Some(author.agent.as_str())
            || !self.accept_sequence(&author.source, sequence)
        {
            return (Disposition::Ignored, false);
        }
        let process_owns = self.process_present(author);
        if !process_owns {
            self.detected = None;
            self.fallback = Status::Unknown;
            self.visible_blocker = false;
            self.observed_at = None;
        }
        self.authority = None;
        (Disposition::Applied, !process_owns)
    }
    fn observe(&mut self, process: Process) -> (Disposition, bool) {
        let at = process.observed_at;
        let previous_agent = self.effective().agent;
        let previous_detected = self.detected;
        let newer_custom = process.exited
            && self
                .authority
                .as_ref()
                .is_some_and(|a| a.author.known() == process.agent && a.at > at);
        let released = process.exited && !newer_custom && previous_agent.is_some();
        let replacement = !process.exited
            && process.agent.is_some()
            && self.recent_exit.as_ref().is_some_and(|(agent, observed)| {
                Some(agent.as_str()) == process.agent.map(|p| p.report_label()) && *observed < at
            });
        self.detected = process.agent;
        if let Some(fallback) = process.fallback {
            self.fallback = fallback.status;
            self.visible_blocker = fallback.visible_blocker && fallback.status == Status::Blocked;
            self.observed_at = Some(at);
        } else if process.exited || replacement || previous_detected != process.agent {
            self.fallback = Status::Unknown;
            self.visible_blocker = false;
            self.observed_at = None;
        }
        if process.exited {
            if let Some(agent) = process.agent {
                self.recent_exit = Some((agent.report_label().to_owned(), at));
            }
            if let Some(source) = self
                .authority
                .as_ref()
                .filter(|a| a.author.known() == process.agent && !newer_custom)
                .map(|a| a.author.source.clone())
            {
                self.sequences.remove(&source);
                self.authority = None;
            }
        } else if process.agent.is_some() {
            self.recent_exit = None;
        }
        let drop_authority = self.authority.as_ref().is_some_and(|a| {
            a.at <= at
                && (process
                    .agent
                    .zip(a.author.known())
                    .is_some_and(|(a, b)| a != b)
                    || (previous_detected.is_some()
                        && process.agent != previous_detected
                        && a.author.known() == previous_detected))
        });
        if drop_authority {
            self.authority = None;
        }
        (Disposition::Applied, released)
    }
    pub(super) fn effective(&self) -> Effective {
        let authority = self.authority.as_ref();
        let blocker = self.visible_blocker
            && authority.is_some_and(|a| {
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
        }
    }
    pub(super) fn explanation_owner(&self) -> Option<Provider> {
        self.effective()
            .agent
            .as_deref()
            .and_then(Provider::lookup)
            .or_else(|| {
                self.recent_exit
                    .is_none()
                    .then_some(self.detected)
                    .flatten()
            })
    }
    fn process_present(&self, author: &Author) -> bool {
        author.known().is_some() && author.known() == self.detected && self.recent_exit.is_none()
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
    pub(super) fn name_owner(&self) -> super::panel_agent::TerminalAgentNameOwner {
        super::panel_agent::TerminalAgentNameOwner {
            agent: self.effective().agent,
            clear: self.recent_exit.is_some() && self.detected.is_none(),
        }
    }
    pub(super) fn view(&self) -> orchbee_workbench_protocol::PanelAgentView {
        let effective = self.effective();
        orchbee_workbench_protocol::PanelAgentView {
            agent: effective.agent,
            state: effective.status,
            authority_source: effective.authority_source,
            message: effective.message,
        }
    }
    pub(super) fn admitted_bytes(
        &self,
    ) -> Result<usize, orchbee_workbench_protocol::PanelAgentFailure> {
        use orchbee_workbench_protocol::PanelAgentFailure as Failure;
        let mut sources: std::collections::BTreeSet<&str> =
            self.sequences.keys().map(String::as_str).collect();
        if let Some(a) = &self.authority {
            sources.insert(&a.author.source);
        }
        if sources.len() > 128 {
            return Err(Failure::SourceLimit);
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|_| Failure::ResourceLimit)?
            .len();
        if bytes > 512 * 1024 {
            return Err(Failure::ResourceLimit);
        }
        Ok(bytes)
    }
    pub(super) fn recent_exit_agent(&self) -> Option<Provider> {
        self.recent_exit
            .as_ref()
            .and_then(|(agent, _)| Provider::lookup(agent))
    }
}
