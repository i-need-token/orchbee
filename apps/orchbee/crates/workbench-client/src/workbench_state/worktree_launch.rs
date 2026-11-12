//! Client-owned Worktree drafts, immutable confirmation and retained observation; Git and terminal
//! attachment remain with their existing owners.
pub(crate) mod addressed;
mod draft;
mod origin;
mod removal;
use crate::client_context::ClientContextLocation;
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use draft::Draft;
pub(crate) use draft::{WorktreeEdit, WorktreeStage};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRequestId, WorkbenchWorktreeOutcome,
    WorkerId, WorktreeIntent, WorktreeOperationStatus, WorktreeQueryEntry, WorktreeQuerySource,
    WorktreeSource, same_directory_spelling,
};
pub(crate) use origin::{Origin, Preparation};
use removal::Recovery;
pub(crate) use removal::Removal;

pub(crate) use crate::localization::WorktreeMode;
pub(crate) use crate::localization::WorktreeNotice;

/// One non-reused draw/input authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorktreeToken(pub(crate) u64);
#[cfg(test)]
impl WorktreeToken {
    pub(crate) fn for_test(value: u64) -> Self {
        Self(value)
    }
}

/// Read-only projection consumed by renderer and input adapters.
#[derive(Clone, Copy)]
pub(crate) struct WorktreeView<'a> {
    launcher: &'a Launcher,
}
#[derive(Clone, Debug)]
pub(crate) struct Discovery {
    pub(crate) source: WorktreeQuerySource,
    pub(crate) worker: WorkerId,
    pub(crate) entries: Vec<WorktreeQueryEntry>,
    pub(crate) configured_root: Option<WorkbenchDirectoryPath>,
}
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    pub(crate) sidebar: Option<Box<addressed::SidebarDiscovery>>,
    pub(crate) operation_id: WorkbenchRequestId,
    pub(crate) origin: Origin,
    pub(crate) source: WorktreeSource,
    pub(crate) discovery: Discovery,
    pub(crate) project: Option<WorkbenchProjectId>,
    pub(crate) intent: WorktreeIntent,
    pub(crate) removal: Option<Removal>,
}
#[derive(Clone, Debug)]
pub(crate) struct Operation {
    pub(crate) plan: Plan,
    pub(crate) outcome: Option<WorkbenchWorktreeOutcome>,
    pub(crate) focus_allowed: bool,
    pub(crate) attempt: u32,
}
enum Phase {
    Idle,
    Preparing(Box<Preparation>),
    Querying {
        preparation: Box<Preparation>,
        source: WorktreeSource,
    },
    Draft(Box<Draft>),
    Starting(Box<Plan>),
    Observing(Box<Operation>),
    Result(Box<Operation>),
    /// A retained removal result awaiting one explicit recovery decision.
    Confirm(Box<Operation>, Recovery),
    Resuming(Box<Operation>),
}
pub(crate) struct Launcher {
    phase: Phase,
    last_token: u64,
    visible: bool,
    notice: WorktreeNotice,
    shown: Option<WorktreeToken>,
    /// A current presented frame admitted this decoder-owned edit stream.
    /// Semantic edits revoke submit proof but retain the stream until the
    /// renderer reports absence or a geometry block.
    input_frame_live: bool,
    focus_revoked: bool,
    presentation: PresentationPhase,
    presentation_notice: Option<WorktreeNotice>,
    /// The requested mode, kept for a failure that leaves no draft, such as a failed preparation
    /// or a removal refused before submission.
    requested: WorktreeMode,
    /// This Client process's working directory, supplied only where a process keeps its working
    /// directory in use (Windows).
    client_directory: Option<String>,
}
impl Default for Launcher {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            last_token: 0,
            visible: false,
            notice: WorktreeNotice::Preparing,
            shown: None,
            input_frame_live: false,
            focus_revoked: false,
            presentation: PresentationPhase::Pending,
            presentation_notice: None,
            requested: WorktreeMode::Create,
            client_directory: None,
        }
    }
}
impl Launcher {
    pub(crate) fn set_client_directory(&mut self, directory: Option<String>) {
        self.client_directory = directory;
    }
    pub(crate) fn open_sidebar(
        &mut self,
        origin: Origin,
        sidebar: addressed::SidebarDiscovery,
        mode: WorktreeMode,
        checkout: Option<&WorkbenchDirectoryPath>,
    ) -> bool {
        if !(matches!(self.phase, Phase::Idle) || self.hidden_opened_result())
            || !sidebar.valid(origin.cache.canonical_snapshot())
            || (mode == WorktreeMode::Open) != checkout.is_some()
        {
            return false;
        }
        let mut draft = Draft::new(
            origin,
            sidebar.source.clone(),
            sidebar.discovery.clone(),
            mode,
        );
        if mode == WorktreeMode::Remove {
            let Some(removal) = Removal::capture(&sidebar) else {
                return false;
            };
            draft.removal = Some(removal);
        }
        if let Some(checkout) = checkout {
            let Some(index) = sidebar.discovery.entries.iter().position(|entry| {
                matches!(&entry.checkout.path, orchbee_workbench_protocol::WorktreeEntryText::Utf8(path) if path == checkout.as_str())
            }) else { return false; };
            if !draft.choose_checkout(index) {
                return false;
            }
        }
        draft.sidebar = Some(Box::new(sidebar));
        let Some(token) = self.advance_token() else {
            return false;
        };
        draft.token = token;
        self.phase = Phase::Draft(Box::new(draft));
        self.requested = mode;
        self.visible = true;
        self.focus_revoked = false;
        self.notice = WorktreeNotice::Preparing;
        self.presentation = PresentationPhase::Pending;
        self.presentation_notice = None;
        true
    }
    /// The painted token and exact row authorize one Open, without arming a
    /// replacement token for a selection that has never been drawn.
    pub(crate) fn choose_and_confirm(
        &mut self,
        token: WorktreeToken,
        index: usize,
        operation_id: WorkbenchRequestId,
        origin: &Origin,
        locale: crate::localization::Locale,
    ) -> Option<Plan> {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
        {
            return None;
        }
        let Phase::Draft(draft) = &mut self.phase else {
            return None;
        };
        if !draft.origin.unchanged(origin) || !draft.choose_checkout(index) {
            return None;
        }
        self.confirm(token, operation_id, origin, locale)
    }

    pub(crate) fn scroll_choices(&mut self, token: WorktreeToken, up: bool, lines: usize) -> bool {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
            || !self
                .view()
                .is_some_and(|view| view.stage() == WorktreeStage::Checkout)
        {
            return false;
        }
        self.edit(token, WorktreeEdit::ScrollChoices { up, lines })
    }

    pub(crate) fn visible(&self) -> bool {
        self.visible
    }
    /// No operation, form or result is held, shown or hidden.
    pub(crate) fn idle(&self) -> bool {
        !self.visible && matches!(self.phase, Phase::Idle)
    }
    /// Only a hidden, correlated successful terminal result can yield to a new
    /// explicit operation. Failed/uncertain outcomes retain their recovery owner.
    pub(crate) fn hidden_opened_result(&self) -> bool {
        !self.visible
            && self.notice == WorktreeNotice::Opened
            && matches!(&self.phase, Phase::Result(operation)
                if operation.focus_allowed && matches!(&operation.outcome,
                    Some(WorkbenchWorktreeOutcome::Available { project_id, status, .. })
                        if project_id == &operation.plan.project
                            && matches!(status.as_ref(), WorktreeOperationStatus::Opened { .. })))
    }
    pub(crate) fn token(&self) -> Option<WorktreeToken> {
        self.visible.then_some(WorktreeToken(self.last_token))
    }
    pub(crate) fn accepts_input(&self, token: WorktreeToken) -> bool {
        self.token() == Some(token) && self.presentation.interactive() && self.input_frame_live
    }
    /// The menu already chose one exact checkout. Its complete source form
    /// must still be painted before the choice can start an operation.
    pub(crate) fn sidebar_open_ready(&self) -> bool {
        self.visible
            && self.shown == self.token()
            && self.presentation.interactive()
            && !self.focus_revoked
            && matches!(&self.phase, Phase::Draft(draft)
                if draft.sidebar.is_some() && draft.is_checkout())
    }
    fn focus_effect_allowed(&self) -> bool {
        self.visible
            && self.shown == Some(WorktreeToken(self.last_token))
            && self.presentation.interactive()
            && !self.focus_revoked
    }
    fn advance_token(&mut self) -> Option<WorktreeToken> {
        self.last_token = self.last_token.checked_add(1)?;
        self.shown = None;
        self.input_frame_live = false;
        Some(WorktreeToken(self.last_token))
    }
    pub(crate) fn reopen(&mut self) -> bool {
        if matches!(self.phase, Phase::Idle) {
            return false;
        }
        if self.visible {
            return true;
        }
        self.visible = true;
        self.shown = None;
        self.input_frame_live = false;
        self.presentation = PresentationPhase::Pending;
        true
    }
    pub(crate) fn open(&mut self, mut preparation: Preparation) -> bool {
        if !matches!(self.phase, Phase::Idle) {
            return self.reopen();
        }
        let Some(token) = self.advance_token() else {
            return false;
        };
        preparation.token = token;
        self.requested = preparation.mode;
        self.visible = true;
        self.input_frame_live = false;
        self.notice = WorktreeNotice::Preparing;
        self.focus_revoked = false;
        self.presentation = PresentationPhase::Pending;
        self.presentation_notice = None;
        self.phase = Phase::Preparing(Box::new(preparation));
        true
    }
    pub(crate) fn preparation(&self) -> Option<&Preparation> {
        match &self.phase {
            Phase::Preparing(p) => Some(p),
            _ => None,
        }
    }
    pub(crate) fn prepared(&mut self, p: &Preparation, source: Option<WorktreeSource>) -> bool {
        if self.preparation() != Some(p) {
            return false;
        }
        let Some(source) = source else {
            self.fail(WorktreeNotice::SourceUnavailable);
            return true;
        };
        self.phase = Phase::Querying {
            preparation: Box::new(p.clone()),
            source,
        };
        true
    }
    pub(crate) fn query_input(&self) -> Option<(&Preparation, &WorktreeSource)> {
        match &self.phase {
            Phase::Querying {
                preparation,
                source,
            } => Some((preparation, source)),
            _ => None,
        }
    }
    pub(crate) fn discovered(
        &mut self,
        token: WorktreeToken,
        result: Result<Discovery, WorktreeNotice>,
    ) -> bool {
        let Phase::Querying {
            preparation,
            source,
        } = &self.phase
        else {
            return false;
        };
        if preparation.token != token {
            return false;
        }
        let discovery = match result {
            Ok(d) => d,
            Err(n) => {
                self.fail(n);
                return true;
            }
        };
        if !discovery.matches_source(&preparation.origin, source) {
            self.fail(WorktreeNotice::SourceChanged);
            return true;
        }
        let mut draft = Draft::new(
            preparation.origin.clone(),
            source.clone(),
            discovery,
            preparation.mode,
        )
        .starting(preparation.agent);
        let Some(next) = self.advance_token() else {
            self.fail(WorktreeNotice::SourceUnavailable);
            return true;
        };
        draft.token = next;
        self.notice = WorktreeNotice::Preparing;
        self.phase = Phase::Draft(Box::new(draft));
        true
    }
    pub(crate) fn edit(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        if !self.accepts_input(token) {
            return false;
        }
        let choice_stage = matches!(&self.phase, Phase::Draft(draft) if draft.is_checkout());
        let revokes_input_frame = matches!(
            &edit,
            WorktreeEdit::NextField
                | WorktreeEdit::Previous
                | WorktreeEdit::Next
                | WorktreeEdit::ScrollChoices { .. }
        ) || choice_stage
            && matches!(
                &edit,
                WorktreeEdit::Left | WorktreeEdit::Right | WorktreeEdit::Home | WorktreeEdit::End
            );
        if matches!(edit, WorktreeEdit::Completeness(false)) {
            self.shown = None;
        }
        let Phase::Draft(draft) = &mut self.phase else {
            return false;
        };
        let changed = match draft.edit(edit) {
            Ok(changed) => changed,
            Err(n) => {
                self.notice = n;
                self.shown = None;
                return true;
            }
        };
        if changed {
            self.notice = WorktreeNotice::Preparing;
            let retain_input_frame = self.input_frame_live && !revokes_input_frame;
            if let Some(next) = self.advance_token() {
                if let Phase::Draft(draft) = &mut self.phase {
                    draft.token = next;
                    self.input_frame_live = retain_input_frame;
                }
            } else {
                // A changed draft can never retain a submit proof when its
                // revision token cannot advance.
                self.shown = None;
                self.input_frame_live = false;
            }
            if revokes_input_frame {
                self.input_frame_live = false;
            }
        }
        true
    }
    pub(crate) fn draw(&mut self, token: Option<WorktreeToken>) {
        self.shown = token.filter(|token| self.token() == Some(*token));
        self.input_frame_live = self.shown.is_some();
        if self.shown.is_some() {
            self.presentation.presented();
            if let Some(notice) = self.presentation_notice.take()
                && self.notice == WorktreeNotice::PresentationUnavailable
            {
                self.notice = notice;
            }
        }
    }
    pub(crate) fn geometry_blocked(&mut self) -> Option<GeometrySettlement> {
        if !self.visible {
            return None;
        }
        self.shown = None;
        self.input_frame_live = false;
        let settlement = self.presentation.geometry_blocked();
        let accepted = matches!(
            &self.phase,
            Phase::Starting(_)
                | Phase::Observing(_)
                | Phase::Result(_)
                | Phase::Confirm(..)
                | Phase::Resuming(_)
        );
        if accepted {
            self.suppress_focus();
        }
        if settlement == GeometrySettlement::Refused {
            self.visible = false;
            if accepted {
                self.presentation_notice.get_or_insert(self.notice);
            } else {
                self.phase = Phase::Idle;
                self.presentation_notice = None;
            }
            self.notice = WorktreeNotice::PresentationUnavailable;
        }
        Some(settlement)
    }
    pub(crate) fn entry_started(&mut self) {
        self.visible = false;
        self.shown = None;
        self.input_frame_live = false;
    }
    pub(crate) fn confirm(
        &mut self,
        token: WorktreeToken,
        operation_id: WorkbenchRequestId,
        origin: &Origin,
        locale: crate::localization::Locale,
    ) -> Option<Plan> {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self.presentation.interactive()
        {
            return None;
        }
        let Phase::Draft(draft) = &mut self.phase else {
            return None;
        };
        if !draft.origin.unchanged(origin) {
            self.fail(WorktreeNotice::SourceChanged);
            return None;
        }
        if draft.removal.is_some()
            && draft.sidebar.as_ref().is_none_or(|source| {
                !source.valid(origin.cache.canonical_snapshot())
                    || Removal::capture(source) != draft.removal
            })
        {
            self.fail(WorktreeNotice::SourceChanged);
            return None;
        }
        // Git would fail only after the Workspace closed, so nothing is submitted.
        if let (Some(removal), Some(client)) = (&draft.removal, &self.client_directory)
            && removal::inside_checkout(client, removal.path.as_str())
        {
            self.fail(WorktreeNotice::ClientInCheckout);
            return None;
        }
        let Some(mut plan) = draft.plan(operation_id) else {
            self.notice = WorktreeNotice::InvalidInput;
            return None;
        };
        // A create whose Branch was left blank takes a generated branch, so its Workspace takes
        // the first unused short name (Worktree 1, Worktree 2, …) instead of that branch.
        if let WorktreeIntent::Create {
            branch: None,
            label,
            ..
        } = &mut plan.intent
        {
            let workspaces = &origin.cache.canonical_snapshot().navigation.workspaces;
            let generated = (1..=workspaces.len() + 1)
                .map(|ordinal| locale.worktree_default_workspace(ordinal))
                .find(|candidate| {
                    workspaces
                        .iter()
                        .all(|workspace| workspace.label != *candidate)
                })?;
            *label = Some(orchbee_workbench_protocol::WorkbenchLabel::parse(&generated).ok()?);
        }
        // Keep the current frame proof only as automatic-entry eligibility for
        // this accepted operation; the next draw receipt may revoke it.
        self.notice = WorktreeNotice::Pending;
        self.phase = Phase::Starting(Box::new(plan.clone()));
        Some(plan)
    }
    pub(crate) fn start_settled(
        &mut self,
        operation: &WorkbenchRequestId,
        result: Result<WorkbenchWorktreeOutcome, bool>,
    ) -> bool {
        let focus_allowed = self.focus_effect_allowed();
        let Phase::Starting(plan) = &self.phase else {
            return false;
        };
        if &plan.operation_id != operation {
            return false;
        }
        let plan = plan.as_ref().clone();
        let op = Operation {
            plan,
            outcome: None,
            focus_allowed,
            attempt: 1,
        };
        self.phase = Phase::Observing(Box::new(op));
        self.observed(operation, result)
    }
    pub(crate) fn operation(&self) -> Option<&Operation> {
        match &self.phase {
            Phase::Observing(op)
            | Phase::Result(op)
            | Phase::Confirm(op, _)
            | Phase::Resuming(op) => Some(op),
            _ => None,
        }
    }
    pub(crate) fn pending(&self) -> bool {
        matches!(self.phase, Phase::Observing(_))
    }
    pub(crate) fn begin_read(&mut self) -> Option<Plan> {
        if let Phase::Result(op) = &self.phase {
            let mut op = op.clone();
            op.focus_allowed = false;
            self.phase = Phase::Observing(op);
        }
        match &self.phase {
            Phase::Observing(op) => Some(op.plan.clone()),
            _ => None,
        }
    }
    pub(crate) fn carrier_lost(&mut self) -> bool {
        if let Phase::Resuming(op) = &self.phase {
            let operation = op.plan.operation_id.clone();
            return self.resume_settled(&operation, Err(true));
        }
        if matches!(self.phase, Phase::Starting(_)) {
            self.fail(WorktreeNotice::Unknown);
            return true;
        }
        if self.pending() && self.notice != WorktreeNotice::Unknown {
            self.notice = WorktreeNotice::Unknown;
            return true;
        }
        false
    }
    pub(crate) fn observed(
        &mut self,
        operation: &WorkbenchRequestId,
        result: Result<WorkbenchWorktreeOutcome, bool>,
    ) -> bool {
        let focus_effect_allowed = self.focus_effect_allowed();
        let Phase::Observing(op) = &mut self.phase else {
            return false;
        };
        if &op.plan.operation_id != operation {
            return false;
        }
        let mut terminal = false;
        match result {
            Err(submitted) => {
                self.notice = if submitted {
                    WorktreeNotice::Unknown
                } else {
                    WorktreeNotice::NotSubmitted
                };
                if !submitted && op.outcome.is_none() {
                    terminal = true;
                }
            }
            Ok(outcome) => {
                if let WorkbenchWorktreeOutcome::Available {
                    project_id,
                    attempt,
                    status,
                } = &outcome
                {
                    if project_id != &op.plan.project
                        || (op.plan.removal.is_some() && *attempt != op.attempt)
                    {
                        self.notice = WorktreeNotice::Unknown;
                        return true;
                    }
                    match status.as_ref() {
                        WorktreeOperationStatus::Pending { .. } => {
                            self.notice = WorktreeNotice::Pending
                        }
                        WorktreeOperationStatus::Opened { checkout, .. } => {
                            terminal = true;
                            op.focus_allowed &= focus_effect_allowed;
                            let path_matches = match &op.plan.intent {
                                WorktreeIntent::Remove { .. } => false,
                                WorktreeIntent::Open {
                                    selector:
                                        orchbee_workbench_protocol::WorktreeSelector::Path { path },
                                    ..
                                } => same_directory_spelling(path.as_str(), checkout.path.as_str()),
                                // The Server/Worker owns Create destination identity. Its correlated
                                // receipt is canonical; the configured operand preserves spelling.
                                _ => true,
                            };
                            self.notice = if checkout.repository_root
                                == op.plan.discovery.source.repository_root
                                && checkout.common_directory
                                    == op.plan.discovery.source.common_directory
                                && path_matches
                            {
                                if op.focus_allowed {
                                    WorktreeNotice::Opened
                                } else {
                                    WorktreeNotice::EntryDeferred
                                }
                            } else {
                                op.focus_allowed = false;
                                WorktreeNotice::SourceChanged
                            };
                        }
                        WorktreeOperationStatus::Removed { path, workspace_id } => {
                            terminal = true;
                            op.focus_allowed = false;
                            // Git reports the checkout in its own spelling, which can differ from
                            // the Workspace directory's, such as a Client's launch directory.
                            self.notice = if op.plan.removal.as_ref().is_some_and(|target| {
                                same_directory_spelling(target.path.as_str(), path.as_str())
                                    && &target.workspace_id == workspace_id
                            }) {
                                WorktreeNotice::Removed
                            } else {
                                WorktreeNotice::SourceChanged
                            };
                        }
                        WorktreeOperationStatus::Failed { .. } => {
                            terminal = true;
                            self.notice = WorktreeNotice::Failed;
                        }
                    }
                    op.outcome = Some(outcome);
                } else {
                    terminal = true;
                    if op.outcome.as_ref().is_some_and(|known| {
                        matches!(known, WorkbenchWorktreeOutcome::Available { .. })
                    }) {
                        self.notice = WorktreeNotice::Unknown;
                    } else {
                        self.notice = WorktreeNotice::Failed;
                        op.outcome = Some(outcome);
                    }
                }
            }
        }
        if terminal {
            let Phase::Observing(op) = std::mem::replace(&mut self.phase, Phase::Idle) else {
                return false;
            };
            self.phase = Phase::Result(op);
        }
        true
    }
    pub(crate) fn suppress_focus(&mut self) {
        self.focus_revoked = true;
        if let Phase::Observing(op) | Phase::Result(op) = &mut self.phase {
            op.focus_allowed = false;
        }
    }
    pub(crate) fn entry_notice(&mut self, notice: WorktreeNotice) {
        self.notice = notice;
    }
    pub(crate) fn dismiss(&mut self) {
        self.visible = false;
        self.shown = None;
        self.input_frame_live = false;
        match self.phase {
            Phase::Preparing(_) | Phase::Querying { .. } | Phase::Draft(_) => {
                self.phase = Phase::Idle
            }
            Phase::Result(_) => {}
            Phase::Idle => {}
            _ => self.suppress_focus(),
        }
    }
    pub(crate) fn forget_terminal(&mut self) {
        if matches!(self.phase, Phase::Result(_)) || matches!(self.phase, Phase::Idle) {
            self.phase = Phase::Idle;
            self.visible = false;
        }
    }
    pub(crate) fn terminal_result(&self) -> bool {
        matches!(self.phase, Phase::Result(_))
    }
    pub(crate) fn fail(&mut self, notice: WorktreeNotice) {
        self.notice = notice;
        self.shown = None;
        self.input_frame_live = false;
        let operation = match &self.phase {
            Phase::Starting(plan) => Some(Operation {
                plan: plan.as_ref().clone(),
                outcome: None,
                focus_allowed: false,
                attempt: 1,
            }),
            Phase::Observing(op)
            | Phase::Result(op)
            | Phase::Confirm(op, _)
            | Phase::Resuming(op) => Some(op.as_ref().clone()),
            _ => None,
        };
        self.phase = operation.map_or(Phase::Idle, |op| Phase::Result(Box::new(op)));
    }
    pub(crate) fn notice(&self) -> WorktreeNotice {
        self.notice
    }
    /// `Preparing` also marks a quiet draft; it is stated only while the source is being read.
    pub(crate) fn stated_notice(&self) -> Option<WorktreeNotice> {
        (self.notice != WorktreeNotice::Preparing
            || matches!(self.phase, Phase::Preparing(_) | Phase::Querying { .. }))
        .then_some(self.notice)
    }
    pub(crate) fn view(&self) -> Option<WorktreeView<'_>> {
        self.visible.then_some(WorktreeView { launcher: self })
    }
    pub(crate) fn failure(&self) -> Option<&orchbee_workbench_protocol::WorktreeOperationFailure> {
        if let Some(WorkbenchWorktreeOutcome::Available { status, .. }) =
            self.operation().and_then(|operation| operation.outcome.as_ref())
            && let WorktreeOperationStatus::Failed { progress, .. } = status.as_ref()
            && let Some(reason @ orchbee_workbench_protocol::WorktreeOperationFailure::LaunchRequestTooLarge {}) =
                progress.restoration.as_ref().and_then(|receipt| receipt.failure.as_ref())
        {
            return Some(reason);
        }
        WorktreeView { launcher: self }.removal_failure()
    }
}
impl Discovery {
    fn matches_source(&self, origin: &Origin, source: &WorktreeSource) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        match source {
            WorktreeSource::Workspace { workspace_id } => {
                self.source.workspace_id.as_ref() == Some(workspace_id)
                    && origin.cache.directory().is_some_and(|d| {
                        d.path == self.source.requested_path && d.worker_id == self.worker
                    })
            }
            // Git ran where the Server said the Workspace's first terminal stands, on the Worker
            // that holds the Workspace's own directory when it has one, as the Server requires.
            WorktreeSource::WorkspaceDirectory { workspace_id, path } => {
                path == &self.source.requested_path
                    && self.source.workspace_id.as_ref() == Some(workspace_id)
                    && matches!(origin.cache.location(), ClientContextLocation::Workspace { workspace_id: expected, .. } if expected == workspace_id)
                    && origin
                        .cache
                        .directory()
                        .is_none_or(|d| d.worker_id == self.worker)
            }
            WorktreeSource::Directory { path } => {
                path == &self.source.requested_path
                    && self.source.workspace_id.is_none()
                    && origin
                        .primary_binding()
                        .is_some_and(|binding| binding.worker_id() == &self.worker)
            }
        }
    }
}
#[cfg(test)]
pub(crate) mod tests;
