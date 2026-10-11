//! Painted position proofs feed only the owning Client's startup preference.
use super::{NativeDisplayPhase, NativeSessionView, NativeUi, PaneRef, SessionNotice};
use crate::terminal_session::startup_preference::{Draw, Position};

impl NativeSessionView<'_> {
    pub(crate) fn position_draw<'a>(
        &self,
        panes: impl Iterator<Item = &'a PaneRef>,
    ) -> Option<Draw> {
        if self.retirement.is_some()
            || self.close_confirmation().is_some()
            || self.workspace_close_confirmation().is_some()
            || self.panel_close_confirmation().is_some()
            || self.rename_draft().is_some()
            || self.setup_draft().is_some()
            || self.worktree().is_some()
            || self.agent_board().is_some()
            || self.popup().is_some()
        {
            return None;
        }
        let location = self
            .context_at(self.active_context_position()?.checked_sub(1)?)?
            .location()
            .clone();
        if self.phase() == NativeDisplayPhase::Empty {
            return Some(Draw {
                position: Position {
                    location,
                    member: None,
                },
                focus: None,
            });
        }
        if self.phase() != NativeDisplayPhase::Ready {
            return None;
        }
        let member = self.focused_member()?;
        if self.pane_projection_status(&member.panel)
            != super::NativeProjectionStatus::FrameAvailable
            || self.pane_projection(&member.panel)?.frame().is_none()
        {
            return None;
        }
        let focus = self.pane_ref_for_panel(&member.panel)?;
        if !panes.into_iter().any(|pane| pane == &focus) {
            return None;
        }
        Some(Draw {
            position: Position {
                location,
                member: Some(member),
            },
            focus: Some(focus),
        })
    }
    pub(in crate::terminal_session) fn accepts_position_draw(&self, draw: &Draw) -> bool {
        self.position_draw(draw.focus.iter()).as_ref() == Some(draw)
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn report_position_notice(&mut self, changed: bool) {
        if let Some(state) = self.state.as_mut() {
            state.set_notice(if changed {
                SessionNotice::PositionChanged
            } else {
                SessionNotice::PositionUnavailable
            });
        }
    }
}

#[cfg(test)]
mod tests;
