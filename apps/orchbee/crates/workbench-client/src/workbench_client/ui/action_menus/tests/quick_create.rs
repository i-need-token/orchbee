//! Quick entries delegate to the existing capability, identity and creation owners.
use super::*;

#[test]
fn tab_create_revalidates_painted_source_and_uses_the_existing_membership_action() {
    for stale in [false, true] {
        let (mut ui, mailbox, _, _) = tests::move_membership::ready_ui();
        let mut target = ui.view().tab_create_target().unwrap();
        if stale {
            target.source.revision = orchbee_workbench_protocol::WorkbenchRevision::new(999);
        }
        let action = ui.tab_create_action(&target);
        assert_eq!(action, (!stale).then_some(ClientAction::CreateTab));
        if let Some(action) = action {
            assert_eq!(
                ui.apply_client_action(action),
                ClientActionDisposition::DismissNavigator
            );
            let command = mailbox.try_recv().unwrap().unwrap();
            assert!(matches!(command.record(), QueuedCommand::Create(_)));
        } else {
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}

#[test]
fn tab_create_rejects_an_equal_revision_in_another_context_and_preserves_a_capturing_menu() {
    let (mut ui, mailbox, _, _) = tests::context_selection::ready(false);
    let mut target = ui.view().tab_create_target().unwrap();
    target.location = ui.state.as_ref().unwrap().cache().navigation()[1]
        .location
        .clone();
    assert_eq!(ui.tab_create_action(&target), None);
    assert!(mailbox.try_recv().unwrap().is_none());
    let target = ui.view().tab_create_target().unwrap();
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let token = ui.view().action_menu().unwrap().token;
    assert_eq!(ui.tab_create_action(&target), None);
    assert_eq!(ui.view().action_menu().unwrap().token, token);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn tab_strip_create_is_disjoint_from_tabs_breadcrumb_and_hidden_tiers() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                let (ui, _, _, _) = tests::context_selection::ready(false);
                let view = crate::client_view::ClientView::new(width);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let hits = hits.finish();
                let cells: Vec<_> = (0..width)
                    .filter(|x| {
                        matches!(
                            hits.interaction_at(*x, 0),
                            Some(fingerprint)
                                if matches!(fingerprint.target, crate::tui_hit_map::InteractionTarget::TabCreate(_))
                                    && fingerprint.spec.primary == crate::tui_hit_map::PrimaryContract::ActivateOnRelease
                        )
                    })
                    .collect();
                if width >= 60 && height >= 12 {
                    assert_eq!(cells.len(), 3);
                    let x = cells[0];
                    assert_eq!(terminal.backend().buffer()[(x + 1, 0)].symbol(), "+");
                    for x in cells {
                        assert_eq!(
                            hits.hit_target_at(x, 0),
                            Some(&crate::tui_hit_map::HitTarget::TabCreate(
                                ui.view().tab_create_target().unwrap()
                            ))
                        );
                    }
                    assert!((0..width).any(|x| matches!(
                        hits.hit_target_at(x, 0),
                        Some(crate::tui_hit_map::HitTarget::Tab(_))
                    )));
                } else {
                    assert!(cells.is_empty());
                }
            }
        }
    }
}

/// Register both native shortcuts only where there is no competing pane or chrome owner.
#[test]
fn blank_creation_regions_preserve_live_panes_and_exact_context() {
    use crate::tui_hit_map::{HitTarget, InteractionTarget, PrimaryContract};
    for empty in [false, true] {
        for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                    let mut snapshot = tests::context_selection::snapshot(false);
                    if empty {
                        for context in &mut snapshot.terminal_contexts {
                            context.tabs.clear();
                        }
                    }
                    let ui = if empty {
                        crate::workbench_client::ui::tests::quick_workspace::empty_ui(snapshot).0
                    } else {
                        tests::move_membership::ready_ui_with_snapshot(snapshot).0
                    };
                    let view = ClientView::new(width);
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &view,
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let hits = hits.finish();
                    let mut strip = 0;
                    let mut content = 0;
                    for row in 0..height {
                        for column in 0..width {
                            let Some(fingerprint) = hits.interaction_at(column, row) else {
                                continue;
                            };
                            if fingerprint.spec.primary != PrimaryContract::ActivateOnDoubleClick {
                                continue;
                            }
                            assert_eq!(
                                fingerprint.target,
                                InteractionTarget::TabCreate(
                                    ui.view().tab_create_target().unwrap()
                                )
                            );
                            assert!(!matches!(
                                hits.hit_target_at(column, row),
                                Some(HitTarget::Pane { .. })
                            ));
                            if row == 0 {
                                strip += 1;
                            } else {
                                content += 1;
                            }
                        }
                    }
                    if width >= 80 && height >= 24 {
                        assert!(strip > 0);
                        assert_eq!(content > 0, empty);
                    }
                    if !empty {
                        assert_eq!(content, 0);
                    }
                }
            }
        }
    }
}
