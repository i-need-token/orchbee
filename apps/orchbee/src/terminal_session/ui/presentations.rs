//! Presentation navigation uses the existing switch admission and cleanup transaction.
use super::*;
use orchbee_workbench_protocol::TerminalPresentationId;

impl NativeUi {
    pub(super) fn cycle_presentation(&mut self, forward: bool) -> SessionActionDisposition {
        if self.modal_owns_input() {
            return SessionActionDisposition::KeepNavigator;
        }
        let Some((index, panel, presentation)) = self
            .state
            .as_ref()
            .and_then(|state| state.next_presentation(forward))
        else {
            return SessionActionDisposition::KeepNavigator;
        };
        if self.select_presentation(index, &panel, &presentation) {
            SessionActionDisposition::DismissNavigator
        } else {
            SessionActionDisposition::KeepNavigator
        }
    }

    pub(in crate::terminal_session) fn select_presentation(
        &mut self,
        index: usize,
        panel: &WorkbenchPanelId,
        presentation: &TerminalPresentationId,
    ) -> bool {
        let Some(deadlines) = OperationDeadlines::from_sample(self.clock.now()) else {
            return false;
        };
        let (Some(mailbox), Some(state)) =
            (self.mailbox.as_ref().map(Arc::clone), self.state.as_mut())
        else {
            return false;
        };
        let result = mailbox.try_enqueue_switch_for(
            state,
            |state, generation| {
                if !state.invariant_holds() {
                    return Err(ActionBuildError::Protocol);
                }
                if !matches!(
                    state.plan_selection_index(index),
                    SelectionPlan::Switch { .. } | SelectionPlan::NoOp
                ) {
                    return Err(ActionBuildError::Busy);
                }
                let target = state
                    .target_for_presentation(index, panel, presentation, generation, deadlines)
                    .ok_or(ActionBuildError::Busy)?;
                if !state.begin_switch(target.clone()) {
                    return Err(ActionBuildError::Busy);
                }
                Ok(SwitchMarker::new(target))
            },
            |state, result, facts| {
                if result.is_ok() {
                    state.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                }
                finish_action(state, result);
            },
        );
        match result {
            Ok(_) => {
                self.pending_draws.clear();
                self.ordinary_poll_serial = None;
                true
            }
            Err(error) => {
                self.retire_for_admission(&error);
                false
            }
        }
    }
}

impl<'a> NativeSessionView<'a> {
    pub(crate) fn selected_presentation(
        &self,
        panel: &'a super::super::NativePanel,
    ) -> &'a super::super::NativePresentation {
        self.state.as_ref().map_or_else(
            || panel.primary(),
            |state| state.selected_presentation(panel),
        )
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use orchbee_workbench_protocol::{
        PanelNode, Presentation, PresentationBinding, TerminalId, WorkerId,
    };
    #[test]
    fn prefix_choice_queues_exact_child_and_does_not_display_primary_report() {
        let mut source = super::super::membership_tests::snapshot(2, 1);
        let PanelNode::Panel {
            child_presentations,
            ..
        } = &mut source.terminal_contexts[0].tabs[0].root
        else {
            unreachable!()
        };
        let child = TerminalPresentationId::parse(&format!("{:032x}", 9000)).unwrap();
        child_presentations.push(Presentation {
            presentation_id: child.clone(),
            label: "child shell".into(),
            custom_label: None,
            binding: Some(PresentationBinding {
                worker_id: WorkerId::parse(&format!("{:032x}", 9001)).unwrap(),
                terminal_id: TerminalId::parse(&format!("{:032x}", 9002)).unwrap(),
            }),
        });
        let (mut ui, mailbox, _, _) =
            super::super::move_membership_tests::ready_ui_with_snapshot(source);
        assert_eq!(
            ui.apply_client_action(ClientAction::NextPresentation),
            SessionActionDisposition::DismissNavigator
        );
        let command = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(
            command.record(),
            crate::terminal_session::mailbox::QueuedCommand::Switch(_)
        ));
        let view = ui.view();
        let panel = view.tab(0).unwrap().panel(0).unwrap();
        assert_eq!(view.selected_presentation(panel).presentation_id(), &child);
        assert!(matches!(
            view.panel_report(panel),
            crate::terminal_session::PanelReportView::Pending
        ));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &crate::client_view::ClientView::new(120),
                    crate::localization::Locale::EnUs,
                    Some(crate::terminal_session::TerminalSessionView::Native(
                        ui.view(),
                    )),
                    &crate::tui_theme::Theme::rgb_test_dark(),
                    &mut crate::tui_pointer::HitMapBuilder::new(),
                );
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            text.contains("child shell") && text.contains("[2/2]"),
            "{text}"
        );
    }
}
