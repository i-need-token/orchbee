//! Compose canonical Agent identity with an exact Workbench terminal binding.
//! This owner resolves only; existing terminal owners retain observation/input.

use super::*;
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentSafeInteger, AgentSessionDetail, AgentSessionSelector,
};

pub(crate) fn read_agent_terminal(
    target: &AgentSessionSelector,
    history: Option<u16>,
    presentation: super::super::results::ReadPresentation,
) -> Result<(serde_json::Value, Projection), Failure> {
    let (mut client, deadline, snapshot) = snapshot_session()?;
    let target = agent_terminal_target(&mut client, &snapshot, target, deadline)?;
    read_terminal_on_session(client, deadline, &snapshot, &target, history, presentation)
}

/// Resolve once, get once, and refuse a missing or ambiguous live binding.
/// The caller keeps the admission and original deadline for the terminal operation.
pub(crate) fn agent_terminal_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentSessionSelector,
    deadline: Instant,
) -> Result<Target, Failure> {
    resolve_agent_terminal(client, snapshot, target, deadline).map(|resolved| resolved.terminal)
}

pub(crate) struct AgentObservationRead {
    pub(crate) request_id: ObapRequestId,
    pub(crate) revision: AgentSafeInteger,
    pub(crate) session: Box<AgentSessionDetail>,
}

pub(crate) struct ResolvedAgentTerminal {
    pub(crate) terminal: Target,
    pub(crate) observation: AgentObservationRead,
}

pub(crate) fn resolve_agent_terminal(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentSessionSelector,
    deadline: Instant,
) -> Result<ResolvedAgentTerminal, Failure> {
    let session_id = agent_names::resolve_on_session(client, target, deadline, None)?;
    let observation = read_agent_observation(client, &session_id, deadline, None)?;
    let terminal = select_terminal(snapshot, &observation.session)?;
    Ok(ResolvedAgentTerminal {
        terminal,
        observation,
    })
}

pub(crate) fn read_agent_observation(
    client: &mut LocalClient,
    session_id: &AgentSessionId,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<AgentObservationRead, Failure> {
    let request_id = obap_request_id()?;
    let source = super::super::results::agent_source(&request_id);
    let result = client
        .submit_agent_read_before_cancellable(
            AgentReadRequest::Get {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
            },
            deadline,
            wake,
        )
        .map_err(|error| {
            classify_agent_read(error, AgentReadOperation::Get)
                .failure_schema(super::super::schema::AGENT_FAILURE_ID)
        })?;
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
    }
    let AgentReadResult::Get { result, .. } = result else {
        return Err(agent_protocol_invalid(AgentReadOperation::Get)
            .failure_schema(super::super::schema::AGENT_FAILURE_ID));
    };
    match result.outcome {
        AgentGetOutcome::Found { session, revision } => {
            if session.snapshot.session_id != *session_id {
                return Err(agent_protocol_invalid(AgentReadOperation::Get)
                    .failure_schema(super::super::schema::AGENT_FAILURE_ID));
            }
            Ok(AgentObservationRead {
                request_id,
                revision,
                session,
            })
        }
        AgentGetOutcome::Rejected { reason } => {
            Err(
                agent_session_rejection(AgentReadOperation::Get, reason, session_id)
                    .source(source)
                    .failure_schema(super::super::schema::AGENT_FAILURE_ID),
            )
        }
        AgentGetOutcome::Unavailable { .. } => Err(Failure::new(FailureKind::ServiceUnavailable)
            .details(agent_unavailable_details(AgentReadOperation::Get))
            .source(source)
            .failure_schema(super::super::schema::AGENT_FAILURE_ID)),
    }
}

fn select_terminal(snapshot: &Snapshot, session: &AgentSessionDetail) -> Result<Target, Failure> {
    let matches: Vec<_> = super::super::results::terminal_rows(snapshot)
        .into_iter()
        .filter(|row| {
            session.snapshot.lifecycle == AgentLifecycle::Live
                && session
                    .snapshot
                    .presentation_attachments
                    .iter()
                    .any(|attachment| {
                        attachment.navigable
                            && attachment.presentation_id.as_str() == row.presentation_id
                    })
        })
        .collect();
    let target = Target::Label(session.snapshot.session_id.as_str().to_owned());
    let failure = match matches.as_slice() {
        [row] => return Ok(Target::Id(row.terminal_id.clone())),
        [] => super::super::results::not_found(Resource::Terminal, &target),
        _ => super::super::results::ambiguous(
            Resource::Terminal,
            &target,
            matches.into_iter().map(|row| row.terminal_id).collect(),
        ),
    };
    Err(failure.source(super::super::results::snapshot_source(snapshot)))
}
