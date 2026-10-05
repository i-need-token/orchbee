//! Exact display context for one clicked terminal, shared by CLI and native TUI adapters.

use std::{collections::BTreeSet, time::Instant};

use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentListCursor, AgentListOutcome, AgentListRejectionReason,
    AgentObservationFreshness, AgentReadRequest, AgentReadResult, AgentSafeInteger,
    AgentSemanticStatus, AgentSessionSummary, ObapRequestId,
};
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentRequest, PanelAgentState, PanelAgentView,
    PluginInvocationContext, PluginWorktreeContext, PresentationBinding, Project, ProtocolVersion,
    Snapshot, SnapshotReadIntent, WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchWorktreeQuery, WorkbenchWorktreeQueryOutcome, Workspace, WorktreeEntryText,
    WorktreeQueryFailure, WorktreeSource,
};

use crate::{
    local_client::LocalClient,
    terminal_link_activation::{Error, Guard, SnapshotMember},
};

const AGENT_PAGE_SIZE: u64 = 100;
const MAXIMUM_AGENT_SESSIONS: usize = 512;
const MAXIMUM_AGENT_PAGES: usize = MAXIMUM_AGENT_SESSIONS.div_ceil(AGENT_PAGE_SIZE as usize);

struct PanelObservation {
    report_revision: WorkbenchRevision,
    view: Option<PanelAgentView>,
}

/// Observe display fields for the exact clicked Presentation. The caller
/// keeps its exact terminal viewer attached around this call and performs its
/// final link-content read before detaching.
pub(crate) fn observe<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    member: &SnapshotMember,
    pane_directory: Option<String>,
    provided: PluginInvocationContext,
    deadline: Instant,
    guard: &mut G,
) -> Result<PluginInvocationContext, Error<G::Error>> {
    let (tab, _) = member
        .snapshot_member(snapshot)
        .ok_or(Error::SourceChanged)?;
    let mut context = PluginInvocationContext {
        tab_id: Some(member.tab.to_string()),
        tab_label: Some(tab.label.clone()),
        focused_pane_id: Some(member.panel.to_string()),
        focused_pane_cwd: pane_directory.clone(),
        ..PluginInvocationContext::default()
    };
    let workspace = snapshot.navigation.projects.iter().find_map(|project| {
        project
            .workspaces
            .iter()
            .find(|workspace| workspace.terminal_context_id == member.context)
            .map(|workspace| (project, workspace))
    });
    if let Some((project, workspace)) = workspace {
        context.workspace_id = Some(workspace.workspace_id.to_string());
        context.workspace_label = Some(workspace.label.clone());
        context.workspace_cwd = pane_directory.or_else(|| {
            workspace
                .primary_directory_id
                .as_ref()
                .and_then(|directory| {
                    project
                        .directories
                        .iter()
                        .find(|candidate| &candidate.directory_id == directory)
                        .map(|directory| directory.path.as_str().to_owned())
                })
        });
    } else {
        context.workspace_cwd = pane_directory;
    }

    // Hold a report revision across every slower optional observation. The
    // empty second read is a revision fence and cannot substitute another
    // Presentation when the clicked member changes.
    let reported = read_presentation_agent(client, snapshot, member, deadline, guard)?;
    let sessions = agent_sessions(client, &member.presentation, deadline, guard)?;
    if let Some((project, workspace)) = workspace {
        context.worktree = worktree(client, snapshot, project, workspace, deadline, guard)?;
    }
    fence_presentation_agent(client, snapshot, reported.report_revision, deadline, guard)?;

    context.focused_pane_status = reported
        .view
        .as_ref()
        .and_then(|view| panel_state(view.state))
        .or_else(|| aggregate_status(&sessions).map(str::to_owned));
    context.focused_pane_agent = reported
        .view
        .and_then(|view| view.agent)
        .or_else(|| unique_provider(&sessions));

    fence_member(client, snapshot, member, deadline, guard)?;
    guard.check(deadline).map_err(Error::Gate)?;
    let context = context.with_overrides(provided);
    context.validate().map_err(|_| Error::InvalidContext)?;
    Ok(context)
}

/// Re-read canonical membership at an effect handoff after the caller's
/// final attached-viewer content read.
pub(crate) fn fence_member<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    member: &SnapshotMember,
    deadline: Instant,
    guard: &mut G,
) -> Result<(), Error<G::Error>> {
    let fresh = fresh_snapshot(client, deadline, guard)?;
    if fresh.site_id != snapshot.site_id
        || fresh.revision != snapshot.revision
        || member.snapshot_member(&fresh).is_none()
    {
        return Err(Error::SourceChanged);
    }
    Ok(())
}

fn read_presentation_agent<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    member: &SnapshotMember,
    deadline: Instant,
    guard: &mut G,
) -> Result<PanelObservation, Error<G::Error>> {
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        site_id: snapshot.site_id.clone(),
        action: PanelAgentAction::ReadPresentations {
            presentation_ids: vec![member.presentation.clone()],
        },
    };
    guard.check(deadline).map_err(Error::Gate)?;
    let result = client
        .panel_agent(request, deadline, guard.wake())
        .map_err(|_| Error::TransportUnavailable)?;
    guard.check(deadline).map_err(Error::Gate)?;
    match result.outcome {
        PanelAgentOutcome::PresentationsRead {
            workbench_revision,
            report_revision,
            mut presentations,
        } if workbench_revision == snapshot.revision && presentations.len() == 1 => {
            let row = presentations.pop().ok_or(Error::ProtocolInvalid)?;
            let expected = PresentationBinding {
                worker_id: member.worker.clone(),
                terminal_id: member.terminal.clone(),
            };
            if row.presentation_id != member.presentation
                || row.panel.panel_id != member.panel
                || row.panel.binding.as_ref() != Some(&expected)
            {
                return Err(Error::SourceChanged);
            }
            Ok(PanelObservation {
                report_revision,
                view: row.panel.report,
            })
        }
        PanelAgentOutcome::Failed { .. } => Err(Error::SourceChanged),
        _ => Err(Error::ProtocolInvalid),
    }
}

fn fence_presentation_agent<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    expected_revision: WorkbenchRevision,
    deadline: Instant,
    guard: &mut G,
) -> Result<(), Error<G::Error>> {
    let request = PanelAgentRequest {
        protocol_version: ProtocolVersion,
        request_id: request_id()?,
        site_id: snapshot.site_id.clone(),
        action: PanelAgentAction::ReadPresentations {
            presentation_ids: Vec::new(),
        },
    };
    guard.check(deadline).map_err(Error::Gate)?;
    let result = client
        .panel_agent(request, deadline, guard.wake())
        .map_err(|_| Error::TransportUnavailable)?;
    guard.check(deadline).map_err(Error::Gate)?;
    match result.outcome {
        PanelAgentOutcome::PresentationsRead {
            workbench_revision,
            report_revision,
            presentations,
        } if workbench_revision == snapshot.revision
            && report_revision == expected_revision
            && presentations.is_empty() =>
        {
            Ok(())
        }
        PanelAgentOutcome::Failed { .. } => Err(Error::SourceChanged),
        _ => Err(Error::ProtocolInvalid),
    }
}

fn agent_sessions<G: Guard>(
    client: &mut LocalClient,
    presentation: &orchbee_workbench_protocol::TerminalPresentationId,
    deadline: Instant,
    guard: &mut G,
) -> Result<Vec<AgentSessionSummary>, Error<G::Error>> {
    let mut revision = None;
    let mut cursor: Option<AgentListCursor> = None;
    let mut cursors = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut total = 0_usize;
    let mut matched = Vec::new();
    for page in 0..MAXIMUM_AGENT_PAGES {
        guard.check(deadline).map_err(Error::Gate)?;
        let request_id = agent_request_id()?;
        let result = client
            .submit_agent_read_before_cancellable(
                AgentReadRequest::List {
                    request_id: request_id.clone(),
                    include_ended: false,
                    limit: AgentSafeInteger::new(AGENT_PAGE_SIZE),
                    cursor,
                },
                deadline,
                guard.wake(),
            )
            .map_err(|_| Error::TransportUnavailable)?;
        guard.check(deadline).map_err(Error::Gate)?;
        let AgentReadResult::List {
            request_id: actual,
            result,
        } = result
        else {
            return Err(Error::ProtocolInvalid);
        };
        if actual != request_id {
            return Err(Error::ProtocolInvalid);
        }
        let (current, items, next_cursor) = match result.outcome {
            AgentListOutcome::Listed {
                revision,
                items,
                next_cursor,
            } => (revision, items, next_cursor),
            AgentListOutcome::Unavailable { .. } if page == 0 => return Ok(Vec::new()),
            AgentListOutcome::Rejected {
                reason: AgentListRejectionReason::ReadDenied,
            }
            | AgentListOutcome::Unavailable { .. } => {
                return Err(Error::TransportUnavailable);
            }
            AgentListOutcome::Rejected { .. } => return Err(Error::SourceChanged),
        };
        total = total.saturating_add(items.len());
        if revision.is_some_and(|old| old != current)
            || items.len() > AGENT_PAGE_SIZE as usize
            || (items.is_empty() && next_cursor.is_some())
            || total > MAXIMUM_AGENT_SESSIONS
            || items.iter().any(|item| {
                item.lifecycle != AgentLifecycle::Live || !ids.insert(item.session_id.clone())
            })
            || next_cursor
                .as_ref()
                .is_some_and(|next| !cursors.insert(next.as_str().to_owned()))
        {
            return Err(Error::ProtocolInvalid);
        }
        revision = Some(current);
        matched.extend(items.into_iter().filter(|item| {
            item.presentation_attachments.iter().any(|attachment| {
                attachment.navigable && attachment.presentation_id.as_str() == presentation.as_str()
            })
        }));
        cursor = next_cursor;
        if cursor.is_none() {
            return Ok(matched);
        }
    }
    Err(Error::LimitExceeded)
}

fn panel_state(state: PanelAgentState) -> Option<String> {
    Some(
        match state {
            PanelAgentState::Idle => "idle",
            PanelAgentState::Working => "working",
            PanelAgentState::Blocked => "blocked",
            PanelAgentState::Unknown => return None,
        }
        .to_owned(),
    )
}

fn session_status(session: &AgentSessionSummary) -> Option<&'static str> {
    if session.observation.freshness != AgentObservationFreshness::Fresh {
        return None;
    }
    match session.status {
        AgentSemanticStatus::NeedsAttention { .. } => Some("blocked"),
        AgentSemanticStatus::Working { .. } => Some("working"),
        AgentSemanticStatus::Idle {} => Some("idle"),
        _ => None,
    }
}

fn aggregate<'a>(statuses: impl Iterator<Item = Option<&'a str>>) -> Option<&'static str> {
    let mut seen = false;
    let mut unresolved = false;
    let mut highest = 0_u8;
    for status in statuses {
        seen = true;
        highest = highest.max(match status {
            Some("blocked") => 3,
            Some("working") => 2,
            Some("idle") => 1,
            _ => {
                unresolved = true;
                0
            }
        });
    }
    match highest {
        3 => Some("blocked"),
        _ if !seen || unresolved => None,
        2 => Some("working"),
        1 => Some("idle"),
        _ => None,
    }
}

fn aggregate_status(sessions: &[AgentSessionSummary]) -> Option<&'static str> {
    aggregate(sessions.iter().map(session_status))
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut values = values.collect::<BTreeSet<_>>();
    let value = values.pop_first()?;
    values.is_empty().then(|| value.to_owned())
}

fn unique_provider(sessions: &[AgentSessionSummary]) -> Option<String> {
    unique(sessions.iter().map(|session| session.provider_id.as_str()))
}

fn worktree<G: Guard>(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    project: &Project,
    workspace: &Workspace,
    deadline: Instant,
    guard: &mut G,
) -> Result<Option<PluginWorktreeContext>, Error<G::Error>> {
    let Some(directory_id) = workspace.primary_directory_id.as_ref() else {
        return Ok(None);
    };
    let directory = project
        .directories
        .iter()
        .find(|directory| &directory.directory_id == directory_id)
        .ok_or(Error::ProtocolInvalid)?;
    let request = WorkbenchWorktreeQuery {
        protocol_version: snapshot.protocol_version,
        request_id: request_id()?,
        site_id: snapshot.site_id.clone(),
        source: WorktreeSource::Workspace {
            workspace_id: workspace.workspace_id.clone(),
        },
        trust_repository: false,
    };
    guard.check(deadline).map_err(Error::Gate)?;
    let outcome = client
        .query_worktrees(request, deadline, guard.wake())
        .map_err(|_| Error::TransportUnavailable)?
        .outcome;
    guard.check(deadline).map_err(Error::Gate)?;
    let WorkbenchWorktreeQueryOutcome::Found {
        site_id,
        workbench_revision,
        worker_id,
        source,
        entries,
    } = outcome
    else {
        let WorkbenchWorktreeQueryOutcome::Failed { reason } = outcome else {
            unreachable!();
        };
        return match reason {
            WorktreeQueryFailure::NotFound {} | WorktreeQueryFailure::SourceChanged {} => {
                Err(Error::SourceChanged)
            }
            WorktreeQueryFailure::Denied {}
            | WorktreeQueryFailure::Timeout {}
            | WorktreeQueryFailure::TransportLoss {} => Err(Error::TransportUnavailable),
            WorktreeQueryFailure::MalformedListing {} => Err(Error::ProtocolInvalid),
            _ => Ok(None),
        };
    };
    if site_id != snapshot.site_id
        || workbench_revision != snapshot.revision
        || worker_id != directory.worker_id
        || source.workspace_id.as_ref() != Some(&workspace.workspace_id)
        || source.requested_path != directory.path
    {
        return Err(Error::SourceChanged);
    }
    let checkout = entries
        .iter()
        .find(|entry| {
            entry.checkout.path == WorktreeEntryText::Utf8(source.checkout_path.as_str().to_owned())
        })
        .ok_or(Error::ProtocolInvalid)?;
    if !checkout.workspaces.iter().any(|association| {
        association.workspace_id == workspace.workspace_id
            && association.project_id == project.project_id
    }) {
        return Err(Error::ProtocolInvalid);
    }
    let repository_name = source
        .repository_root
        .as_str()
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .ok_or(Error::ProtocolInvalid)?
        .to_owned();
    Ok(Some(PluginWorktreeContext {
        worker_id,
        repository_root: source.repository_root,
        common_directory: source.common_directory,
        repository_name,
        checkout_path: source.checkout_path,
        linked: checkout.linked,
    }))
}

fn fresh_snapshot<G: Guard>(
    client: &mut LocalClient,
    deadline: Instant,
    guard: &mut G,
) -> Result<Snapshot, Error<G::Error>> {
    guard.check(deadline).map_err(Error::Gate)?;
    let result = client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            deadline,
            guard.wake(),
        )
        .map_err(|_| Error::TransportUnavailable)?;
    guard.check(deadline).map_err(Error::Gate)?;
    match result.outcome {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
        _ => Err(Error::SourceChanged),
    }
}

fn request_id<E>() -> Result<WorkbenchRequestId, Error<E>> {
    random_id(WorkbenchRequestId::parse)
}

fn agent_request_id<E>() -> Result<ObapRequestId, Error<E>> {
    random_id(ObapRequestId::parse)
}

fn random_id<T, P, E>(parse: impl FnOnce(&str) -> Result<T, P>) -> Result<T, Error<E>> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::IdentityUnavailable)?;
    let value = format!("{:032x}", u128::from_be_bytes(bytes));
    parse(&value).map_err(|_| Error::IdentityUnavailable)
}

#[cfg(test)]
mod tests {
    use super::{aggregate, unique};

    #[test]
    fn aggregate_omits_unknown_and_preserves_attention_priority() {
        assert_eq!(aggregate([].into_iter()), None);
        assert_eq!(
            aggregate([Some("working"), Some("idle")].into_iter()),
            Some("working")
        );
        assert_eq!(aggregate([Some("working"), None].into_iter()), None);
        assert_eq!(
            aggregate([None, Some("blocked")].into_iter()),
            Some("blocked")
        );
    }

    #[test]
    fn provider_is_reported_only_when_the_observed_value_is_unique() {
        assert_eq!(unique(["codex", "codex"].into_iter()), Some("codex".into()));
        assert_eq!(unique(["codex", "claude"].into_iter()), None);
        assert_eq!(unique([].into_iter()), None);
    }
}
