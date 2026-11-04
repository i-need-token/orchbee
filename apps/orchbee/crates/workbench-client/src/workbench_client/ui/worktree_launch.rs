//! Worktree interaction drives one retained operation and reuses native source and membership owners.
use super::{
    ClientActionDisposition, ClientNotice, CommandMailbox, ModalPresentation, OperationDeadlines,
    WorkbenchClientUi, WorkbenchClientView,
};
use crate::workbench_client::worktree_lane::{Action, ConfiguredRoot, Exchange, Job, Lane, Reply};
use crate::workbench_state::worktree_launch::addressed::SidebarDiscovery;
use crate::workbench_state::worktree_launch::{Launcher, Origin, Plan, Preparation};
pub(crate) use crate::workbench_state::worktree_launch::{
    WorktreeEdit, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
use orchbee_workbench_protocol::WorkbenchRequestId;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod entry;
use entry::Entry;

/// Each accepted Start, Resume or explicit read observes its operation for at most this long.
const OBSERVATION_WINDOW: Duration = Duration::from_secs(120);
/// Server publishes canonical changes only for Opened and Removed, so a pending status is
/// re-read with this bounded backoff until it settles or its observation window ends.
const FIRST_READ_DELAY: Duration = Duration::from_millis(250);
const MAXIMUM_READ_DELAY: Duration = Duration::from_secs(2);

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
    changes: Option<crate::local_client::subscription::ChangeSubscription>,
    changes_dirty: bool,
    observation_until: Option<Instant>,
    /// The last automatic read delay; zero restarts the backoff.
    read_delay: Duration,
    entry: Option<Entry>,
    /// Absent until the first Server configuration snapshot is applied.
    configured_root: Option<ConfiguredRoot>,
}
impl Coordinator {
    /// A hidden Opened result whose terminal entry has finished.
    pub(super) fn settled_opened_result(&self) -> bool {
        self.entry.is_none() && self.launcher.hidden_opened_result()
    }

    pub(super) fn remove_sidebar(&mut self, origin: Origin, discovery: SidebarDiscovery) -> bool {
        if self.entry.is_some() {
            return false;
        }
        self.launcher
            .forget_stale_removal(origin.cache.canonical_snapshot());
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
        if self.entry.is_some() {
            return false;
        }
        self.launcher
            .forget_stale_removal(origin.cache.canonical_snapshot());
        self.launcher.open_sidebar(
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
        self.changes = origin.watch_changes().ok();
        self.changes_dirty = true;
        self.origin = Some(origin);
    }
    pub(super) fn close(&mut self) -> bool {
        self.lane.as_mut().is_none_or(Lane::close)
    }
    /// Process exit: asks the Worktree lane thread to stop without waiting.
    pub(super) fn request_stop(&mut self) {
        if let Some(lane) = &mut self.lane {
            lane.request_stop();
        }
    }
    /// Process exit: observes the Worktree lane thread against the shared deadline.
    pub(super) fn stop_before(&mut self, deadline: Instant) -> std::io::Result<()> {
        self.lane
            .as_mut()
            .map_or(Ok(()), |lane| lane.stop_before(deadline))
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
impl<'a> WorkbenchClientView<'a> {
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
    /// The worktree notice worth stating; a drawn draft has nothing to report.
    pub(crate) fn worktree_stated_notice(&self) -> Option<WorktreeNotice> {
        let launcher = self.worktree?;
        self.worktree_notice().and(launcher.stated_notice())
    }
}
impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn configure_worktrees_directory(
        &mut self,
        directory: Option<&str>,
    ) {
        self.worktrees.configured_root = Some(
            crate::workbench_client::worktree_lane::configured_root(directory),
        );
    }
    pub(in crate::workbench_client) fn choose_worktree(
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
        self.observe_worktree();
        self.submit_worktree_start(plan);
    }

    pub(in crate::workbench_client) fn scroll_worktree(
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

    pub(super) fn request_worktree(&mut self, mode: WorktreeMode) -> ClientActionDisposition {
        if self.retirement_failure.is_some()
            || (self.modal_owns_input() && !self.worktrees.launcher.visible())
            || self.copy_mode_active()
            || self.popup.captured()
        {
            if let Some(state) = self.state.as_mut() {
                state.set_notice(ClientNotice::ModalOwnsInput);
            }
            return ClientActionDisposition::KeepNavigator;
        }
        if self.worktrees.entry.is_none() && self.worktrees.launcher.hidden_opened_result() {
            self.worktrees.launcher.forget_terminal();
        }
        if self.worktrees.entry.is_none()
            && let Some(state) = self.state.as_ref()
        {
            self.worktrees
                .launcher
                .forget_stale_removal(state.cache().canonical_snapshot());
        }
        if self.worktrees.launcher.reopen() {
            return ClientActionDisposition::DismissNavigator;
        }
        let landing = self.switch_landing();
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        let (Some(mailbox), Some(state)) = (self.mailbox.as_ref(), self.state.as_mut()) else {
            return ClientActionDisposition::KeepNavigator;
        };
        // A landing switch settles on its own: the key waits for it rather than being refused.
        if landing {
            self.hold_intent(super::held_intent::HeldIntent::Worktree(mode));
            return ClientActionDisposition::DismissNavigator;
        }
        // A refused W or O starts nothing; say why instead of dropping the key.
        if let Some(notice) = state.worktree_refusal() {
            state.set_notice(notice);
            return ClientActionDisposition::KeepNavigator;
        }
        if !self.worktrees.ensure_lane(&self.clock) {
            return ClientActionDisposition::KeepNavigator;
        }
        if state.background_refresh_only() {
            self.hold_intent(super::held_intent::HeldIntent::Worktree(mode));
            return ClientActionDisposition::DismissNavigator;
        }
        self.board_reads.close_board(&mut self.board);
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
            ClientActionDisposition::DismissNavigator
        } else {
            if !self.worktrees.launcher.visible() {
                state.set_notice(ClientNotice::LocalBusy);
            }
            ClientActionDisposition::KeepNavigator
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
            .is_some_and(|current| p.origin.unchanged(&current));
        changed
            | self
                .worktrees
                .launcher
                .prepared(&p, source.filter(|_| unchanged))
    }
    pub(in crate::workbench_client) fn worktree_input_context(
        &self,
    ) -> Option<WorktreeInputContext> {
        self.view().worktree_input_context()
    }
    #[cfg(test)]
    pub(in crate::workbench_client) fn worktree_capture_claim(&self) -> bool {
        self.view().worktree_capture_claim()
    }
    pub(in crate::workbench_client) fn edit_worktree(
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
            // Status reads run on their own; the only key a running or settled operation takes
            // is the retained removal's Stop Tracking.
            if matches!(edit, WorktreeEdit::Append("x") | WorktreeEdit::Abandon) {
                self.worktrees.launcher.prepare_remove_abandon(token);
            }
            return true;
        }
        self.worktrees.launcher.edit(token, edit)
    }
    pub(in crate::workbench_client) fn dismiss_worktree(&mut self, token: WorktreeToken) {
        if self.worktrees.launcher.token() != Some(token) {
            return;
        }
        if self.worktrees.launcher.cancel_remove_recovery() {
            return;
        }
        if let Some(state) = self.state.as_ref()
            && self
                .worktrees
                .launcher
                .forget_stale_removal(state.cache().canonical_snapshot())
        {
            self.worktrees.entry = None;
            return;
        }
        if self.worktrees.launcher.retains_removal() {
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
    pub(in crate::workbench_client) fn after_worktree_draw(
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
    pub(in crate::workbench_client) fn submit_worktree(&mut self, token: WorktreeToken) {
        if self.worktrees.launcher.prepare_remove_resume(token) {
            return;
        }
        if self.worktrees.launcher.confirm_remove_abandon(token) {
            self.worktrees.entry = None;
            self.worktrees.read_at = None;
            return;
        }
        if let Some((plan, expected_attempt)) = self.worktrees.launcher.confirm_remove_resume(token)
        {
            let operation = plan.operation_id.clone();
            self.worktrees.read_at = None;
            self.observe_worktree();
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
        // An invalid field blocks submission; Enter moves the caret to the first one instead.
        if let Some(first) = self
            .worktrees
            .launcher
            .view()
            .and_then(|view| view.first_invalid_field())
        {
            if self.worktrees.launcher.view().map(|view| view.stage()) != Some(first) {
                self.worktrees.launcher.edit(token, WorktreeEdit::NextField);
            }
            return;
        }
        let Some(origin) = self.state.as_ref().and_then(|s| s.worktree_origin()) else {
            return;
        };
        let Some(operation) = self.worktree_request() else {
            return;
        };
        self.worktrees
            .launcher
            .set_client_directory(windows_client_directory());
        let Some(plan) = self.worktrees.launcher.confirm(token, operation, &origin) else {
            return;
        };
        self.observe_worktree();
        self.submit_worktree_start(plan);
    }
    /// Starts one bounded observation window with a fresh read backoff.
    fn observe_worktree(&mut self) {
        self.worktrees.observation_until = Some(self.clock.now() + OBSERVATION_WINDOW);
        self.worktrees.read_delay = Duration::ZERO;
    }
    /// Schedules the next automatic read while the observation window is open; an earlier
    /// read requested by a change notification or the user is kept.
    fn schedule_worktree_read(&mut self) {
        if self.worktrees.observation_until.is_none() {
            return;
        }
        let delay = if self.worktrees.read_delay.is_zero() {
            FIRST_READ_DELAY
        } else {
            self.worktrees
                .read_delay
                .saturating_mul(2)
                .min(MAXIMUM_READ_DELAY)
        };
        self.worktrees.read_delay = delay;
        let at = self.clock.now() + delay;
        self.worktrees.read_at = Some(self.worktrees.read_at.map_or(at, |current| current.min(at)));
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
        let Ok(requests) = <[WorkbenchRequestId; 6]>::try_from(
            (0..6)
                .map_while(|_| self.worktree_request())
                .collect::<Vec<_>>(),
        ) else {
            return false;
        };
        self.worktrees.lane.as_ref().is_some_and(|lane| {
            lane.submit(Job {
                action,
                requests,
                deadline,
                configured_root: self
                    .worktrees
                    .configured_root
                    .clone()
                    .unwrap_or(Err(WorktreeNotice::ConfigurationUnavailable)),
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
        self.observe_worktree();
        self.submit_worktree_start(plan);
        true
    }
    pub(super) fn drive_worktree(&mut self) -> bool {
        self.worktrees.changes_dirty |= self
            .worktrees
            .changes
            .as_ref()
            .is_some_and(|changes| changes.changed().unwrap_or(true));
        if self.worktrees.changes_dirty && self.worktrees.launcher.pending() {
            self.worktrees.read_at = Some(self.clock.now());
        }
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
                            self.schedule_worktree_read();
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
                    self.observe_worktree();
                }
                if self.issue_worktree(Action::Read(Box::new(plan))) {
                    self.worktrees.changes_dirty = false;
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

/// Where this Client process stands while Windows keeps that folder in use; a POSIX directory can
/// be removed under a process standing in it.
fn windows_client_directory() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    std::env::current_dir()
        .ok()?
        .into_os_string()
        .into_string()
        .ok()
}
