//! Worktree interaction drives one retained operation and reuses native source and membership owners.
use super::{
    CommandMailbox, ModalPresentation, NativeSessionView, NativeUi, OperationDeadlines,
    SessionActionDisposition, SessionNotice,
};
pub(crate) use crate::terminal_session::state::worktree_launch::{
    WorktreeEdit, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
use crate::terminal_session::{
    state::worktree_launch::{Launcher, Origin, Plan, Preparation, addressed::SidebarDiscovery},
    worktree_lane::{Action, Exchange, Job, Lane, Reply},
};
use orchbee_workbench_protocol::WorkbenchRequestId;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod entry;
use entry::Entry;

/// Current token, field owner and exact remaining UTF-8 budget.
#[derive(Clone, Copy)]
pub(crate) struct WorktreeInputContext {
    pub(crate) token: WorktreeToken,
    pub(crate) stage: WorktreeStage,
    pub(crate) remaining_bytes: usize,
}
#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) launcher: Launcher,
    origin: Option<crate::local_client::LocalClientOrigin>,
    lane: Option<Lane>,
    query: Option<WorktreeToken>,
    read_at: Option<Instant>,
    observation_until: Option<Instant>,
    entry: Option<Entry>,
}
impl Coordinator {
    pub(super) fn remove_sidebar(&mut self, origin: Origin, discovery: SidebarDiscovery) -> bool {
        if self.entry.is_some() {
            return false;
        }
        if !self.launcher.hidden_opened_result() && self.launcher.reopen() {
            return true;
        }
        self.launcher
            .open_sidebar(origin, discovery, WorktreeMode::Remove, None)
    }

    pub(super) fn open_sidebar(
        &mut self,
        origin: Origin,
        discovery: SidebarDiscovery,
        checkout: Option<&orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) -> bool {
        self.entry.is_none()
            && self.launcher.open_sidebar(
                origin,
                discovery,
                if checkout.is_some() {
                    WorktreeMode::Open
                } else {
                    WorktreeMode::Create
                },
                checkout,
            )
    }
    pub(super) fn dismiss_focus(&mut self) {
        self.launcher.dismiss();
        self.entry = None;
    }
    pub(super) fn set_origin(&mut self, origin: crate::local_client::LocalClientOrigin) {
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) -> bool {
        self.lane.as_mut().is_none_or(Lane::close)
    }
    pub(super) fn ensure_lane(&mut self, clock: &crate::local_client::LocalOperationClock) -> bool {
        if self.lane.as_ref().is_some_and(Lane::failed) {
            if !self.close() {
                return false;
            }
            self.lane = None;
        }
        if self.lane.is_none() {
            let Some(origin) = self.origin.clone() else {
                return false;
            };
            self.lane = Lane::spawn(origin, clock.clone()).ok();
        }
        self.lane.is_some()
    }
}
impl<'a> NativeSessionView<'a> {
    pub(crate) fn worktree(&self) -> Option<WorktreeView<'a>> {
        self.worktree?.view()
    }
    pub(crate) fn worktree_capture_claim(&self) -> bool {
        self.worktree.is_some_and(Launcher::visible)
    }
    pub(crate) fn worktree_input_context(&self) -> Option<WorktreeInputContext> {
        let launcher = self.worktree?;
        let view = launcher.view()?;
        let token = view.token();
        launcher
            .accepts_input(token)
            .then_some(WorktreeInputContext {
                token,
                stage: view.stage(),
                remaining_bytes: view.remaining_bytes(),
            })
    }
    pub(crate) fn worktree_notice(&self) -> Option<WorktreeNotice> {
        let launcher = self.worktree?;
        (launcher.visible()
            || launcher.operation().is_some()
            || launcher.notice() == WorktreeNotice::PresentationUnavailable)
            .then_some(launcher.notice())
    }
}
impl NativeUi {
    pub(in crate::terminal_session) fn choose_worktree(
        &mut self,
        token: WorktreeToken,
        index: usize,
    ) {
        let Some(origin) = self
            .state
            .as_ref()
            .and_then(|state| state.worktree_origin())
        else {
            return;
        };
        let Some(operation) = self.worktree_request() else {
            return;
        };
        let Some(plan) = self
            .worktrees
            .launcher
            .choose_and_confirm(token, index, operation, &origin)
        else {
            return;
        };
        self.worktrees.observation_until = Some(self.clock.now() + Duration::from_secs(120));
        self.submit_worktree_start(plan);
    }

    pub(in crate::terminal_session) fn scroll_worktree(
        &mut self,
        token: WorktreeToken,
        up: bool,
        lines: usize,
    ) -> Option<WorktreeToken> {
        self.worktrees
            .launcher
            .scroll_choices(token, up, lines)
            .then(|| self.worktrees.launcher.token())
            .flatten()
    }

    pub(super) fn request_worktree(&mut self, mode: WorktreeMode) -> SessionActionDisposition {
        if self.retirement_failure.is_some()
            || (self.modal_owns_input() && !self.worktrees.launcher.visible())
            || self.copy_mode_active()
            || self.popup.captured()
        {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(SessionNotice::ModalOwnsInput);
            }
            return SessionActionDisposition::KeepNavigator;
        }
        if self.worktrees.entry.is_none() && self.worktrees.launcher.hidden_opened_result() {
            self.worktrees.launcher.forget_terminal();
        }
        if self.worktrees.launcher.reopen() {
            return SessionActionDisposition::DismissNavigator;
        }
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return SessionActionDisposition::KeepNavigator;
        };
        if !self.worktrees.ensure_lane(&self.clock) {
            return SessionActionDisposition::KeepNavigator;
        }
        self.board.close();
        if mailbox.try_enqueue_worktree_preparation_for(
            state,
            &mut self.worktrees.launcher,
            mode,
            deadlines,
            &self.clock,
        ) {
            self.worktrees.query = None;
            self.worktrees.entry = None;
            self.worktrees.read_at = None;
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }
    pub(super) fn apply_worktree_preparation(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        wait: super::super::mailbox::UiWaitKey,
        p: Preparation,
        source: Option<orchbee_workbench_protocol::WorktreeSource>,
        outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
    ) -> bool {
        let changed = self.apply_refresh_event(mailbox, wait, p.generation, outcome);
        if self.retirement_failure.is_some() {
            self.worktrees
                .launcher
                .fail(WorktreeNotice::SourceUnavailable);
            return true;
        }
        let unchanged = self
            .state
            .as_ref()
            .and_then(|s| s.worktree_origin())
            .is_some_and(|current| p.origin.unchanged(&current) && current.cache == p.origin.cache);
        changed
            | self
                .worktrees
                .launcher
                .prepared(&p, source.filter(|_| unchanged))
    }
    pub(in crate::terminal_session) fn worktree_input_context(
        &self,
    ) -> Option<WorktreeInputContext> {
        self.view().worktree_input_context()
    }
    #[cfg(test)]
    pub(in crate::terminal_session) fn worktree_capture_claim(&self) -> bool {
        self.view().worktree_capture_claim()
    }
    pub(in crate::terminal_session) fn edit_worktree(
        &mut self,
        token: WorktreeToken,
        edit: WorktreeEdit<'_>,
    ) -> bool {
        if self.worktrees.launcher.token() != Some(token) {
            return false;
        }
        if !self.worktrees.launcher.accepts_input(token) {
            return true;
        }
        let stage = self.worktrees.launcher.view().map(|v| v.stage());
        if matches!(stage, Some(WorktreeStage::Waiting | WorktreeStage::Result)) {
            if matches!(edit, WorktreeEdit::Append("r") | WorktreeEdit::ReadStatus) {
                self.worktrees.read_at = Some(self.clock.now());
            }
            return true;
        }
        self.worktrees.launcher.edit(token, edit)
    }
    pub(in crate::terminal_session) fn dismiss_worktree(&mut self, token: WorktreeToken) {
        if self.worktrees.launcher.token() != Some(token) {
            return;
        }
        if self.worktrees.launcher.cancel_remove_resume() {
            return;
        }
        if self.worktrees.launcher.view().is_some_and(|view| {
            view.can_resume_remove()
                || (view.removal_target().is_some() && view.notice() == WorktreeNotice::Unknown)
        }) {
            self.worktrees.launcher.dismiss();
            return;
        }
        if self.worktrees.launcher.terminal_result() {
            self.worktrees.launcher.forget_terminal();
            self.worktrees.entry = None;
        } else {
            self.worktrees.launcher.dismiss();
        }
    }
    pub(in crate::terminal_session) fn after_worktree_draw(
        &mut self,
        draw: Option<ModalPresentation<WorktreeToken>>,
    ) {
        match draw {
            Some(ModalPresentation::Presented(token)) => {
                self.worktrees.launcher.draw(Some(token));
                if self.worktrees.launcher.sidebar_open_ready() {
                    self.submit_worktree(token);
                } else {
                    self.submit_default_worktree_if_ready();
                }
            }
            Some(ModalPresentation::NotPresented) => {
                if self.worktrees.launcher.geometry_blocked().is_some() {
                    self.worktrees.entry = None;
                }
            }
            None => self.worktrees.launcher.draw(None),
        }
    }
    pub(in crate::terminal_session) fn submit_worktree(&mut self, token: WorktreeToken) {
        if self.worktrees.launcher.prepare_remove_resume(token) {
            return;
        }
        if let Some((plan, expected_attempt)) = self.worktrees.launcher.confirm_remove_resume(token)
        {
            let operation = plan.operation_id.clone();
            self.worktrees.read_at = None;
            self.worktrees.observation_until = Some(self.clock.now() + Duration::from_secs(120));
            if !self.issue_worktree(Action::Resume {
                plan: Box::new(plan),
                expected_attempt,
            }) {
                self.worktrees
                    .launcher
                    .resume_settled(&operation, Err(false));
            }
            return;
        }
        let Some(origin) = self.state.as_ref().and_then(|s| s.worktree_origin()) else {
            return;
        };
        let Some(operation) = self.worktree_request() else {
            return;
        };
        let Some(plan) = self.worktrees.launcher.confirm(token, operation, &origin) else {
            return;
        };
        self.worktrees.observation_until = Some(self.clock.now() + Duration::from_secs(120));
        self.submit_worktree_start(plan);
    }
    fn worktree_request(&self) -> Option<WorkbenchRequestId> {
        self.mailbox.as_ref()?.allocate_workbench_request_id().ok()
    }
    pub(super) fn issue_worktree(&mut self, action: Action) -> bool {
        self.issue_worktree_before(action, self.clock.now() + Duration::from_secs(10))
    }
    pub(super) fn issue_worktree_before(&mut self, action: Action, deadline: Instant) -> bool {
        if !self.worktrees.ensure_lane(&self.clock) {
            return false;
        }
        if self.clock.now() >= deadline
            || self.worktrees.lane.as_ref().is_none_or(|lane| !lane.idle())
        {
            return false;
        }
        let (Some(a), Some(b), Some(c), Some(d), Some(e)) = (
            self.worktree_request(),
            self.worktree_request(),
            self.worktree_request(),
            self.worktree_request(),
            self.worktree_request(),
        ) else {
            return false;
        };
        let Some(mailbox) = self.mailbox.as_ref() else {
            return false;
        };
        let (Ok(v1), Ok(v2), Ok(v3), Ok(v4)) = (
            mailbox.allocate_obcs_request_id(),
            mailbox.allocate_obcs_request_id(),
            mailbox.allocate_obcs_request_id(),
            mailbox.allocate_obcs_request_id(),
        ) else {
            return false;
        };
        self.worktrees.lane.as_ref().is_some_and(|lane| {
            lane.submit(Job {
                action,
                requests: [a, b, c, d, e],
                viewers: [v1, v2, v3, v4],
                deadline,
            })
        })
    }
    fn submit_worktree_start(&mut self, plan: Plan) {
        let operation = plan.operation_id.clone();
        if !self.issue_worktree(Action::Start(Box::new(plan))) {
            self.worktrees
                .launcher
                .start_settled(&operation, Err(false));
        }
    }
    fn submit_default_worktree_if_ready(&mut self) -> bool {
        if self.worktrees.origin.is_none() || !self.worktrees.launcher.default_create_ready() {
            return false;
        }
        let Some(origin) = self
            .state
            .as_ref()
            .and_then(|state| state.worktree_origin())
        else {
            return false;
        };
        let Some(operation_id) = self.worktree_request() else {
            return false;
        };
        let Some(plan) = self.worktrees.launcher.create_with_defaults(
            operation_id,
            &origin,
            crate::localization::select_locale(|name| std::env::var_os(name)),
        ) else {
            return false;
        };
        self.worktrees.observation_until = Some(self.clock.now() + Duration::from_secs(120));
        self.submit_worktree_start(plan);
        true
    }
    pub(super) fn drive_worktree(&mut self) -> bool {
        let mut changed = false;
        if let Some(reply) = self.worktrees.lane.as_ref().and_then(Lane::take) {
            changed = true;
            match reply {
                Reply::SidebarQuery { episode, result } => {
                    self.settle_sidebar_worktrees(episode, result)
                }
                Reply::Query { token, result } => {
                    self.worktrees.query = None;
                    if let Some((p, _)) = self.worktrees.launcher.query_input()
                        && p.token == token
                    {
                        if self
                            .state
                            .as_ref()
                            .and_then(|s| s.worktree_origin())
                            .is_none_or(|current| !p.origin.unchanged(&current))
                        {
                            self.worktrees.launcher.fail(WorktreeNotice::SourceChanged);
                        } else {
                            self.worktrees.launcher.discovered(token, result);
                            self.submit_default_worktree_if_ready();
                        }
                    }
                }
                Reply::Operation {
                    operation,
                    exchange,
                    result,
                    snapshot,
                } => {
                    let applied = match exchange {
                        Exchange::Resume => {
                            self.worktrees.launcher.resume_settled(&operation, result)
                        }
                        Exchange::Start => {
                            self.worktrees.launcher.start_settled(&operation, result)
                        }
                        Exchange::Read => self.worktrees.launcher.observed(&operation, result),
                    };
                    if applied {
                        if self.worktrees.launcher.terminal_result()
                            && self
                                .worktrees
                                .launcher
                                .operation()
                                .is_some_and(|op| op.plan.removal.is_some())
                            && let Some(state) = self.state.as_mut()
                        {
                            state.note_publication_hint();
                        }
                        if self.worktrees.launcher.pending() {
                            self.worktrees.read_at =
                                Some(self.clock.now() + Duration::from_millis(100));
                        } else {
                            self.worktrees.read_at = None;
                            if self.worktrees.launcher.notice() == WorktreeNotice::Opened {
                                self.begin_worktree_entry(snapshot);
                            }
                        }
                    }
                }
            }
        }
        changed |= self.drive_sidebar_worktrees();
        if self.worktrees.lane.as_ref().is_some_and(Lane::failed) {
            changed |= self.worktrees.launcher.carrier_lost();
            self.worktrees.read_at = None;
        }
        if let Some((p, source)) = self.worktrees.launcher.query_input()
            && self.worktrees.query.is_none()
            && self.worktrees.lane.as_ref().is_some_and(Lane::idle)
        {
            let (token, origin, source) = (p.token, p.origin.clone(), source.clone());
            if self.issue_worktree(Action::Query {
                token,
                source,
                origin: Box::new(origin),
            }) {
                self.worktrees.query = Some(token);
                changed = true;
            } else {
                self.worktrees
                    .launcher
                    .fail(WorktreeNotice::ListingUnavailable);
                changed = true;
            }
        }
        if self.worktrees.launcher.pending()
            && self
                .worktrees
                .observation_until
                .is_some_and(|until| self.clock.now() >= until)
        {
            self.worktrees
                .launcher
                .entry_notice(WorktreeNotice::Unknown);
            self.worktrees.read_at = None;
            self.worktrees.observation_until = None;
            changed = true;
        }
        if self
            .worktrees
            .read_at
            .is_some_and(|at| self.clock.now() >= at)
            && self.worktrees.lane.as_ref().is_none_or(Lane::idle)
        {
            if let Some(plan) = self.worktrees.launcher.begin_read() {
                if self.worktrees.observation_until.is_none() {
                    self.worktrees.observation_until =
                        Some(self.clock.now() + Duration::from_secs(120));
                }
                if self.issue_worktree(Action::Read(Box::new(plan))) {
                    self.worktrees.read_at = None;
                    changed = true;
                } else {
                    self.worktrees.read_at = None;
                    changed |= self.worktrees.launcher.carrier_lost();
                }
            } else {
                self.worktrees.read_at = None;
            }
        }
        changed | self.drive_worktree_entry()
    }
}
