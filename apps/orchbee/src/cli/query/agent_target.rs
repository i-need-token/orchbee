//! Compose native Panel selectors with the Agent owner's canonical resolver.
//! Workbench owns the primary presentation; OBAP owns unique live attachments.

use super::*;
use crate::invocation::{AgentTarget, QueryCommand};
use orchbee_agent_observation_protocol::{
    AgentName, AgentNameTarget, AgentNamesRequest, AgentNamesResult, AgentPresentationId,
    AgentResolveOutcome, AgentResolveRejectionReason, AgentSessionSelector,
};

pub(super) struct PreparedTarget {
    pub(super) selector: AgentSessionSelector,
    pub(in crate::cli) presentation: Option<String>,
    /// Preserve the actual correlated Resolve when positional precedence won.
    pub(super) resolved: Option<AgentNamesResult>,
}

pub(in crate::cli) struct NamedTarget {
    pub(in crate::cli) target: AgentNameTarget,
    pub(in crate::cli) presentation: Option<String>,
}

#[cfg(not(windows))]
pub(super) struct ResolvedTarget {
    pub(super) session_id: AgentSessionId,
    #[cfg(not(windows))]
    pub(in crate::cli) presentation: Option<String>,
}

pub(super) fn prepare(
    client: &mut LocalClient,
    target: &AgentTarget,
    snapshot: Option<&Snapshot>,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<PreparedTarget, Failure> {
    let plain = |selector| PreparedTarget {
        selector,
        presentation: None,
        resolved: None,
    };
    let panel = match target {
        AgentTarget::Session { session_id } => {
            return Ok(plain(AgentSessionSelector::Session {
                session_id: session_id.clone(),
            }));
        }
        AgentTarget::Name { name } => {
            return Ok(plain(AgentSessionSelector::Name { name: name.clone() }));
        }
        AgentTarget::Panel { target } => target.clone(),
        AgentTarget::Auto { target } => {
            let selector = AgentSessionSelector::Auto {
                target: target.clone(),
            };
            if AgentPresentationId::parse(target.as_str()).is_err() {
                return Ok(plain(selector));
            }
            let answer = resolve(client, selector, total, wake)?;
            match &answer {
                AgentNamesResult::Resolve { result, .. }
                    if matches!(
                        result.outcome,
                        AgentResolveOutcome::Rejected {
                            reason: AgentResolveRejectionReason::NotFound
                        }
                    ) => {}
                _ => {
                    let selector = match resolved_target(&answer)? {
                        AgentNameTarget::Session { session_id } => {
                            AgentSessionSelector::Session { session_id }
                        }
                        AgentNameTarget::Terminal { binding } => {
                            AgentSessionSelector::Terminal { binding }
                        }
                    };
                    return Ok(PreparedTarget {
                        selector,
                        presentation: None,
                        resolved: Some(answer),
                    });
                }
            }
            Target::Id(target.as_str().to_owned())
        }
    };
    let owned_snapshot;
    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => {
            owned_snapshot = read_snapshot(client, total, wake)?;
            &owned_snapshot
        }
    };
    let presentation = primary_presentation(snapshot, &panel)?;
    let presentation_id = AgentPresentationId::parse(&presentation)
        .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?;
    Ok(PreparedTarget {
        selector: AgentSessionSelector::Presentation { presentation_id },
        presentation: Some(presentation),
        resolved: None,
    })
}

#[cfg(not(windows))]
pub(super) fn resolve_on_session(
    client: &mut LocalClient,
    target: &AgentTarget,
    snapshot: Option<&Snapshot>,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<ResolvedTarget, Failure> {
    let resolved = resolve_named(client, target, snapshot, total, wake)?;
    let AgentNameTarget::Session { session_id } = resolved.target else {
        return Err(crate::cli::agent_names::rejection(
            serde_json::json!("session-not-found"),
            None,
        )
        .failure_schema(crate::cli::agent_names::FAILURE_SCHEMA));
    };
    Ok(ResolvedTarget {
        session_id,
        #[cfg(not(windows))]
        presentation: resolved.presentation,
    })
}

pub(in crate::cli) fn resolve_named(
    client: &mut LocalClient,
    target: &AgentTarget,
    snapshot: Option<&Snapshot>,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<NamedTarget, Failure> {
    // Preserve direct reads for explicit IDs and opaque non-Panel IDs.
    let direct = match target {
        AgentTarget::Session { session_id } => Some(session_id),
        AgentTarget::Auto { target }
            if AgentName::parse(target.as_str()).is_err()
                && AgentPresentationId::parse(target.as_str()).is_err() =>
        {
            Some(target)
        }
        _ => None,
    };
    if let Some(session_id) = direct {
        return Ok(NamedTarget {
            target: AgentNameTarget::Session {
                session_id: session_id.clone(),
            },
            presentation: None,
        });
    }
    let prepared = prepare(client, target, snapshot, total, wake)?;
    let answer = match prepared.resolved {
        Some(answer) => answer,
        None => resolve(client, prepared.selector, total, wake)?,
    };
    Ok(NamedTarget {
        target: resolved_target(&answer)?,
        presentation: prepared.presentation,
    })
}

pub(super) fn resolve(
    client: &mut LocalClient,
    target: AgentSessionSelector,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<AgentNamesResult, Failure> {
    let answer = client
        .submit_agent_names_before_cancellable(
            AgentNamesRequest::Resolve {
                request_id: obap_request_id()?,
                target,
            },
            total,
            wake,
        )
        .map_err(agent_names::exchange_failure)?;
    if Instant::now() >= total {
        return Err(Failure::new(FailureKind::Timeout)
            .failure_schema(crate::cli::agent_names::FAILURE_SCHEMA));
    }
    Ok(answer)
}

pub(super) fn resolved_target(answer: &AgentNamesResult) -> Result<AgentNameTarget, Failure> {
    let schema = crate::cli::agent_names::FAILURE_SCHEMA;
    let source = serde_json::json!({"kind":"agent-names", "requestId":answer.request_id()});
    let AgentNamesResult::Resolve { result, .. } = answer else {
        return Err(Failure::new(FailureKind::ProtocolInvalid).failure_schema(schema));
    };
    match &result.outcome {
        AgentResolveOutcome::Resolved { session_id, .. } => Ok(AgentNameTarget::Session {
            session_id: session_id.clone(),
        }),
        AgentResolveOutcome::ResolvedTarget { target, .. } => Ok(target.clone()),
        AgentResolveOutcome::Rejected { reason } => Err(crate::cli::agent_names::rejection(
            serde_json::json!(reason),
            None,
        )
        .source(source)
        .failure_schema(schema)),
        AgentResolveOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable)
                .source(source)
                .failure_schema(schema))
        }
    }
}

pub(super) fn read_snapshot(
    client: &mut LocalClient,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Snapshot, Failure> {
    let result = client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            total,
            wake,
        )
        .map_err(|error| classify_read(error).failure_schema(super::super::schema::FAILURE_ID))?;
    if Instant::now() >= total {
        return Err(
            Failure::new(FailureKind::Timeout).failure_schema(super::super::schema::FAILURE_ID)
        );
    }
    match result.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        WorkbenchSnapshotOutcome::Denied {} => {
            Err(Failure::new(FailureKind::Denied).failure_schema(super::super::schema::FAILURE_ID))
        }
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable)
                .details(stage_details("snapshot"))
                .failure_schema(super::super::schema::FAILURE_ID))
        }
    }
}

fn primary_presentation(snapshot: &Snapshot, panel: &Target) -> Result<String, Failure> {
    let failure = |error: Failure| {
        error
            .source(super::super::results::snapshot_source(snapshot))
            .failure_schema(super::super::schema::FAILURE_ID)
    };
    let projected =
        super::super::results::project(&QueryCommand::PanelGet(panel.clone()), snapshot)
            .map_err(failure)?;
    let Projection::PanelGet(panel_doc) = projected else {
        return Err(failure(Failure::new(FailureKind::InternalFailure)));
    };
    if panel_doc.primary_presentation.terminal_id.is_none() {
        return Err(failure(super::super::results::not_found(
            Resource::Terminal,
            panel,
        )));
    }
    Ok(panel_doc.primary_presentation.presentation_id)
}
