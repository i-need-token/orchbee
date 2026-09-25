pub(super) mod list;
mod runtime;
use super::*;
use crate::cli::{
    Failure, FailureKind,
    client_control::windows::{exchange, submit_control},
    query, results,
    source::{ArmedSignals, Finish, SignalWatch},
    success_output,
};
use orchbee_agent_observation_protocol::{
    AgentObservationFreshness, AgentSemanticStatus, AgentSessionId,
};
use orchbee_workbench_protocol::{
    ClientAgentAttentionSnapshot, ClientViewAction, ClientViewAvailability, ClientViewControl,
    ClientViewControlStatus, ClientViewIntent, ClientViewObservation, ClientViewOutcome,
    ClientViewPhase, Snapshot, WorkbenchRequestId,
};
use std::time::{Duration, Instant};

pub(super) fn run(
    command: &QueryCommand,
    target: &AgentTarget,
    selected: Option<&WorkbenchClientId>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                view_failure(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    // The session is dropped by this closure before the one signal arbitration.
    let result = (|| {
        let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
        check(deadline, &mut signals)?;
        let id = query::agent_attention_target(
            &mut client,
            &snapshot,
            target,
            deadline,
            signals.poll_fd(),
        )?;
        let attention = selected
            .map(|selected| {
                read(
                    &mut client,
                    &snapshot,
                    selected,
                    &id,
                    deadline,
                    &mut signals,
                )
            })
            .transpose()?;
        let initial = query::read_agent_observation(&mut client, &id, deadline, signals.poll_fd())?;
        let (canonical, runtime) =
            runtime::read(&mut client, &snapshot, initial, deadline, &mut signals)?;
        let view = attention
            .map(|attention| {
                attention
                    .corroborate(&mut client, &snapshot, &canonical, deadline, &mut signals)
                    .map(Box::new)
            })
            .transpose()?;
        check(deadline, &mut signals)?;
        Ok((
            results::agent_source(&canonical.request_id),
            results::Projection::AgentInspection(Box::new(results::AgentInspectionDoc {
                revision: canonical.revision,
                session: canonical.session,
                client: view,
                runtime,
            })),
        ))
    })();
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(view_failure(FailureKind::Cancelled))
        }
        Err(_) => Err(view_failure(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
        Err(failure) => failure_output(classify(failure), Some(format), locale),
    };
    output.finalization = finalization;
    output
}

pub(in crate::cli) struct ClientAgentRead {
    client_id: WorkbenchClientId,
    operation_id: WorkbenchRequestId,
    view: ClientViewObservation,
    attention: ClientAgentAttentionSnapshot,
}

pub(in crate::cli) fn read(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    session: &AgentSessionId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientAgentRead, Failure> {
    read_many(
        client,
        snapshot,
        selected,
        std::slice::from_ref(session),
        deadline,
        signals,
    )
}
fn current_client(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Box<orchbee_workbench_protocol::ClientViewRecord>, Failure> {
    check(deadline, signals)?;
    let result = exchange(
        client,
        &snapshot.site_id,
        ClientViewAction::Get {
            client_id: selected.clone(),
        },
        deadline,
        signals,
    )
    .map_err(classify)?;
    check(deadline, signals)?;
    let record = match result.outcome {
        ClientViewOutcome::Found { client } if client.client_id == *selected => client,
        ClientViewOutcome::NotFound {} => {
            return Err(crate::cli::client_control::selection::failure(
                FailureKind::NotFound,
                "client-not-found",
                vec![selected.to_string()],
            ));
        }
        ClientViewOutcome::Denied {} => return Err(view_failure(FailureKind::Denied)),
        ClientViewOutcome::Unavailable { .. } => {
            return Err(view_failure(FailureKind::ServiceUnavailable));
        }
        _ => return Err(view_failure(FailureKind::ProtocolInvalid)),
    };
    if record.availability != ClientViewAvailability::Current {
        return Err(crate::cli::client_control::selection::failure(
            FailureKind::ServiceUnavailable,
            "client-stale",
            vec![selected.to_string()],
        ));
    }
    Ok(record)
}
fn read_many(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    sessions: &[AgentSessionId],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientAgentRead, Failure> {
    let record = current_client(client, snapshot, selected, deadline, signals)?;
    let command = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: selected.clone(),
        expected_view_revision: record.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::AgentAttention {
            session_ids: sessions.to_vec(),
        },
    };
    // This existing owner waits on the retained operation and authenticates the
    // changes cursor before returning any AttentionRead payload.
    let answer = submit_control(
        client,
        &snapshot.site_id,
        command.clone(),
        deadline,
        signals,
    )
    .map_err(classify)?;
    check(deadline, signals)?;
    if answer.exit_code() != 0 {
        if let Some(reply) = &answer.reply {
            match reply.outcome {
                ClientViewOutcome::Denied {} => return Err(view_failure(FailureKind::Denied)),
                ClientViewOutcome::NotFound {} => return Err(view_failure(FailureKind::NotFound)),
                _ => {}
            }
        }
        return Err(view_failure(match answer.exit_code() {
            3 => FailureKind::Rejected,
            4 => FailureKind::ServiceUnavailable,
            5 | 8 => FailureKind::TransportLoss,
            7 => FailureKind::Timeout,
            130 => FailureKind::Cancelled,
            _ => FailureKind::InternalFailure,
        }));
    }
    let Some(orchbee_workbench_protocol::WorkbenchClientViewResult {
        outcome: ClientViewOutcome::Operation { operation },
        ..
    }) = answer.reply
    else {
        return Err(view_failure(FailureKind::ProtocolInvalid));
    };
    if operation.command != command {
        return Err(view_failure(FailureKind::ProtocolInvalid));
    }
    match operation.status {
        ClientViewControlStatus::AttentionRead { attention, view } => Ok(ClientAgentRead {
            client_id: selected.clone(),
            operation_id: command.operation_id,
            view,
            attention: *attention,
        }),
        _ => Err(view_failure(FailureKind::ProtocolInvalid)),
    }
}

impl ClientAgentRead {
    pub(in crate::cli) fn corroborate(
        self,
        client: &mut crate::local_client::LocalClient,
        snapshot: &Snapshot,
        canonical: &query::AgentObservationRead,
        deadline: Instant,
        signals: &mut ArmedSignals,
    ) -> Result<results::AgentClientViewDoc, Failure> {
        let unchanged = query::attention_covers_observation(
            client,
            &self.attention,
            canonical,
            deadline,
            signals.poll_fd(),
        )
        .map_err(|failure| failure.failure_schema(crate::cli::schema::AGENT_FAILURE_ID))?;
        check(deadline, signals)?;
        self.join(snapshot, canonical, unchanged)
    }

    pub(in crate::cli) fn join(
        self,
        snapshot: &Snapshot,
        canonical: &query::AgentObservationRead,
        unchanged: bool,
    ) -> Result<results::AgentClientViewDoc, Failure> {
        if self.attention.entries.len() != 1
            || self.attention.entries[0].session_id != canonical.session.snapshot.session_id
        {
            return Err(view_failure(FailureKind::ProtocolInvalid));
        }
        self.project(
            snapshot,
            canonical.revision,
            AgentViewFacts::detail(&canonical.session),
            unchanged,
        )
    }
    fn project(
        &self,
        snapshot: &Snapshot,
        revision: orchbee_agent_observation_protocol::AgentSafeInteger,
        facts: AgentViewFacts<'_>,
        unchanged: bool,
    ) -> Result<results::AgentClientViewDoc, Failure> {
        use results::AgentClientVersionMatch;
        if self.attention.revision.value() > revision.value() {
            return Err(view_failure(FailureKind::ProtocolInvalid));
        }
        let entry = self
            .attention
            .entries
            .iter()
            .find(|entry| &entry.session_id == facts.session_id);
        let version = entry
            .and_then(|entry| entry.state.as_ref())
            .filter(|state| {
                unchanged
                    && state.generation == facts.observation.generation
                    && state.sequence == facts.observation.sequence
            });
        let state = if version.is_some() {
            AgentClientVersionMatch::Current
        } else if entry.is_none_or(|entry| entry.state.is_none()) {
            AgentClientVersionMatch::Missing
        } else {
            AgentClientVersionMatch::Stale
        };
        let results::Projection::PanelList(panels) =
            results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
        else {
            return Err(view_failure(FailureKind::InternalFailure));
        };
        let focused = facts.lifecycle == orchbee_agent_observation_protocol::AgentLifecycle::Live
            && self.view.phase == ClientViewPhase::Ready
            && self.attention.outer_focused != Some(false)
            && self.view.selection.as_ref().is_some_and(|selection| {
                panels.iter().any(|panel| {
                    selection.terminal_context_id.as_str() == panel.terminal_context_id
                        && selection
                            .panel_id
                            .as_ref()
                            .is_some_and(|selected| selected.as_str() == panel.panel_id)
                        && selection
                            .tab_id
                            .as_ref()
                            .is_some_and(|tab| tab.as_str() == panel.tab_id)
                        && facts.attachments.iter().any(|binding| {
                            binding.navigable
                                && binding.presentation_id.as_str() == panel.primary_presentation_id
                        })
                })
            });
        Ok(results::AgentClientViewDoc {
            client_id: self.client_id.clone(),
            operation_id: self.operation_id.clone(),
            phase: self.view.phase,
            outer_focused: self.attention.outer_focused,
            focused,
            attention_revision: self.attention.revision,
            state,
            seen: version.map(|state| state.seen),
            state_change_revision: version.map(|state| state.state_change_revision),
            status: projected_status(&facts, version.map(|state| state.seen)),
        })
    }
}

struct AgentViewFacts<'a> {
    session_id: &'a AgentSessionId,
    lifecycle: orchbee_agent_observation_protocol::AgentLifecycle,
    observation: &'a orchbee_agent_observation_protocol::AgentObservation,
    attachments: &'a [orchbee_agent_observation_protocol::AgentPresentationAttachment],
    status: &'a AgentSemanticStatus,
}
impl<'a> AgentViewFacts<'a> {
    fn detail(detail: &'a orchbee_agent_observation_protocol::AgentSessionDetail) -> Self {
        let snapshot = &detail.snapshot;
        Self {
            session_id: &snapshot.session_id,
            lifecycle: snapshot.lifecycle,
            observation: &snapshot.observation,
            attachments: &snapshot.presentation_attachments,
            status: &detail.status,
        }
    }
    fn summary(summary: &'a orchbee_agent_observation_protocol::AgentSessionSummary) -> Self {
        Self {
            session_id: &summary.session_id,
            lifecycle: summary.lifecycle,
            observation: &summary.observation,
            attachments: &summary.presentation_attachments,
            status: &summary.status,
        }
    }
}

fn projected_status(facts: &AgentViewFacts<'_>, seen: Option<bool>) -> Option<&'static str> {
    if matches!(facts.status, AgentSemanticStatus::Ended { .. }) {
        return Some("ended");
    }
    match facts.observation.freshness {
        AgentObservationFreshness::Stale => return Some("stale"),
        AgentObservationFreshness::Disconnected => return Some("disconnected"),
        AgentObservationFreshness::Fresh => {}
    }
    match facts.status {
        AgentSemanticStatus::Idle { .. }
        | AgentSemanticStatus::Succeeded { .. }
        | AgentSemanticStatus::Failed { .. }
        | AgentSemanticStatus::Canceled { .. }
        | AgentSemanticStatus::Interrupted { .. } => {
            seen.map(|seen| if seen { "idle" } else { "done" })
        }
        AgentSemanticStatus::Working { .. } => Some("working"),
        AgentSemanticStatus::NeedsAttention { .. } => Some("blocked"),
        AgentSemanticStatus::Unknown { .. } => Some("unknown"),
        AgentSemanticStatus::Ended { .. } => Some("ended"),
    }
}

fn check(deadline: Instant, signals: &mut ArmedSignals) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| view_failure(FailureKind::InternalFailure))?
        .is_some()
    {
        Err(view_failure(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(view_failure(FailureKind::Timeout))
    } else {
        Ok(())
    }
}

// Raw Client exchanges have no Agent source or receipt to publish. Give those
// failures the composition boundary, preserving any already classified owner.
fn classify(failure: Failure) -> Failure {
    if failure.failure_schema.is_none() && failure.details.is_none() && failure.source.is_none() {
        failure
            .version(2)
            .failure_schema("orchbee.cli.failure.client-control")
            .details(crate::cli::stage_details("client-agent-read"))
    } else {
        failure
    }
}
fn view_failure(kind: FailureKind) -> Failure {
    classify(Failure::new(kind))
}
