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
use crate::terminal_find::{FindBar, FindOutcome, FindRead, FindStep};
use crate::terminal_text::line_editor::LineKey;
use crate::terminal_viewport::ViewportNav;
use crate::workbench_client::copy_episode::{EntryOrigin, PendingQuery};
use crate::workbench_state::ClientNotice;
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
        if self.retirement_failure.is_some() || self.modal_owns_input() {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        if self.state.as_ref().is_none_or(|state| {
            state.ready().is_none() || state.close_suppresses_attachment(key.pane().anchor())
        }) {
            self.find_notice(ClientNotice::FindUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let readable = self
            .projections
            .iter()
            .find(|projection| projection.key == key)
            .filter(|projection| projection.status == ClientProjectionStatus::FrameAvailable)
            .and_then(|projection| projection.value.frame())
            .is_some_and(|frame| {
                frame.geometry.columns > 0 && frame.geometry.rows > 0 && !frame.rows.is_empty()
            });
        if !readable {
            self.find_notice(ClientNotice::FindUnavailable);
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
        let now = self.clock.now();
        if NEXT.iter().any(|binding| binding.matches(event)) {
            self.step_find(true);
            return;
        }
        if PREVIOUS.iter().any(|binding| binding.matches(event)) {
            self.step_find(false);
            return;
        }
        if MATCH_CASE.matches(event) {
            if let Some(bar) = self.copy_mode.find_mut() {
                bar.toggle_match_case(now);
            }
            self.supersede_find_read();
            self.drive_find();
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
            if viewport.recovery_pending() {
                return;
            }
            let rows = i64::from(viewport.geometry().1);
            let nav = if older {
                ViewportNav::Lines(rows)
            } else {
                ViewportNav::Lines(-rows)
            };
            let effect = viewport.nav(nav);
            self.settle_viewport_effect(&mailbox, &key, effect);
            self.admit_viewport_steps(&mailbox, &key);
        } else if older {
            self.enter_pane_history_rows(&key, None);
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
            self.find_notice(ClientNotice::FindClosed);
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
