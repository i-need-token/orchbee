//! Exact logical/physical sources, bounded match edges, and correlated read-only Client queries.
use super::{WorkbenchClientSession, capture::Capture};
use crate::{
    local_terminal_service::semantic_events::OutputRead, site_coordinator::ProcessObservationTarget,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControl, ClientViewControlStatus,
    ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewportObservation,
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState, PanelAgentView,
    SemanticEvent, SemanticEventData as Data, SemanticEventFilter as Filter,
    SemanticEventKind as Kind, SemanticEventPosition, SemanticEventSource,
    SemanticEventsFailure as Failure, SemanticOutputMatch, SemanticOutputSource, SemanticPane,
    SiteId, TerminalPresentationId, WorkbenchClientId, WorkbenchRequestId, WorkbenchRevision,
};
use regex::{Regex, RegexBuilder};
use std::time::Instant;
pub(super) const MAXIMUM_PROBE_BYTES: usize = 128 * 1024;

struct Source {
    selector: SemanticEventSource,
    presentation_id: TerminalPresentationId,
    expected: ProcessObservationTarget,
}
impl Source {
    fn prepare(
        session: &WorkbenchClientSession<'_>,
        site: &SiteId,
        selector: &SemanticEventSource,
    ) -> Result<Self, Failure> {
        let (pane, _, expected) = session.service.terminals.event_source(site, selector)?;
        Ok(Self {
            selector: selector.clone(),
            presentation_id: pane.presentation_id,
            expected,
        })
    }
    fn current(
        &self,
        session: &WorkbenchClientSession<'_>,
        site: &SiteId,
    ) -> Result<(SemanticPane, WorkbenchRevision, ProcessObservationTarget), Failure> {
        let current = session
            .service
            .terminals
            .event_source(site, &self.selector)?;
        if current.2 != self.expected || current.0.presentation_id != self.presentation_id {
            return Err(Failure::SourceChanged);
        }
        Ok(current)
    }
}
enum Matcher {
    Substring(String),
    Regex(Regex),
}
impl Matcher {
    fn prepare(matcher: &SemanticOutputMatch) -> Result<Self, Failure> {
        match matcher {
            SemanticOutputMatch::Substring { value } => Ok(Self::Substring(value.clone())),
            SemanticOutputMatch::Regex { value } => RegexBuilder::new(value)
                .size_limit(1024 * 1024)
                .dfa_size_limit(1024 * 1024)
                .build()
                .map(Self::Regex)
                .map_err(|_| Failure::InvalidRegex),
        }
    }
    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Substring(value) => text.contains(value),
            Self::Regex(regex) => regex.is_match(text),
        }
    }
}
pub(super) struct Output {
    source: Source,
    kind: SemanticOutputSource,
    lines: u16,
    strip_ansi: bool,
    matcher: Matcher,
    matching: bool,
    observed: bool,
    client: Option<WorkbenchClientId>,
    query: Option<ClientViewControl>,
}
pub(super) struct Agent {
    source: Source,
    state: Option<PanelAgentState>,
    last: Option<PanelAgentView>,
    initial: Option<(SemanticPane, WorkbenchRevision, PanelAgentView)>,
}
pub(super) struct Scroll {
    source: Source,
    client: WorkbenchClientId,
    last: Option<ClientViewportObservation>,
    query: Option<ClientViewControl>,
}
pub(super) enum Probe {
    Output(Box<Output>),
    Agent(Box<Agent>),
    Scroll(Box<Scroll>),
}

pub(super) fn prepare(
    session: &WorkbenchClientSession<'_>,
    site: &SiteId,
    filters: &[Filter],
) -> Result<Vec<Probe>, Failure> {
    let mut probes = Vec::new();
    for filter in filters {
        match filter {
            Filter::OutputMatched {
                target,
                source,
                lines,
                matcher,
                strip_ansi,
                client_id,
            } => {
                // Compile before admitting any connection-owned state or physical effect.
                let matcher = Matcher::prepare(matcher)?;
                let source_admission = Source::prepare(session, site, target)?;
                if *source == SemanticOutputSource::Viewport {
                    check_primary(session, site, &source_admission)?;
                }
                probes.push(Probe::Output(Box::new(Output {
                    source: source_admission,
                    kind: *source,
                    lines: *lines,
                    strip_ansi: *strip_ansi,
                    matcher,
                    matching: false,
                    observed: false,
                    client: client_id.clone(),
                    query: None,
                })));
            }
            Filter::AgentStatusChanged { target, state } => {
                let source = Source::prepare(session, site, target)?;
                let (pane, revision, _) = source.current(session, site)?;
                let report = agent_report(session, site, &pane, &fresh_id()?)?;
                let initial = state
                    .is_some_and(|state| state == report.state)
                    .then(|| (pane, revision, report.clone()));
                probes.push(Probe::Agent(Box::new(Agent {
                    source,
                    state: *state,
                    last: Some(report),
                    initial,
                })));
            }
            Filter::ScrollChanged { target, client_id } => {
                let source = Source::prepare(session, site, target)?;
                check_primary(session, site, &source)?;
                probes.push(Probe::Scroll(Box::new(Scroll {
                    source,
                    client: client_id.clone(),
                    last: None,
                    query: None,
                })));
            }
            Filter::Event { .. } => {}
        }
    }
    Ok(probes)
}
fn check_primary(
    session: &WorkbenchClientSession<'_>,
    site: &SiteId,
    source: &Source,
) -> Result<(), Failure> {
    let primary = session.service.terminals.event_source(
        site,
        &SemanticEventSource {
            panel_id: source.selector.panel_id.clone(),
            presentation_id: None,
        },
    )?;
    if primary.2 != source.expected {
        return Err(Failure::InvalidIntent);
    }
    Ok(())
}
pub(super) fn journal_filters(filters: &[Filter], probes: &[Probe]) -> Vec<Filter> {
    filters
        .iter()
        .map(|filter| {
            if let Filter::AgentStatusChanged { target, state } = filter {
                let id = probes.iter().find_map(|probe| match probe {
                    Probe::Agent(agent) if &agent.source.selector == target => {
                        // The admitted primary identity is resolved once, so child reports do not borrow it.
                        Some(&agent.source)
                    }
                    _ => None,
                });
                if let Some(source) = id {
                    return Filter::AgentStatusChanged {
                        target: SemanticEventSource {
                            panel_id: source.selector.panel_id.clone(),
                            presentation_id: Some(source.presentation_id.clone()),
                        },
                        state: *state,
                    };
                }
            }
            filter.clone()
        })
        .collect()
}
impl Probe {
    pub(super) fn has_baseline(&self) -> bool {
        match self {
            Self::Output(output) => output.observed,
            Self::Agent(agent) => agent.last.is_some(),
            Self::Scroll(scroll) => scroll.last.is_some(),
        }
    }
    pub(super) fn initial(
        &mut self,
        session: &WorkbenchClientSession<'_>,
        subscription: &WorkbenchRequestId,
        sequence: u64,
    ) -> Result<Option<SemanticEvent>, Failure> {
        let Self::Agent(agent) = self else {
            return Ok(None);
        };
        let Some((pane, revision, report)) = agent.initial.as_ref() else {
            return Ok(None);
        };
        let event = event(
            session,
            subscription,
            sequence,
            *revision,
            Kind::PaneAgentStatusChanged,
            Data::Agent {
                pane: pane.clone(),
                report: report.clone(),
                exited: false,
            },
        )?;
        agent.initial = None;
        Ok(Some(event))
    }
    pub(super) fn journal_observed(&mut self, event: &SemanticEvent) {
        if let Self::Agent(agent) = self
            && let Data::Agent { pane, report, .. } = &event.data
            && pane.panel_id == agent.source.selector.panel_id
            && agent.source.presentation_id == pane.presentation_id
            && pane.binding.as_ref() == Some(&agent.source.expected.binding)
        {
            agent.last = Some(report.clone());
        }
    }
    pub(super) fn observe(
        &mut self,
        session: &WorkbenchClientSession<'_>,
        site: &SiteId,
        subscription: &WorkbenchRequestId,
        sequence: u64,
        index: usize,
        capture: &mut Capture,
    ) -> Result<Option<SemanticEvent>, Failure> {
        match self {
            Self::Agent(agent) => {
                let (pane, revision, _) = agent.source.current(session, site)?;
                let report = agent_report(session, site, &pane, subscription)?;
                let changed = agent
                    .last
                    .as_ref()
                    .is_none_or(|last| last.agent != report.agent || last.state != report.state);
                let matched = agent.state.is_none_or(|state| state == report.state);
                let event = if changed && matched {
                    Some(event(
                        session,
                        subscription,
                        sequence,
                        revision,
                        Kind::PaneAgentStatusChanged,
                        Data::Agent {
                            pane,
                            report: report.clone(),
                            exited: false,
                        },
                    )?)
                } else {
                    None
                };
                agent.last = Some(report);
                Ok(event)
            }
            Self::Scroll(scroll) => {
                let (pane, revision, _) = scroll.source.current(session, site)?;
                let Some(observation) =
                    viewport(session, &scroll.client, &pane, &mut scroll.query)?
                else {
                    return Ok(None);
                };
                let same = match (&scroll.last, &observation) {
                    (
                        Some(ClientViewportObservation::Observed {
                            mode: old_mode,
                            scroll: old_scroll,
                            ..
                        }),
                        ClientViewportObservation::Observed { mode, scroll, .. },
                    ) => old_mode == mode && old_scroll == scroll,
                    _ => false,
                };
                let observed = if same || scroll.last.is_none() {
                    None
                } else {
                    Some(event(
                        session,
                        subscription,
                        sequence,
                        revision,
                        Kind::PaneScrollChanged,
                        Data::Scroll {
                            client_id: scroll.client.clone(),
                            pane,
                            observation: observation.clone(),
                        },
                    )?)
                };
                scroll.last = Some(observation);
                Ok(observed)
            }
            Self::Output(output) => {
                if capture.busy_for_another(index) {
                    return Ok(None);
                }
                let (pane, revision, target) = output.source.current(session, site)?;
                let selected_viewport = if output.kind == SemanticOutputSource::Viewport
                    && !capture.pending_for(index)
                {
                    let client = output.client.as_ref().ok_or(Failure::InvalidIntent)?;
                    let Some(observation) = viewport(session, client, &pane, &mut output.query)?
                    else {
                        return Ok(None);
                    };
                    Some(observation)
                } else {
                    None
                };
                let Some(sample) = capture.sample(
                    index,
                    std::sync::Arc::clone(&session.service.terminals),
                    &target,
                    &pane,
                    revision,
                    OutputRead {
                        source: output.kind,
                        lines: output.lines,
                        strip_ansi: output.strip_ansi,
                        viewport: selected_viewport,
                    },
                )?
                else {
                    return Ok(None);
                };
                output.source.current(session, site)?;
                output.observed = true;
                let matched = sample
                    .lines
                    .into_iter()
                    .find(|line| output.matcher.matches(line));
                let matching = matched.is_some();
                let observed = match matched {
                    Some(line) if !output.matching => Some(event(
                        session,
                        subscription,
                        sequence,
                        sample.workbench_revision,
                        Kind::PaneOutputMatched,
                        Data::Output {
                            pane: sample.pane,
                            content_generation: sample.content_generation,
                            content_revision: sample.content_revision,
                            matched_line: Some(line),
                        },
                    )?),
                    _ => None,
                };
                output.matching = matching;
                Ok(observed)
            }
        }
    }
}
fn agent_report(
    session: &WorkbenchClientSession<'_>,
    site: &SiteId,
    pane: &SemanticPane,
    request_id: &WorkbenchRequestId,
) -> Result<PanelAgentView, Failure> {
    let request = PanelAgentRequest {
        protocol_version: orchbee_workbench_protocol::ProtocolVersion,
        request_id: request_id.clone(),
        site_id: site.clone(),
        action: PanelAgentAction::ReadPresentations {
            presentation_ids: vec![pane.presentation_id.clone()],
        },
    };
    let result = session
        .service
        .terminals
        .execute_panel_agent(request.clone());
    if result.request_id != request.request_id
        || result.site_id != request.site_id
        || result.validate().is_err()
    {
        return Err(Failure::Unavailable);
    }
    let PanelAgentOutcome::PresentationsRead { presentations, .. } = result.outcome else {
        return Err(Failure::SourceUnavailable);
    };
    if presentations.len() != 1 {
        return Err(Failure::Unavailable);
    }
    let row = presentations
        .into_iter()
        .next()
        .ok_or(Failure::SourceUnavailable)?;
    if row.presentation_id != pane.presentation_id
        || row.panel.panel_id != pane.panel_id
        || row.panel.binding != pane.binding
    {
        return Err(Failure::SourceChanged);
    }
    row.panel.report.ok_or(Failure::SourceUnavailable)
}
fn event(
    session: &WorkbenchClientSession<'_>,
    subscription: &WorkbenchRequestId,
    sequence: u64,
    revision: WorkbenchRevision,
    kind: Kind,
    data: Data,
) -> Result<SemanticEvent, Failure> {
    let context = match &data {
        Data::Agent { pane, .. } | Data::Output { pane, .. } | Data::Scroll { pane, .. } => {
            &pane.terminal_context_id
        }
        _ => return Err(Failure::InvalidIntent),
    };
    let journal = session
        .service
        .terminals
        .event_journal()
        .ok_or(Failure::Unavailable)?;
    let event = SemanticEvent {
        workspace_id: journal.workspace_for_context(context),
        position: SemanticEventPosition::Probe {
            subscription_id: subscription.clone(),
            sequence: WorkbenchRevision::new(sequence),
        },
        workbench_revision: revision,
        event: kind,
        data,
    };
    event.validate().map_err(|_| Failure::Unavailable)?;
    if serde_json::to_vec(&event)
        .map_err(|_| Failure::Unavailable)?
        .len()
        > MAXIMUM_PROBE_BYTES
    {
        return Err(Failure::SourceUnavailable);
    }
    Ok(event)
}
fn fresh_id() -> Result<WorkbenchRequestId, Failure> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::Unavailable)?;
    WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::Unavailable)
}
fn viewport(
    session: &WorkbenchClientSession<'_>,
    client_id: &WorkbenchClientId,
    pane: &SemanticPane,
    pending: &mut Option<ClientViewControl>,
) -> Result<Option<ClientViewportObservation>, Failure> {
    let mut clients = session
        .service
        .client_views
        .try_lock()
        .map_err(|_| Failure::Busy)?;
    let now = Instant::now();
    let ClientViewOutcome::Found { client } = clients.execute(
        session.generation,
        ClientViewAction::Get {
            client_id: client_id.clone(),
        },
        now,
    ) else {
        return Err(Failure::SourceUnavailable);
    };
    if client.availability != ClientViewAvailability::Current
        || !matches!(
            client.view.phase,
            ClientViewPhase::Ready | ClientViewPhase::Busy
        )
        || !client.view.selection.as_ref().is_some_and(|selected| {
            selected.terminal_context_id == pane.terminal_context_id
                && selected.tab_id.as_ref() == Some(&pane.tab_id)
        })
    {
        return Err(Failure::SourceUnavailable);
    }
    if let Some(command) = pending.as_ref() {
        let result = clients.execute(
            session.generation,
            ClientViewAction::Read {
                operation_id: command.operation_id.clone(),
            },
            now,
        );
        let ClientViewOutcome::Operation { operation } = result else {
            return Err(Failure::SourceUnavailable);
        };
        if operation.command != *command || operation.validate().is_err() {
            return Err(Failure::SourceChanged);
        }
        match &operation.status {
            ClientViewControlStatus::Queued {} | ClientViewControlStatus::Delivered {} => {
                return Ok(None);
            }
            ClientViewControlStatus::ViewportRead { viewport, view } => {
                let current =
                    command.expected_view_revision == client.view_revision && view == &client.view;
                let row = viewport.panels.first().ok_or(Failure::SourceUnavailable)?;
                let expected = pane.binding.as_ref().ok_or(Failure::SourceUnavailable)?;
                if row.panel_id != pane.panel_id
                    || row.primary_terminal_id.as_ref() != Some(&expected.terminal_id)
                {
                    return Err(Failure::SourceChanged);
                }
                let observation = row.observation.clone();
                clients.finish_observation(&command.operation_id, now);
                *pending = None;
                if !current {
                    return Ok(None);
                }
                return match observation {
                    ClientViewportObservation::Observed { .. } => Ok(Some(observation)),
                    ClientViewportObservation::Pending { .. } => Ok(None),
                    ClientViewportObservation::Unavailable { .. } => {
                        Err(Failure::SourceUnavailable)
                    }
                };
            }
            ClientViewControlStatus::Rejected { .. } => {
                clients.finish_observation(&command.operation_id, now);
                *pending = None;
                return Ok(None);
            }
            _ => return Err(Failure::SourceUnavailable),
        }
    }
    if clients.observation_busy(client_id) {
        return Ok(None);
    }
    let command = ClientViewControl {
        operation_id: fresh_id()?,
        client_id: client_id.clone(),
        expected_view_revision: client.view_revision,
        minimum_workbench_revision: client.view.workbench_revision,
        intent: ClientViewIntent::Viewports {
            panel_ids: vec![pane.panel_id.clone()],
        },
    };
    let ClientViewOutcome::Operation { operation } = clients.execute(
        session.generation,
        ClientViewAction::Submit {
            command: Box::new(command.clone()),
        },
        now,
    ) else {
        return Err(Failure::SourceUnavailable);
    };
    if operation.command != command || operation.validate().is_err() {
        return Err(Failure::SourceChanged);
    }
    if !matches!(
        operation.status,
        ClientViewControlStatus::Queued {} | ClientViewControlStatus::Delivered {}
    ) {
        return Err(Failure::SourceUnavailable);
    }
    *pending = Some(command);
    Ok(None)
}
