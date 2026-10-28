//! Bounded physical Agent checkpoints and UI-owned cross-machine observation jobs.
use crate::local_client::cancellation::Cancellation;
use crate::{
    local_client::LocalClient,
    machine_discovery::display_label,
    machine_picker::Choice,
    workbench_client::{ClientContextLocation, ClientContextTarget},
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    AgentViewAction, AgentViewOutcome, AgentViewRequest, AgentViewSnapshot, AgentViewSpec,
    ClientTerminalAgentAttentionSnapshot, MAXIMUM_PANEL_AGENT_READ_ROWS,
    MAXIMUM_PANEL_METADATA_READ_ROWS, MAXIMUM_TERMINAL_AGENT_STATE_ROWS,
    MAXIMUM_TERMINAL_TITLE_ROWS, MetadataState, MetadataTokens, PanelAgentAction,
    PanelAgentOutcome, PanelAgentRequest, PanelAgentState, PanelAgentView, PanelMetadataAction,
    PanelMetadataEffective, PanelMetadataOutcome, PanelMetadataRequest, PanelMetadataRow,
    PanelNode, PresentationAgentRow, PresentationBinding, PresentationMetadataRow, ProtocolVersion,
    SessionId, Snapshot, TerminalAgentStateRow, TerminalAgentStateSnapshot, TerminalPresentationId,
    TerminalTitleObservation, WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId,
};
use std::{sync::Arc, time::Instant};

mod attention;
mod metadata;
mod names;
mod titles;
pub(crate) mod view;
pub(crate) use attention::Attention;

const MAXIMUM_ROWS: usize = 4096;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub(crate) context: ClientContextTarget,
    pub(crate) tab: WorkbenchTabId,
    pub(crate) panel: WorkbenchPanelId,
    pub(crate) member: TerminalPresentationId,
    pub(crate) terminal: PresentationBinding,
    pub(crate) physical: AgentLaunchBinding,
    pub(crate) agent: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub(crate) seen: bool,
    pub(crate) name: Option<String>,
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
pub struct Checkpoint {
    pub(crate) session_id: SessionId,
    pub(crate) report_revision: WorkbenchRevision,
    pub(crate) contexts: Arc<[ClientContextTarget]>,
    pub(crate) view: AgentViewSnapshot,
    pub(crate) rows: Arc<[Row]>,
}
impl std::ops::Deref for Checkpoint {
    type Target = [Row];
    fn deref(&self) -> &[Row] {
        &self.rows
    }
}
pub type Observation = crate::machine_observation::Observation<Checkpoint>;
pub type Collector = crate::machine_observation::Collector<Checkpoint>;
pub(crate) use crate::machine_observation::snapshot;
use crate::machine_observation::{check, request_id};

fn source_view(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<AgentViewSnapshot, ()> {
    check(deadline, cancel)?;
    match client
        .agent_view(
            AgentViewRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
                session_id: snapshot.session_id.clone(),
                action: AgentViewAction::Read {},
            },
            deadline,
            cancel.wake(),
        )
        .map_err(|_| ())?
        .outcome
    {
        AgentViewOutcome::Available { snapshot } => Ok(snapshot),
        AgentViewOutcome::Failed { .. } => Err(()),
    }
}
fn states(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[TerminalPresentationId],
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<(WorkbenchRevision, Vec<TerminalAgentStateRow>), ()> {
    check(deadline, cancel)?;
    match client
        .panel_agent(
            PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
                session_id: snapshot.session_id.clone(),
                action: PanelAgentAction::States {
                    presentation_ids: ids.to_vec(),
                },
            },
            deadline,
            cancel.wake(),
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
        PanelAgentOutcome::Titles { .. }
        | PanelAgentOutcome::PresentationsRead { .. }
        | PanelAgentOutcome::Explained { .. }
        | PanelAgentOutcome::States { .. }
        | PanelAgentOutcome::Observed { .. }
        | PanelAgentOutcome::Read { .. }
        | PanelAgentOutcome::PresentationReported { .. }
        | PanelAgentOutcome::Reported { .. }
        | PanelAgentOutcome::Failed { .. } => Err(()),
    }
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub(crate) order: PlacementOrder,
    pub context: ClientContextTarget,
    pub tab: WorkbenchTabId,
    pub panel: WorkbenchPanelId,
    pub member: TerminalPresentationId,
    pub terminal: PresentationBinding,
    label: String,
    workspace: String,
    tab_label: Option<String>,
}
pub(crate) fn placements(snapshot: &Snapshot) -> Result<Vec<Placement>, ()> {
    let mut locations = Vec::new();
    locations.extend(
        snapshot
            .navigation
            .ordered_workspaces()
            .map_err(|_| ())?
            .into_iter()
            .map(|(p, w)| {
                (
                    ClientContextLocation::Workspace {
                        workspace_id: w.workspace_id.clone(),
                        terminal_context_id: w.terminal_context_id.clone(),
                    },
                    p.map_or_else(
                        || display_label(&w.label),
                        |project| {
                            format!(
                                "{} / {}",
                                display_label(&project.label),
                                display_label(&w.label)
                            )
                        },
                    ),
                )
            }),
    );
    let mut result = Vec::new();
    for (workspace_order, (location, workspace)) in locations.into_iter().enumerate() {
        let id = match &location {
            ClientContextLocation::NoWorkspace => return Err(()),
            ClientContextLocation::Workspace {
                terminal_context_id,
                ..
            } => terminal_context_id,
        };
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|c| &c.terminal_context_id == id)
            .ok_or(())?;
        let target = ClientContextTarget {
            session_id: snapshot.session_id.clone(),
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
    context: &ClientContextTarget,
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
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn read(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Arc<Checkpoint>, ()> {
    let snapshot = snapshot(client, deadline, cancel)?;
    read_at(client, &snapshot, deadline, cancel)
}

/// Reuse the physical checkpoint owner inside a caller's already-admitted snapshot span.
pub(crate) fn read_at(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Arc<Checkpoint>, ()> {
    let initial_view = source_view(client, snapshot, deadline, cancel)?;
    let StateCheckpoint {
        placements,
        mut observed,
        revision: head,
        mut bytes,
    } = read_states_at(client, snapshot, deadline, cancel)?;
    let mut titles = titles::read(client, snapshot, &observed, deadline, cancel, &mut bytes)?;
    let mut metadata = metadata::read(
        client,
        snapshot,
        &placements,
        deadline,
        cancel,
        &mut bytes,
        Some(head),
    )?;
    let names = names::read(client, deadline, cancel, &mut bytes)?;
    confirm(client, snapshot, head, deadline, cancel)?;
    if source_view(client, snapshot, deadline, cancel)? != initial_view {
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
                name: names.iter().find_map(|named| match &named.target {
                    orchbee_agent_observation_protocol::AgentNameTarget::Terminal { binding }
                        if binding == &state.binding =>
                    {
                        Some(named.name.as_str().to_owned())
                    }
                    orchbee_agent_observation_protocol::AgentNameTarget::Session { .. }
                    | orchbee_agent_observation_protocol::AgentNameTarget::Terminal { .. } => None,
                }),
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
    let mut contexts = Vec::new();
    contexts.extend(
        snapshot
            .navigation
            .ordered_workspaces()
            .map_err(|_| ())?
            .into_iter()
            .map(|(_, w)| ClientContextTarget {
                session_id: snapshot.session_id.clone(),
                location: ClientContextLocation::Workspace {
                    workspace_id: w.workspace_id.clone(),
                    terminal_context_id: w.terminal_context_id.clone(),
                },
            }),
    );
    Ok(Arc::new(Checkpoint {
        contexts: contexts.into(),
        session_id: snapshot.session_id.clone(),
        report_revision: head,
        view: initial_view,
        rows: rows.into(),
    }))
}

/// Undecorated complete physical states preserve no-Agent incarnation witnesses.
/// The caller must confirm the revision after its other source reads.
pub struct StateCheckpoint {
    pub placements: Vec<Placement>,
    pub observed:
        std::collections::BTreeMap<TerminalPresentationId, Option<TerminalAgentStateSnapshot>>,
    pub revision: WorkbenchRevision,
    bytes: usize,
}
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn read_states_at(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<StateCheckpoint, ()> {
    let placements = placements(snapshot)?;
    let ids: Vec<_> = placements.iter().map(|p| p.member.clone()).collect();
    let mut revision = None;
    let mut observed = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for chunk in ids.chunks(MAXIMUM_TERMINAL_AGENT_STATE_ROWS) {
        let (head, rows) = states(client, snapshot, chunk, deadline, cancel)?;
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
    let revision = match revision {
        Some(revision) => revision,
        None => {
            let (head, rows) = states(client, snapshot, &[], deadline, cancel)?;
            if !rows.is_empty() {
                return Err(());
            }
            head
        }
    };
    Ok(StateCheckpoint {
        placements,
        observed,
        revision,
        bytes,
    })
}

/// A final report fence lets a composite read prove overlap with another source.
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn confirm(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    revision: WorkbenchRevision,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<(), ()> {
    let (head, rows) = states(client, snapshot, &[], deadline, cancel)?;
    if head != revision || !rows.is_empty() {
        return Err(());
    }
    check(deadline, cancel)
}
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn verify(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    deadline: Instant,
    cancel: Cancellation<'_>,
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

#[cfg(any(test, feature = "test-support"))]
pub(crate) mod test_support;
#[cfg(test)]
pub(crate) mod tests;

/// Crate-internal `Row` placement the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait RowTestAccess {
    fn target(&self) -> &Target;
}

#[cfg(any(test, feature = "test-support"))]
impl RowTestAccess for Row {
    fn target(&self) -> &Target {
        &self.target
    }
}

/// Crate-internal `Target` identity the application's tests read.
#[cfg(any(test, feature = "test-support"))]
pub trait TargetTestAccess {
    fn context(&self) -> &ClientContextTarget;
    fn tab(&self) -> &WorkbenchTabId;
    fn panel(&self) -> &WorkbenchPanelId;
    fn member(&self) -> &TerminalPresentationId;
}

#[cfg(any(test, feature = "test-support"))]
impl TargetTestAccess for Target {
    fn context(&self) -> &ClientContextTarget {
        &self.context
    }
    fn tab(&self) -> &WorkbenchTabId {
        &self.tab
    }
    fn panel(&self) -> &WorkbenchPanelId {
        &self.panel
    }
    fn member(&self) -> &TerminalPresentationId {
        &self.member
    }
}
