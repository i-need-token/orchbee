//! Agent reads select one primary Panel; Client control owns the later focus.
use super::{
    AGENT_TOTAL_BUDGET, AgentObservationRead, AgentReadOperation, AgentSafeInteger, AgentSessionId,
    BorrowedFd, Failure, FailureKind, Instant, LocalClient, Projection, Resource, Snapshot, Target,
    agent_protocol_invalid, agent_rejection_details, agent_target, agent_timeout_details,
    agent_unavailable_details, classify_agent_read, named_terminal, obap_request_id,
    read_agent_observation, terminal_agent,
};
use crate::{
    cli::results,
    invocation::{AgentTarget, QueryCommand},
};
use orchbee_agent_observation_protocol::{AgentLifecycle, AgentSessionDetail};

pub(crate) fn agent_attention_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentTarget,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<orchbee_workbench_protocol::ClientViewIntent, Failure> {
    let resolved = agent_target::resolve_named(client, target, Some(snapshot), deadline, wake)?;
    Ok(match resolved.target {
        orchbee_agent_observation_protocol::AgentNameTarget::Session { session_id } => {
            orchbee_workbench_protocol::ClientViewIntent::AgentAttention {
                session_ids: vec![session_id],
            }
        }
        orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding } => {
            orchbee_workbench_protocol::ClientViewIntent::TerminalAgentAttention {
                bindings: vec![binding],
            }
        }
    })
}

/// The Client observation is not a Server-authoritative boot identity. Authenticate
/// its opaque checkpoint on this admitted Server before publishing seen/unseen.
pub(crate) fn authenticate_attention(
    client: &mut LocalClient,
    attention: &orchbee_workbench_protocol::ClientAgentAttentionSnapshot,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(), Failure> {
    authenticate_attention_read(client, attention, deadline, wake)
        .map_err(|failure| failure.failure_schema(super::super::schema::AGENT_FAILURE_ID))
}

fn authenticate_attention_read(
    client: &mut LocalClient,
    attention: &orchbee_workbench_protocol::ClientAgentAttentionSnapshot,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<(), Failure> {
    use orchbee_agent_observation_protocol::{
        AgentChangesOutcome, AgentChangesRejectionReason, AgentChangesRequest, AgentChangesResult,
    };
    let operation = AgentReadOperation::Changes;
    let request_id = obap_request_id()?;
    let AgentChangesResult::Read { result, .. } = client
        .submit_agent_changes_before_cancellable(
            AgentChangesRequest::Read {
                request_id: request_id.clone(),
                cursor: Some(attention.cursor.clone()),
                limit: AgentSafeInteger::new(1),
            },
            deadline.min(Instant::now() + AGENT_TOTAL_BUDGET),
            wake,
        )
        .map_err(|error| classify_agent_read(error, operation))?;
    let valid = match result.outcome {
        AgentChangesOutcome::Current { revision, .. } => revision == attention.revision,
        AgentChangesOutcome::Changes { changes, .. } => {
            changes.first().map(|change| change.revision.value())
                == attention.revision.value().checked_add(1)
        }
        AgentChangesOutcome::ResetRequired { revision, .. } => {
            revision.value() >= attention.revision.value()
        }
        AgentChangesOutcome::Rejected { reason } => {
            return Err(
                Failure::new(if reason == AgentChangesRejectionReason::ReadDenied {
                    FailureKind::Denied
                } else {
                    FailureKind::Rejected
                })
                .details(agent_rejection_details(
                    operation,
                    if reason == AgentChangesRejectionReason::ReadDenied {
                        "read-denied"
                    } else {
                        "cursor-invalid"
                    },
                ))
                .source(results::agent_source(&request_id)),
            );
        }
        AgentChangesOutcome::Unavailable { .. } => {
            return Err(Failure::new(FailureKind::ServiceUnavailable)
                .details(agent_unavailable_details(operation))
                .source(results::agent_source(&request_id)));
        }
    };
    if valid {
        Ok(())
    } else {
        Err(agent_protocol_invalid(operation))
    }
}

/// Equal provider counters cannot fence server-derived freshness changes. Prove
/// no target event intervened between the authenticated Client checkpoint and Get.
pub(crate) fn attention_covers_observation(
    client: &mut LocalClient,
    attention: &orchbee_workbench_protocol::ClientAgentAttentionSnapshot,
    canonical: &AgentObservationRead,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<bool, Failure> {
    attention_changed_sessions(
        client,
        attention,
        canonical.revision,
        std::slice::from_ref(&canonical.session.snapshot.session_id),
        deadline,
        wake,
    )
    .map(|changed| changed.is_some_and(|changed| changed.is_empty()))
}

pub(crate) fn attention_changed_sessions(
    client: &mut LocalClient,
    attention: &orchbee_workbench_protocol::ClientAgentAttentionSnapshot,
    revision: AgentSafeInteger,
    targets: &[AgentSessionId],
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Option<Vec<AgentSessionId>>, Failure> {
    use orchbee_agent_observation_protocol::{
        AgentChangesOutcome, AgentChangesRequest, AgentChangesResult,
    };
    let mut changed = Vec::new();
    let mut cursor = attention.cursor.clone();
    let mut observed = attention.revision;
    if observed.value() > revision.value() {
        return Err(agent_protocol_invalid(AgentReadOperation::Changes));
    }
    let mut remaining = 512_u64;
    while observed.value() < revision.value() {
        if remaining == 0 {
            return Ok(None);
        }
        let request_id = obap_request_id()?;
        let AgentChangesResult::Read { result, .. } = client
            .submit_agent_changes_before_cancellable(
                AgentChangesRequest::Read {
                    request_id: request_id.clone(),
                    cursor: Some(cursor),
                    limit: AgentSafeInteger::new(remaining.min(100)),
                },
                deadline,
                wake,
            )
            .map_err(|error| classify_agent_read(error, AgentReadOperation::Changes))?;
        if Instant::now() >= deadline {
            return Err(Failure::new(FailureKind::Timeout)
                .details(agent_timeout_details(AgentReadOperation::Changes)));
        }
        match result.outcome {
            AgentChangesOutcome::Changes {
                changes,
                cursor: next,
                observed_through_revision,
                runtime_revision,
                ..
            } => {
                if changes.first().map(|change| change.revision.value())
                    != observed.value().checked_add(1)
                    || runtime_revision.value() < revision.value()
                {
                    return Err(agent_protocol_invalid(AgentReadOperation::Changes));
                }
                for change in changes
                    .iter()
                    .filter(|change| change.revision.value() <= revision.value())
                {
                    if targets.contains(&change.session.session_id)
                        && !changed.contains(&change.session.session_id)
                    {
                        changed.push(change.session.session_id.clone());
                    }
                }
                if targets.iter().all(|id| changed.contains(id)) {
                    return Ok(Some(changed));
                }
                remaining = remaining.saturating_sub(changes.len() as u64);
                observed = observed_through_revision;
                cursor = next;
            }
            AgentChangesOutcome::ResetRequired {
                revision: reset_revision,
                ..
            } if reset_revision.value() >= revision.value() => {
                return Ok(None);
            }
            AgentChangesOutcome::Rejected { reason } => {
                return Err(Failure::new(if reason == orchbee_agent_observation_protocol::AgentChangesRejectionReason::ReadDenied { FailureKind::Denied } else { FailureKind::Rejected })
                    .details(agent_rejection_details(AgentReadOperation::Changes,
                        if reason == orchbee_agent_observation_protocol::AgentChangesRejectionReason::ReadDenied { "read-denied" } else { "cursor-invalid" }))
                    .source(results::agent_source(&request_id)).failure_schema(super::super::schema::AGENT_FAILURE_ID));
            }
            AgentChangesOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(AgentReadOperation::Changes)));
            }
            AgentChangesOutcome::Current { .. } | AgentChangesOutcome::ResetRequired { .. } => {
                return Err(agent_protocol_invalid(AgentReadOperation::Changes));
            }
        }
    }
    Ok(Some(changed))
}

pub(crate) fn agent_focus_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentTarget,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<orchbee_workbench_protocol::ClientViewIntent, Failure> {
    use orchbee_agent_observation_protocol::AgentNameTarget;
    use orchbee_workbench_protocol::{
        ClientViewIntent, TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId,
        WorkbenchTerminalContextId,
    };
    let resolved = agent_target::resolve_named(client, target, Some(snapshot), deadline, wake)?;
    let panel = match &resolved.target {
        AgentNameTarget::Session { session_id } => {
            let observation = read_agent_observation(client, session_id, deadline, wake)?;
            select_primary_panel(
                snapshot,
                &observation.session,
                resolved.presentation.as_deref(),
            )?
        }
        AgentNameTarget::Terminal { binding } => {
            let read = terminal_agent::read(client, snapshot, binding, None, deadline, wake)?;
            let row = named_terminal::terminal_row(snapshot, binding)?;
            if read.observation.view.agent.is_none() {
                return Err(terminal_agent::failure(
                    FailureKind::NotFound,
                    "binding-changed",
                ));
            }
            Target::Id(row.panel_id)
        }
    };
    let Projection::PanelList(rows) = results::project(
        &QueryCommand::PanelList {
            tab: None,
            workspace: None,
        },
        snapshot,
    )?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let row = rows
        .into_iter()
        .find(|row| panel == Target::Id(row.panel_id.clone()))
        .ok_or_else(|| results::not_found(Resource::Panel, &panel))?;
    if let AgentNameTarget::Terminal { binding } = &resolved.target
        && row.primary_presentation_id != binding.presentation_id.as_str()
    {
        return Err(
            results::not_found(Resource::Panel, &panel).source(results::snapshot_source(snapshot))
        );
    }
    let invalid = |_| Failure::new(FailureKind::ProtocolInvalid);
    Ok(ClientViewIntent::AgentFocus {
        terminal_context_id: WorkbenchTerminalContextId::parse(&row.terminal_context_id)
            .map_err(invalid)?,
        tab_id: WorkbenchTabId::parse(&row.tab_id).map_err(invalid)?,
        panel_id: WorkbenchPanelId::parse(&row.panel_id).map_err(invalid)?,
        presentation_id: TerminalPresentationId::parse(&row.primary_presentation_id)
            .map_err(invalid)?,
        target: resolved.target,
    })
}

fn select_primary_panel(
    snapshot: &Snapshot,
    session: &AgentSessionDetail,
    presentation: Option<&str>,
) -> Result<Target, Failure> {
    let Projection::PanelList(rows) = results::project(
        &QueryCommand::PanelList {
            tab: None,
            workspace: None,
        },
        snapshot,
    )?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let matches: Vec<_> = rows
        .into_iter()
        .filter(|panel| {
            session.snapshot.lifecycle == AgentLifecycle::Live
                && panel.primary_terminal_id.is_some()
                && presentation.is_none_or(|id| id == panel.primary_presentation_id)
                && session
                    .snapshot
                    .presentation_attachments
                    .iter()
                    .any(|attachment| {
                        attachment.navigable
                            && attachment.presentation_id.as_str() == panel.primary_presentation_id
                    })
        })
        .collect();
    let target = Target::Label(session.snapshot.session_id.as_str().to_owned());
    let error = match matches.as_slice() {
        [panel] => return Ok(Target::Id(panel.panel_id.clone())),
        [] => results::not_found(Resource::Panel, &target),
        _ => results::ambiguous(
            Resource::Panel,
            &target,
            matches.into_iter().map(|panel| panel.panel_id).collect(),
        ),
    };
    Err(error.source(results::snapshot_source(snapshot)))
}
