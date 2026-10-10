//! Client-owned Worktree drafts, immutable confirmation and retained observation; Git and terminal
//! attachment remain with their existing owners.
mod draft;
mod origin;
use draft::Draft;
pub(crate) use draft::{WorktreeEdit, WorktreeStage, WorktreeToken, WorktreeView};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkbenchProjectId, WorkbenchRequestId, WorkbenchWorktreeOutcome,
    WorkerId, WorktreeIntent, WorktreeOperationStatus, WorktreeQueryEntry, WorktreeQuerySource,
    WorktreeSource,
};
pub(in crate::terminal_session) use origin::{Origin, Preparation};

/// The requested everyday Worktree operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeMode {
    Create,
    Open,
}
/// A presentation reason, independent of transport diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeNotice {
    Preparing,
    InvalidInput,
    InputLimit,
    SourceUnavailable,
    ListingUnavailable,
    ConfigurationUnavailable,
    Busy,
    NotSubmitted,
    Pending,
    Unknown,
    Failed,
    Opened,
    EntryPending,
    EntryFailed,
    SourceChanged,
}
#[derive(Clone, Debug)]
pub(in crate::terminal_session) struct Discovery {
    pub source: WorktreeQuerySource,
    pub worker: WorkerId,
    pub entries: Vec<WorktreeQueryEntry>,
    pub configured_root: Option<WorkbenchDirectoryPath>,
}
#[derive(Clone, Debug)]
pub(in crate::terminal_session) struct Plan {
    pub operation_id: WorkbenchRequestId,
    pub origin: Origin,
    pub source: WorktreeSource,
    pub discovery: Discovery,
    pub project: Option<WorkbenchProjectId>,
    pub intent: WorktreeIntent,
}
#[derive(Clone, Debug)]
pub(in crate::terminal_session) struct Operation {
    pub plan: Plan,
    pub outcome: Option<WorkbenchWorktreeOutcome>,
    pub focus_allowed: bool,
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
}
pub(in crate::terminal_session) struct Launcher {
    phase: Phase,
    last_token: u64,
    visible: bool,
    notice: WorktreeNotice,
    shown: Option<WorktreeToken>,
    focus_revoked: bool,
}
impl Default for Launcher {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            last_token: 0,
            visible: false,
            notice: WorktreeNotice::Preparing,
            shown: None,
            focus_revoked: false,
        }
    }
}
impl Launcher {
    /// The painted token and exact row authorize one Open, without arming a
    /// replacement token for a selection that has never been drawn.
    pub fn choose_and_confirm(
        &mut self,
        token: WorktreeToken,
        index: usize,
        operation_id: WorkbenchRequestId,
        origin: &Origin,
    ) -> Option<Plan> {
        if self.token() != Some(token) || self.shown != Some(token) {
            return None;
        }
        let Phase::Draft(draft) = &mut self.phase else {
            return None;
        };
        if !draft.origin.unchanged(origin) || !draft.choose_checkout(index) {
            return None;
        }
        self.confirm(token, operation_id, origin)
    }

    pub fn scroll_choices(&mut self, token: WorktreeToken, up: bool) -> bool {
        if self.token() != Some(token)
            || self.shown != Some(token)
            || !self
                .view()
                .is_some_and(|view| view.stage() == WorktreeStage::Checkout)
        {
            return false;
        }
        self.edit(
            token,
            if up {
                WorktreeEdit::Previous
            } else {
                WorktreeEdit::Next
            },
        )
    }

    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn token(&self) -> Option<WorktreeToken> {
        self.visible.then_some(WorktreeToken(self.last_token))
    }
    fn advance_token(&mut self) -> Option<WorktreeToken> {
        self.last_token = self.last_token.checked_add(1)?;
        self.shown = None;
        Some(WorktreeToken(self.last_token))
    }
    pub fn reopen(&mut self) -> bool {
        if matches!(self.phase, Phase::Idle) {
            return false;
        }
        self.visible = true;
        self.shown = None;
        true
    }
    pub fn open(&mut self, mut preparation: Preparation) -> bool {
        if !matches!(self.phase, Phase::Idle) {
            return self.reopen();
        }
        let Some(token) = self.advance_token() else {
            return false;
        };
        preparation.token = token;
        self.visible = true;
        self.notice = WorktreeNotice::Preparing;
        self.focus_revoked = false;
        self.phase = Phase::Preparing(Box::new(preparation));
        true
    }
    pub fn preparation(&self) -> Option<&Preparation> {
        match &self.phase {
            Phase::Preparing(p) => Some(p),
            _ => None,
        }
    }
    pub fn prepared(&mut self, p: &Preparation, source: Option<WorktreeSource>) -> bool {
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
    pub fn query_input(&self) -> Option<(&Preparation, &WorktreeSource)> {
        match &self.phase {
            Phase::Querying {
                preparation,
                source,
            } => Some((preparation, source)),
            _ => None,
        }
    }
    pub fn discovered(
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
        );
        let Some(next) = self.advance_token() else {
            self.fail(WorktreeNotice::SourceUnavailable);
            return true;
        };
        draft.token = next;
        self.notice = WorktreeNotice::Preparing;
        self.phase = Phase::Draft(Box::new(draft));
        true
    }
    pub fn edit(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        if self.token() != Some(token) {
            return false;
        }
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
            if let Some(next) = self.advance_token()
                && let Phase::Draft(draft) = &mut self.phase
            {
                draft.token = next;
            }
        }
        true
    }
    pub fn draw(&mut self, token: Option<WorktreeToken>) {
        self.shown = token.filter(|token| self.token() == Some(*token));
    }
    pub fn entry_started(&mut self) {
        self.visible = false;
        self.shown = None;
    }
    pub fn confirm(
        &mut self,
        token: WorktreeToken,
        operation_id: WorkbenchRequestId,
        origin: &Origin,
    ) -> Option<Plan> {
        if self.token() != Some(token) || self.shown != Some(token) {
            return None;
        }
        let Phase::Draft(draft) = &mut self.phase else {
            return None;
        };
        if !draft.origin.unchanged(origin) {
            self.fail(WorktreeNotice::SourceChanged);
            return None;
        }
        let Some(plan) = draft.plan(operation_id) else {
            self.notice = WorktreeNotice::InvalidInput;
            return None;
        };
        self.shown = None;
        self.notice = WorktreeNotice::Pending;
        self.phase = Phase::Starting(Box::new(plan.clone()));
        Some(plan)
    }
    /// A Create click already supplies the intent; no additional form or draw approval is needed.
    pub fn create_with_defaults(
        &mut self,
        operation_id: WorkbenchRequestId,
        origin: &Origin,
        locale: crate::localization::Locale,
    ) -> Option<Plan> {
        let Phase::Draft(draft) = &self.phase else {
            return None;
        };
        if !draft.is_create()
            || !draft.origin.unchanged(origin)
            || !self.visible
            || self.focus_revoked
        {
            return None;
        }
        let mut plan = draft.plan(operation_id)?;
        let workspaces = &origin.cache.canonical_snapshot().navigation.workspaces;
        let generated = (1..=workspaces.len() + 1)
            .map(|ordinal| locale.worktree_default_workspace(ordinal))
            .find(|candidate| {
                workspaces
                    .iter()
                    .all(|workspace| workspace.label != *candidate)
            })?;
        let WorktreeIntent::Create { label, .. } = &mut plan.intent else {
            return None;
        };
        *label = Some(orchbee_workbench_protocol::WorkbenchLabel::parse(&generated).ok()?);
        self.shown = None;
        self.notice = WorktreeNotice::Pending;
        self.phase = Phase::Starting(Box::new(plan.clone()));
        Some(plan)
    }
    pub fn start_settled(
        &mut self,
        operation: &WorkbenchRequestId,
        result: Result<WorkbenchWorktreeOutcome, bool>,
    ) -> bool {
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
            focus_allowed: self.visible && !self.focus_revoked,
        };
        self.phase = Phase::Observing(Box::new(op));
        self.observed(operation, result)
    }
    pub fn operation(&self) -> Option<&Operation> {
        match &self.phase {
            Phase::Observing(op) | Phase::Result(op) => Some(op),
            _ => None,
        }
    }
    pub fn pending(&self) -> bool {
        matches!(self.phase, Phase::Observing(_))
    }
    pub fn begin_read(&mut self) -> Option<Plan> {
        if let Phase::Result(op) = &self.phase {
            let mut op = op.clone();
            op.focus_allowed = false;
            self.phase = Phase::Observing(op);
        }
        self.operation().map(|op| op.plan.clone())
    }
    pub fn carrier_lost(&mut self) -> bool {
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
    pub fn observed(
        &mut self,
        operation: &WorkbenchRequestId,
        result: Result<WorkbenchWorktreeOutcome, bool>,
    ) -> bool {
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
                    project_id, status, ..
                } = &outcome
                {
                    if project_id != &op.plan.project {
                        self.notice = WorktreeNotice::Unknown;
                        return true;
                    }
                    match status.as_ref() {
                        WorktreeOperationStatus::Pending { .. } => {
                            self.notice = WorktreeNotice::Pending
                        }
                        WorktreeOperationStatus::Opened { checkout, .. } => {
                            terminal = true;
                            let path_matches = match &op.plan.intent {
                                WorktreeIntent::Open {
                                    selector:
                                        orchbee_workbench_protocol::WorktreeSelector::Path { path },
                                    ..
                                } => path == &checkout.path,
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
                                WorktreeNotice::Opened
                            } else {
                                op.focus_allowed = false;
                                WorktreeNotice::SourceChanged
                            };
                        }
                        _ => {
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
    pub fn suppress_focus(&mut self) {
        self.focus_revoked = true;
        if let Phase::Observing(op) | Phase::Result(op) = &mut self.phase {
            op.focus_allowed = false;
        }
    }
    pub fn entry_notice(&mut self, notice: WorktreeNotice) {
        self.notice = notice;
    }
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.shown = None;
        match self.phase {
            Phase::Preparing(_) | Phase::Querying { .. } | Phase::Draft(_) => {
                self.phase = Phase::Idle
            }
            Phase::Result(_) => {}
            Phase::Idle => {}
            _ => self.suppress_focus(),
        }
    }
    pub fn forget_terminal(&mut self) {
        if matches!(self.phase, Phase::Result(_)) || matches!(self.phase, Phase::Idle) {
            self.phase = Phase::Idle;
            self.visible = false;
        }
    }
    pub fn terminal_result(&self) -> bool {
        matches!(self.phase, Phase::Result(_))
    }
    pub fn fail(&mut self, notice: WorktreeNotice) {
        self.notice = notice;
        self.shown = None;
        let operation = match &self.phase {
            Phase::Starting(plan) => Some(Operation {
                plan: plan.as_ref().clone(),
                outcome: None,
                focus_allowed: false,
            }),
            Phase::Observing(op) | Phase::Result(op) => Some(op.as_ref().clone()),
            _ => None,
        };
        self.phase = operation.map_or(Phase::Idle, |op| Phase::Result(Box::new(op)));
    }
    pub fn notice(&self) -> WorktreeNotice {
        self.notice
    }
    pub fn view(&self) -> Option<WorktreeView<'_>> {
        self.visible.then_some(WorktreeView { launcher: self })
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
pub(in crate::terminal_session) mod tests;
