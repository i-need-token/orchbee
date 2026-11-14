//! Find in terminal coordination: opens the bar on the focused pane, owns its keyboard capture,
//! admits its reads on the copy query lane and reveals the active match in the pane's history.
//!
//! The bar is a copy episode with the Find origin, so it shares copy mode's capture slot, query
//! lane and pane fence. It never changes the pane's selection, and closing it never recovers to
//! live: the pane stays where Find left it.

use super::{ClientProjectionStatus, WorkbenchClientUi};
use crate::client_view::{ClientActionDisposition, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::terminal_content::{ContentSource, TerminalScreen};
use crate::terminal_copy::{TerminalTextPoint, TerminalTextRange};
use crate::terminal_copy_mode::SearchEdit;
use crate::terminal_copy_query::{
    Action as QueryAction, Admission as QueryAdmission, QueryIdentity, Request as QueryRequest,
    Value as QueryValue, lane::JOB_BUDGET,
};
use crate::terminal_events::keys::Binding;
use crate::terminal_find::{FindBar, FindControl, FindOutcome, FindRead, FindStep, FindToken};
use crate::terminal_text::line_editor::LineKey;
use crate::terminal_viewport::ViewportNav;
use crate::workbench_client::copy_episode::{EntryOrigin, PendingQuery};
use crate::workbench_state::{ClientNotice, PaneRef};
use std::{sync::Arc, time::Duration};

/// While a read runs, the bar checks for its outcome at this pace.
const OUTCOME_POLL: Duration = Duration::from_millis(16);

const CLOSE: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);
const NEXT: [Binding; 2] = [
    Binding::new(KeyCode::Enter, Modifiers::NONE),
    Binding::new(KeyCode::Down, Modifiers::NONE),
];
const PREVIOUS: [Binding; 2] = [
    Binding::new(KeyCode::Enter, Modifiers::NONE.with_shift()),
    Binding::new(KeyCode::Up, Modifiers::NONE),
];
const MATCH_CASE: Binding = Binding::new(KeyCode::Char('c'), Modifiers::NONE.with_alt());
const PAGE_OLDER: Binding = Binding::new(KeyCode::PageUp, Modifiers::NONE);
const PAGE_NEWER: Binding = Binding::new(KeyCode::PageDown, Modifiers::NONE);
/// A Panel narrower or shorter than this refuses the bar.
const MINIMUM_COLUMNS: u16 = 20;
const MINIMUM_ROWS: u16 = 2;

impl WorkbenchClientUi {
    /// Whether the Find bar owns the copy capture slot.
    pub(in crate::workbench_client) fn find_bar_active(&self) -> bool {
        self.copy_mode.origin() == Some(EntryOrigin::Find)
    }

    /// Opens the Find bar on the focused pane. A bar already open keeps its query; an open copy
    /// mode keeps the pane. Nothing is sent to the pane.
    pub(super) fn find_in_terminal(&mut self) -> ClientActionDisposition {
        if self.copy_mode.active() {
            return ClientActionDisposition::DismissNavigator;
        }
        if self.modal_owns_input() {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(key) = self
            .state
            .as_ref()
            .and_then(|state| state.focus_pane_ref())
            .filter(|key| self.find_pane_ready(key))
        else {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        let fits = self
            .state
            .as_ref()
            .and_then(|state| state.pane_by_panel(key.pane().panel()))
            .is_some_and(|record| {
                let geometry = record.observed_geometry();
                geometry.columns() >= MINIMUM_COLUMNS && geometry.rows() >= MINIMUM_ROWS
            });
        if !fits {
            self.find_notice(ClientNotice::PanelTooSmall);
            return ClientActionDisposition::KeepNavigator;
        }
        let now = self.clock.now();
        if self.copy_mode.begin_find(key, now).is_none() {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        self.copy_query.cancel();
        self.drive_find();
        ClientActionDisposition::DismissNavigator
    }

    /// Whether the bar can open on `key` as far as its terminal goes: the Client is ready, the
    /// pane is not closing and its frame has cells to read. The shortcut and the menu rows share
    /// this rule, so a disabled row means the shortcut refuses too.
    pub(super) fn find_pane_ready(&self, key: &PaneRef) -> bool {
        self.retirement_failure.is_none()
            && self.state.as_ref().is_some_and(|state| {
                state.ready().is_some() && !state.close_suppresses_attachment(key.pane().anchor())
            })
            && self
                .projections
                .iter()
                .find(|projection| &projection.key == key)
                .filter(|projection| projection.status == ClientProjectionStatus::FrameAvailable)
                .and_then(|projection| projection.value.frame())
                .is_some_and(|frame| {
                    frame.geometry.columns > 0 && frame.geometry.rows > 0 && !frame.rows.is_empty()
                })
    }

    /// Closes the bar, keeping the view where Find left it.
    pub(super) fn close_find(&mut self) -> bool {
        if !self.find_bar_active() {
            return false;
        }
        self.copy_query.cancel();
        self.copy_mode.cancel_episode();
        true
    }

    /// The bar owns every decoded key; nothing typed reaches the pane.
    pub(super) fn capture_find_key(&mut self, event: &KeyEvent) {
        if event.kind == KeyKind::Release {
            return;
        }
        let event = *event;
        if CLOSE.matches(event) {
            self.close_find();
            return;
        }
        if NEXT.iter().any(|binding| binding.matches(event)) {
            self.step_find(true);
            return;
        }
        if PREVIOUS.iter().any(|binding| binding.matches(event)) {
            self.step_find(false);
            return;
        }
        if MATCH_CASE.matches(event) {
            self.toggle_find_case();
            return;
        }
        if PAGE_OLDER.matches(event) || PAGE_NEWER.matches(event) {
            self.page_find(PAGE_OLDER.matches(event));
            return;
        }
        let (code, modifiers) = event.normalized();
        let edit = match (code, modifiers) {
            (KeyCode::Backspace, Modifiers::NONE) | (KeyCode::Char('h'), Modifiers::CTRL) => {
                Some(SearchEdit::Backspace)
            }
            (KeyCode::Delete, Modifiers::NONE) => Some(SearchEdit::Delete),
            (KeyCode::Left, Modifiers::NONE) => Some(SearchEdit::Left),
            (KeyCode::Right, Modifiers::NONE) => Some(SearchEdit::Right),
            (KeyCode::Home, Modifiers::NONE) => Some(SearchEdit::Home),
            (KeyCode::End, Modifiers::NONE) => Some(SearchEdit::End),
            _ => LineKey::for_key(event).map(SearchEdit::Line),
        };
        if let Some(edit) = edit {
            self.edit_find(edit);
            return;
        }
        let character = if !event.mods.ctrl && !event.mods.alt && !event.mods.super_ {
            match event.code {
                KeyCode::Char(character) => Some(character),
                KeyCode::Space => Some(' '),
                _ => None,
            }
        } else {
            None
        };
        if let Some(character) = character {
            let mut encoded = [0; 4];
            self.edit_find(SearchEdit::Insert(character.encode_utf8(&mut encoded)));
        }
    }

    /// The token the open bar's painted targets carry.
    fn find_token(&self) -> Option<FindToken> {
        if !self.find_bar_active() {
            return None;
        }
        self.copy_mode.id().map(FindToken)
    }

    /// A released control of the bar acts as its key does; a control of an earlier bar does
    /// nothing.
    pub(super) fn click_find_control(&mut self, token: FindToken, control: FindControl) {
        if self.find_token() != Some(token) {
            return;
        }
        match control {
            FindControl::MatchCase => self.toggle_find_case(),
            FindControl::Previous => self.step_find(false),
            FindControl::Next => self.step_find(true),
            FindControl::Close => {
                self.close_find();
            }
        }
    }

    /// One wheel notch over the searched Panel scrolls its history three rows; the bar and its
    /// query stay.
    pub(super) fn scroll_find(&mut self, token: FindToken, up: bool) -> bool {
        if self.find_token() != Some(token) {
            return false;
        }
        self.scroll_find_rows(Some(3), up)
    }

    /// A press on the searched Panel's content closes the bar.
    pub(super) fn close_find_at(&mut self, token: FindToken) -> bool {
        self.find_token() == Some(token) && self.close_find()
    }

    fn toggle_find_case(&mut self) {
        let now = self.clock.now();
        if let Some(bar) = self.copy_mode.find_mut() {
            bar.toggle_match_case(now);
        }
        self.supersede_find_read();
        self.drive_find();
    }

    /// A paste edits the query atomically; a refused paste keeps the draft.
    pub(super) fn capture_find_paste(&mut self, text: Option<&str>) {
        match text {
            Some(text) => self.edit_find(SearchEdit::Insert(text)),
            None => self.find_notice(ClientNotice::CopyModeSearchLimit),
        }
    }

    fn edit_find(&mut self, edit: SearchEdit<'_>) {
        let now = self.clock.now();
        let Some(bar) = self.copy_mode.find_mut() else {
            return;
        };
        match bar.edit(edit, now) {
            Ok(true) => self.supersede_find_read(),
            Ok(false) => {}
            Err(_) => self.find_notice(ClientNotice::CopyModeSearchLimit),
        }
    }

    /// A newer query or case makes the read in flight obsolete.
    fn supersede_find_read(&mut self) {
        if self.copy_mode.take_pending_query().is_some() {
            self.copy_query.cancel();
        }
    }

    fn step_find(&mut self, forward: bool) {
        let now = self.clock.now();
        if self.find_source().is_none() {
            return;
        }
        let Some(bar) = self.copy_mode.find_mut() else {
            return;
        };
        match bar.step(forward, now) {
            Ok(FindStep::Moved) => self.reveal_find_match(),
            Ok(FindStep::Read(read)) => self.admit_find_read(read),
            Ok(FindStep::None) => {}
            Err(_) => self.find_notice(ClientNotice::FindFailed),
        }
    }

    /// PageUp and PageDown read the history under the bar; the query stays.
    fn page_find(&mut self, older: bool) {
        self.scroll_find_rows(None, older);
    }

    /// Moves the history under the bar by `rows`, or by one page when `None`; a live pane enters
    /// its history only when moving older. Reports whether the pane moved.
    fn scroll_find_rows(&mut self, rows: Option<u32>, older: bool) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return false;
        };
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return false;
        };
        if let Some(viewport) = self.projections[index].viewport.as_mut() {
            if viewport.recovery_pending() {
                return false;
            }
            let rows = rows.map_or_else(|| i64::from(viewport.geometry().1), i64::from);
            let nav = if older {
                ViewportNav::Lines(rows)
            } else {
                ViewportNav::Lines(-rows)
            };
            let effect = viewport.nav(nav);
            self.settle_viewport_effect(&mailbox, &key, effect);
            self.admit_viewport_steps(&mailbox, &key);
            true
        } else if older {
            self.enter_pane_history_rows(&key, rows);
            true
        } else {
            false
        }
    }

    /// The pane's current admitted content, which the bar reads and highlights against.
    fn find_source(&self) -> Option<ContentSource> {
        let key = self.copy_mode.key()?;
        self.admitted_copy_source(key)
    }

    /// Advances the bar on the shared fair tick: closes it with its pane, takes the read outcome,
    /// drops matches the terminal no longer fits, and starts a search whose delay has passed.
    pub(super) fn tick_find(&mut self) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        if self.retirement_failure.is_some()
            || self
                .state
                .as_ref()
                .and_then(|state| state.focus_pane_ref())
                .as_ref()
                != Some(&key)
        {
            self.close_find();
            return true;
        }
        let mut changed = self.take_find_outcome();
        if let (Some(current), Some(previous)) = (self.find_source(), self.copy_mode.source())
            && !super::copy_mode::entry_source_matches(previous, &current)
        {
            let now = self.clock.now();
            self.supersede_find_read();
            if let Some(bar) = self.copy_mode.find_mut() {
                bar.invalidate(now);
            }
            self.copy_mode.set_source(current);
            changed = true;
        }
        let pending = self.copy_mode.find().and_then(FindBar::wake);
        changed |= self.drive_find();
        changed || pending != self.copy_mode.find().and_then(FindBar::wake)
    }

    /// Starts the delayed search once its time has come and the pane has admitted content.
    fn drive_find(&mut self) -> bool {
        if self.find_source().is_none() {
            return false;
        }
        let now = self.clock.now();
        let Some(bar) = self.copy_mode.find_mut() else {
            return false;
        };
        match bar.take_due(now) {
            Ok(Some(read)) => {
                self.admit_find_read(read);
                true
            }
            Ok(None) => false,
            Err(_) => {
                self.find_notice(ClientNotice::FindFailed);
                true
            }
        }
    }

    /// When the bar next needs a tick: its delayed search, or the outcome of a running read.
    pub(super) fn find_wake(&self) -> Option<std::time::Instant> {
        let bar = self.copy_mode.find()?;
        bar.wake().or_else(|| {
            bar.running()
                .and_then(|_| self.clock.now().checked_add(OUTCOME_POLL))
        })
    }

    fn admit_find_read(&mut self, read: FindRead) {
        let (Some(episode), Some(source)) = (self.copy_mode.id(), self.find_source()) else {
            self.fail_find_read(read.query);
            return;
        };
        let cursor = read.previous.map_or_else(
            || newest_point(&source),
            |previous: TerminalTextRange| Some(previous.start),
        );
        let Some(cursor) = cursor else {
            self.fail_find_read(read.query);
            return;
        };
        let identity = QueryIdentity {
            episode,
            query: read.query,
        };
        // The bar expects one read at a time; any older one is obsolete.
        self.copy_query.cancel();
        self.copy_mode.take_pending_query();
        let pending = PendingQuery {
            identity,
            source: source.clone(),
            search: Some(read.text.clone()),
        };
        match self.copy_query.admit(QueryRequest {
            identity,
            source: source.clone(),
            action: QueryAction::Search {
                cursor,
                query: read.text,
                case: read.case,
                direction: read.direction,
                previous: read.previous,
            },
        }) {
            QueryAdmission::Accepted => {
                self.copy_mode.set_pending_query(pending);
                self.copy_mode.set_source(source);
            }
            QueryAdmission::Busy | QueryAdmission::Unavailable | QueryAdmission::LimitExceeded => {
                self.fail_find_read(read.query);
            }
        }
    }

    fn fail_find_read(&mut self, query: crate::terminal_copy_mode::QueryId) {
        let now = self.clock.now();
        let failed = self
            .copy_mode
            .find_mut()
            .is_some_and(|bar| bar.fail(query, now, JOB_BUDGET));
        if failed {
            self.find_outcome_notice();
        }
    }

    fn take_find_outcome(&mut self) -> bool {
        let Some(outcome) = self.copy_query.take_outcome() else {
            return false;
        };
        let Some(pending) = self.copy_mode.take_pending_query() else {
            return false;
        };
        if outcome.identity != pending.identity
            || self.copy_mode.id() != Some(pending.identity.episode)
        {
            self.fail_find_read(pending.identity.query);
            return true;
        }
        match outcome.result {
            Ok(QueryValue::Search(result)) => {
                let settled = self
                    .copy_mode
                    .find_mut()
                    .is_some_and(|bar| bar.settle(pending.identity.query, &result));
                if settled {
                    self.reveal_find_match();
                }
            }
            Ok(QueryValue::Motion(_)) | Err(_) => self.fail_find_read(pending.identity.query),
        }
        true
    }

    fn find_outcome_notice(&mut self) {
        match self.copy_mode.find().map(FindBar::outcome) {
            Some(FindOutcome::Stopped) => self.find_notice(ClientNotice::FindStopped),
            Some(FindOutcome::Failed) => self.find_notice(ClientNotice::FindFailed),
            _ => {}
        }
    }

    /// Scrolls the active match into view, centred, only when it is not already visible. A live
    /// primary pane enters its history at that window; an alternate screen shows all it retains.
    fn reveal_find_match(&mut self) {
        let Some(row) = self
            .copy_mode
            .find()
            .and_then(FindBar::matches)
            .and_then(crate::terminal_find::FindMatches::active_range)
            .map(|range| u64::from(range.start.row))
        else {
            return;
        };
        let Some(source) = self.find_source() else {
            return;
        };
        let rows = &source.rows;
        if source.screen == TerminalScreen::Alternate
            || (row >= rows.start_row && row < rows.start_row + u64::from(rows.row_count))
        {
            return;
        }
        let height = u64::from(rows.row_count);
        let last_start = rows.total_rows.saturating_sub(height);
        let target = row.saturating_sub(height / 2).min(last_start);
        let Some(key) = self.copy_mode.key().cloned() else {
            return;
        };
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        let Some(index) = self.projections.iter().position(|p| p.key == key) else {
            return;
        };
        if let Some(viewport) = self.projections[index].viewport.as_mut() {
            let nav = if target >= last_start {
                ViewportNav::Newest
            } else {
                let delta = i128::from(rows.start_row) - i128::from(target);
                ViewportNav::Lines(i64::try_from(delta).unwrap_or(i64::MAX))
            };
            let effect = viewport.nav(nav);
            self.settle_viewport_effect(&mailbox, &key, effect);
            self.admit_viewport_steps(&mailbox, &key);
        } else {
            let before = rows
                .total_rows
                .saturating_sub(target.saturating_add(height));
            if let Ok(before) = u32::try_from(before)
                && before > 0
            {
                self.enter_pane_history_rows(&key, Some(before));
            }
        }
    }

    fn find_notice(&mut self, notice: ClientNotice) {
        if let Some(state) = self.state.as_mut() {
            state.set_notice(notice);
        }
    }
}

/// The last cell of the retained buffer, where the first search starts so it finds the newest
/// match.
fn newest_point(source: &ContentSource) -> Option<TerminalTextPoint> {
    let row = source.rows.total_rows.checked_sub(1)?;
    Some(TerminalTextPoint {
        row: u32::try_from(row).ok()?,
        col: source.rows.columns.checked_sub(1)?,
    })
}
