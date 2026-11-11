//! Agent start state: the Session's Agent catalog as the Server last sent it, each accepted
//! start, and what its outcome asks of the composition.
//!
//! The catalog arrives on the composition's catalog stream, and the start workflows run in
//! [`jobs`], which the composition owns and which settles each outcome back into this state; the
//! input thread never waits on either. Each accepted operation keeps its exact generation and
//! Panel and guards only that Panel. A failed start leaves one status notice; a start refused
//! because its Panel stopped being an idle prompt asks the composition to start the same provider
//! in a new Tab instead.

use std::time::Instant;

use orchbee_agent_observation_protocol::AgentProviderId;
use orchbee_workbench_protocol::{WorkbenchAgentCatalog, WorkbenchPanelId};

use crate::workbench_state::{AgentStartNotice, AgentStartRefusal};

pub(crate) mod jobs;

/// What the Session's catalog stream reported.
#[derive(Clone, Debug)]
pub(crate) enum CatalogEvent {
    /// The catalog as it stands; every frame of the current stream replaces the last.
    Catalog(WorkbenchAgentCatalog),
    /// The Session has no Worker yet or its Server is stopping; a catalog may follow.
    Unavailable,
    /// The Server refused this Session's catalog, or the stream could not open or ended.
    Refused,
    /// The stream now follows another origin or Session: nothing held belongs to it.
    Reset,
}

/// The Session's catalog as this Client holds it.
#[derive(Clone, Debug)]
enum Feed {
    /// No catalog yet: the stream is opening, or the Session has no Worker yet.
    Awaited,
    /// No catalog came, and the Server refused it or the stream ended: nothing starts here.
    Refused,
    /// The catalog the stream sent last; a later failure keeps it.
    Held(WorkbenchAgentCatalog),
}

pub(crate) use crate::workbench_state::AgentStartPhase as Phase;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Drain {
    pub(super) changed: bool,
    pub(super) started: bool,
}

impl Phase {
    pub(super) const fn running(self) -> bool {
        matches!(self, Self::Starting | Self::WaitingInput)
    }

    /// The outcome's status notice; a start that succeeded or was cancelled by teardown says
    /// nothing.
    const fn notice(self) -> Option<AgentStartNotice> {
        Some(match self {
            Self::Starting | Self::WaitingInput | Self::Started | Self::Cancelled => return None,
            Self::Failed => AgentStartNotice::Failed,
            Self::NotFound(product) => AgentStartNotice::NotFound(product),
            Self::Exited(product) => AgentStartNotice::Exited(product),
            Self::Refused(refusal) => AgentStartNotice::Refused(refusal),
            Self::Unknown => AgentStartNotice::Unknown,
        })
    }
}

/// Where one start runs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StartPlace {
    /// The focused Panel, which looked like an idle shell at an empty prompt.
    ThisPanel(WorkbenchPanelId),
    /// A new Tab in the current Workspace.
    NewTab,
    /// A new Panel split to the right of the focused Panel.
    SplitRight,
    /// A new Panel split below the focused Panel.
    SplitDown,
    /// The first shell of a new worktree's Workspace, after the New Worktree form is submitted.
    NewWorktree,
}

/// What a start in its Panel became once its outcome settled: a refusal that typed nothing
/// because the Panel stopped being an idle prompt moves the start to a new Tab; every other
/// outcome stands.
pub(crate) const fn place_after(fallback: bool, phase: Phase) -> Option<StartPlace> {
    match phase {
        Phase::Refused(AgentStartRefusal::NotAtPrompt | AgentStartRefusal::Unmarked)
            if fallback =>
        {
            Some(StartPlace::NewTab)
        }
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct Operation {
    generation: u64,
    panel: WorkbenchPanelId,
    provider: &'static str,
    phase: Phase,
    /// Whether a refusal that typed nothing moves this start to a new Tab.
    fallback: bool,
    /// The workflow deadline while an Unknown result may still hold the Panel; cleared once it
    /// passes.
    held_until: Option<Instant>,
}

impl Operation {
    /// Whether this operation still guards its Panel against another start: its workflow runs, or
    /// its result may have typed the command and its deadline has not passed.
    fn guards(&self) -> bool {
        self.phase.running() || (self.phase == Phase::Unknown && self.held_until.is_some())
    }
}

pub(crate) struct State {
    launch_supported: bool,
    feed: Feed,
    /// Advances whenever anything the picker lists from this state changes.
    revision: u64,
    /// Providers whose start in an existing Panel found no command there; the Server marks only
    /// the starts it delivers itself.
    missing: Vec<&'static str>,
    generation: u64,
    /// At most one operation per Panel.
    operations: Vec<Operation>,
    /// The latest notice a job settled, for the composition's status line.
    notice: Option<AgentStartNotice>,
    /// Starts that must move to a new Tab, oldest first.
    fallbacks: Vec<&'static str>,
    #[cfg(test)]
    accept_without_task: bool,
}

impl State {
    pub(super) fn new(launch_supported: bool) -> Self {
        Self {
            launch_supported,
            feed: Feed::Awaited,
            revision: 0,
            missing: Vec::new(),
            generation: 0,
            operations: Vec::new(),
            notice: None,
            fallbacks: Vec::new(),
            #[cfg(test)]
            accept_without_task: false,
        }
    }

    fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Takes one report of the catalog stream and returns whether anything listed changed. A
    /// refusal or a missing Worker keeps a catalog already held; only a new origin or Session, or
    /// the end of the connection, forgets it.
    pub(super) fn receive_catalog(&mut self, event: CatalogEvent) -> bool {
        let next = match (event, &self.feed) {
            (CatalogEvent::Catalog(catalog), _) => Feed::Held(catalog),
            (CatalogEvent::Unavailable, Feed::Refused) => Feed::Awaited,
            (CatalogEvent::Refused, Feed::Awaited) => Feed::Refused,
            (CatalogEvent::Unavailable | CatalogEvent::Refused, _) => return false,
            (CatalogEvent::Reset, _) => {
                self.missing.clear();
                Feed::Awaited
            }
        };
        self.feed = next;
        self.touch();
        true
    }

    /// The catalog the stream sent last, whatever happened to the stream since.
    pub(super) const fn catalog(&self) -> Option<&WorkbenchAgentCatalog> {
        match &self.feed {
            Feed::Held(catalog) => Some(catalog),
            Feed::Awaited | Feed::Refused => None,
        }
    }

    /// Whether this host and the Session's machine may start Agents, as far as this Client knows:
    /// until a catalog or a refusal says otherwise, they may.
    pub(super) const fn startable(&self) -> bool {
        self.launch_supported
            && match &self.feed {
                Feed::Awaited => true,
                Feed::Refused => false,
                Feed::Held(catalog) => catalog.startable,
            }
    }

    pub(super) fn missing(&self) -> &[&'static str] {
        &self.missing
    }

    /// Whether one of this Client's starts still guards `panel`.
    pub(super) fn guards(&self, panel: &WorkbenchPanelId) -> bool {
        self.operations
            .iter()
            .any(|operation| &operation.panel == panel && operation.guards())
    }

    /// The latest notice a job settled since the last call.
    pub(crate) fn take_notice(&mut self) -> Option<AgentStartNotice> {
        self.notice.take()
    }

    pub(crate) fn set_notice(&mut self, notice: AgentStartNotice) {
        self.notice = Some(notice);
    }

    /// The next start that must move to a new Tab.
    pub(crate) fn take_fallback(&mut self) -> Option<&'static str> {
        (!self.fallbacks.is_empty()).then(|| self.fallbacks.remove(0))
    }

    /// Whether a start may be accepted now: the Session's catalog says its machine can start
    /// Agents.
    pub(super) const fn can_start(&self) -> bool {
        self.launch_supported && matches!(&self.feed, Feed::Held(catalog) if catalog.startable)
    }

    /// Accepts one start of `provider` in `panel` and returns what the jobs must run. Only an
    /// operation that still guards the same Panel refuses it.
    fn begin_start(
        &mut self,
        provider: &'static str,
        panel: WorkbenchPanelId,
        fallback: bool,
        deadline: Instant,
    ) -> Option<StartOperation> {
        if !self.can_start() || self.guards(&panel) {
            return None;
        }
        let id = AgentProviderId::parse(provider).ok()?;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.operations.retain(|operation| operation.panel != panel);
        self.operations.push(Operation {
            generation,
            panel: panel.clone(),
            provider,
            phase: Phase::Starting,
            fallback,
            held_until: Some(deadline),
        });
        Some(StartOperation {
            generation,
            panel,
            provider: id,
            #[cfg(test)]
            without_task: self.accept_without_task,
        })
    }

    fn settle_start_spawn_failed(&mut self, generation: u64) {
        if let Some(operation) = self.operation_mut(generation) {
            operation.phase = Phase::Failed;
            self.notice = Phase::Failed.notice();
        }
    }

    fn operation_mut(&mut self, generation: u64) -> Option<&mut Operation> {
        self.operations
            .iter_mut()
            .find(|operation| operation.generation == generation)
    }

    /// Progress moves only an operation that is still starting or waiting.
    fn settle_start_progress(&mut self, generation: u64, phase: Phase) -> bool {
        let Some(operation) = self.operation_mut(generation) else {
            return false;
        };
        if !operation.phase.running() {
            return false;
        }
        let changed = operation.phase != phase;
        operation.phase = phase;
        changed
    }

    /// A worker that disappears after accepting an operation cannot prove
    /// whether terminal input landed. Preserve recovery state.
    fn settle_start_lost(&mut self, generation: u64) -> bool {
        let Some(operation) = self.operation_mut(generation) else {
            return false;
        };
        let changed = operation.phase != Phase::Unknown;
        operation.phase = Phase::Unknown;
        if changed {
            self.notice = Phase::Unknown.notice();
        }
        changed
    }

    fn apply_completion(
        &mut self,
        generation: u64,
        panel: &WorkbenchPanelId,
        phase: Phase,
    ) -> bool {
        let Some(operation) = self.operation_mut(generation) else {
            return false;
        };
        if &operation.panel != panel {
            return false;
        }
        operation.phase = phase;
        let provider = operation.provider;
        if place_after(operation.fallback, phase) == Some(StartPlace::NewTab) {
            self.fallbacks.push(provider);
            self.notice = Some(AgentStartNotice::PanelBusy);
            return true;
        }
        if let Phase::NotFound(_) = phase {
            self.note_not_found(provider);
        }
        if let Some(notice) = phase.notice() {
            self.notice = Some(notice);
        }
        true
    }

    /// Lists `provider` under not found: its start in an existing Panel found no command there.
    fn note_not_found(&mut self, provider: &'static str) {
        if !self.missing.contains(&provider) {
            self.missing.push(provider);
            self.touch();
        }
    }

    /// Ends the hold of every operation whose workflow deadline has passed.
    pub(super) fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for operation in &mut self.operations {
            if operation.held_until.is_some_and(|deadline| now >= deadline) {
                operation.held_until = None;
                changed = true;
            }
        }
        changed
    }

    /// Test seam: the next frame of [`test_catalog`] with nothing recent.
    #[cfg(test)]
    pub(super) fn install_catalog(&mut self, found: &[&'static str]) {
        let revision = self.catalog().map_or(0, |catalog| catalog.revision.value()) + 1;
        let mut catalog = test_catalog(found, &[]);
        catalog.revision = orchbee_workbench_protocol::WorkbenchRevision::new(revision);
        self.receive_catalog(CatalogEvent::Catalog(catalog));
    }

    #[cfg(test)]
    pub(super) fn accept_without_task(&mut self) {
        self.accept_without_task = true;
    }

    #[cfg(test)]
    pub(super) fn phase(&self, generation: u64) -> Option<Phase> {
        self.operations
            .iter()
            .find(|operation| operation.generation == generation)
            .map(|operation| operation.phase)
    }

    #[cfg(test)]
    pub(super) fn latest(&self) -> Option<(u64, &WorkbenchPanelId)> {
        self.operations
            .iter()
            .max_by_key(|operation| operation.generation)
            .map(|operation| (operation.generation, &operation.panel))
    }
}

/// The accepted start that the jobs run for one generation and Panel.
struct StartOperation {
    generation: u64,
    panel: WorkbenchPanelId,
    provider: AgentProviderId,
    #[cfg(test)]
    without_task: bool,
}

/// Test seam: a startable Session's catalog after a completed lookup that found `found`, listing
/// `recent` as its recent providers; every other provider reads not found, or can't check for a
/// generic command.
#[cfg(test)]
pub(super) fn test_catalog(
    found: &[&'static str],
    recent: &[&'static str],
) -> WorkbenchAgentCatalog {
    use orchbee_workbench_protocol::{
        AgentCatalogEntry, AgentCatalogEntryState, AgentCatalogLookup, AgentCatalogLookupStatus,
        AgentStartSelector, WorkbenchRevision,
    };
    let selector = |id: &str| AgentStartSelector::parse(id).expect("a startable provider");
    let agents = crate::agent_provider::choices()
        .map(|choice| AgentCatalogEntry {
            provider_id: selector(choice.id),
            product_name: choice.product.to_owned(),
            command: std::iter::once(choice.command)
                .chain(
                    crate::agent_provider::launch_arguments(choice.id)
                        .iter()
                        .copied(),
                )
                .collect::<Vec<_>>()
                .join(" "),
            state: if found.contains(&choice.id) {
                AgentCatalogEntryState::Found
            } else if choice.checkable {
                AgentCatalogEntryState::NotFound
            } else {
                AgentCatalogEntryState::CantCheck
            },
        })
        .collect();
    WorkbenchAgentCatalog {
        revision: WorkbenchRevision::new(1),
        startable: true,
        lookup: AgentCatalogLookup {
            status: AgentCatalogLookupStatus::Complete,
            checked_at: Some(1),
        },
        recent: recent.iter().map(|id| selector(id)).collect(),
        agents,
    }
}
