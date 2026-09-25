//! Agent reads select one primary Panel; Client control owns the later focus.
use super::*;
use crate::{
    cli::results,
    invocation::{AgentTarget, QueryCommand},
};
use orchbee_agent_observation_protocol::{AgentLifecycle, AgentSessionDetail};

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
