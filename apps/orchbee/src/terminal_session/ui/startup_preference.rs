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
mod tests {
    use super::*;
    use crate::{
        client_view::{ClientAction, ClientView},
        localization::Locale,
        terminal_session::TerminalSessionView,
        tui_pointer::HitMapBuilder,
    };
    use ratatui::{Terminal, backend::TestBackend};

    fn draw(
        ui: &NativeUi,
        shell: &ClientView,
        width: u16,
        height: u16,
    ) -> crate::terminal_session::NativeDrawReceipt {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut receipt = Default::default();
        terminal
            .draw(|frame| {
                receipt = crate::tui_render::render_terminal_shell(
                    frame,
                    shell,
                    Locale::EnUs,
                    Some(TerminalSessionView::Native(ui.view())),
                    &crate::tui_theme::Theme::rgb_test_dark(),
                    &mut HitMapBuilder::new(),
                )
            })
            .unwrap();
        receipt
    }

    #[test]
    fn startup_preference_render_proof_rejects_covered_small_and_stale_selection() {
        let (mut ui, _, _, _) = super::super::move_membership_tests::ready_ui();
        super::super::viewport_tests::install_projection(&mut ui, Some((9, false, 24)), 0);
        let mut shell = ClientView::new(120);
        let receipt = draw(&ui, &shell, 120, 40);
        let proof = receipt
            .position()
            .expect("uncovered focused live pane")
            .clone();
        assert!(ui.view().accepts_position_draw(&proof));
        assert!(draw(&ui, &shell, 2, 2).position().is_none());
        use crate::client_view::{
            KeyCode, KeyEvent, Modifiers, PrefixInput, SessionActionDisposition,
        };
        for (character, modifiers) in [('b', Modifiers::CTRL), ('?', Modifiers::NONE)] {
            let transition = shell.decide(PrefixInput::Key(KeyEvent::press(
                KeyCode::Char(character),
                modifiers,
            )));
            shell.commit(transition, SessionActionDisposition::KeepNavigator);
        }
        assert!(draw(&ui, &shell, 120, 40).position().is_none());
        ui.apply_client_action(ClientAction::SelectNextTab);
        assert!(!ui.view().accepts_position_draw(&proof));
        assert!(
            draw(&ui, &ClientView::new(120), 120, 40)
                .position()
                .is_none()
        );
    }
}
