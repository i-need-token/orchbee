//! Compose canonical Agent identity with an exact Workbench terminal binding.
//! This owner resolves only; existing terminal owners retain observation/input.

use super::*;
use crate::invocation::AgentTarget;
use orchbee_agent_observation_protocol::{AgentLifecycle, AgentSafeInteger, AgentSessionDetail};

pub(crate) fn read_agent_terminal(
    target: &AgentTarget,
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
    target: &AgentTarget,
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

/// A read-only preflight, not an atomic process lease over the later send.
#[cfg(windows)]
pub(crate) fn verify_agent_terminal(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selected: ResolvedAgentTerminal,
    deadline: Instant,
) -> Result<ResolvedAgentTerminal, Failure> {
    use super::super::processes::{Selection, interpret, provider};
    let before = &selected.observation.session.snapshot;
    let (source, document) =
        read_terminal_processes(client, deadline, snapshot, &selected.terminal)?;
    let view = interpret(&document.info);
    let expected = provider(before.provider_id.as_str())
        .filter(|canonical| *canonical == before.provider_id.as_str());
    let detected = view.foreground.provider;
    let refusal = |reason: &str| {
        Failure::new(FailureKind::Rejected)
            .failure_schema("orchbee.cli.failure.agent-runtime")
            .source(source.clone())
            .details(serde_json::Map::from_iter([
                ("stage".into(), serde_json::json!("agent-runtime")),
                ("reason".into(), serde_json::json!(reason)),
                (
                    "sessionId".into(),
                    serde_json::json!(before.session_id.as_str()),
                ),
                (
                    "expectedProvider".into(),
                    serde_json::json!(before.provider_id.as_str()),
                ),
                ("detectedProvider".into(), serde_json::json!(detected)),
            ]))
    };
    if expected.is_none() {
        return Err(refusal("agent-provider-unsupported"));
    }
    match view.foreground.selection {
        Selection::Ambiguous => return Err(refusal("agent-runtime-ambiguous")),
        Selection::Root | Selection::Unavailable => return Err(refusal("agent-not-running")),
        Selection::AgentChain => {}
    }
    if expected != detected {
        return Err(refusal("agent-provider-mismatch"));
    }
    let observation = read_agent_observation(client, &before.session_id, deadline, None)?;
    let latest = &observation.session.snapshot;
    if latest.provider_id != before.provider_id
        || latest.lifecycle != AgentLifecycle::Live
        || !latest.presentation_attachments.iter().any(|binding| {
            binding.navigable
                && binding.presentation_id.as_str() == document.terminal.presentation_id
                && before.presentation_attachments.iter().any(|previous| {
                    previous.navigable
                        && previous.presentation_id == binding.presentation_id
                        && previous.generation == binding.generation
                })
        })
    {
        return Err(refusal("agent-binding-changed"));
    }
    Ok(ResolvedAgentTerminal {
        terminal: selected.terminal,
        observation,
    })
}

pub(crate) fn resolve_agent_terminal(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentTarget,
    deadline: Instant,
) -> Result<ResolvedAgentTerminal, Failure> {
    let resolved =
        agent_target::resolve_on_session(client, target, Some(snapshot), deadline, None)?;
    let session_id = resolved.session_id;
    let observation = read_agent_observation(client, &session_id, deadline, None)?;
    let terminal = select_terminal_for_presentation(
        snapshot,
        &observation.session,
        resolved.presentation.as_deref(),
    )?;
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

fn select_terminal_for_presentation(
    snapshot: &Snapshot,
    session: &AgentSessionDetail,
    presentation: Option<&str>,
) -> Result<Target, Failure> {
    let matches: Vec<_> = super::super::results::terminal_rows(snapshot)
        .into_iter()
        .filter(|row| {
            presentation.is_none_or(|id| id == row.presentation_id)
                && session.snapshot.lifecycle == AgentLifecycle::Live
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
