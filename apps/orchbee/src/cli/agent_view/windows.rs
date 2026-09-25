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
    selected: &WorkbenchClientId,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
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
        let attention = read(
            &mut client,
            &snapshot,
            selected,
            &id,
            deadline,
            &mut signals,
        )?;
        let canonical =
            query::read_agent_observation(&mut client, &id, deadline, signals.poll_fd())?;
        let unchanged = query::attention_covers_observation(
            &mut client,
            &attention.attention,
            &canonical,
            deadline,
            signals.poll_fd(),
        )
        .map_err(|failure| failure.failure_schema(crate::cli::schema::AGENT_FAILURE_ID))?;
        check(deadline, &mut signals)?;
        let view = attention.join(&snapshot, &canonical, unchanged)?;
        Ok((
            results::agent_source(&canonical.request_id),
            results::Projection::AgentClientGet(Box::new(results::AgentClientGetDoc {
                revision: canonical.revision,
                session: canonical.session,
                client: view,
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
            Err(Failure::new(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
        Err(failure) => failure_output(failure, Some(format), locale),
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
    check(deadline, signals)?;
    let result = exchange(
        client,
        &snapshot.site_id,
        ClientViewAction::Get {
            client_id: selected.clone(),
        },
        deadline,
        signals,
    )?;
    check(deadline, signals)?;
    let record = match result.outcome {
        ClientViewOutcome::Found { client } if client.client_id == *selected => client,
        ClientViewOutcome::NotFound {} => return Err(Failure::new(FailureKind::NotFound)),
        ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
        ClientViewOutcome::Unavailable { .. } => {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
    };
    if record.availability != ClientViewAvailability::Current {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    }
    let command = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: selected.clone(),
        expected_view_revision: record.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent: ClientViewIntent::AgentAttention {
            session_ids: vec![session.clone()],
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
    )?;
    check(deadline, signals)?;
    if answer.exit_code() != 0 {
        if let Some(reply) = &answer.reply {
            match reply.outcome {
                ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
                ClientViewOutcome::NotFound {} => return Err(Failure::new(FailureKind::NotFound)),
                _ => {}
            }
        }
        return Err(Failure::new(match answer.exit_code() {
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
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    };
    if operation.command != command {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    match operation.status {
        ClientViewControlStatus::AttentionRead { attention, view } => Ok(ClientAgentRead {
            client_id: selected.clone(),
            operation_id: command.operation_id,
            view,
            attention: *attention,
        }),
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}

impl ClientAgentRead {
    pub(in crate::cli) fn join(
        self,
        snapshot: &Snapshot,
        canonical: &query::AgentObservationRead,
        unchanged: bool,
    ) -> Result<results::AgentClientViewDoc, Failure> {
        use results::AgentClientVersionMatch;
        if self.attention.revision.value() > canonical.revision.value() {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        let [entry] = self.attention.entries.as_slice() else {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        };
        let agent = &canonical.session;
        if entry.session_id != agent.snapshot.session_id {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        let version = entry.state.as_ref().filter(|state| {
            unchanged
                && state.generation == agent.snapshot.observation.generation
                && state.sequence == agent.snapshot.observation.sequence
        });
        let state = if version.is_some() {
            AgentClientVersionMatch::Current
        } else if entry.state.is_none() {
            AgentClientVersionMatch::Missing
        } else {
            AgentClientVersionMatch::Stale
        };
        let results::Projection::PanelList(panels) =
            results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
        else {
            return Err(Failure::new(FailureKind::InternalFailure));
        };
        let focused = agent.snapshot.lifecycle
            == orchbee_agent_observation_protocol::AgentLifecycle::Live
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
                        && agent
                            .snapshot
                            .presentation_attachments
                            .iter()
                            .any(|binding| {
                                binding.navigable
                                    && binding.presentation_id.as_str()
                                        == panel.primary_presentation_id
                            })
                })
            });
        Ok(results::AgentClientViewDoc {
            client_id: self.client_id,
            operation_id: self.operation_id,
            phase: self.view.phase,
            outer_focused: self.attention.outer_focused,
            focused,
            attention_revision: self.attention.revision,
            state,
            seen: version.map(|state| state.seen),
            state_change_revision: version.map(|state| state.state_change_revision),
            status: projected_status(agent, version.map(|state| state.seen)),
        })
    }
}

fn projected_status(
    agent: &orchbee_agent_observation_protocol::AgentSessionDetail,
    seen: Option<bool>,
) -> Option<&'static str> {
    if matches!(agent.status, AgentSemanticStatus::Ended { .. }) {
        return Some("ended");
    }
    match agent.snapshot.observation.freshness {
        AgentObservationFreshness::Stale => return Some("stale"),
        AgentObservationFreshness::Disconnected => return Some("disconnected"),
        AgentObservationFreshness::Fresh => {}
    }
    match agent.status {
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
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some()
    {
        Err(Failure::new(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(Failure::new(FailureKind::Timeout))
    } else {
        Ok(())
    }
}
