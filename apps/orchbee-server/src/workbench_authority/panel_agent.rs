//! Bounded explicit reports commit after generation, capacity and receipt checks under the Site gate.
use super::{
    WorkbenchAuthority,
    agent_reports::{Author, Event, Reports},
    report_membership::{BindingKey, ReportMembership, key},
};
use orchbee_agent_observation_protocol::{
    AgentProviderSessionReference as Reference, AgentSessionStartSource as Start,
};
use orchbee_capability_node_protocol::{HostTerminalIdentity, WorkerGeneration};
use orchbee_workbench_protocol::{
    PanelAgentAction as Action, PanelAgentFailure as Failure, PanelAgentOutcome as Outcome,
    PanelAgentReport as Report, PanelAgentRequest, PanelAgentResult, PanelAgentRow,
    PresentationBinding, ProtocolVersion, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    serialize_workbench_request, serialize_workbench_response,
};
use std::{collections::BTreeMap, time::Instant};
const MAXIMUM_RESOURCES: usize = 4096;
const MAXIMUM_BYTES: usize = 32 * 1024 * 1024;
mod managed_clear;
pub use managed_clear::PreparedManagedAgentClear;

/// Pure name-ownership observation; never grants canonical Agent authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerminalAgentNameOwner {
    pub agent: Option<String>,
    pub reference: Option<Reference>,
    pub clear: bool,
}

/// Composition supplies a uniquely retained physical generation for a committed logical terminal.
/// A report request cannot construct this fact or claim its process is alive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalAgentBinding {
    pub binding: PresentationBinding,
    pub worker_generation: WorkerGeneration,
    pub identity: HostTerminalIdentity,
}
#[derive(Clone, Debug)]
struct Resource {
    physical: TerminalAgentBinding,
    reports: Reports,
    explicit_revision: u64,
    bytes: usize,
}
#[derive(Default)]
pub(super) struct PanelAgents {
    resources: BTreeMap<BindingKey, Resource>,
    revision: u64,
    bytes: usize,
    origin: Option<Instant>,
}
impl PanelAgents {
    pub(super) fn has_restart_session(&self, binding: &PresentationBinding) -> bool {
        self.resources
            .get(&key(binding))
            .is_some_and(|resource| resource.reports.view().session.is_some())
    }

    pub(super) fn checkpoint_session(
        &self,
        physical: &TerminalAgentBinding,
    ) -> Option<Option<orchbee_workbench_protocol::PanelAgentSession>> {
        self.resources
            .get(&key(&physical.binding))
            .filter(|resource| &resource.physical == physical)
            .map(|resource| resource.reports.view().session)
    }

    pub(super) fn seed_sessions(
        &mut self,
        seeds: Vec<(
            TerminalAgentBinding,
            orchbee_workbench_protocol::PanelAgentSession,
        )>,
        now: Instant,
    ) -> Result<(), Failure> {
        let mut prepared = BTreeMap::new();
        let mut bytes = self.bytes;
        for (physical, session) in seeds {
            let id = key(&physical.binding);
            if let Some(current) = self.resources.get(&id) {
                if current.physical != physical {
                    return Err(Failure::BindingChanged);
                }
                continue;
            }
            let reports = Reports::from_restart_session(&session).ok_or(Failure::InvalidRequest)?;
            let added = retained_bytes(&reports)?;
            bytes = bytes
                .checked_add(added)
                .filter(|n| *n <= MAXIMUM_BYTES)
                .ok_or(Failure::ResourceLimit)?;
            if prepared.insert(id, (physical, reports, added)).is_some() {
                return Err(Failure::BindingChanged);
            }
        }
        if prepared.is_empty() {
            return Ok(());
        }
        if self.resources.len() + prepared.len() > MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        self.time(now)?;
        for (id, (physical, reports, bytes)) in prepared {
            self.resources.insert(
                id,
                Resource {
                    physical,
                    reports,
                    explicit_revision: revision,
                    bytes,
                },
            );
        }
        self.bytes = bytes;
        self.revision = revision;
        self.origin.get_or_insert(now);
        Ok(())
    }

    fn time(&self, now: Instant) -> Result<u64, Failure> {
        self.time_from(now, now)
    }
    fn time_from(&self, now: Instant, initial_origin: Instant) -> Result<u64, Failure> {
        let origin = self.origin.unwrap_or(initial_origin);
        let distance = if now >= origin {
            now.duration_since(origin)
        } else {
            origin.duration_since(now)
        };
        let nanos = u64::try_from(distance.as_nanos()).map_err(|_| Failure::ClockOverflow)?;
        if now >= origin {
            (1_u64 << 63).checked_add(nanos)
        } else {
            (1_u64 << 63).checked_sub(nanos)
        }
        .ok_or(Failure::ClockOverflow)
    }
    fn maintain(
        &mut self,
        membership: &ReportMembership,
        facts: &BTreeMap<BindingKey, &TerminalAgentBinding>,
    ) -> Result<(), Failure> {
        let remove: Vec<_> = self
            .resources
            .iter()
            .filter(|(key, resource)| {
                !membership.live.contains(*key)
                    || facts
                        .get(*key)
                        .is_some_and(|fact| **fact != resource.physical)
            })
            .map(|(key, _)| key.clone())
            .collect();
        if remove.is_empty() {
            return Ok(());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Failure::RevisionExhausted)?;
        for key in remove {
            if let Some(resource) = self.resources.remove(&key) {
                self.bytes -= resource.bytes;
            }
        }
        self.revision = revision;
        Ok(())
    }
}
fn retained_bytes(reports: &Reports) -> Result<usize, Failure> {
    if reports == &Reports::default() {
        Ok(0)
    } else {
        reports.admitted_bytes().map(|bytes| bytes + 256)
    }
}
fn event(report: &Report, absolute: fn(&str) -> bool) -> Result<Event, Failure> {
    let author =
        |source: &str, agent: &str| Author::new(source, agent).ok_or(Failure::InvalidRequest);
    let reference = |source: &str, agent: &str, id: &Option<String>, path: &Option<String>| {
        let normalized = orchbee_agent_observation_protocol::normalize_agent_report_label(agent)?;
        Reference::from_report(
            source,
            &normalized,
            id.as_deref(),
            path.as_deref(),
            absolute,
        )
    };
    Ok(match report {
        Report::State {
            source,
            agent,
            state,
            message,
            sequence,
            agent_session_id,
            agent_session_path,
        } => Event::State {
            author: author(source, agent)?,
            status: *state,
            message: message.clone(),
            sequence: *sequence,
            reference: reference(source, agent, agent_session_id, agent_session_path),
        },
        Report::Session {
            source,
            agent,
            sequence,
            agent_session_id,
            agent_session_path,
            session_start_source,
        } => Event::Session {
            author: author(source, agent)?,
            sequence: *sequence,
            reference: reference(source, agent, agent_session_id, agent_session_path),
            start: session_start_source.as_deref().and_then(Start::normalize),
        },
        Report::Clear { source, sequence } => Event::Clear {
            source: source.clone(),
            sequence: *sequence,
        },
        Report::Release {
            source,
            agent,
            sequence,
        } => Event::Release {
            author: author(source, agent)?,
            sequence: *sequence,
        },
    })
}
impl WorkbenchAuthority {
    /// Executes an explicit report or read against current committed membership and retained facts.
    /// The caller holds the Site gate; source strings never supply physical or process proof.
    pub fn panel_agent(
        &mut self,
        request: PanelAgentRequest,
        facts: &[TerminalAgentBinding],
        now: Instant,
        absolute: fn(&str) -> bool,
    ) -> PanelAgentResult {
        let outcome = self
            .panel_agent_outcome(&request, facts, now, absolute)
            .unwrap_or_else(|reason| Outcome::Failed { reason });
        PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
    fn panel_agent_outcome(
        &mut self,
        request: &PanelAgentRequest,
        facts: &[TerminalAgentBinding],
        now: Instant,
        absolute: fn(&str) -> bool,
    ) -> Result<Outcome, Failure> {
        serialize_workbench_request(&WorkbenchRequest::PanelAgent(request.clone()))
            .map_err(|_| Failure::InvalidRequest)?;
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if request.site_id != *self.site_id() {
            return Err(Failure::SiteMismatch);
        }
        if facts.len() > MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let membership = ReportMembership::read(self.snapshot());
        let mut physical = BTreeMap::new();
        for fact in facts {
            let key = key(&fact.binding);
            if !membership.live.contains(&key) {
                return Err(Failure::BindingChanged);
            }
            if physical
                .insert(key, fact)
                .is_some_and(|previous| previous != fact)
            {
                return Err(Failure::BindingChanged);
            }
        }
        self.panel_agent.maintain(&membership, &physical)?;
        let workbench_revision = self.snapshot().revision;
        let result = |outcome: Outcome| PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome,
        };
        let admit_result = |outcome: &Outcome| {
            serialize_workbench_response(&WorkbenchResponse::PanelAgent(result(outcome.clone())))
                .map(|_| ())
                .map_err(|_| Failure::ResultTooLarge)
        };
        match &request.action {
            Action::Read { panel_ids } => {
                let mut panels = Vec::new();
                for id in panel_ids {
                    let binding = membership.panels.get(id).ok_or(Failure::NotFound)?.clone();
                    let report = binding.as_ref().map(|b| {
                        self.panel_agent
                            .resources
                            .get(&key(b))
                            .map_or_else(Default::default, |r| r.reports.view())
                    });
                    panels.push(PanelAgentRow {
                        panel_id: id.clone(),
                        binding,
                        report,
                    });
                }
                let outcome = Outcome::Read {
                    workbench_revision,
                    report_revision: WorkbenchRevision::new(self.panel_agent.revision),
                    panels,
                };
                admit_result(&outcome)?;
                Ok(outcome)
            }
            Action::Report {
                panel_id,
                expected_binding,
                report,
            } => {
                let binding = membership
                    .panels
                    .get(panel_id)
                    .ok_or(Failure::NotFound)?
                    .as_ref()
                    .ok_or(Failure::Unbound)?;
                if binding != expected_binding {
                    return Err(Failure::BindingChanged);
                }
                let key = key(binding);
                let physical = physical.get(&key).ok_or(Failure::Unavailable)?;
                let prior = self.panel_agent.resources.get(&key);
                let current = prior.map_or_else(Reports::default, |r| r.reports.clone());
                let transition =
                    current.transition(event(report, absolute)?, self.panel_agent.time(now)?);
                let (next_bytes, bytes) = if transition.changed {
                    let next_bytes = retained_bytes(&transition.next)?;
                    let bytes = self
                        .panel_agent
                        .bytes
                        .checked_sub(prior.map_or(0, |r| r.bytes))
                        .and_then(|n| n.checked_add(next_bytes))
                        .filter(|n| *n <= MAXIMUM_BYTES)
                        .ok_or(Failure::ResourceLimit)?;
                    if next_bytes > 0
                        && prior.is_none()
                        && self.panel_agent.resources.len() >= MAXIMUM_RESOURCES
                    {
                        return Err(Failure::ResourceLimit);
                    }
                    (next_bytes, bytes)
                } else {
                    (prior.map_or(0, |r| r.bytes), self.panel_agent.bytes)
                };
                let revision = if transition.changed {
                    self.panel_agent
                        .revision
                        .checked_add(1)
                        .ok_or(Failure::RevisionExhausted)?
                } else {
                    self.panel_agent.revision
                };
                let outcome = Outcome::Reported {
                    workbench_revision,
                    report_revision: WorkbenchRevision::new(revision),
                    panel_id: panel_id.clone(),
                    binding: binding.clone(),
                    disposition: transition.disposition,
                    changed: transition.changed,
                    view: Box::new(transition.next.view()),
                };
                admit_result(&outcome)?;
                if transition.changed {
                    if next_bytes == 0 {
                        self.panel_agent.resources.remove(&key);
                    } else {
                        self.panel_agent.resources.insert(
                            key,
                            Resource {
                                physical: (*physical).clone(),
                                reports: transition.next,
                                explicit_revision: revision,
                                bytes: next_bytes,
                            },
                        );
                    }
                    self.panel_agent.bytes = bytes;
                    self.panel_agent.revision = revision;
                    self.panel_agent.origin.get_or_insert(now);
                }
                Ok(outcome)
            }
        }
    }
}
#[cfg(test)]
mod tests;

/// Trusted process facts; only a Host observation adapter may supply these to the Site owner.
/// Sample failure is not this value, and cannot be represented as a fabricated exit.
#[derive(Clone, Copy, Debug)]
pub struct TerminalAgentProcessObservation {
    pub observed_at: Instant,
    pub agent: Option<orchbee_agent_observation_protocol::AgentReportProvider>,
    pub fallback: Option<TerminalAgentScreenObservation>,
    pub exited: bool,
}
/// Fresh screen evidence associated with this exact process sample and its start time.
#[derive(Clone, Copy, Debug)]
pub struct TerminalAgentScreenObservation {
    pub state: orchbee_workbench_protocol::PanelAgentState,
    pub visible_blocker: bool,
}
/// Both report and metadata candidates have committed when this result is returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalAgentProcessOutcome {
    pub changed: bool,
    pub agent_released: bool,
    pub report_revision: WorkbenchRevision,
    pub metadata_revision: WorkbenchRevision,
}
impl PanelAgents {
    pub(super) fn blocks_metadata(
        &self,
        binding: &PresentationBinding,
        source: &str,
        patch: &orchbee_workbench_protocol::PanelMetadataPatch,
    ) -> bool {
        use orchbee_agent_observation_protocol::{
            AgentReportProvider as Provider, AgentReportSourcePolicy as Policy,
            agent_report_source_policy,
        };
        let Some(exited) = self
            .resources
            .get(&key(binding))
            .and_then(|r| r.reports.recent_exit_agent())
        else {
            return false;
        };
        patch
            .agent
            .as_ref()
            .and_then(|a| Provider::lookup(a.as_str()))
            == Some(exited)
            || agent_report_source_policy(source, exited.report_label()) != Policy::Custom
            || patch.applies_to_source.as_ref().is_some_and(|source| {
                agent_report_source_policy(source.as_str(), exited.report_label()) != Policy::Custom
            })
    }
}
impl WorkbenchAuthority {
    /// Per-binding explicit-report input stamp; process confirmations never advance it.
    /// The physical identity prevents a replacement from inheriting a previous stamp.
    #[must_use]
    pub fn terminal_agent_input_revision(&self, target: &TerminalAgentBinding) -> u64 {
        self.panel_agent
            .resources
            .get(&key(&target.binding))
            .filter(|r| &r.physical == target)
            .map_or(0, |r| r.explicit_revision)
    }
    /// Commits a trusted sample and its metadata cleanup together after all capacity/revision checks.
    /// `target` must still match an exact retained fact and committed primary or child membership.
    /// # Errors
    /// Refuses retired, rebound, unavailable, overflowing or over-capacity candidates before their commit.
    pub fn observe_terminal_agent(
        &mut self,
        target: &TerminalAgentBinding,
        facts: &[TerminalAgentBinding],
        observation: TerminalAgentProcessObservation,
        now: Instant,
    ) -> Result<TerminalAgentProcessOutcome, Failure> {
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if observation.observed_at > now {
            return Err(Failure::InvalidRequest);
        }
        let membership = ReportMembership::read(self.snapshot());
        let key = key(&target.binding);
        if !membership.live.contains(&key) {
            return Err(Failure::BindingChanged);
        }
        if facts.len() > MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let mut found = false;
        for fact in facts.iter().filter(|fact| fact.binding == target.binding) {
            if fact != target {
                return Err(Failure::BindingChanged);
            }
            found = true;
        }
        if !found {
            return Err(Failure::Unavailable);
        }
        let prior = self.panel_agent.resources.get(&key);
        // A changed physical generation starts from no report facts. No prior source may leak across it.
        let current = prior
            .filter(|r| &r.physical == target)
            .map_or_else(Reports::default, |r| r.reports.clone());
        let observation_tick = self.panel_agent.time_from(observation.observed_at, now)?;
        let transition = current.transition(
            Event::Process(super::agent_reports::Process {
                observed_at: observation_tick,
                agent: observation.agent,
                fallback: observation
                    .fallback
                    .map(|f| super::agent_reports::Fallback {
                        status: f.state,
                        visible_blocker: f.visible_blocker,
                    }),
                exited: observation.exited,
            }),
            self.panel_agent.time(now)?,
        );
        let metadata = transition
            .metadata_exit
            .map(|exit| {
                if exit.observed_at != observation_tick {
                    return Err(Failure::InvalidRequest);
                }
                self.panel_metadata
                    .prepare_exit(&target.binding, exit.agent, observation.observed_at)
                    .map_err(|reason| {
                        if reason
                            == orchbee_workbench_protocol::PanelMetadataFailure::RevisionExhausted
                        {
                            Failure::RevisionExhausted
                        } else {
                            Failure::ResourceLimit
                        }
                    })
            })
            .transpose()?
            .flatten();
        let changed = transition.changed || prior.is_some_and(|r| &r.physical != target);
        let (next_bytes, bytes, revision) = if changed {
            let next_bytes = retained_bytes(&transition.next)?;
            if next_bytes > 0
                && prior.is_none()
                && self.panel_agent.resources.len() >= MAXIMUM_RESOURCES
            {
                return Err(Failure::ResourceLimit);
            }
            let bytes = self
                .panel_agent
                .bytes
                .checked_sub(prior.map_or(0, |r| r.bytes))
                .and_then(|n| n.checked_add(next_bytes))
                .filter(|n| *n <= MAXIMUM_BYTES)
                .ok_or(Failure::ResourceLimit)?;
            let revision = self
                .panel_agent
                .revision
                .checked_add(1)
                .ok_or(Failure::RevisionExhausted)?;
            (next_bytes, bytes, revision)
        } else {
            (0, self.panel_agent.bytes, self.panel_agent.revision)
        };
        let any_changed = changed || metadata.is_some();
        if changed {
            if next_bytes == 0 {
                self.panel_agent.resources.remove(&key);
            } else {
                self.panel_agent.resources.insert(
                    key,
                    Resource {
                        physical: target.clone(),
                        reports: transition.next,
                        explicit_revision: prior
                            .filter(|r| &r.physical == target)
                            .map_or(0, |r| r.explicit_revision),
                        bytes: next_bytes,
                    },
                );
            }
            self.panel_agent.bytes = bytes;
            self.panel_agent.revision = revision;
            self.panel_agent.origin.get_or_insert(now);
        }
        if let Some(metadata) = metadata {
            self.panel_metadata.commit_exit(metadata);
        }
        Ok(TerminalAgentProcessOutcome {
            changed: any_changed,
            agent_released: transition.released,
            report_revision: WorkbenchRevision::new(revision),
            metadata_revision: self.panel_metadata.revision(),
        })
    }
}

impl WorkbenchAuthority {
    /// Retires explicit reports only for removed memberships or positively replaced retained generations.
    /// Missing Host facts preserve historical report state and never synthesize process exit.
    /// # Errors
    /// Refuses conflicting physical facts, excessive facts or exhausted report revisions.
    pub fn reconcile_terminal_agent_bindings(
        &mut self,
        facts: &[TerminalAgentBinding],
    ) -> Result<(), Failure> {
        if self.retired {
            return Err(Failure::Unavailable);
        }
        if facts.len() > MAXIMUM_RESOURCES {
            return Err(Failure::ResourceLimit);
        }
        let membership = ReportMembership::read(self.snapshot());
        let mut physical = BTreeMap::new();
        for fact in facts {
            let key = key(&fact.binding);
            if !membership.live.contains(&key) {
                return Err(Failure::BindingChanged);
            }
            if physical.insert(key, fact).is_some_and(|old| old != fact) {
                return Err(Failure::BindingChanged);
            }
        }
        self.panel_agent.maintain(&membership, &physical)
    }
}

impl WorkbenchAuthority {
    /// The caller proves current membership and physical generation under its commit guard.
    pub fn terminal_agent_name_owner(
        &self,
        physical: &TerminalAgentBinding,
    ) -> TerminalAgentNameOwner {
        self.panel_agent
            .resources
            .get(&key(&physical.binding))
            .filter(|resource| &resource.physical == physical)
            .map_or_else(TerminalAgentNameOwner::default, |resource| {
                resource.reports.name_owner()
            })
    }
}
