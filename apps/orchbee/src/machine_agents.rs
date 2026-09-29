//! Bounded physical Agent checkpoints and UI-owned cross-machine observation jobs.
use crate::{
    local_client::LocalClient,
    machine_discovery::{Connection, display_label},
    machine_picker::Choice,
    machines::ProfileId,
    terminal_session::{NativeContextLocation, NativeContextTarget},
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod metadata;
pub(crate) mod view;

const MAXIMUM_ROWS: usize = 4096;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;
const FRESH_FOR: Duration = Duration::from_secs(10);

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
    pub(crate) target: Target,
    pub(crate) label: String,
    pub(crate) workspace: String,
    pub(crate) state: PanelAgentState,
    pub(crate) sequence: WorkbenchRevision,
    pub(crate) metadata: PanelMetadataEffective,
    pub(crate) tokens: MetadataTokens,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    Loading,
    Ready(Arc<[Row]>),
    Unavailable,
}
pub(crate) struct Source {
    pub(crate) choice: Choice,
    pub(crate) connection: Connection,
}

fn request_id() -> Result<WorkbenchRequestId, ()> {
    WorkbenchRequestId::parse(ProfileId::generate().map_err(|_| ())?.as_str()).map_err(|_| ())
}
fn check(deadline: Instant, cancel: &AtomicU8) -> Result<(), ()> {
    if Instant::now() >= deadline || cancel.load(Ordering::Acquire) != 0 {
        Err(())
    } else {
        Ok(())
    }
}
pub(crate) fn snapshot(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: &AtomicU8,
) -> Result<Snapshot, ()> {
    check(deadline, cancel)?;
    match client
        .submit_workbench_snapshot_read_before_cancellable(
            SnapshotReadIntent {
                protocol_version: ProtocolVersion,
                request_id: request_id()?,
            },
            deadline,
            Some(orchbee_local_ipc::LocalIo::Wake(cancel, 1)),
        )
        .map_err(|_| ())?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
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
    context: NativeContextTarget,
    tab: WorkbenchTabId,
    panel: WorkbenchPanelId,
    member: TerminalPresentationId,
    terminal: PresentationBinding,
    label: String,
    workspace: String,
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
    for (location, workspace) in locations {
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
        for tab in &context.tabs {
            collect(&tab.root, &target, &tab.tab_id, &workspace, &mut result)?;
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
) -> Result<Arc<[Row]>, ()> {
    let snapshot = snapshot(client, deadline, cancel)?;
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
    check(deadline, cancel)?;
    let mut rows = Vec::new();
    for placement in placements {
        if let Some(Some(state)) = observed.remove(&placement.member)
            && let Some(agent) = state.agent
        {
            let extra = metadata.remove(&placement.member).ok_or(())?;
            let report = extra.report.ok_or(())?;
            if report.agent.as_ref() != Some(&agent) || report.state != state.state {
                return Err(());
            }
            let effective = extra
                .metadata
                .presentation(Some(&agent), report.authority_source.as_deref());
            rows.push(Row {
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
                state: state.state,
            });
        }
    }
    Ok(rows.into())
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

struct Completed {
    at: Instant,
    rows: Result<Arc<[Row]>, ()>,
}
struct Job {
    cancel: Arc<AtomicU8>,
    task: Option<JoinHandle<Completed>>,
}
impl Job {
    fn start(source: Connection, deadline: Instant) -> Result<Self, ()> {
        let cancel = Arc::new(AtomicU8::new(0));
        let flag = cancel.clone();
        let task = thread::Builder::new()
            .name("orchbee-machine-agents".into())
            .spawn(move || {
                let result = (|| {
                    let mut client = source.reconnect(deadline, &flag).map_err(|_| ())?;
                    read(&mut client, deadline, &flag)
                })();
                Completed {
                    at: Instant::now(),
                    rows: result,
                }
            })
            .map_err(|_| ())?;
        Ok(Self {
            cancel,
            task: Some(task),
        })
    }
    fn stop(&self) {
        self.cancel.store(1, Ordering::Release);
    }
    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = self.task.take() {
            let _settled = task.join();
        }
    }
}
struct Entry {
    choice: Choice,
    job: Option<Job>,
    observation: Observation,
    observed: Option<Instant>,
    next: Instant,
}
#[derive(Default)]
pub(crate) struct Collector {
    entries: Vec<Entry>,
    retired: Vec<Job>,
    cursor: usize,
}
impl Collector {
    pub(crate) fn clear(&mut self) {
        for mut entry in self.entries.drain(..) {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
        }
    }
    pub(crate) fn tick(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
    ) -> Vec<(Choice, Observation)> {
        self.tick_with_start(sources, now, Job::start)
    }
    fn tick_with_start(
        &mut self,
        sources: Vec<Source>,
        now: Instant,
        mut start: impl FnMut(Connection, Instant) -> Result<Job, ()>,
    ) -> Vec<(Choice, Observation)> {
        self.retired.retain(|job| !job.finished());
        let mut old = std::mem::take(&mut self.entries);
        for source in &sources {
            let entry = old
                .iter()
                .position(|e| e.choice == source.choice)
                .map(|i| old.remove(i))
                .unwrap_or_else(|| Entry {
                    choice: source.choice.clone(),
                    job: None,
                    observation: Observation::Loading,
                    observed: None,
                    next: now,
                });
            self.entries.push(entry);
        }
        for mut entry in old {
            if let Some(job) = entry.job.take() {
                job.stop();
                self.retired.push(job);
            }
        }
        for entry in &mut self.entries {
            if entry.job.as_ref().is_some_and(Job::finished) {
                let mut job = entry.job.take().expect("finished job");
                entry.observation = match job.task.take().expect("owned job").join() {
                    Ok(Completed { at, rows: Ok(rows) }) => {
                        entry.observed = Some(at);
                        Observation::Ready(rows)
                    }
                    _ => {
                        entry.observed = None;
                        Observation::Unavailable
                    }
                };
                entry.next = now + Duration::from_secs(2);
            }
        }
        let mut active =
            self.retired.len() + self.entries.iter().filter(|e| e.job.is_some()).count();
        let len = self.entries.len();
        for offset in 0..len {
            let index = (self.cursor + offset) % len;
            let entry = &mut self.entries[index];
            if active >= 4 {
                break;
            }
            if entry.job.is_none() && now >= entry.next {
                match start(
                    sources[index].connection.clone(),
                    now + Duration::from_secs(5),
                ) {
                    Ok(job) => {
                        entry.job = Some(job);
                        active += 1;
                    }
                    Err(()) => {
                        entry.observation = Observation::Unavailable;
                        entry.next = now + Duration::from_secs(2);
                    }
                }
            }
        }
        if len > 0 {
            self.cursor = (self.cursor + 1) % len;
        }
        self.entries
            .iter()
            .map(|entry| {
                (
                    entry.choice.clone(),
                    if entry
                        .observed
                        .is_some_and(|at| now.saturating_duration_since(at) > FRESH_FOR)
                    {
                        Observation::Unavailable
                    } else {
                        entry.observation.clone()
                    },
                )
            })
            .collect()
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        self.clear();
        for job in &self.retired {
            job.stop();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
