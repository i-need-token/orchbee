//! Capture and restore of reading positions through the ordinary viewport gates.
//! The reading-position owner keeps the choices; this composition reads projections and
//! drives viewport entry, history reads, and the loss notice.

use super::history_positions::Choice;
use super::{ClientNotice, ClientProjectionStatus, WorkbenchClientUi};
use crate::terminal_content::TerminalScreen;

impl WorkbenchClientUi {
    /// Capture committed reading choices before navigation and after a successful draw.
    pub(super) fn remember_history_positions(&mut self) {
        let Some(state) = self.state.as_ref().filter(|state| state.ready().is_some()) else {
            return;
        };
        let view = self.view();
        let mut updates = Vec::new();
        let mut live = Vec::new();
        for projection in &self.projections {
            if !state.pane_projected(&projection.key) {
                continue;
            }
            if self.copy_mode.key() == Some(&projection.key) {
                live.push(projection.key.clone());
                continue;
            }
            if let Some(viewport) = &projection.viewport
                && projection.tail.is_none()
            {
                if viewport.returning_to_live() {
                    live.push(projection.key.clone());
                } else if !viewport.resuming()
                    && viewport.committed_window().is_some()
                    && let Some(source) = view.painted_content_source(projection.key.pane().panel())
                {
                    updates.push((
                        source,
                        viewport
                            .reading_position()
                            .map_or(Choice::Unavailable, Choice::History),
                    ));
                }
            } else if self.history_positions.examined(&projection.key)
                && let Some(source) = view.painted_content_source(projection.key.pane().panel())
            {
                updates.push((source, Choice::Live));
            }
        }
        self.history_positions.bank.prune(state);
        for key in live {
            self.history_positions.bank.live(&key);
        }
        for (source, choice) in updates {
            self.history_positions.bank.set(source, choice);
        }
    }

    /// Restore on a fresh readable incarnation, through ordinary Enter/read/RowsApplied gates.
    pub(super) fn drive_history_positions(&mut self) -> bool {
        let Some(state) = self.state.as_ref().filter(|state| state.ready().is_some()) else {
            return false;
        };
        self.history_positions.bank.prune(state);
        self.history_positions
            .retain_examined(|key| self.projections.iter().any(|p| &p.key == key));
        if self.copy_mode.active() {
            return false;
        }
        let keys = self
            .projections
            .iter()
            .filter(|p| {
                p.status == ClientProjectionStatus::FrameAvailable && state.pane_projected(&p.key)
            })
            .map(|p| p.key.clone())
            .collect::<Vec<_>>();
        let mut changed = false;
        for key in keys {
            if self.history_positions.examined(&key) || self.viewport_admission_blocked(&key) {
                continue;
            }
            let Some(source) = self.view().painted_content_source(key.pane().panel()) else {
                continue;
            };
            let saved = self.history_positions.bank.get(&source.member).cloned();
            let Some(saved) = saved else {
                self.history_positions.mark_examined(key);
                continue;
            };
            if matches!(saved.choice, Choice::Live) {
                self.history_positions.mark_examined(key);
                continue;
            }
            let Some(index) = self.projections.iter().position(|p| p.key == key) else {
                continue;
            };
            if self.projections[index].tail.is_some() {
                if let Some(mailbox) = self.mailbox.as_ref().map(std::sync::Arc::clone)
                    && let Some(viewport) = self.projections[index].viewport.as_mut()
                {
                    let effect = viewport.nav(super::ViewportNav::Exit);
                    self.settle_viewport_effect(&mailbox, &key, effect);
                    changed = true;
                }
                continue;
            }
            if self.projections[index].viewport.is_some() {
                // A new explicit user choice takes precedence over a saved choice.
                self.history_positions.mark_examined(key);
                continue;
            }
            // Live content may be locally cropped; the core uses physical frame geometry.
            let Some(frame) = self.projections[index].value.frame() else {
                continue;
            };
            let position = match saved.choice {
                Choice::History(position)
                    if saved.source.host_terminal == source.host_terminal
                        && saved.source.rows.host_epoch == source.rows.host_epoch
                        && position.generation == source.rows.generation
                        && source.screen == TerminalScreen::Primary
                        && position.columns == source.rows.columns
                        && position.viewport_rows == frame.geometry.rows
                        && source.rows.total_rows >= position.minimum_total =>
                {
                    Some(position)
                }
                Choice::Live | Choice::History(_) | Choice::Unavailable => None,
            };
            let Some(position) = position else {
                self.history_positions.bank.set(source, Choice::Live);
                self.history_positions.mark_examined(key);
                if let Some(state) = self.state.as_mut() {
                    state.set_notice(ClientNotice::HistoryPositionLost);
                }
                changed = true;
                continue;
            };
            self.enter_pane_history_rows(&key, Some(1));
            if let Some(viewport) = self
                .projections
                .iter_mut()
                .find(|p| p.key == key)
                .and_then(|p| p.viewport.as_mut())
            {
                viewport.resume_position(position.clone());
                self.history_positions
                    .bank
                    .set(source, Choice::History(position));
                self.history_positions.mark_examined(key);
                changed = true;
            }
        }
        changed
    }
}
