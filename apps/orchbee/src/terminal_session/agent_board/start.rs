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
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use orchbee_agent_observation_protocol::{AgentName, AgentProviderId};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_workbench_protocol::WorkbenchPanelId;

use crate::{
    agent_start::{Report, Request, ReservationState, Verdict},
    cli::FailureKind,
    client_view::{KeyCode, KeyEvent, KeyKind, Modifiers},
    local_client::{LocalClientOrigin, LocalOperationClock},
};

const START_DEADLINE: Duration = Duration::from_secs(30);
const MAXIMUM_NAME_BYTES: usize = 32;

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
    Loading,
    Ready,
    Unavailable,
}

enum Catalog {
    Unloaded,
    Loading,
    Ready(Box<[&'static str]>),
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    Starting,
    Cancelling,
    Started,
    Failed,
    Unknown,
    Cancelled,
}

impl Phase {
    pub(super) const fn running(self) -> bool {
        matches!(self, Self::Starting | Self::Cancelling)
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
    catalog: Catalog,
    catalog_revision: u64,
    catalog_task: Option<CatalogTask>,
    name: String,
    cursor: usize,
    selected_provider: usize,
    draft_revision: u64,
    generation: u64,
    operation: Option<Operation>,
    task: Option<StartTask>,
    origin: Option<LocalClientOrigin>,
    #[cfg(test)]
    accept_without_task: bool,
}

impl State {
    pub(super) fn new() -> Self {
        Self {
            catalog: Catalog::Unloaded,
            catalog_revision: 0,
            catalog_task: None,
            name: String::new(),
            cursor: 0,
            selected_provider: 0,
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
        self.origin = Some(origin);
    }

    pub(super) fn ensure_catalog(&mut self, path: Option<OsString>) {
        if !matches!(self.catalog, Catalog::Unloaded) {
            return;
        }
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        if self.origin.as_ref().is_none_or(|origin| !origin.is_local()) {
            // This PATH belongs to the Client process. It is evidence only
            // for a local Session; presenting it for a tunneled target would
            // falsely claim that a command exists in the target terminal.
            self.catalog = Catalog::Unavailable;
            return;
        }
        let revision = self.catalog_revision;
        match CatalogTask::spawn(revision, path) {
            Ok(task) => {
                self.catalog = Catalog::Loading;
                self.catalog_task = Some(task);
            }
            Err(_) => self.catalog = Catalog::Unavailable,
        }
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
            Phase::Starting | Phase::Cancelling | Phase::Unknown
        ) {
            // Closing retires only presentation. Reopening adopts an accepted
            // operation so an in-flight completion stays visible and an
            // unknown delivery can never become retryable just because the
            // overlay was toggled. Known terminal results are presentation
            // feedback for their accepting episode and may retire here.
            operation.episode = episode;
        } else {
            self.operation = None;
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
            Catalog::Ready(_) => CatalogStatus::Ready,
            Catalog::Unavailable => CatalogStatus::Unavailable,
        }
    }

    pub(super) fn providers(&self) -> &[&'static str] {
        match &self.catalog {
            Catalog::Ready(providers) => providers,
            _ => &[],
        }
    }

    pub(super) fn selected_provider(&self) -> Option<&'static str> {
        self.providers().get(self.selected_provider).copied()
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) const fn cursor(&self) -> usize {
        self.cursor
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
        self.draft_editable()
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
        if !self.draft_editable()
            || index >= self.providers().len()
            || self.selected_provider == index
        {
            return false;
        }
        self.selected_provider = index;
        self.touch_draft();
        true
    }

    pub(super) fn move_provider(&mut self, delta: isize) -> bool {
        let Some(maximum) = self.providers().len().checked_sub(1) else {
            return false;
        };
        let selected = self
            .selected_provider
            .saturating_add_signed(delta)
            .min(maximum);
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
            _ => false,
        };
        if changed {
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
        if operation.phase != Phase::Starting {
            return false;
        }
        operation.phase = Phase::Cancelling;
        if let Some(task) = &mut self.task {
            task.cancel();
        }
        true
    }

    pub(super) fn drain(&mut self) -> bool {
        let mut changed = self.drain_catalog();
        let completion = self.task.as_ref().and_then(StartTask::take);
        if let Some(completion) = completion {
            changed |= self.apply_completion(
                completion.generation,
                &completion.panel,
                report_phase(&completion.report),
            );
            self.task.take();
        } else if self.task.as_ref().is_some_and(StartTask::finished) {
            if let Some(operation) = self.operation.as_mut() {
                operation.phase = Phase::Failed;
            }
            self.task.take();
            changed = true;
        }
        changed
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
        let Some((revision, providers)) = self.catalog_task.as_ref().and_then(CatalogTask::take)
        else {
            if self
                .catalog_task
                .as_ref()
                .is_some_and(CatalogTask::finished)
            {
                self.catalog_task.take();
                self.catalog = Catalog::Unavailable;
                self.catalog_revision = self.catalog_revision.wrapping_add(1);
                return true;
            }
            return false;
        };
        self.catalog_task.take();
        if revision != self.catalog_revision {
            return false;
        }
        self.catalog = Catalog::Ready(providers.into_boxed_slice());
        self.selected_provider = self
            .selected_provider
            .min(self.providers().len().saturating_sub(1));
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        true
    }

    pub(super) fn close(&mut self) {
        if let Some(task) = &mut self.task {
            task.cancel();
        }
        self.task.take();
        self.catalog_task.take();
    }

    #[cfg(test)]
    pub(super) fn install_catalog(&mut self, providers: &[&'static str]) {
        self.catalog_task.take();
        self.catalog = Catalog::Ready(providers.to_vec().into_boxed_slice());
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        self.selected_provider = 0;
    }

    #[cfg(test)]
    pub(super) fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
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
}

fn report_phase(report: &Report) -> Phase {
    match &report.verdict {
        Verdict::Started { .. } => Phase::Started,
        Verdict::Failed { failure, .. }
            if failure.kind == FailureKind::OutcomeUnknown
                || report.reservation_state == ReservationState::Unknown =>
        {
            Phase::Unknown
        }
        Verdict::Failed { failure, .. } if failure.kind == FailureKind::Cancelled => {
            Phase::Cancelled
        }
        Verdict::Failed { .. } => Phase::Failed,
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.close();
    }
}

struct CatalogTask {
    outcomes: Receiver<(u64, Vec<&'static str>)>,
    cancelled: Arc<AtomicBool>,
    task: Option<JoinHandle<()>>,
}

impl CatalogTask {
    fn spawn(revision: u64, path: Option<OsString>) -> std::io::Result<Self> {
        let (sender, outcomes) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let task_cancelled = Arc::clone(&cancelled);
        let task = thread::Builder::new()
            .name("orchbee-agent-start-catalog".into())
            .spawn(move || {
                let providers =
                    crate::agent_provider::installed_start_providers_while(path.as_deref(), || {
                        !task_cancelled.load(Ordering::Acquire)
                    });
                let _ = sender.send((revision, providers));
            })?;
        Ok(Self {
            outcomes,
            cancelled,
            task: Some(task),
        })
    }

    fn take(&self) -> Option<(u64, Vec<&'static str>)> {
        self.outcomes.try_recv().ok()
    }

    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for CatalogTask {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
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
                );
                let _ = sender.send(Completion {
                    generation,
                    panel: completion_panel,
                    report,
                });
            })?;
        Ok(Self {
            outcomes,
            wake,
            task: Some(task),
            cancelled: false,
        })
    }

    fn take(&self) -> Option<Completion> {
        self.outcomes.try_recv().ok()
    }

    fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
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
