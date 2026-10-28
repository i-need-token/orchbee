//! Mouse intents enter the same episode, selection and query owners as keyboard intents.
use super::{CopyModeKey, Mode, Refusal, TerminalTextPoint, WorkbenchClientUi};
use crate::terminal_copy_mode::{CopyControl, CopyModeCursor, CopyModeToken, CopySurfaceToken};

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn click_copy_control(
        &mut self,
        token: CopyModeToken,
        control: CopyControl,
    ) {
        if self.copy_mode_view().and_then(|view| view.pointer) != Some(token) {
            return;
        }
        if control == CopyControl::Exit {
            self.cancel_copy_mode();
            return;
        }
        if control == CopyControl::CancelSearch {
            if let Some(mode) = self.copy_mode.mode_mut() {
                mode.cancel_search();
            }
            return;
        }
        if self.copy_mode.surface_pending() {
            return;
        }
        match control {
            CopyControl::ReverseSearch => {
                if let Some(mode) = self.copy_mode.mode_mut() {
                    mode.reverse_search();
                }
            }
            CopyControl::SubmitSearch => {
                let result = self
                    .copy_mode
                    .mode_mut()
                    .ok_or(Refusal::Busy)
                    .and_then(Mode::submit_search);
                self.settle_copy_refusal(result.err());
                self.drive_copy_effects();
            }
            control
                if self
                    .copy_mode
                    .mode()
                    .is_some_and(|mode| mode.view().search.is_none()) =>
            {
                let action = match control {
                    CopyControl::Copy => CopyModeKey::Copy,
                    CopyControl::ToggleSelection => CopyModeKey::ToggleSelection,
                    CopyControl::Search => {
                        CopyModeKey::Search(crate::terminal_copy::CopySearchDirection::Forward)
                    }
                    CopyControl::RepeatSearch { reverse } => CopyModeKey::RepeatSearch { reverse },
                    _ => return,
                };
                self.apply_copy_action(action);
            }
            _ => {}
        }
    }

    pub(in crate::workbench_client) fn copy_pointer_cell(
        &mut self,
        token: CopySurfaceToken,
        cell: CopyModeCursor,
        start: bool,
    ) -> bool {
        if self.copy_mode.surface_pending()
            || self
                .copy_mode_view()
                .and_then(|view| view.pointer)
                .and_then(CopyModeToken::surface)
                != Some(token)
        {
            return false;
        }
        let Some(key) = self.copy_mode.key().cloned() else {
            return false;
        };
        let result = token
            .surface
            .point(cell)
            .ok_or(Refusal::Invalid)
            .and_then(|point| self.normalize_committed_copy_target(&key, token.surface, point))
            .and_then(|point| {
                let mode = self.copy_mode.mode_mut().ok_or(Refusal::Busy)?;
                if start {
                    mode.start_pointer_selection(point)
                } else {
                    mode.request_local_target(point)
                }
            });
        let admitted = result.is_ok();
        self.settle_copy_refusal(result.err());
        self.drive_copy_effects();
        admitted
    }

    pub(in crate::workbench_client) fn scroll_copy_mode(
        &mut self,
        token: CopyModeToken,
        up: bool,
        lines: usize,
    ) -> Option<CopyModeToken> {
        if self.copy_mode_view()?.pointer != Some(token)
            || token.surface().is_none()
            || self.copy_mode.surface_pending()
        {
            return None;
        }
        let mode = self.copy_mode.mode()?;
        let source = mode.surface();
        let desired = mode.desired();
        let distance = u32::try_from(lines).unwrap_or(u32::MAX);
        let target = TerminalTextPoint {
            row: if up {
                desired.row.saturating_sub(distance)
            } else {
                desired
                    .row
                    .saturating_add(distance)
                    .min((source.total_rows - 1) as u32)
            },
            col: desired.col,
        };
        let key = self.copy_mode.key()?.clone();
        let target = if source.contains(target) {
            self.normalize_committed_copy_target(&key, source, target)
                .ok()?
        } else {
            target
        };
        let result = self.copy_mode.mode_mut()?.request_local_target(target);
        self.settle_copy_refusal(result.err());
        self.drive_copy_effects();
        self.copy_mode_view()?.pointer
    }
}
