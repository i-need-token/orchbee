//! Agent reads select one primary Panel; Client control owns the later focus.
use super::*;
use crate::{
    cli::results,
    invocation::{AgentTarget, QueryCommand},
};
use orchbee_agent_observation_protocol::{AgentLifecycle, AgentSessionDetail};

#[cfg(windows)]
pub(crate) fn agent_attention_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentTarget,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<AgentSessionId, Failure> {
    agent_target::resolve_on_session(client, target, Some(snapshot), deadline, wake)
        .map(|resolved| resolved.session_id)
}

/// The Client observation is not a Server-authoritative boot identity. Authenticate
/// its opaque checkpoint on this admitted Server before publishing seen/unseen.
#[cfg(windows)]
pub(crate) fn authenticate_attention(
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

pub(crate) fn agent_focus_target(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &AgentTarget,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Target, Failure> {
    let resolved =
        agent_target::resolve_on_session(client, target, Some(snapshot), deadline, wake)?;
    let observation = read_agent_observation(client, &resolved.session_id, deadline, wake)?;
    select_primary_panel(
        snapshot,
        &observation.session,
        resolved.presentation.as_deref(),
    )
}

fn select_primary_panel(
    snapshot: &Snapshot,
    session: &AgentSessionDetail,
    presentation: Option<&str>,
) -> Result<Target, Failure> {
    let Projection::PanelList(rows) =
        results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
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
