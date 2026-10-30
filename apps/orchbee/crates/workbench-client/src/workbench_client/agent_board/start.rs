//! Empty-board Agent startup state and its two private background tasks.
//!
//! PATH inspection and the startup workflow never run on the input thread.
//! The accepted operation keeps its exact generation and Panel target; closing
//! the Board retires presentation only. Explicit cancellation wakes the owned
//! workflow, while session teardown cancels and joins it within the workflow's
//! existing bounded I/O/release contract.

use std::{
    ffi::OsString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::{AgentName, AgentProviderId};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::WorkbenchPanelId;

use crate::{
    agent_start::{Progress, Report, Request, ReservationState, Verdict},
    client_view::{KeyCode, KeyEvent, KeyKind, Modifiers},
    local_client::{LocalClientOrigin, LocalOperationClock},
};

const START_DEADLINE: Duration = Duration::from_secs(300);
const CATALOG_DEADLINE: Duration = Duration::from_secs(6);
const MAXIMUM_NAME_BYTES: usize = 32;

mod catalog;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DrawSource {
    catalog_revision: u64,
    draft_revision: u64,
    operation_generation: Option<u64>,
    operation_phase: Option<Phase>,
}

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

pub(crate) use crate::workbench_client::state::AgentStartPhase as Phase;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Drain {
    pub(super) changed: bool,
    pub(super) started: bool,
}

impl Phase {
    pub(super) const fn running(self) -> bool {
        matches!(self, Self::Starting | Self::WaitingInput | Self::Cancelling)
    }
}

#[derive(Clone, Debug)]
struct Operation {
    generation: u64,
    episode: u64,
    panel: WorkbenchPanelId,
    name: AgentName,
    provider: AgentProviderId,
    phase: Phase,
}

pub(super) struct State {
    launch_supported: bool,
    catalog: Catalog,
    catalog_revision: u64,
    catalog_task: Option<CatalogTask>,
    retiring_catalog_tasks: Vec<CatalogTask>,
    name: String,
    automatic_name: bool,
    provider_chosen: bool,
    providers: Vec<&'static str>,
    detected: Vec<&'static str>,
    cursor: usize,
    /// Unset while local detection may still find an installed provider, so the form never
    /// presents an undetected default that later jumps.
    selected_provider: Option<usize>,
    draft_revision: u64,
    generation: u64,
    operation: Option<Operation>,
    task: Option<StartTask>,
    origin: Option<LocalClientOrigin>,
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
            catalog_revision: 0,
            catalog_task: None,
            retiring_catalog_tasks: Vec::new(),
            name: String::new(),
            automatic_name: true,
            provider_chosen: false,
            providers: crate::agent_provider::CANONICAL_IDS.to_vec(),
            detected: Vec::new(),
            cursor: 0,
            selected_provider: None,
            draft_revision: 0,
            generation: 0,
            operation: None,
            task: None,
            origin: None,
            #[cfg(test)]
            accept_without_task: false,
        }
    }

    pub(super) fn set_origin(&mut self, origin: LocalClientOrigin) {
        let locality_changed = self
            .origin
            .as_ref()
            .is_some_and(|current| current.is_local() != origin.is_local());
        if locality_changed {
            self.retire_catalog();
        }
        self.origin = Some(origin);
    }

    pub(super) fn ensure_catalog(&mut self, path: Option<OsString>) -> bool {
        self.reap_catalog_tasks();
        if !matches!(self.catalog, Catalog::Unloaded) {
            return false;
        }
        if !self.retiring_catalog_tasks.is_empty() {
            return false;
        }
        if !self.launch_supported {
            self.catalog = Catalog::Unsupported;
            return true;
        }
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        if self.origin.as_ref().is_none_or(|origin| !origin.is_local()) {
            // This PATH belongs to the Client process. It is evidence only
            // for a local Session; presenting it for a tunneled target would
            // falsely claim that a command exists in the target terminal.
            self.catalog = Catalog::Unavailable;
            self.settle_default_provider();
            return true;
        }
        let revision = self.catalog_revision;
        match CatalogTask::spawn(revision, path) {
            Ok(task) => {
                self.catalog = Catalog::Loading;
                self.catalog_task = Some(task);
            }
            Err(_) => {
                self.catalog = Catalog::Unavailable;
                self.settle_default_provider();
            }
        }
        true
    }

    pub(super) fn opened(&mut self, episode: u64) {
        let Some(operation) = self.operation.as_mut() else {
            return;
        };
        if operation.episode == episode {
            return;
        }
        if matches!(
            operation.phase,
            Phase::Starting
                | Phase::WaitingInput
                | Phase::Cancelling
                | Phase::Unknown
                | Phase::Unconfirmed
        ) {
            // Closing retires only presentation. Reopening adopts an accepted
            // operation so an in-flight completion stays visible and an
            // unknown delivery can never become retryable just because the
            // overlay was toggled. Known terminal results are presentation
            // feedback for their accepting episode and may retire here.
            operation.episode = episode;
        } else {
            self.operation = None;
            self.suggest_name();
        }
    }

    pub(super) fn draw_source(&self) -> DrawSource {
        DrawSource {
            catalog_revision: self.catalog_revision,
            draft_revision: self.draft_revision,
            operation_generation: self
                .operation
                .as_ref()
                .map(|operation| operation.generation),
            operation_phase: self.operation.as_ref().map(|operation| operation.phase),
        }
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

    pub(super) fn providers(&self) -> &[&'static str] {
        &self.providers
    }

    pub(super) fn detected(&self) -> &[&'static str] {
        &self.detected
    }

    fn suggest_name(&mut self) {
        if self.automatic_name && self.draft_editable() {
            let provider = self.selected_provider().unwrap_or("agent");
            let base = crate::agent_provider::executable(provider).unwrap_or(provider);
            self.name = format!("{base}-{}", self.generation.saturating_add(1));
            self.cursor = self.name.len();
            self.touch_draft();
        }
    }

    pub(super) fn selected_provider(&self) -> Option<&'static str> {
        self.providers().get(self.selected_provider?).copied()
    }

    /// Without a detected or chosen provider, a settled catalog falls back to the first
    /// canonical provider so the form stays startable by explicit confirmation.
    fn settle_default_provider(&mut self) {
        if self.selected_provider.is_none() && self.draft_editable() && !self.providers.is_empty() {
            self.selected_provider = Some(0);
            self.suggest_name();
        }
    }

    /// Detected providers lead, each group in canonical order; the selection follows its
    /// provider identity rather than its former position.
    fn order_providers(&mut self) {
        let selected = self.selected_provider();
        let canonical = crate::agent_provider::CANONICAL_IDS;
        self.providers = canonical
            .iter()
            .copied()
            .filter(|provider| self.detected.contains(provider))
            .chain(
                canonical
                    .iter()
                    .copied()
                    .filter(|provider| !self.detected.contains(provider)),
            )
            .collect();
        self.selected_provider =
            selected.and_then(|id| self.providers.iter().position(|provider| *provider == id));
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) const fn cursor(&self) -> usize {
        self.cursor
    }

    pub(super) fn operation_phase(&self) -> Option<Phase> {
        self.operation.as_ref().map(|operation| operation.phase)
    }

    pub(super) fn phase(&self, episode: u64) -> Option<Phase> {
        self.operation.as_ref().and_then(|operation| {
            (operation.phase.running() || operation.episode == episode).then_some(operation.phase)
        })
    }

    pub(super) fn operation_identity(
        &self,
        episode: u64,
    ) -> Option<(&AgentName, &AgentProviderId)> {
        self.operation.as_ref().and_then(|operation| {
            (operation.phase.running() || operation.episode == episode)
                .then_some((&operation.name, &operation.provider))
        })
    }

    pub(super) fn can_start(&self) -> bool {
        self.launch_supported
            && self.draft_editable()
            && self.selected_provider().is_some()
            && AgentName::parse(&self.name).is_ok()
            && self.origin.is_some()
    }

    pub(super) fn draft_editable(&self) -> bool {
        self.operation
            .as_ref()
            .is_none_or(|operation| matches!(operation.phase, Phase::Failed | Phase::Cancelled))
    }

    pub(super) fn select_provider(&mut self, index: usize) -> bool {
        if !self.draft_editable() || index >= self.providers().len() {
            return false;
        }
        self.provider_chosen = true;
        if self.selected_provider == Some(index) {
            return false;
        }
        self.selected_provider = Some(index);
        self.suggest_name();
        self.touch_draft();
        true
    }

    pub(super) fn move_provider(&mut self, delta: isize) -> bool {
        let Some(maximum) = self.providers().len().checked_sub(1) else {
            return false;
        };
        let selected = self
            .selected_provider
            .map_or(0, |index| index.saturating_add_signed(delta).min(maximum));
        self.select_provider(selected)
    }

    pub(super) fn edit(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyKind::Release || !self.draft_editable() {
            return false;
        }
        let changed = match (event.code, event.mods) {
            (KeyCode::Char(character), modifiers)
                if !modifiers.ctrl && !modifiers.alt && !modifiers.super_ =>
            {
                let character = character.to_ascii_lowercase();
                if (character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, '-' | '_'))
                    && self.name.len() < MAXIMUM_NAME_BYTES
                {
                    if self.automatic_name {
                        self.name.clear();
                        self.cursor = 0;
                    }
                    self.name.insert(self.cursor, character);
                    self.cursor += 1;
                    true
                } else {
                    false
                }
            }
            (KeyCode::Backspace, Modifiers::NONE) if self.cursor > 0 => {
                self.cursor -= 1;
                self.name.remove(self.cursor);
                true
            }
            (KeyCode::Delete, Modifiers::NONE) if self.cursor < self.name.len() => {
                self.name.remove(self.cursor);
                true
            }
            (KeyCode::Left, Modifiers::NONE) if self.cursor > 0 => {
                self.cursor -= 1;
                true
            }
            (KeyCode::Right, Modifiers::NONE) if self.cursor < self.name.len() => {
                self.cursor += 1;
                true
            }
            (KeyCode::Home, Modifiers::NONE) if self.cursor != 0 => {
                self.cursor = 0;
                true
            }
            (KeyCode::End, Modifiers::NONE) if self.cursor != self.name.len() => {
                self.cursor = self.name.len();
                true
            }
            (KeyCode::Char('u'), Modifiers::CTRL) if !self.name.is_empty() => {
                self.name.clear();
                self.cursor = 0;
                true
            }
            _ => false,
        };
        if changed {
            self.automatic_name = false;
            self.provider_chosen = true;
            self.touch_draft();
        }
        changed
    }

    fn touch_draft(&mut self) {
        self.draft_revision = self.draft_revision.wrapping_add(1);
        if self
            .operation
            .as_ref()
            .is_some_and(|operation| matches!(operation.phase, Phase::Failed | Phase::Cancelled))
        {
            self.operation = None;
        }
    }

    pub(super) fn start(
        &mut self,
        episode: u64,
        panel: WorkbenchPanelId,
        clock: LocalOperationClock,
    ) -> bool {
        if !self.can_start() || self.task.is_some() {
            return false;
        }
        let (Ok(name), Some(provider), Some(origin)) = (
            AgentName::parse(&self.name),
            self.selected_provider()
                .and_then(|provider| AgentProviderId::parse(provider).ok()),
            self.origin.clone(),
        ) else {
            return false;
        };
        let Some(deadline) = Instant::now().checked_add(START_DEADLINE) else {
            return false;
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        #[cfg(test)]
        if self.accept_without_task {
            self.operation = Some(Operation {
                generation,
                episode,
                panel,
                name,
                provider,
                phase: Phase::Starting,
            });
            return true;
        }
        let request = Request::new(name.clone(), provider.clone(), Vec::new());
        let Ok(task) =
            StartTask::spawn(generation, panel.clone(), origin, clock, request, deadline)
        else {
            self.operation = Some(Operation {
                generation,
                episode,
                panel,
                name,
                provider,
                phase: Phase::Failed,
            });
            return true;
        };
        self.operation = Some(Operation {
            generation,
            episode,
            panel,
            name,
            provider,
            phase: Phase::Starting,
        });
        self.task = Some(task);
        true
    }

    pub(super) fn cancel(&mut self) -> bool {
        let Some(operation) = self.operation.as_mut() else {
            return false;
        };
        if !matches!(operation.phase, Phase::Starting | Phase::WaitingInput) {
            return false;
        }
        operation.phase = Phase::Cancelling;
        if let Some(task) = &mut self.task {
            task.cancel();
        }
        true
    }

    pub(super) fn drain(&mut self) -> Drain {
        self.reap_catalog_tasks();
        let mut drained = Drain {
            changed: self.drain_catalog(),
            started: false,
        };
        if let Some(progress) = self.task.as_ref().and_then(|task| {
            task.progress
                .try_lock()
                .ok()
                .and_then(|mut progress| progress.take())
        }) && let Some(operation) = self.operation.as_mut()
            && matches!(operation.phase, Phase::Starting | Phase::WaitingInput)
        {
            let phase = match progress {
                Progress::Observing => Phase::Starting,
                Progress::WaitingInput => Phase::WaitingInput,
            };
            drained.changed |= operation.phase != phase;
            operation.phase = phase;
        }
        let outcome = self.task.as_ref().map(StartTask::take);
        match outcome {
            Some(Ok(completion)) => {
                let phase = report_phase(&completion.report);
                let applied =
                    self.apply_completion(completion.generation, &completion.panel, phase);
                drained.changed |= applied;
                drained.started = applied && phase == Phase::Started;
                self.task.take();
            }
            Some(Err(TryRecvError::Disconnected)) => {
                // A worker that disappears after accepting an operation cannot
                // prove whether terminal input landed. Preserve recovery state.
                if let Some(operation) = self.operation.as_mut() {
                    drained.changed |= operation.phase != Phase::Unknown;
                    operation.phase = Phase::Unknown;
                }
                self.task.take();
            }
            Some(Err(TryRecvError::Empty)) | None => {}
        }
        drained
    }

    fn apply_completion(
        &mut self,
        generation: u64,
        panel: &WorkbenchPanelId,
        phase: Phase,
    ) -> bool {
        let Some(operation) = self.operation.as_mut() else {
            return false;
        };
        if operation.generation != generation || &operation.panel != panel {
            return false;
        }
        operation.phase = phase;
        true
    }

    fn drain_catalog(&mut self) -> bool {
        let mut changed = false;
        // One candidate per canonical provider plus one final verdict is the
        // complete bounded mailbox. Input never waits for a lookup or a join.
        for _ in 0..=crate::agent_provider::CANONICAL_IDS.len() {
            let Some(task) = self.catalog_task.as_ref() else {
                break;
            };
            let expected = task.revision;
            match task.take() {
                Ok((revision, _)) if revision != expected => continue,
                Ok((_, CatalogUpdate::Found(provider))) => {
                    if !self.detected.contains(&provider) {
                        let first = self.detected.is_empty();
                        self.detected.push(provider);
                        self.detected.sort_unstable();
                        self.order_providers();
                        if first
                            && !self.provider_chosen
                            && self.draft_editable()
                            && let Some(index) =
                                self.providers.iter().position(|id| *id == provider)
                        {
                            self.selected_provider = Some(index);
                            self.suggest_name();
                        }
                        changed = true;
                    }
                }
                Ok((_, CatalogUpdate::Finished(complete))) => {
                    self.retire_active_catalog_task();
                    self.catalog = if complete {
                        Catalog::Ready
                    } else {
                        Catalog::Unavailable
                    };
                    self.settle_default_provider();
                    changed = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    self.retire_active_catalog_task();
                    self.catalog = Catalog::Unavailable;
                    self.settle_default_provider();
                    changed = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if changed {
            self.catalog_revision = self.catalog_revision.wrapping_add(1);
        }
        changed
    }

    pub(super) fn close(&mut self) {
        if let Some(task) = &mut self.task {
            task.cancel();
        }
        self.task.take();
        self.retire_catalog();
        for mut task in self.retiring_catalog_tasks.drain(..) {
            task.cancel();
            drop(task);
        }
    }

    pub(super) fn retire_catalog(&mut self) {
        self.retire_active_catalog_task();
        if !matches!(self.catalog, Catalog::Unloaded) {
            self.catalog = Catalog::Unloaded;
            self.catalog_revision = self.catalog_revision.wrapping_add(1);
            self.detected.clear();
            self.order_providers();
        }
    }

    fn retire_active_catalog_task(&mut self) {
        if let Some(mut task) = self.catalog_task.take() {
            task.cancel();
            self.retiring_catalog_tasks.push(task);
        }
    }

    fn reap_catalog_tasks(&mut self) {
        self.retiring_catalog_tasks.retain(|task| !task.finished());
    }

    #[cfg(test)]
    pub(super) fn install_catalog(&mut self, providers: &[&'static str]) {
        self.retire_active_catalog_task();
        self.catalog = Catalog::Ready;
        self.detected = providers.to_vec();
        self.providers = providers
            .iter()
            .copied()
            .chain(
                crate::agent_provider::CANONICAL_IDS
                    .iter()
                    .copied()
                    .filter(|provider| !providers.contains(provider)),
            )
            .collect();
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        self.selected_provider = Some(0);
        self.suggest_name();
    }

    #[cfg(test)]
    pub(super) fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
        self.automatic_name = false;
        self.cursor = self.name.len();
        self.touch_draft();
    }

    #[cfg(test)]
    pub(super) fn accept_without_task(&mut self) {
        self.accept_without_task = true;
    }

    #[cfg(test)]
    pub(super) fn operation_panel(&self) -> Option<&WorkbenchPanelId> {
        self.operation.as_ref().map(|operation| &operation.panel)
    }

    #[cfg(test)]
    pub(super) fn queue_completion(&mut self, report: Report) -> bool {
        let Some(operation) = &self.operation else {
            return false;
        };
        let (sender, outcomes) = mpsc::sync_channel(1);
        if sender
            .send(Completion {
                generation: operation.generation,
                panel: operation.panel.clone(),
                report,
            })
            .is_err()
        {
            return false;
        }
        drop(sender);
        let Ok((_reader, wake)) = LocalStream::pair() else {
            return false;
        };
        self.task = Some(StartTask {
            progress: Arc::new(Mutex::new(None)),
            outcomes,
            wake,
            task: None,
            cancelled: false,
        });
        true
    }
}

fn report_phase(report: &Report) -> Phase {
    match &report.verdict {
        Verdict::Started { .. } => Phase::Started,
        Verdict::Failed { failure, .. }
            if failure.timed_out()
                && report.delivery == crate::agent_start::DeliveryDisposition::Applied =>
        {
            Phase::Unconfirmed
        }
        Verdict::Failed { failure, .. }
            if failure.unknown()
                || matches!(
                    report.reservation_state,
                    ReservationState::Pending | ReservationState::Unknown
                )
                || report.delivery.possibly_applied() =>
        {
            Phase::Unknown
        }
        Verdict::Failed { failure, .. } if failure.cancelled() => Phase::Cancelled,
        Verdict::Failed { .. } => Phase::Failed,
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.close();
    }
}

enum CatalogUpdate {
    Found(&'static str),
    Finished(bool),
}

struct CatalogTask {
    revision: u64,
    outcomes: Receiver<(u64, CatalogUpdate)>,
    cancelled: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}

impl CatalogTask {
    fn spawn(revision: u64, path: Option<OsString>) -> std::io::Result<Self> {
        let deadline = Instant::now()
            .checked_add(CATALOG_DEADLINE)
            .ok_or_else(|| std::io::Error::other("Agent catalog deadline overflow"))?;
        let (sender, outcomes) = mpsc::sync_channel(crate::agent_provider::CANONICAL_IDS.len() + 1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = thread::Builder::new()
            .name("orchbee-agent-start-catalog".into())
            .spawn(move || {
                let complete = catalog::client_path_candidates(
                    path.as_deref(),
                    task_cancelled.as_ref(),
                    deadline,
                    |provider| {
                        let _ = sender.send((revision, CatalogUpdate::Found(provider)));
                    },
                );
                let _ = sender.send((revision, CatalogUpdate::Finished(complete)));
            })?;
        Ok(Self {
            revision,
            outcomes,
            cancelled,
            task: Some(task),
        })
    }

    fn take(&self) -> Result<(u64, CatalogUpdate), TryRecvError> {
        self.outcomes.try_recv()
    }

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for CatalogTask {
    fn drop(&mut self) {
        self.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

struct Completion {
    generation: u64,
    panel: WorkbenchPanelId,
    report: Report,
}

struct StartTask {
    progress: Arc<Mutex<Option<Progress>>>,
    outcomes: Receiver<Completion>,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
    cancelled: bool,
}

impl StartTask {
    fn spawn(
        generation: u64,
        panel: WorkbenchPanelId,
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        request: Request,
        deadline: Instant,
    ) -> std::io::Result<Self> {
        let (sender, outcomes) = mpsc::sync_channel(1);
        let (reader, wake) = LocalStream::pair()?;
        let completion_panel = panel.clone();
        let progress = Arc::new(Mutex::new(None));
        let task_progress = Arc::clone(&progress);
        let task = thread::Builder::new()
            .name("orchbee-agent-start".into())
            .spawn(move || {
                let started = Instant::now();
                let report = crate::agent_start::run_on_origin(
                    request,
                    started,
                    deadline,
                    Some(reader.as_local_io()),
                    &origin,
                    clock,
                    panel,
                    &|value| {
                        if let Ok(mut progress) = task_progress.lock() {
                            *progress = Some(value);
                        }
                    },
                );
                let _ = sender.send(Completion {
                    generation,
                    panel: completion_panel,
                    report,
                });
            })?;
        Ok(Self {
            progress,
            outcomes,
            wake,
            task: Some(task),
            cancelled: false,
        })
    }

    fn take(&self) -> Result<Completion, TryRecvError> {
        self.outcomes.try_recv()
    }

    fn cancel(&mut self) {
        if self.cancelled {
            return;
        }
        self.cancelled = true;
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for StartTask {
    fn drop(&mut self) {
        self.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
