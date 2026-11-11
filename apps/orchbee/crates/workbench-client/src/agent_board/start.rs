//! Agent start state: the Client-PATH catalog, each accepted start, and what its outcome asks
//! of the composition.
//!
//! The Client-PATH catalog and the start workflows run in [`jobs`], which the composition owns
//! and which settles each outcome back into this state; the input thread never waits on either.
//! Each accepted operation keeps its exact generation and Panel and guards only that Panel. A
//! failed start leaves one status notice; a start refused because its Panel stopped being an
//! idle prompt asks the composition to start the same provider in a new Tab instead.

use std::time::Instant;

use orchbee_agent_observation_protocol::AgentProviderId;
use orchbee_workbench_protocol::WorkbenchPanelId;

use crate::workbench_state::{AgentStartNotice, AgentStartRefusal};

pub(crate) mod jobs;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatalogStatus {
    Unsupported,
    Loading,
    Ready,
    Unavailable,
}

enum Catalog {
    Unsupported,
    Unloaded,
    Loading,
    Ready,
    Unavailable,
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
    catalog: Catalog,
    /// Advances whenever anything the picker lists from this state changes.
    revision: u64,
    /// The providers the last complete lookup found, or the first lookup's partial findings.
    detected: Vec<&'static str>,
    /// The running lookup's findings; they replace `detected` once it completes.
    scanned: Vec<&'static str>,
    catalog_completed: bool,
    /// Providers whose start found no command in the target terminal.
    missing: Vec<&'static str>,
    /// This Client's own starts, newest first, one per provider.
    started: Vec<&'static str>,
    generation: u64,
    /// At most one operation per Panel.
    operations: Vec<Operation>,
    /// The latest notice a job settled, for the composition's status line.
    notice: Option<AgentStartNotice>,
    /// Starts that must move to a new Tab, oldest first.
    fallbacks: Vec<&'static str>,
    /// Locality of the current Client origin; `None` until one is set.
    origin_local: Option<bool>,
    #[cfg(test)]
    accept_without_task: bool,
}

impl State {
    pub(super) fn new(launch_supported: bool) -> Self {
        Self {
            launch_supported,
            catalog: if launch_supported {
                Catalog::Unloaded
            } else {
                Catalog::Unsupported
            },
            revision: 0,
            detected: Vec::new(),
            scanned: Vec::new(),
            catalog_completed: false,
            missing: Vec::new(),
            started: Vec::new(),
            generation: 0,
            operations: Vec::new(),
            notice: None,
            fallbacks: Vec::new(),
            origin_local: None,
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

    pub(super) fn set_origin_locality(&mut self, local: bool) {
        if self.origin_local.is_some_and(|current| current != local) {
            self.retire_catalog();
        }
        self.origin_local = Some(local);
    }

    /// Admits the next catalog load. The jobs report whether a retired lookup
    /// still runs and begin a lookup only for `CatalogBegin::Load`.
    fn begin_catalog(&mut self, retiring: bool) -> CatalogBegin {
        if !matches!(self.catalog, Catalog::Unloaded) || retiring {
            return CatalogBegin::Unchanged;
        }
        self.touch();
        if !self.launch_supported {
            self.catalog = Catalog::Unsupported;
            return CatalogBegin::Settled;
        }
        if self.origin_local != Some(true) {
            // This PATH belongs to the Client process. It is evidence only
            // for a local Session; presenting it for a tunneled target would
            // falsely claim that a command exists in the target terminal.
            self.catalog = Catalog::Unavailable;
            return CatalogBegin::Settled;
        }
        self.catalog = Catalog::Loading;
        self.scanned.clear();
        CatalogBegin::Load(self.revision)
    }

    const fn catalog_loading(&self) -> bool {
        matches!(self.catalog, Catalog::Loading)
    }

    fn settle_catalog_found(&mut self, provider: &'static str) -> bool {
        if self.scanned.contains(&provider) {
            return false;
        }
        self.scanned.push(provider);
        // Until one lookup completes, findings show as they arrive; later lookups replace the
        // listed set only when they complete, so rows never flicker on reopen.
        if !self.catalog_completed && !self.detected.contains(&provider) {
            self.detected.push(provider);
            self.touch();
            return true;
        }
        false
    }

    /// A finished, incomplete, or lost lookup settles the catalog.
    fn settle_catalog_finished(&mut self, complete: bool) {
        if complete {
            self.detected = std::mem::take(&mut self.scanned);
            self.catalog_completed = true;
            self.catalog = Catalog::Ready;
        } else {
            self.catalog = Catalog::Unavailable;
        }
        self.touch();
    }

    pub(super) fn catalog_status(&self) -> CatalogStatus {
        match self.catalog {
            Catalog::Unloaded | Catalog::Loading => CatalogStatus::Loading,
            Catalog::Ready => CatalogStatus::Ready,
            Catalog::Unavailable => CatalogStatus::Unavailable,
            Catalog::Unsupported => CatalogStatus::Unsupported,
        }
    }

    pub(super) fn catalog_unloaded(&self) -> bool {
        matches!(self.catalog, Catalog::Unloaded)
    }

    /// The providers the picker may call installed, or `None` when nothing on this computer
    /// says what the target terminal has: a remote Session or an unusable lookup.
    pub(super) fn installed(&self) -> Option<&[&'static str]> {
        match self.catalog {
            Catalog::Unloaded | Catalog::Loading | Catalog::Ready => Some(&self.detected),
            Catalog::Unavailable if self.catalog_completed => Some(&self.detected),
            Catalog::Unavailable | Catalog::Unsupported => None,
        }
    }

    /// The providers whose check has not completed: until the first lookup completes, every
    /// checkable provider it has not found yet. They are neither installed nor not found; a later
    /// lookup keeps the completed list instead.
    pub(super) fn pending(&self) -> Vec<&'static str> {
        if self.catalog_completed || !matches!(self.catalog, Catalog::Unloaded | Catalog::Loading) {
            return Vec::new();
        }
        crate::agent_provider::CANONICAL_IDS
            .iter()
            .copied()
            .filter(|id| {
                crate::agent_provider::detection_executable(id).is_some()
                    && !self.detected.contains(id)
            })
            .collect()
    }

    pub(super) fn missing(&self) -> &[&'static str] {
        &self.missing
    }

    pub(super) fn started(&self) -> &[&'static str] {
        &self.started
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

    pub(super) const fn launch_supported(&self) -> bool {
        self.launch_supported
    }

    pub(super) const fn can_start(&self) -> bool {
        self.launch_supported && self.origin_local.is_some()
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
        self.started.retain(|started| *started != provider);
        self.started.insert(0, provider);
        self.touch();
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
        if let Phase::NotFound(_) = phase
            && !self.missing.contains(&provider)
        {
            self.missing.push(provider);
            self.touch();
        }
        if let Some(notice) = phase.notice() {
            self.notice = Some(notice);
        }
        true
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

    /// Forgets the catalog so the next picker opening looks again. The listed providers stay
    /// until that lookup completes.
    pub(super) fn refresh_catalog(&mut self) {
        if matches!(self.catalog, Catalog::Ready | Catalog::Unavailable) {
            self.catalog = Catalog::Unloaded;
        }
    }

    /// The origin changed or the connection ended: nothing found for the old origin stays.
    pub(super) fn retire_catalog(&mut self) {
        if !matches!(self.catalog, Catalog::Unloaded) || self.catalog_completed {
            self.catalog = Catalog::Unloaded;
            self.detected.clear();
            self.scanned.clear();
            self.catalog_completed = false;
            self.missing.clear();
            self.touch();
        }
    }

    #[cfg(test)]
    pub(super) fn install_catalog(&mut self, providers: &[&'static str]) {
        self.catalog = Catalog::Ready;
        self.detected = providers.to_vec();
        self.catalog_completed = true;
        self.touch();
    }

    /// Test seam: the first local lookup begins and reports `found` as its results arrive.
    #[cfg(test)]
    pub(super) fn begin_lookup_finding(&mut self, found: &[&'static str]) {
        self.origin_local = Some(true);
        let _begun = self.begin_catalog(false);
        for provider in found {
            self.settle_catalog_found(provider);
        }
    }

    /// Test seam: one more result of the running lookup arrives.
    #[cfg(test)]
    pub(super) fn find(&mut self, provider: &'static str) -> bool {
        self.settle_catalog_found(provider)
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

/// Whether a drain may begin the Client-PATH catalog, and the PATH it reads.
pub(crate) enum CatalogAdmission {
    Closed,
    Open(Option<std::ffi::OsString>),
}

enum CatalogBegin {
    Unchanged,
    Settled,
    Load(u64),
}

/// The accepted start that the jobs run for one generation and Panel.
struct StartOperation {
    generation: u64,
    panel: WorkbenchPanelId,
    provider: AgentProviderId,
    #[cfg(test)]
    without_task: bool,
}
