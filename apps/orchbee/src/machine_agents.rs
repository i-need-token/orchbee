//! Bounded physical Agent checkpoints and UI-owned cross-machine observation jobs.
use crate::{
    local_client::LocalClient,
    machine_discovery::display_label,
    machine_picker::Choice,
    terminal_session::{NativeContextLocation, NativeContextTarget},
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::*;
use std::{
    sync::{Arc, atomic::AtomicU8},
    time::Instant,
};

mod attention;
mod metadata;
mod titles;
pub(crate) mod view;
pub(crate) use attention::Attention;

const MAXIMUM_ROWS: usize = 4096;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    pub(crate) context: NativeContextTarget,
    pub(crate) tab: WorkbenchTabId,
    pub(crate) panel: WorkbenchPanelId,
    pub(crate) member: TerminalPresentationId,
    pub(crate) terminal: PresentationBinding,
    pub(crate) physical: AgentLaunchBinding,
    pub(crate) agent: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) seen: bool,
    pub(crate) order: PlacementOrder,
    pub(crate) target: Target,
    pub(crate) label: String,
    pub(crate) workspace: String,
    pub(crate) tab_label: Option<String>,
    pub(crate) terminal_title: TerminalTitleObservation,
    pub(crate) state: PanelAgentState,
    pub(crate) sequence: WorkbenchRevision,
    pub(crate) metadata: PanelMetadataEffective,
    pub(crate) tokens: MetadataTokens,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlacementOrder {
    pub(crate) workspace: Option<u64>,
    pub(crate) tab: u64,
    pub(crate) pane: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Checkpoint {
    pub(crate) site_id: SiteId,
    pub(crate) report_revision: WorkbenchRevision,
    pub(crate) contexts: Arc<[NativeContextTarget]>,
    pub(crate) view: AgentViewSnapshot,
    pub(crate) rows: Arc<[Row]>,
}
impl std::ops::Deref for Checkpoint {
    type Target = [Row];
    fn deref(&self) -> &[Row] {
        &self.rows
    }
}
pub(crate) type Observation = crate::machine_observation::Observation<Checkpoint>;
pub(crate) type Collector = crate::machine_observation::Collector<Checkpoint>;
pub(crate) use crate::machine_observation::snapshot;
use crate::machine_observation::{check, request_id};

fn source_view(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<AgentViewSnapshot, ()> {
    check(deadline, cancel)?;
    match client
        .agent_view(
            AgentViewRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
                site_id: snapshot.site_id.clone(),
                action: AgentViewAction::Read {},
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?
        .outcome
    {
        AgentViewOutcome::Available { snapshot } => Ok(snapshot),
        _ => Err(()),
    }
}
fn states(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[TerminalPresentationId],
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<(WorkbenchRevision, Vec<TerminalAgentStateRow>), ()> {
    check(deadline, cancel)?;
    match client
        .panel_agent(
            PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
                site_id: snapshot.site_id.clone(),
                action: PanelAgentAction::States {
                    presentation_ids: ids.to_vec(),
                },
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?
        .outcome
    {
        PanelAgentOutcome::States {
            workbench_revision,
            report_revision,
            presentations,
        } if workbench_revision == snapshot.revision
            && presentations.len() == ids.len()
            && ids.iter().all(|id| {
                presentations
                    .iter()
                    .filter(|r| &r.presentation_id == id)
                    .count()
                    == 1
            }) =>
        {
            Ok((report_revision, presentations))
        }
        _ => Err(()),
    }
}
struct Placement {
    order: PlacementOrder,
    context: NativeContextTarget,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    member: TerminalPresentationId,
    terminal: PresentationBinding,
    label: String,
    workspace: String,
    tab_label: Option<String>,
}
fn placements(snapshot: &Snapshot) -> Result<Vec<Placement>, ()> {
    let mut locations = vec![(
        NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        ),
        String::new(),
    )];
    locations.extend(
        snapshot
            .navigation
            .ordered_workspaces()
            .map_err(|_| ())?
            .into_iter()
            .map(|(p, w)| {
                (
                    NativeContextLocation::Workspace {
                        project_id: p.project_id.clone(),
                        workspace_id: w.workspace_id.clone(),
                        terminal_context_id: w.terminal_context_id.clone(),
                    },
                    format!("{} / {}", display_label(&p.label), display_label(&w.label)),
                )
            }),
    );
    let mut result = Vec::new();
    for (workspace_order, (location, workspace)) in locations.into_iter().enumerate() {
        let id = match &location {
            NativeContextLocation::Standalone(id) => id,
            NativeContextLocation::Workspace {
                terminal_context_id,
                ..
            } => terminal_context_id,
        };
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|c| &c.terminal_context_id == id)
            .ok_or(())?;
        let target = NativeContextTarget {
            site_id: snapshot.site_id.clone(),
            location,
        };
        for (tab_order, tab) in context.tabs.iter().enumerate() {
            let start = result.len();
            collect(&tab.root, &target, &tab.tab_id, &workspace, &mut result)?;
            for (pane_order, row) in result[start..].iter_mut().enumerate() {
                row.tab_label = (context.tabs.len() > 1 || !tab.automatic_label)
                    .then(|| display_label(&tab.display_label(tab_order)));
                row.order = PlacementOrder {
                    workspace: workspace_order.checked_sub(1).map(|i| i as u64),
                    tab: tab_order as u64,
                    pane: pane_order as u64,
                };
            }
        }
    }
    Ok(result)
}
fn collect(
    node: &PanelNode,
    context: &NativeContextTarget,
    tab: &WorkbenchTabId,
    workspace: &str,
    rows: &mut Vec<Placement>,
) -> Result<(), ()> {
    match node {
        PanelNode::Split { children, .. } => {
            for child in children.iter() {
                collect(&child.node, context, tab, workspace, rows)?;
            }
        }
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => {
            for member in std::iter::once(primary_presentation).chain(child_presentations) {
                if let Some(binding) = &member.binding {
                    if rows.len() == MAXIMUM_ROWS {
                        return Err(());
                    }
                    rows.push(Placement {
                        order: PlacementOrder::default(),
                        context: context.clone(),
                        tab: tab.clone(),
                        panel: panel_id.clone(),
                        member: member.presentation_id.clone(),
                        terminal: binding.clone(),
                        label: display_label(
                            member
                                .custom_label
                                .as_ref()
                                .map_or(member.label.as_str(), |l| l.as_str()),
                        ),
                        workspace: workspace.into(),
                        tab_label: None,
                    });
                }
            }
        }
    }
    Ok(())
}
pub(crate) fn read(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<Arc<Checkpoint>, ()> {
    let snapshot = snapshot(client, deadline, cancel)?;
    let initial_view = source_view(client, &snapshot, deadline, cancel)?;
    let placements = placements(&snapshot)?;
    let ids: Vec<_> = placements.iter().map(|p| p.member.clone()).collect();
    let mut revision = None;
    let mut observed = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for chunk in ids.chunks(MAXIMUM_TERMINAL_AGENT_STATE_ROWS) {
        let (head, rows) = states(client, &snapshot, chunk, deadline, cancel)?;
        if revision.is_some_and(|old| old != head) {
            return Err(());
        }
        revision = Some(head);
        for row in rows {
            bytes = bytes
                .checked_add(serde_json::to_vec(&row).map_err(|_| ())?.len())
                .ok_or(())?;
            if bytes > MAXIMUM_BYTES || observed.insert(row.presentation_id, row.snapshot).is_some()
            {
                return Err(());
            }
        }
    }
    let mut titles = titles::read(client, &snapshot, &observed, deadline, cancel, &mut bytes)?;
    let mut metadata = metadata::read(
        client,
        &snapshot,
        &placements,
        deadline,
        cancel,
        &mut bytes,
        revision,
    )?;
    let (head, fence) = states(client, &snapshot, &[], deadline, cancel)?;
    if !fence.is_empty() || revision.is_some_and(|old| old != head) {
        return Err(());
    }
    if source_view(client, &snapshot, deadline, cancel)? != initial_view {
        return Err(());
    }
    check(deadline, cancel)?;
    let mut rows = Vec::new();
    for placement in placements {
        if let Some(Some(state)) = observed.remove(&placement.member)
            && let Some(agent) = state.agent
        {
            let terminal_title = titles.remove(&placement.member).ok_or(())?;
            let extra = metadata.remove(&placement.member).ok_or(())?;
            let report = extra.report.ok_or(())?;
            if report.agent.as_ref() != Some(&agent) || report.state != state.state {
                return Err(());
            }
            let effective = extra
                .metadata
                .presentation(Some(&agent), report.authority_source.as_deref());
            rows.push(Row {
                seen: true,
                order: placement.order,
                metadata: effective,
                tokens: extra.metadata.tokens,
                sequence: state.sequence,
                target: Target {
                    context: placement.context,
                    tab: placement.tab,
                    panel: placement.panel,
                    member: placement.member,
                    terminal: placement.terminal,
                    physical: state.binding,
                    agent,
                },
                label: placement.label,
                workspace: placement.workspace,
                tab_label: placement.tab_label,
                terminal_title,
                state: state.state,
            });
        }
    }
    let mut contexts = vec![NativeContextTarget {
        site_id: snapshot.site_id.clone(),
        location: NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        ),
    }];
    contexts.extend(
        snapshot
            .navigation
            .ordered_workspaces()
            .map_err(|_| ())?
            .into_iter()
            .map(|(p, w)| NativeContextTarget {
                site_id: snapshot.site_id.clone(),
                location: NativeContextLocation::Workspace {
                    project_id: p.project_id.clone(),
                    workspace_id: w.workspace_id.clone(),
                    terminal_context_id: w.terminal_context_id.clone(),
                },
            }),
    );
    Ok(Arc::new(Checkpoint {
        contexts: contexts.into(),
        site_id: snapshot.site_id,
        report_revision: head,
        view: initial_view,
        rows: rows.into(),
    }))
}
pub(crate) fn verify(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<(), ()> {
    if !placements(snapshot)?.iter().any(|p| {
        p.context == target.context
            && p.tab == target.tab
            && p.panel == target.panel
            && p.member == target.member
            && p.terminal == target.terminal
    }) {
        return Err(());
    }
    let (_, rows) = states(
        client,
        snapshot,
        std::slice::from_ref(&target.member),
        deadline,
        cancel,
    )?;
    if rows
        .first()
        .and_then(|r| r.snapshot.as_ref())
        .is_none_or(|state| {
            state.binding != target.physical || state.agent.as_ref() != Some(&target.agent)
        })
    {
        return Err(());
    }
    check(deadline, cancel)
}

#[cfg(test)]
pub(crate) mod tests;
