//! Reuse a bounded Host sample and revalidate the Agent binding before publishing it.
use super::*;
use crate::cli::{results::AgentRuntimeDoc, terminal_observation::Observation};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentPresentationAttachment, AgentSessionDetail,
};
use serde_json::{Value, json};

pub(super) fn read(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    initial: query::AgentObservationRead,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(query::AgentObservationRead, AgentRuntimeDoc, Option<Value>), Failure> {
    if initial.session.snapshot.lifecycle != AgentLifecycle::Live {
        return Ok((initial, AgentRuntimeDoc::Unavailable("session-ended"), None));
    }
    let results::Projection::PanelList(panels) =
        results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
    else {
        return Err(view_failure(FailureKind::InternalFailure));
    };
    let candidates: Vec<_> = panels
        .iter()
        .filter_map(|panel| {
            let terminal = panel.primary_terminal_id.as_ref()?;
            let binding = initial
                .session
                .snapshot
                .presentation_attachments
                .iter()
                .find(|binding| {
                    binding.navigable
                        && binding.presentation_id.as_str() == panel.primary_presentation_id
                })?;
            Some((terminal, binding))
        })
        .collect();
    let (terminal, binding) = match candidates.as_slice() {
        [(terminal, binding)] => ((*terminal).clone(), (*binding).clone()),
        [] => return Ok((initial, AgentRuntimeDoc::Unavailable("unbound"), None)),
        _ => {
            return Ok((
                initial,
                AgentRuntimeDoc::Unavailable("ambiguous-primary"),
                None,
            ));
        }
    };
    let sample = query::terminal_metadata::read(
        client,
        snapshot,
        &crate::invocation::Target::Id(terminal.clone()),
        deadline,
        signals.poll_fd(),
    );
    check(deadline, signals)?;
    let observation = match sample {
        Ok(read) => Observation::Observed(Box::new(read)),
        Err(error) if error.kind == FailureKind::ServiceUnavailable => Observation::Unavailable,
        Err(error) => return Err(error),
    };
    let canonical = query::read_agent_observation(
        client,
        &initial.session.snapshot.session_id,
        deadline,
        signals.poll_fd(),
    )?;
    check(deadline, signals)?;
    if canonical.revision.value() < initial.revision.value() {
        return Err(view_failure(FailureKind::ProtocolInvalid));
    }
    let final_snapshot = query::refresh_snapshot(
        client,
        &snapshot.site_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    check(deadline, signals)?;
    if final_snapshot.revision != snapshot.revision
        || !binding_unchanged(&initial.session, &canonical.session, &binding)
    {
        return Ok((
            canonical,
            AgentRuntimeDoc::Unavailable("binding-changed"),
            None,
        ));
    }
    let mut terminal_fields = observation.fields();
    terminal_fields["terminalId"] = json!(terminal);
    terminal_fields["source"] = results::snapshot_source(snapshot);
    let Observation::Observed(mut read) = observation else {
        return Ok((
            canonical,
            AgentRuntimeDoc::Unavailable("terminal-unavailable"),
            Some(terminal_fields),
        ));
    };
    let Some((source, processes)) = read.processes.take() else {
        return Ok((
            canonical,
            AgentRuntimeDoc::Unavailable("process-unavailable"),
            Some(terminal_fields),
        ));
    };
    let view = crate::cli::processes::interpret(&processes.info);
    let agent = &canonical.session;
    let expected = crate::cli::processes::provider(agent.snapshot.provider_id.as_str());
    let prompt_ready = agent.snapshot.observation.freshness == AgentObservationFreshness::Fresh
        && agent.snapshot.pending_requests.is_empty()
        && !matches!(
            agent.status,
            AgentSemanticStatus::Unknown { .. }
                | AgentSemanticStatus::NeedsAttention { .. }
                | AgentSemanticStatus::Ended { .. }
        )
        && view.foreground.selection == crate::cli::processes::Selection::AgentChain
        && expected.is_some_and(|expected| {
            expected == agent.snapshot.provider_id.as_str()
                && view.foreground.provider == Some(expected)
        });
    Ok((
        canonical,
        AgentRuntimeDoc::Observed {
            source,
            processes: Box::new(processes),
            prompt_ready,
        },
        Some(terminal_fields),
    ))
}
fn binding_unchanged(
    before: &AgentSessionDetail,
    after: &AgentSessionDetail,
    binding: &AgentPresentationAttachment,
) -> bool {
    after.snapshot.lifecycle == AgentLifecycle::Live
        && before.snapshot.provider_id == after.snapshot.provider_id
        && after
            .snapshot
            .presentation_attachments
            .iter()
            .any(|latest| {
                latest.navigable
                    && latest.presentation_id == binding.presentation_id
                    && latest.generation == binding.generation
            })
}
