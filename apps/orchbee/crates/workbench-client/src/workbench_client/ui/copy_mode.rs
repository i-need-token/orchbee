//! Native keyboard-copy coordination over exact pane, viewport, query,
//! selection-export and clipboard identities.

use super::{ClientProjectionStatus, WorkbenchClientUi};
use crate::client_view::{
    CopyModeKey, CopySearchCommand, KeyCode, KeyEvent, KeyKind, Modifiers, copy_mode_key,
    copy_search_command,
};
use crate::terminal_content::{ContentSource, TerminalScreen};
use crate::terminal_copy::TerminalTextPoint;
use crate::terminal_copy_mode::{
    CopyModeView, Effect, Mode, Navigation, Refusal, SearchEdit, Surface,
};
use crate::terminal_copy_query::{
    Action as QueryAction, Admission as QueryAdmission, Failure as QueryFailure, QueryIdentity,
    Request as QueryRequest, Value as QueryValue,
};
use crate::terminal_text::line_editor::LineKey;
use crate::terminal_viewport::{TerminalViewport, ViewportNav, ViewportStep};
use crate::workbench_client::copy_episode::{EntryOrigin, PendingQuery};
use crate::workbench_client::mailbox::{AdmissionError, CommandMailbox, QueuedCommand, Tagged};
use crate::workbench_client::terminal_selection;
use crate::workbench_state::{CarrierFailure, ClientNotice, PaneRef};
use orchbee_terminal_protocol::TerminalTextRow;
use std::{num::NonZeroU64, sync::Arc};
mod pointer;

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn copy_mode_active(&self) -> bool {
        self.copy_mode.active()
    }

    pub(super) fn copy_mode_view(&self) -> Option<CopyModeView<'_>> {
        let key = self.copy_mode.key()?;
        (self.state.as_ref()?.focus_pane_ref().as_ref() == Some(key))
            .then(|| self.copy_mode.view())
            .flatten()
    }

    fn copy_entry_selection_settled(&self, key: &PaneRef) -> bool {
        self.projections
            .iter()
            .find(|projection| &projection.key == key)
            .and_then(|projection| projection.selection.as_ref())
            .is_none_or(|driver| driver.gesture_settled() && !driver.export_pending())
    }

    fn continue_copy_entry(&mut self, mailbox: &Arc<CommandMailbox>) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let Some(entry) = self.copy_mode.entry().cloned() else {
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
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        }
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| projection.key == key)
        else {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        if self.viewport_admission_blocked(&key) {
            return false;
        }

        if entry.needs_viewport_enter() {
            let Some(source) = self.admitted_copy_source(&key) else {
                self.cancel_copy_mode();
                self.copy_notice(ClientNotice::CopyModeInvalidated);
                return true;
            };
            if !entry_source_matches(entry.fence(), &source)
                || self.projections[index].viewport.is_some()
            {
                self.cancel_copy_mode();
                self.copy_notice(ClientNotice::CopyModeInvalidated);
                return true;
            }
            let Some((attachment, epoch, columns, rows)) =
                self.projections[index].value.frame().map(|frame| {
                    (
                        *self.projections[index].value.attachment(),
                        *self.projections[index].value.epoch(),
                        frame.geometry.columns,
                        frame.geometry.rows,
                    )
                })
            else {
                self.cancel_copy_mode();
                self.copy_notice(ClientNotice::CopyModeInvalidated);
                return true;
            };
            let first_read_sequence = self.projections[index].read_watermark.saturating_add(1);
            match mailbox.try_enqueue(QueuedCommand::Viewport(Tagged::new(
                key.clone(),
                ViewportStep::Enter,
            ))) {
                Ok(_) => {
                    self.projections[index].viewport = Some(TerminalViewport::entering(
                        attachment,
                        epoch,
                        columns,
                        rows,
                        0,
                        first_read_sequence,
                        self.clock.now(),
                    ));
                    self.copy_mode.viewport_entered();
                    return true;
                }
                Err(AdmissionError::Full | AdmissionError::Busy) => return false,
                Err(AdmissionError::Closed(closed)) => {
                    self.copy_query.cancel();
                    self.copy_mode.cancel_episode();
                    self.retire(closed.failure());
                    return true;
                }
                Err(_) => {
                    self.copy_query.cancel();
                    self.copy_mode.cancel_episode();
                    self.retire(CarrierFailure::Protocol);
                    return true;
                }
            }
        }

        let Some(source) = self.admitted_copy_source(&key) else {
            let waiting = self.projections[index]
                .viewport
                .as_ref()
                .is_some_and(TerminalViewport::copy_commit_pending);
            if waiting {
                return false;
            }
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        if !entry_source_matches(entry.fence(), &source) || !self.copy_entry_selection_settled(&key)
        {
            if self.copy_entry_selection_settled(&key) {
                self.cancel_copy_mode();
                self.copy_notice(ClientNotice::CopyModeInvalidated);
                return true;
            }
            return false;
        }
        let Some(surface) = surface(&source) else {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        let origin = self.copy_mode.origin();
        let cursor = if origin == Some(EntryOrigin::HistorySearch) {
            self.normalize_copy_point(&source, surface.top_left())
        } else {
            self.copy_initial_cursor(&source, surface)
        };
        let Some(cursor) = cursor else {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        if !self.copy_mode.activate(source, surface, cursor) {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        }
        if origin == Some(EntryOrigin::HistorySearch) {
            let draft = self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Invalid)
                .and_then(|mode| {
                    mode.begin_search(crate::terminal_copy::CopySearchDirection::Forward)
                });
            self.settle_copy_refusal(draft.err());
        }
        true
    }

    /// The launch failure of the current Tab's selected Panel, which Copy mode shows instead of
    /// entering.
    fn focused_launch_failure(&self) -> Option<crate::localization::LaunchFailure<'_>> {
        let view = self.view();
        view.active_position()
            .and_then(|position| view.tab(position - 1))
            .and_then(|tab| tab.active_panel().and_then(|index| tab.panel(index)))
            .and_then(|panel| view.launch_failure(panel))
    }

    /// Whether the focused Panel shows a launch failure; its Copy mode row then stays available.
    pub(super) fn focused_launch_failed(&self) -> bool {
        self.focused_launch_failure().is_some()
    }

    pub(super) fn enter_copy_mode(&mut self) -> crate::client_view::ClientActionDisposition {
        self.enter_copy_mode_from(EntryOrigin::Command)
    }

    /// `/` while the focused pane reads its retained history opens the copy search over the
    /// window being read; the key never reaches the shell, refused entry included.
    pub(super) fn search_history(&mut self) {
        self.enter_copy_mode_from(EntryOrigin::HistorySearch);
    }

    fn enter_copy_mode_from(
        &mut self,
        origin: EntryOrigin,
    ) -> crate::client_view::ClientActionDisposition {
        use crate::client_view::ClientActionDisposition;
        if self.retirement_failure.is_none()
            && !self.modal_owns_input()
            && let Some(failure) = self.focused_launch_failure()
        {
            use orchbee_workbench_protocol::SettingsLocale;
            let locale = match self.preferences.locale {
                SettingsLocale::EnUs => crate::localization::Locale::EnUs,
                SettingsLocale::ZhCn => crate::localization::Locale::ZhCn,
                SettingsLocale::System => {
                    crate::localization::select_locale(|name| std::env::var_os(name))
                }
            };
            let text = locale.terminal_launch_failure(failure);
            if let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref())
                && !self.effects.enqueue_diagnostic(key, text)
            {
                self.copy_notice(ClientNotice::CopyFailed);
            }
            return ClientActionDisposition::DismissNavigator;
        }
        if self.copy_mode.active() {
            return ClientActionDisposition::DismissNavigator;
        }
        if self.retirement_failure.is_some() || self.modal_owns_input() {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(key) = self.state.as_ref().and_then(|state| state.focus_pane_ref()) else {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        if self.state.as_ref().is_none_or(|state| {
            state.ready().is_none() || state.close_suppresses_attachment(key.pane().anchor())
        }) {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| projection.key == key)
        else {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        if self.projections[index].status != ClientProjectionStatus::FrameAvailable {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let (alternate_screen, columns, rows) = {
            let Some(frame) = self.projections[index].value.frame() else {
                self.copy_notice(ClientNotice::CopyModeUnavailable);
                return ClientActionDisposition::KeepNavigator;
            };
            if frame.geometry.columns == 0 || frame.geometry.rows == 0 || frame.rows.is_empty() {
                self.copy_notice(ClientNotice::CopyModeUnavailable);
                return ClientActionDisposition::KeepNavigator;
            }
            (
                frame.alternate_screen,
                frame.geometry.columns,
                frame.geometry.rows,
            )
        };

        if alternate_screen && self.projections[index].viewport.is_some() {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }

        let Some(source) = self.admitted_copy_source(&key) else {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        let Some(entry_surface) = surface(&source) else {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        };
        if entry_surface.screen
            != if alternate_screen {
                TerminalScreen::Alternate
            } else {
                TerminalScreen::Primary
            }
            || entry_surface.columns != columns
            || entry_surface.row_count != rows
        {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        let needs_viewport_enter = !alternate_screen && self.projections[index].viewport.is_none();
        self.projections[index].tail = None;

        self.effects.cancel_selection(&key);
        if let Some(selection) = self.projections[index].selection.as_mut()
            && let Err(failure) = selection.cancel_before_viewport()
        {
            self.settle_selection_failure(failure);
            return ClientActionDisposition::KeepNavigator;
        }
        if self
            .copy_mode
            .begin_entering(key, source, needs_viewport_enter, origin)
            .is_none()
        {
            self.copy_notice(ClientNotice::CopyModeUnavailable);
            return ClientActionDisposition::KeepNavigator;
        }
        self.copy_query.cancel();
        self.admit_selections(&mailbox);
        self.continue_copy_entry(&mailbox);
        ClientActionDisposition::DismissNavigator
    }

    /// Copy mode owns every decoded key packet. `true` therefore means
    /// captured, including releases and unknown sequences.
    pub(in crate::workbench_client) fn capture_copy_mode_key(&mut self, event: &KeyEvent) -> bool {
        if !self.copy_mode.active() {
            return false;
        }
        if event.code == KeyCode::Esc && event.kind != KeyKind::Release {
            self.leave_copy_mode();
            return true;
        }
        if self.copy_mode.surface_pending() {
            if self
                .copy_mode
                .mode()
                .and_then(|mode| mode.view().search)
                .is_some()
            {
                if matches!(
                    copy_search_command(*event),
                    Some(CopySearchCommand::Submit | CopySearchCommand::Reverse)
                ) {
                    self.copy_notice(ClientNotice::CopyModeBusy);
                } else {
                    self.capture_copy_search_key(event);
                }
                return true;
            }
            if event.kind != KeyKind::Release
                && matches!(copy_mode_key(*event), Some(CopyModeKey::Exit))
            {
                self.leave_copy_mode();
            } else if event.kind != KeyKind::Release {
                self.copy_notice(ClientNotice::CopyModeBusy);
            }
            return true;
        }
        if self
            .copy_mode
            .mode()
            .and_then(|mode| mode.view().search)
            .is_some()
        {
            self.capture_copy_search_key(event);
            return true;
        }
        if event.kind == KeyKind::Release {
            return true;
        }
        let Some(key) = copy_mode_key(*event) else {
            return true;
        };
        self.apply_copy_action(key);
        true
    }

    fn apply_copy_action(&mut self, key: CopyModeKey) {
        if key == CopyModeKey::Copy
            && !self
                .capabilities
                .supports(crate::tui_capabilities::Capability::Clipboard)
        {
            self.copy_notice(ClientNotice::Unsupported(
                crate::tui_capabilities::Capability::Clipboard,
            ));
            return;
        }
        let result = match key {
            CopyModeKey::Left => self.copy_direct_move(-1, 0),
            CopyModeKey::Right => self.copy_direct_move(1, 0),
            CopyModeKey::Up => self.copy_direct_move(0, -1),
            CopyModeKey::Down => self.copy_direct_move(0, 1),
            CopyModeKey::Motion(motion) => self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)
                .and_then(|mode| mode.request_motion(motion)),
            CopyModeKey::Page { older } => self.copy_page(older),
            CopyModeKey::Oldest => self.copy_buffer_end(false),
            CopyModeKey::Newest => self.copy_buffer_end(true),
            CopyModeKey::Search(direction) => self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)
                .and_then(|mode| mode.begin_search(direction)),
            CopyModeKey::RepeatSearch { reverse } => {
                let repeated = self
                    .copy_mode
                    .mode_mut()
                    .ok_or(Refusal::Busy)
                    .and_then(|mode| mode.repeat_search(reverse));
                // Without an earlier search there is no match to repeat; the generic invalid
                // refusal would otherwise ask the user to start a selection before copying.
                if matches!(repeated, Err(Refusal::Invalid)) {
                    self.copy_notice(ClientNotice::CopyModeNoMatch);
                    self.drive_copy_effects();
                    return;
                }
                repeated
            }
            CopyModeKey::ToggleSelection => self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)
                .and_then(Mode::toggle_selection),
            CopyModeKey::Copy => {
                let active = self.copy_host_selection_active();
                self.copy_mode
                    .mode_mut()
                    .ok_or(Refusal::Busy)
                    .and_then(|mode| mode.request_copy(active))
            }
            CopyModeKey::Exit => {
                self.leave_copy_mode();
                return;
            }
            CopyModeKey::Help => Ok(()),
        };
        self.settle_copy_refusal(result.err());
        self.drive_copy_effects();
    }

    pub(in crate::workbench_client) fn capture_copy_mode_paste(
        &mut self,
        text: Option<&str>,
    ) -> bool {
        if !self.copy_mode.active() {
            return false;
        }
        if self.copy_mode.surface_pending()
            && self
                .copy_mode
                .mode()
                .and_then(|mode| mode.view().search)
                .is_none()
        {
            self.copy_notice(ClientNotice::CopyModeBusy);
            return true;
        }
        if self
            .copy_mode
            .mode()
            .and_then(|mode| mode.view().search)
            .is_none()
        {
            return true;
        }
        let result = text.map_or(Err(Refusal::Limit), |text| {
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Invalid)?
                .edit_search(SearchEdit::Insert(text))
                .map(|_| ())
        });
        self.settle_copy_refusal(result.err());
        true
    }

    /// A person's own exit: Esc, `q` or the toolbar's close control. An episode opened by `/` in
    /// retained history returns to that history where the episode left it; any other returns to
    /// live output.
    fn leave_copy_mode(&mut self) -> bool {
        let recover = self.copy_mode.origin() != Some(EntryOrigin::HistorySearch);
        self.end_copy_mode(recover)
    }

    /// Ends the episode for any other reason, including a delivered copy, and returns the pane
    /// to live output.
    pub(in crate::workbench_client) fn cancel_copy_mode(&mut self) -> bool {
        self.end_copy_mode(true)
    }

    fn end_copy_mode(&mut self, recover: bool) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let cleanup_source = self.copy_mode.source().cloned();
        let cleanup = cleanup_source
            .as_ref()
            .and_then(|source| self.copy_selection_cleanup(&key, source));
        let copy_owned = self
            .projections
            .iter()
            .find(|projection| projection.key == key)
            .and_then(|projection| projection.selection.as_ref())
            .is_some_and(terminal_selection::Driver::copy_selection_owned);
        self.copy_query.cancel();
        self.copy_mode.cancel_episode();
        self.effects.cancel_selection(&key);
        if let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) {
            let mut missing_owned_surface = false;
            let cleanup_result = self
                .projections
                .iter_mut()
                .find(|projection| projection.key == key)
                .and_then(|projection| projection.selection.as_mut())
                .map(|driver| {
                    if copy_owned {
                        if let Some((generation, columns, rows, cell)) = cleanup {
                            driver.cancel_copy_selection_before_viewport(
                                generation, columns, rows, cell,
                            )
                        } else {
                            missing_owned_surface = true;
                            driver.abandon_copy_selection_before_viewport()
                        }
                    } else {
                        driver.cancel_before_viewport()
                    }
                });
            if let Some(Err(failure)) = cleanup_result {
                self.settle_selection_failure(failure);
            } else if missing_owned_surface {
                self.copy_notice(ClientNotice::CopyModeInvalidated);
            }
            self.admit_selections(&mailbox);
            if recover
                && let Some(viewport) = self
                    .projections
                    .iter_mut()
                    .find(|projection| projection.key == key)
                    .and_then(|projection| projection.viewport.as_mut())
            {
                let effect = viewport.nav(ViewportNav::Exit);
                self.settle_viewport_effect(&mailbox, &key, effect);
            }
        }
        true
    }

    pub(super) fn tick_copy_mode(&mut self) -> bool {
        if !self.copy_mode.active() {
            return false;
        }
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
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        let mut changed = self.continue_copy_entry(&mailbox);
        if !self.copy_mode.active() {
            return true;
        }
        if self.copy_mode.mode().is_none() {
            return changed;
        }
        changed |= self.take_copy_query_outcome();
        let Some(source) = self.admitted_copy_source(&key) else {
            // A same-incarnation repaint has no admissible coordinates until
            // its exact RowsApplied settles. Keep the episode, but expose no
            // semantic cursor or actions against the uncommitted window.
            let waiting = self.copy_surface_refresh_pending(&key);
            if waiting {
                changed |= self.copy_mode.set_surface_pending(true);
                return changed;
            }
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        changed |= self.copy_mode.set_surface_pending(false);
        let Some(next_surface) = surface(&source) else {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
            return true;
        };
        let previous = self.copy_mode.source().cloned();
        if previous.as_ref() != Some(&source) {
            let moving = self.copy_mode.mode().is_some_and(|mode| {
                mode.view().status == crate::terminal_copy_mode::CopyModeStatus::Moving
            });
            let result = if moving {
                let normalized = self
                    .copy_mode
                    .mode()
                    .map(Mode::desired)
                    .filter(|point| next_surface.contains(*point))
                    .map(|point| {
                        self.normalize_copy_point(&source, point)
                            .ok_or(Refusal::Invalid)
                    })
                    .transpose();
                normalized.and_then(|normalized| {
                    let mode = self.copy_mode.mode_mut().ok_or(Refusal::Invalid)?;
                    if let Some(point) = normalized {
                        mode.request_local_target(point)?;
                    }
                    mode.viewport_settled(next_surface)
                })
            } else {
                let rebased = self
                    .copy_mode
                    .mode()
                    .ok_or(Refusal::Invalid)
                    .and_then(|mode| mode.revision_points(next_surface));
                rebased.and_then(|(cursor, desired)| {
                    let cursor = self
                        .normalize_copy_point(&source, cursor)
                        .ok_or(Refusal::Invalid)?;
                    let desired = self
                        .normalize_copy_point(&source, desired)
                        .ok_or(Refusal::Invalid)?;
                    self.copy_mode.mode_mut().ok_or(Refusal::Invalid)?.revision(
                        next_surface,
                        cursor,
                        desired,
                    )
                })
            };
            if result.is_err() {
                self.cancel_copy_mode();
                self.copy_notice(ClientNotice::CopyModeInvalidated);
                return true;
            }
            self.copy_mode.set_source(source);
            changed = true;
        }
        changed |= self.settle_copy_selection_gesture();
        self.drive_copy_effects();
        changed
    }

    pub(super) fn copy_selection_failed(&mut self) {
        let retained = self.copy_host_selection_active();
        let failed = self.copy_mode.mode_mut().is_some_and(|mode| {
            if mode.awaiting_selection() {
                let _ = mode.selection_settled(false);
                true
            } else {
                mode.export_failed(retained)
            }
        });
        if failed {
            self.copy_notice(ClientNotice::CopyFailed);
        }
    }

    pub(super) fn copy_selection_evicted(&mut self, key: &PaneRef) {
        if self.copy_mode.key() == Some(key) {
            self.cancel_copy_mode();
            self.copy_notice(ClientNotice::CopyModeInvalidated);
        }
    }

    pub(super) fn copy_export_ready(
        &mut self,
        key: &PaneRef,
        generation: NonZeroU64,
        sequence: NonZeroU64,
        text: String,
    ) -> bool {
        let copy_export = self.copy_mode.key() == Some(key)
            && self.copy_mode.mode().is_some_and(Mode::awaiting_export);
        if !copy_export {
            return self
                .effects
                .enqueue_selection(key.clone(), generation, text);
        }
        let Some(episode) = self.copy_mode.id() else {
            return false;
        };
        if self
            .copy_mode
            .mode_mut()
            .is_none_or(|mode| mode.export_ready(sequence).is_err())
        {
            return false;
        }
        if self.effects.enqueue_copy_selection(
            key.clone(),
            generation,
            sequence,
            episode.get(),
            text,
        ) {
            true
        } else {
            self.copy_selection_failed();
            false
        }
    }

    pub(super) fn copy_effect_outcome(
        &mut self,
        key: &PaneRef,
        generation: NonZeroU64,
        export: NonZeroU64,
        episode: NonZeroU64,
        outcome: crate::workbench_client::terminal_effects::Outcome,
    ) -> bool {
        if !self
            .copy_mode
            .effect_current(key, generation, export, episode)
        {
            return false;
        }
        use crate::workbench_client::terminal_effects::Outcome;
        match outcome {
            Outcome::Delivered => {
                self.cancel_copy_mode();
            }
            Outcome::Failed | Outcome::Unknown | Outcome::Unsupported => {
                if let Some(mode) = self.copy_mode.mode_mut() {
                    mode.clipboard_not_delivered(export);
                }
                self.copy_notice(if outcome == Outcome::Unsupported {
                    ClientNotice::Unsupported(crate::tui_capabilities::Capability::Clipboard)
                } else if outcome == Outcome::Unknown {
                    ClientNotice::CopyUnknown
                } else {
                    ClientNotice::CopyFailed
                });
            }
            Outcome::Cancelled => {
                if let Some(mode) = self.copy_mode.mode_mut() {
                    mode.clipboard_not_delivered(export);
                }
                self.copy_notice(ClientNotice::CopyFailed);
            }
        }
        true
    }

    fn capture_copy_search_key(&mut self, event: &KeyEvent) {
        if event.kind == KeyKind::Release {
            return;
        }
        match copy_search_command(*event) {
            Some(CopySearchCommand::Back) => {
                if let Some(mode) = self.copy_mode.mode_mut() {
                    mode.cancel_search();
                }
                return;
            }
            Some(CopySearchCommand::Submit) => {
                let result = self
                    .copy_mode
                    .mode_mut()
                    .ok_or(Refusal::Invalid)
                    .and_then(Mode::submit_search);
                self.settle_copy_refusal(result.err());
                self.drive_copy_effects();
                return;
            }
            Some(CopySearchCommand::Reverse) => {
                if let Some(mode) = self.copy_mode.mode_mut() {
                    mode.reverse_search();
                }
                return;
            }
            None => {}
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
            _ => LineKey::for_key(*event).map(SearchEdit::Line),
        };
        if let Some(edit) = edit {
            let result = self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Invalid)
                .and_then(|mode| mode.edit_search(edit).map(|_| ()));
            self.settle_copy_refusal(result.err());
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
            let value = character.encode_utf8(&mut encoded);
            let result = self
                .copy_mode
                .mode_mut()
                .ok_or(Refusal::Invalid)
                .and_then(|mode| mode.edit_search(SearchEdit::Insert(value)).map(|_| ()));
            self.settle_copy_refusal(result.err());
        }
    }

    fn copy_page(&mut self, older: bool) -> Result<(), Refusal> {
        let key = self.copy_mode.key().cloned().ok_or(Refusal::Invalid)?;
        let mode = self.copy_mode.mode().ok_or(Refusal::Busy)?;
        let surface = mode.surface();
        let desired = mode.desired();
        let distance = u32::from(surface.row_count);
        let target = TerminalTextPoint {
            row: if older {
                desired.row.saturating_sub(distance)
            } else {
                desired
                    .row
                    .saturating_add(distance)
                    .min((surface.total_rows - 1) as u32)
            },
            col: desired.col,
        };
        if surface.contains(target) {
            let target = self.normalize_committed_copy_target(&key, surface, target)?;
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)?
                .request_local_target(target)
        } else {
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)?
                .request_page(older)
        }
    }

    fn copy_buffer_end(&mut self, newest: bool) -> Result<(), Refusal> {
        let key = self.copy_mode.key().cloned().ok_or(Refusal::Invalid)?;
        let mode = self.copy_mode.mode().ok_or(Refusal::Busy)?;
        let surface = mode.surface();
        let target = TerminalTextPoint {
            row: if newest {
                (surface.total_rows - 1) as u32
            } else {
                0
            },
            col: mode.desired().col.min(surface.columns - 1),
        };
        if surface.contains(target) {
            let target = self.normalize_committed_copy_target(&key, surface, target)?;
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)?
                .request_local_target(target)
        } else if newest {
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)?
                .request_newest()
        } else {
            self.copy_mode
                .mode_mut()
                .ok_or(Refusal::Busy)?
                .request_oldest()
        }
    }

    fn normalize_committed_copy_target(
        &self,
        key: &PaneRef,
        expected: Surface,
        target: TerminalTextPoint,
    ) -> Result<TerminalTextPoint, Refusal> {
        let source = self.admitted_copy_source(key).ok_or(Refusal::Busy)?;
        if surface(&source) != Some(expected) {
            return Err(Refusal::Busy);
        }
        self.normalize_copy_point(&source, target)
            .ok_or(Refusal::Invalid)
    }

    fn copy_direct_move(&mut self, horizontal: i8, vertical: i8) -> Result<(), Refusal> {
        let key = self.copy_mode.key().cloned().ok_or(Refusal::Invalid)?;
        let mode = self.copy_mode.mode().ok_or(Refusal::Busy)?;
        let current_surface = mode.surface();
        let mut target = mode.desired();
        if vertical < 0 {
            target.row = target.row.saturating_sub(1);
        } else if vertical > 0 {
            target.row = target
                .row
                .saturating_add(1)
                .min((current_surface.total_rows - 1) as u32);
        }
        if horizontal < 0 {
            target.col = target.col.saturating_sub(1);
        } else if horizontal > 0 {
            let width = self
                .admitted_copy_source(&key)
                .and_then(|source| self.copy_cell_width(&source, target))
                .unwrap_or(1);
            target.col = target
                .col
                .saturating_add(width)
                .min(current_surface.columns - 1);
        }
        if current_surface.contains(target) {
            target = self.normalize_committed_copy_target(&key, current_surface, target)?;
        }
        self.copy_mode
            .mode_mut()
            .ok_or(Refusal::Busy)?
            .request_local_target(target)
    }

    fn drive_copy_effects(&mut self) {
        let Some(mailbox) = self.mailbox.as_ref().map(Arc::clone) else {
            return;
        };
        for _ in 0..4 {
            let effect = self.copy_mode.mode_mut().and_then(Mode::take_effect);
            let Some(effect) = effect else { break };
            match effect {
                Effect::Navigate(navigation) => {
                    if !self.apply_copy_navigation(&mailbox, navigation) {
                        self.cancel_copy_mode();
                        self.copy_notice(ClientNotice::CopyModeInvalidated);
                    }
                    break;
                }
                Effect::Selection(effect) => {
                    let Some(key) = self.copy_mode.key().cloned() else {
                        break;
                    };
                    let source = self.admitted_copy_source(&key);
                    if !self.semantic_copy_selection(&mailbox, &key, source.as_ref(), effect) {
                        let retained = self.copy_host_selection_active();
                        if let Some(mode) = self.copy_mode.mode_mut() {
                            if mode.awaiting_export() {
                                mode.export_failed(retained);
                            } else if mode.awaiting_selection() {
                                let _ = mode.selection_settled(false);
                            }
                        }
                        self.copy_notice(ClientNotice::CopyFailed);
                    }
                    break;
                }
                Effect::Motion {
                    identity,
                    cursor,
                    motion,
                } => self.admit_copy_query(
                    QueryIdentity {
                        episode: self.copy_mode.id().unwrap(),
                        query: identity,
                    },
                    QueryAction::Motion { cursor, motion },
                    None,
                ),
                Effect::Search {
                    identity,
                    cursor,
                    query,
                    direction,
                    previous,
                } => {
                    let retained = query.clone();
                    self.admit_copy_query(
                        QueryIdentity {
                            episode: self.copy_mode.id().unwrap(),
                            query: identity,
                        },
                        QueryAction::Search {
                            cursor,
                            query,
                            case: crate::terminal_copy::CopySearchCase::Smart,
                            direction,
                            previous,
                        },
                        Some(retained),
                    );
                }
            }
        }
    }

    fn admit_copy_query(
        &mut self,
        identity: QueryIdentity,
        action: QueryAction,
        search: Option<Box<str>>,
    ) {
        let Some(key) = self.copy_mode.key().cloned() else {
            return;
        };
        let Some(source) = self.admitted_copy_source(&key) else {
            self.fail_copy_query(identity.query, ClientNotice::CopyModeQueryFailed);
            return;
        };
        let pending = PendingQuery {
            identity,
            source: source.clone(),
            search,
        };
        match self.copy_query.admit(QueryRequest {
            identity,
            source,
            action,
        }) {
            QueryAdmission::Accepted => {
                self.copy_mode.set_pending_query(pending);
            }
            QueryAdmission::Busy => {
                self.fail_copy_query(identity.query, ClientNotice::CopyModeBusy)
            }
            QueryAdmission::Unavailable => {
                self.fail_copy_query(identity.query, ClientNotice::CopyModeQueryFailed)
            }
            QueryAdmission::LimitExceeded => {
                self.fail_copy_query(identity.query, ClientNotice::CopyModeQueryFailed)
            }
        }
    }

    fn take_copy_query_outcome(&mut self) -> bool {
        let Some(outcome) = self.copy_query.take_outcome() else {
            return false;
        };
        let pending = self.copy_mode.take_pending_query();
        let Some(pending) = pending else { return false };
        let current = self
            .copy_mode
            .key()
            .cloned()
            .and_then(|key| self.admitted_copy_source(&key));
        if outcome.identity != pending.identity
            || outcome.source != pending.source
            || current.as_ref() != Some(&pending.source)
            || self.copy_mode.id() != Some(outcome.identity.episode)
        {
            self.fail_copy_query(outcome.identity.query, ClientNotice::CopyModeQueryFailed);
            return true;
        }
        match (outcome.result, pending.search) {
            (Ok(QueryValue::Motion(point)), None) => {
                if self.copy_mode.mode_mut().is_none_or(|mode| {
                    mode.query_motion_completed(outcome.identity.query, point)
                        .is_err()
                }) {
                    if let Some(mode) = self.copy_mode.mode_mut() {
                        mode.query_failed(outcome.identity.query);
                    }
                    self.copy_notice(ClientNotice::CopyModeQueryFailed);
                }
            }
            (Ok(QueryValue::Search(result)), Some(query)) => {
                match self.copy_mode.mode_mut().and_then(|mode| {
                    mode.query_search_completed(outcome.identity.query, query, result)
                        .ok()
                }) {
                    Some(true) => {}
                    Some(false) => self.copy_notice(ClientNotice::CopyModeNoMatch),
                    None => {
                        if let Some(mode) = self.copy_mode.mode_mut() {
                            mode.query_failed(outcome.identity.query);
                        }
                        self.copy_notice(ClientNotice::CopyModeQueryFailed);
                    }
                }
            }
            (Err(failure), _) => {
                self.fail_copy_query(outcome.identity.query, query_failure_notice(failure));
            }
            (Ok(_), _) => {
                self.fail_copy_query(outcome.identity.query, ClientNotice::CopyModeQueryFailed)
            }
        }
        self.drive_copy_effects();
        true
    }

    fn fail_copy_query(&mut self, query: crate::terminal_copy_mode::QueryId, notice: ClientNotice) {
        if self
            .copy_mode
            .mode_mut()
            .is_some_and(|mode| mode.query_failed(query))
        {
            self.copy_notice(notice);
        }
    }

    fn apply_copy_navigation(
        &mut self,
        mailbox: &Arc<CommandMailbox>,
        navigation: Navigation,
    ) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let Some(index) = self
            .projections
            .iter()
            .position(|projection| projection.key == key)
        else {
            return false;
        };
        let Some(surface) = self.copy_mode.mode().map(Mode::surface) else {
            return false;
        };
        let Some(nav) = copy_viewport_nav(surface, navigation) else {
            return false;
        };
        let Some(viewport) = self.projections[index].viewport.as_mut() else {
            return false;
        };
        let effect = viewport.nav(nav);
        self.settle_viewport_effect(mailbox, &key, effect);
        self.admit_viewport_steps(mailbox, &key);
        true
    }

    fn settle_copy_selection_gesture(&mut self) -> bool {
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let Some(mode) = self.copy_mode.mode() else {
            return false;
        };
        if mode.selection_effect_pending()
            || (!mode.awaiting_selection() && !mode.awaiting_export())
        {
            return false;
        }
        let Some(driver) = self
            .projections
            .iter()
            .find(|projection| projection.key == key)
            .and_then(|projection| projection.selection.as_ref())
        else {
            return false;
        };
        if !driver.gesture_settled() {
            return false;
        }
        if mode.awaiting_selection() {
            let intent_active = driver.gesture_intent_active();
            if let Some(mode) = self.copy_mode.mode_mut() {
                let _ = mode.selection_settled(intent_active);
            }
            return true;
        }
        if !driver.export_pending() {
            if let Some(mode) = self.copy_mode.mode_mut() {
                mode.export_failed(false);
            }
            self.copy_notice(ClientNotice::CopyModeNoSelection);
            return true;
        }
        false
    }

    fn copy_host_selection_active(&self) -> bool {
        let Some(key) = self.copy_mode.key() else {
            return false;
        };
        self.projections
            .iter()
            .find(|projection| &projection.key == key)
            .and_then(|projection| projection.selection.as_ref())
            .is_some_and(|driver| driver.copy_selection_owned() && driver.host_selection_active())
    }

    fn admitted_copy_source(&self, key: &PaneRef) -> Option<ContentSource> {
        let projection = self
            .projections
            .iter()
            .find(|projection| &projection.key == key)?;
        if let Some(viewport) = projection.viewport.as_ref() {
            let committed = viewport.committed_window()?;
            let source = self.view().copy_content_source(key.pane().panel())?;
            (source.rows.sequence == committed.route.sequence
                && source.rows.generation == committed.route.generation
                && source.rows.revision == committed.route.revision)
                .then_some(source)
        } else {
            self.view().copy_content_source(key.pane().panel())
        }
    }

    fn copy_surface_refresh_pending(&self, key: &PaneRef) -> bool {
        let Some(expected) = self.copy_mode.source() else {
            return false;
        };
        let Some(projection) = self
            .projections
            .iter()
            .find(|projection| &projection.key == key)
        else {
            return false;
        };
        if projection
            .viewport
            .as_ref()
            .is_none_or(|viewport| !viewport.copy_commit_pending())
        {
            return false;
        }
        self.view()
            .copy_content_source(key.pane().panel())
            .is_some_and(|current| entry_source_matches(expected, &current))
    }

    fn copy_initial_cursor(
        &self,
        source: &ContentSource,
        surface: Surface,
    ) -> Option<TerminalTextPoint> {
        let application = self
            .projections
            .iter()
            .find(|projection| projection.key == source.pane)
            .and_then(|projection| projection.value.frame())
            .filter(|frame| {
                frame.cursor.visible
                    && frame.route.host_epoch == source.rows.host_epoch
                    && frame.route.generation == source.rows.generation
                    && frame.route.revision == source.rows.revision
                    && frame.geometry.columns == source.rows.columns
                    && TerminalScreen::from_frame(frame) == source.screen
            })
            .and_then(|frame| {
                let row = frame
                    .scrollback_rows
                    .checked_add(u64::from(frame.cursor.row))?;
                let point = TerminalTextPoint {
                    row: u32::try_from(row).ok()?,
                    col: frame.cursor.column,
                };
                surface.contains(point).then_some(point)
            });
        self.normalize_copy_point(source, application.unwrap_or_else(|| surface.bottom_left()))
    }

    pub(super) fn copy_selection_cleanup(
        &self,
        key: &PaneRef,
        expected: &ContentSource,
    ) -> Option<(
        NonZeroU64,
        u16,
        u16,
        crate::workbench_client::input::PointerSelectionCell,
    )> {
        let current = self.admitted_copy_source(key)?;
        if &current != expected || current.rows.columns == 0 || current.rows.row_count == 0 {
            return None;
        }
        let absolute = TerminalTextPoint {
            row: u32::try_from(current.rows.start_row).ok()?,
            col: 0,
        };
        let normalized = self.normalize_copy_point(&current, absolute)?;
        let row = u64::from(normalized.row).checked_sub(current.rows.start_row)?;
        Some((
            current.rows.generation,
            current.rows.columns,
            current.rows.row_count,
            crate::workbench_client::input::PointerSelectionCell {
                column: normalized.col,
                row: u16::try_from(row).ok()?,
            },
        ))
    }

    fn copy_rows<'a>(&'a self, source: &ContentSource) -> Option<&'a [TerminalTextRow]> {
        let projection = self
            .projections
            .iter()
            .find(|projection| projection.key == source.pane)?;
        if source.rows.history {
            let window = projection.viewport.as_ref()?.committed_window()?;
            (window.route.sequence == source.rows.sequence).then_some(window.rows.as_slice())
        } else {
            let frame = projection.value.frame()?;
            (frame.route.sequence == source.rows.sequence).then_some(frame.rows.as_slice())
        }
    }

    fn normalize_copy_point(
        &self,
        source: &ContentSource,
        mut point: TerminalTextPoint,
    ) -> Option<TerminalTextPoint> {
        let local = u64::from(point.row).checked_sub(source.rows.start_row)?;
        if local >= u64::from(source.rows.row_count) || point.col >= source.rows.columns {
            return None;
        }
        let local = u16::try_from(local).ok()?;
        if let Some(row) = self
            .copy_rows(source)?
            .iter()
            .find(|row| row.index == local)
            && row
                .cells
                .get(usize::from(point.col))
                .is_some_and(|cell| cell.width == 2)
        {
            point.col = point.col.saturating_sub(1);
        }
        Some(point)
    }

    fn copy_cell_width(&self, source: &ContentSource, point: TerminalTextPoint) -> Option<u16> {
        let local = u16::try_from(u64::from(point.row).checked_sub(source.rows.start_row)?).ok()?;
        let row = self
            .copy_rows(source)?
            .iter()
            .find(|row| row.index == local)?;
        let cell = row.cells.get(usize::from(point.col))?;
        Some(if cell.width == 1 { 2 } else { 1 })
    }

    fn settle_copy_refusal(&mut self, refusal: Option<Refusal>) {
        let Some(refusal) = refusal else { return };
        self.copy_notice(match refusal {
            Refusal::Busy => ClientNotice::CopyModeBusy,
            Refusal::Limit => ClientNotice::CopyModeSearchLimit,
            Refusal::Invalid => ClientNotice::CopyModeNoSelection,
            Refusal::IdentityExhausted => ClientNotice::CopyModeUnavailable,
        });
    }

    fn copy_notice(&mut self, notice: ClientNotice) {
        if let Some(state) = self.state.as_mut() {
            state.set_notice(notice);
        }
    }
}

fn entry_source_matches(fence: &ContentSource, current: &ContentSource) -> bool {
    fence.pane == current.pane
        && fence.member == current.member
        && fence.host_terminal == current.host_terminal
        && fence.screen == current.screen
        && fence.rows.attachment == current.rows.attachment
        && fence.rows.host_epoch == current.rows.host_epoch
        && fence.rows.generation == current.rows.generation
        && fence.rows.columns == current.rows.columns
        && fence.rows.row_count == current.rows.row_count
}

fn surface(source: &ContentSource) -> Option<Surface> {
    Surface::checked(
        source.screen,
        source.rows.generation,
        source.rows.revision,
        source.rows.columns,
        source.rows.total_rows,
        source.rows.start_row,
        source.rows.row_count,
    )
}

fn copy_viewport_nav(surface: Surface, navigation: Navigation) -> Option<ViewportNav> {
    Some(match navigation {
        Navigation::Oldest => ViewportNav::Oldest,
        Navigation::Newest => ViewportNav::Newest,
        Navigation::Reveal(target) => {
            let target = u64::from(target.row);
            let end = surface.start_row + u64::from(surface.row_count);
            if target < surface.start_row {
                ViewportNav::Lines(i64::try_from(surface.start_row - target).unwrap_or(i64::MAX))
            } else if target >= end && target + 1 == surface.total_rows {
                // Ordinary history delta exits at the live tail. Copy mode
                // instead installs Tail(0) and remains Viewing.
                ViewportNav::Newest
            } else if target >= end {
                ViewportNav::Lines(-i64::try_from(target - end + 1).unwrap_or(i64::MAX))
            } else {
                return None;
            }
        }
    })
}

const fn query_failure_notice(_failure: QueryFailure) -> ClientNotice {
    // Query limits describe the bounded retained snapshot, not the user's
    // <=512-scalar draft. Only synchronous draft admission uses SearchLimit.
    ClientNotice::CopyModeQueryFailed
}

#[cfg(test)]
mod tests;
