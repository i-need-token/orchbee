//! Empty-board Agent startup state.
//!
//! The Client-PATH catalog and the startup workflow run in [`jobs`], which the
//! composition owns and which settles each outcome back into this state; the
//! input thread never waits on either. The accepted operation keeps its exact
//! generation and Panel target; closing the Board retires presentation only.
//! Explicit cancellation marks the operation, and the jobs wake the workflow.

use orchbee_agent_observation_protocol::{AgentName, AgentProviderId};
use orchbee_workbench_protocol::WorkbenchPanelId;

use crate::client_view::{KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};

const MAXIMUM_NAME_BYTES: usize = 32;

/// The name editor; its ASCII-only admission keeps byte offsets equal to display columns.
fn name_editor(name: String) -> LineEditor {
    LineEditor::new(name, LengthLimit::Bytes(MAXIMUM_NAME_BYTES))
}

pub(crate) mod jobs;

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

pub(crate) use crate::workbench_state::AgentStartPhase as Phase;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Drain {
    pub(super) changed: bool,
    pub(super) started: bool,
}

impl Phase {
    pub(super) const fn running(self) -> bool {
        matches!(self, Self::Starting | Self::WaitingInput | Self::Cancelling)
    }

    /// A settled result that proved no input and no held reservation, so the draft may start
    /// again.
    pub(super) const fn retryable(self) -> bool {
        matches!(self, Self::Failed | Self::Refused(_) | Self::Cancelled)
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

pub(crate) struct State {
    launch_supported: bool,
    catalog: Catalog,
    catalog_revision: u64,
    name: LineEditor,
    automatic_name: bool,
    provider_chosen: bool,
    providers: Vec<&'static str>,
    detected: Vec<&'static str>,
    /// Unset while local detection may still find an installed provider, so the form never
    /// presents an undetected default that later jumps.
    selected_provider: Option<usize>,
    draft_revision: u64,
    generation: u64,
    operation: Option<Operation>,
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
            catalog_revision: 0,
            name: name_editor(String::new()),
            automatic_name: true,
            provider_chosen: false,
            providers: crate::agent_provider::CANONICAL_IDS.to_vec(),
            detected: Vec::new(),
            selected_provider: None,
            draft_revision: 0,
            generation: 0,
            operation: None,
            origin_local: None,
            #[cfg(test)]
            accept_without_task: false,
        }
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
        if !self.launch_supported {
            self.catalog = Catalog::Unsupported;
            return CatalogBegin::Settled;
        }
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
        if self.origin_local != Some(true) {
            // This PATH belongs to the Client process. It is evidence only
            // for a local Session; presenting it for a tunneled target would
            // falsely claim that a command exists in the target terminal.
            self.catalog = Catalog::Unavailable;
            self.settle_default_provider();
            return CatalogBegin::Settled;
        }
        self.catalog = Catalog::Loading;
        CatalogBegin::Load(self.catalog_revision)
    }

    const fn catalog_loading(&self) -> bool {
        matches!(self.catalog, Catalog::Loading)
    }

    fn settle_catalog_found(&mut self, provider: &'static str) -> bool {
        if self.detected.contains(&provider) {
            return false;
        }
        let first = self.detected.is_empty();
        self.detected.push(provider);
        self.detected.sort_unstable();
        self.order_providers();
        if first
            && !self.provider_chosen
            && self.draft_editable()
            && let Some(index) = self.providers.iter().position(|id| *id == provider)
        {
            self.selected_provider = Some(index);
            self.suggest_name();
        }
        true
    }

    /// A finished, incomplete, or lost lookup settles the catalog and the default provider.
    fn settle_catalog_finished(&mut self, complete: bool) {
        self.catalog = if complete {
            Catalog::Ready
        } else {
            Catalog::Unavailable
        };
        self.settle_default_provider();
    }

    fn touch_catalog(&mut self) {
        self.catalog_revision = self.catalog_revision.wrapping_add(1);
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
            self.name = name_editor(format!("{base}-{}", self.generation.saturating_add(1)));
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
        self.name.text()
    }

    pub(super) fn cursor(&self) -> usize {
        self.name.cursor()
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
            && AgentName::parse(self.name.text()).is_ok()
            && self.origin_local.is_some()
    }

    pub(super) fn draft_editable(&self) -> bool {
        self.operation
            .as_ref()
            .is_none_or(|operation| operation.phase.retryable())
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
        // A shared readline key edits the suggested name in place rather than replacing it.
        let changed = if let Some(key) = LineKey::for_key(event) {
            self.name.apply(key) == Ok(true)
        } else {
            match (event.code, event.mods) {
                (KeyCode::Char(character), modifiers)
                    if !modifiers.ctrl && !modifiers.alt && !modifiers.super_ =>
                {
                    let character = character.to_ascii_lowercase();
                    if character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '-' | '_')
                    {
                        if self.automatic_name {
                            self.name.clear();
                        }
                        self.name.insert(&character.to_string()).is_ok()
                    } else {
                        false
                    }
                }
                (KeyCode::Backspace, Modifiers::NONE) => self.name.backspace(),
                (KeyCode::Delete, Modifiers::NONE) => self.name.delete(),
                (KeyCode::Left, Modifiers::NONE) => self.name.left(),
                (KeyCode::Right, Modifiers::NONE) => self.name.right(),
                (KeyCode::Home, Modifiers::NONE) => self.name.home(),
                (KeyCode::End, Modifiers::NONE) => self.name.end(),
                _ => false,
            }
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
            .is_some_and(|operation| operation.phase.retryable())
        {
            self.operation = None;
        }
    }

    /// Accepts one start for `episode` and returns what the jobs must run.
    fn begin_start(&mut self, episode: u64, panel: WorkbenchPanelId) -> Option<StartOperation> {
        if !self.can_start() {
            return None;
        }
        let (Ok(name), Some(provider)) = (
            AgentName::parse(self.name.text()),
            self.selected_provider()
                .and_then(|provider| AgentProviderId::parse(provider).ok()),
        ) else {
            return None;
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.operation = Some(Operation {
            generation,
            episode,
            panel: panel.clone(),
            name: name.clone(),
            provider: provider.clone(),
            phase: Phase::Starting,
        });
        Some(StartOperation {
            generation,
            panel,
            name,
            provider,
            #[cfg(test)]
            without_task: self.accept_without_task,
        })
    }

    fn settle_start_spawn_failed(&mut self, generation: u64) {
        if let Some(operation) = self.operation.as_mut()
            && operation.generation == generation
        {
            operation.phase = Phase::Failed;
        }
    }

    fn cancelling(&self) -> bool {
        self.operation
            .as_ref()
            .is_some_and(|operation| operation.phase == Phase::Cancelling)
    }

    pub(super) fn cancel(&mut self) -> bool {
        let Some(operation) = self.operation.as_mut() else {
            return false;
        };
        if !matches!(operation.phase, Phase::Starting | Phase::WaitingInput) {
            return false;
        }
        operation.phase = Phase::Cancelling;
        true
    }

    /// Progress moves only an operation that is still starting or waiting.
    fn settle_start_progress(&mut self, phase: Phase) -> bool {
        let Some(operation) = self.operation.as_mut() else {
            return false;
        };
        if !matches!(operation.phase, Phase::Starting | Phase::WaitingInput) {
            return false;
        }
        let changed = operation.phase != phase;
        operation.phase = phase;
        changed
    }

    /// A worker that disappears after accepting an operation cannot prove
    /// whether terminal input landed. Preserve recovery state.
    fn settle_start_lost(&mut self) -> bool {
        let Some(operation) = self.operation.as_mut() else {
            return false;
        };
        let changed = operation.phase != Phase::Unknown;
        operation.phase = Phase::Unknown;
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

    pub(super) fn retire_catalog(&mut self) {
        if !matches!(self.catalog, Catalog::Unloaded) {
            self.catalog = Catalog::Unloaded;
            self.catalog_revision = self.catalog_revision.wrapping_add(1);
            self.detected.clear();
            self.order_providers();
        }
    }

    #[cfg(test)]
    pub(super) fn install_catalog(&mut self, providers: &[&'static str]) {
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
        self.name = name_editor(name.to_owned());
        self.automatic_name = false;
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
    name: AgentName,
    provider: AgentProviderId,
    #[cfg(test)]
    without_task: bool,
}
