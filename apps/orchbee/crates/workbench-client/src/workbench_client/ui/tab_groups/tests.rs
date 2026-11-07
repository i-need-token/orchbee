use super::super::tests::workspace_setup::{place, ready_snapshot};
use super::super::*;
use super::*;
use crate::action_menu::{MenuDraw, MenuToken};
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::workbench_state::workspace_setup::SetupEdit;

use crate::workbench_client::mailbox::CommandLease;
use orchbee_workbench_protocol::{
    Snapshot, TabGroup, WorkbenchAction, WorkbenchRevision, configure_tabs,
};
mod creation;

mod close;
mod interaction;
mod presentation;

fn grouped() -> Snapshot {
    let mut snapshot = super::super::tests::membership::snapshot(3, 1);
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1900)).unwrap(),
        label: "源组".into(),
        color: TabGroupColor::Green,
    };
    for tab in &mut snapshot.terminal_contexts[0].tabs[1..] {
        tab.group_id = Some(group.group_id.clone());
    }
    snapshot.terminal_contexts[0].groups.push(group);
    snapshot
}

fn present_action_menu(ui: &mut WorkbenchClientUi) -> MenuToken {
    let (token, selected, visible_rows, result_actions, hidden) = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        (
            menu.token,
            menu.selected,
            menu.actions.len(),
            menu.result_actions.to_vec(),
            menu.candidate_hidden,
        )
    };
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        selected,
        candidate_visible: !hidden,
        token,
        first: 0,
        visible_rows,
        result_actions,
    })));
    token
}

fn choose(ui: &mut WorkbenchClientUi, action: MenuAction) {
    if !ui.view().action_menu().unwrap().actions.contains(&action) {
        let parent = match action {
            MenuAction::CreateGroup | MenuAction::JoinGroup(_) => MenuAction::ManageGroups,
            _ => panic!("action must be reachable through its owning menu: {action:?}"),
        };
        choose(ui, parent);
    }
    // Each key renews the searchable menu's draw fence before the next key can be captured.
    present_action_menu(ui);
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let row = menu
        .actions
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap();
    // Navigation revokes any previous painted window before confirmation.
    ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
        crate::client_view::KeyCode::Home,
        crate::client_view::Modifiers::NONE,
    ));
    present_action_menu(ui);
    for _ in 0..row {
        ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Down,
            crate::client_view::Modifiers::NONE,
        ));
        present_action_menu(ui);
    }
    ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
        crate::client_view::KeyCode::Enter,
        crate::client_view::Modifiers::NONE,
    ));
    let picked = ui.take_action_menu_action(false);
    if action.submenu() {
        assert!(picked.is_none());
        assert!(ui.action_menu_active());
    } else {
        ui.apply_client_action(picked.expect("painted menu handoff"));
    }
}

fn accept_configuration(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    snapshot: &mut Snapshot,
    created: Option<WorkbenchTabGroupId>,
) -> TabConfiguration {
    let old = ui.state.as_ref().unwrap().desired().cloned();
    let baseline = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    let lease: CommandLease = mailbox.try_recv().unwrap().expect("configuration command");
    let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
        panic!("setup lane")
    };
    let WorkbenchAction::TabsConfigure { operation, .. } = &marker.0.command.action else {
        panic!("typed metadata operation")
    };
    let operation = operation.clone();
    assert_eq!(
        mailbox.mark_setup_in_flight(&marker.0).unwrap(),
        MutationResult::Applied
    );
    let candidate =
        configure_tabs(&snapshot.terminal_contexts[0], &operation, created.as_ref()).unwrap();
    let unchanged = candidate.is_none();
    if let Some(context) = candidate {
        snapshot.terminal_contexts[0] = context;
        snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    }
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &baseline,
        "queueing is not a canonical write"
    );
    let wait = place(
        mailbox,
        clock,
        &lease,
        WorkbenchSetupCommit::TabsConfigured {
            revision: snapshot.revision,
            group_id: created,
            unchanged,
        },
        snapshot.clone(),
    );
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("refresh settlement")
    };
    assert_eq!(settlement.decision(), &RefreshDecision::Continue);
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(
        ui.state.as_ref().unwrap().desired(),
        old.as_ref(),
        "same viewer, incarnation and binding"
    );
    assert_eq!(ui.view().phase(), DisplayPhase::Ready);
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        snapshot
    );
    operation
}

#[test]
fn selected_tab_configuration_menu_marks_exact_set_and_preserves_active_terminal() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let ids: Vec<_> = snapshot.terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect();
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    // Ctrl+click marks a Tab beside the current one.
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::SelectTab,
        tab_id: Some(ids[2].clone()),
    });
    assert_eq!(
        ui.view().selected_tab_ids(),
        vec![ids[0].clone(), ids[2].clone()]
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());

    assert!(ui.open_action_menu(MenuTarget::Tab(ids[2].clone()), None, false));
    choose(&mut ui, MenuAction::PinTab);
    let operation = accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert_eq!(
        operation,
        TabConfiguration::SetTabsPinned {
            tab_ids: vec![ids[0].clone(), ids[2].clone()].try_into().unwrap(),
            pinned: true
        }
    );
    assert_eq!(
        snapshot.terminal_contexts[0]
            .tabs
            .iter()
            .map(|tab| (&tab.tab_id, tab.pinned))
            .collect::<Vec<_>>(),
        vec![(&ids[0], true), (&ids[2], true), (&ids[1], false)]
    );

    assert!(ui.open_action_menu(MenuTarget::TabGroupChoices(ids[2].clone()), None, false));
    choose(&mut ui, MenuAction::CreateGroup);
    let token = ui.view().setup_draft().unwrap().token();
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).unwrap();
        let mut hits = crate::tui_hit_map::HitMapBuilder::new();
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell(
                    frame,
                    &crate::client_view::ClientView::new(120),
                    locale,
                    Some(ui.view()),
                    &crate::tui_theme::Theme::rgb_test_dark(),
                    &mut hits,
                );
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(
            text.replace(' ', "")
                .contains(&locale.setup_group_name_hint().replace(' ', ""))
        );
        assert!(!text.contains("1–512"));
    }
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(token, SetupEdit::Clear);
    ui.submit_setup_draft(token);
    let group = WorkbenchTabGroupId::parse(&format!("{:032x}", 1902)).unwrap();
    let operation = accept_configuration(
        &mut ui,
        &mailbox,
        &clock,
        &mut snapshot,
        Some(group.clone()),
    );
    assert!(
        matches!(operation, TabConfiguration::CreateGroupForTabs { tab_ids, label, .. } if tab_ids.as_slice() == [ids[0].clone(),ids[2].clone()] && label.as_str().is_empty())
    );
    assert!(
        snapshot.terminal_contexts[0].tabs[..2]
            .iter()
            .all(|tab| !tab.pinned && tab.group_id.as_ref() == Some(&group))
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    // A background target outside the selected set only configures that exact Tab.
    assert!(ui.open_action_menu(MenuTarget::Tab(ids[1].clone()), None, false));
    choose(&mut ui, MenuAction::PinTab);
    let operation = accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert!(
        matches!(operation, TabConfiguration::SetTabsPinned { tab_ids, .. } if tab_ids.as_slice() == [ids[1].clone()])
    );
    assert_eq!(
        ui.view().selected_tab_ids(),
        vec![ids[0].clone(), ids[2].clone()]
    );
}

#[test]
fn tab_group_toggle_all_clear_and_stale_menu_proof_stay_client_local() {
    let (mut ui, mailbox, _, snapshot) = ready_snapshot(grouped());
    let ids: Vec<_> = snapshot.terminal_contexts[0]
        .tabs
        .iter()
        .map(|tab| tab.tab_id.clone())
        .collect();
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    for (action, expected) in [
        (MenuAction::SelectTab, vec![ids[0].clone(), ids[2].clone()]),
        (MenuAction::SelectTab, vec![ids[0].clone()]),
        (MenuAction::SelectAllTabs, ids.clone()),
        (MenuAction::ClearTabSelection, vec![ids[0].clone()]),
    ] {
        if action == MenuAction::SelectTab {
            // Ctrl+click toggles one Tab beside the current Tab.
            ui.apply_client_action(ClientAction::TabMenu {
                action,
                tab_id: Some(ids[2].clone()),
            });
        } else {
            // The global menu's Tab section marks or clears every Tab.
            assert!(ui.open_action_menu(MenuTarget::Global, None, false));
            choose(&mut ui, action);
        }
        assert_eq!(ui.view().selected_tab_ids(), expected);
        assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
        assert!(mailbox.try_recv().unwrap().is_none());
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
    }
    assert!(ui.open_action_menu(MenuTarget::Tab(ids[0].clone()), None, false));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let source = menu.source.clone();
    let token = menu.token;
    let result_actions = menu.result_actions.to_vec();
    let row = menu
        .actions
        .iter()
        .position(|a| *a == MenuAction::PinTab)
        .unwrap();
    ui.after_action_menu_draw(Some(ModalPresentation::Presented(MenuDraw {
        selected: ui.view().action_menu().unwrap().selected,
        candidate_visible: !ui.view().action_menu().unwrap().candidate_hidden,
        token,
        first: 0,
        visible_rows: ui.view().action_menu().unwrap().actions.len(),
        result_actions,
    })));
    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&ids[2], crate::workbench_state::TabSelectionChange::Toggle)
        .unwrap();
    ui.activate_action_menu_row(&source, token, row);
    assert_eq!(
        ui.take_action_menu_action(false),
        None,
        "a painted selection cannot expand after confirmation"
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn tab_group_move_menu_uses_local_disclosure_and_keeps_the_current_terminal() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    // The global menu's movement rows act on the current Tab, here left of the folded group.
    let target = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
    let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
    let original = ui.state.as_ref().unwrap().desired().cloned();
    ui.apply_client_action(ClientAction::ToggleTabGroup(group));
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    choose(&mut ui, MenuAction::MoveTabNext);
    let operation = accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert!(matches!(
        operation,
        TabConfiguration::MoveTab {
            adjacent_group_collapsed: true,
            ..
        }
    ));
    assert_eq!(snapshot.terminal_contexts[0].tabs[2].tab_id, target);
    assert_eq!(snapshot.terminal_contexts[0].tabs[2].group_id, None);
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
}

#[test]
fn tab_group_menus_submit_canonical_operations_and_preserve_selected_terminal() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let original = snapshot.clone();
    let tab = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
    assert!(ui.open_action_menu(MenuTarget::Tab(tab.clone()), None, false));
    choose(&mut ui, MenuAction::ManageGroups);
    choose(&mut ui, MenuAction::CreateGroup);
    let token = ui.view().setup_draft().unwrap().token();
    ui.submit_setup_draft(token);
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "draft also requires a complete paint"
    );
    ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
    ui.edit_setup_draft(token, SetupEdit::Clear);
    ui.edit_setup_draft(token, SetupEdit::Paste("构建 🦀"));
    ui.submit_setup_draft(token);
    accept_configuration(
        &mut ui,
        &mailbox,
        &clock,
        &mut snapshot,
        Some(WorkbenchTabGroupId::parse(&format!("{:032x}", 1901)).unwrap()),
    );
    for action in [
        MenuAction::GroupColor(TabGroupColor::Orange),
        MenuAction::JoinGroup(0),
        MenuAction::RemoveFromGroup,
        MenuAction::PinTab,
        MenuAction::UnpinTab,
        MenuAction::JoinGroup(0),
        MenuAction::Ungroup,
    ] {
        ui.apply_client_action(ClientAction::TabMenu {
            action,
            tab_id: None,
        });
        accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
        for tab in &snapshot.terminal_contexts[0].tabs {
            let old = original.terminal_contexts[0]
                .tabs
                .iter()
                .find(|old| old.tab_id == tab.tab_id)
                .unwrap();
            assert_eq!(tab.root, old.root);
        }
    }
    assert!(snapshot.terminal_contexts[0].groups.is_empty());
    assert_eq!(
        ui.state
            .as_ref()
            .unwrap()
            .desired()
            .unwrap()
            .anchor()
            .tab_id(),
        &tab
    );
}

#[test]
fn collapse_is_client_local_does_not_select_or_enqueue_and_survives_shared_changes() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
    let selected = ui.state.as_ref().unwrap().desired().cloned();
    ui.apply_client_action(ClientAction::ToggleTabGroup(group.clone()));
    assert!(ui.view().group_collapsed(&group));
    assert_eq!(ui.state.as_ref().unwrap().desired(), selected.as_ref());
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.apply_client_action(ClientAction::TabMenu {
        action: MenuAction::PinTab,
        tab_id: None,
    });
    accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert!(ui.view().group_collapsed(&group));
    assert_eq!(
        ui.state.as_ref().unwrap().cache().groups(),
        snapshot.terminal_contexts[0].groups
    );
    let (other, _, _, _) = ready_snapshot(snapshot);
    assert!(
        !other.view().group_collapsed(&group),
        "another Client owns independent disclosure"
    );
}

#[test]
fn background_tab_group_menu_does_not_select_its_target() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let original = ui.state.as_ref().unwrap().desired().cloned();
    let target = snapshot.terminal_contexts[0].tabs[1].tab_id.clone();
    assert!(ui.open_action_menu(MenuTarget::Tab(target.clone()), None, false));
    choose(&mut ui, MenuAction::PinTab);
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
    accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert_eq!(snapshot.terminal_contexts[0].tabs[0].tab_id, target);
    assert!(snapshot.terminal_contexts[0].tabs[0].pinned);
    assert_eq!(ui.state.as_ref().unwrap().desired(), original.as_ref());
}

#[test]
fn tab_groups_composed_screen_keeps_headers_members_and_localized_menu_hits() {
    use crate::{
        localization::Locale,
        tui_hit_map::{HitMapBuilder, HitTarget},
        tui_theme::{ColorLevel, GlyphSet, Theme},
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 24), (80, 20), (40, 10), (20, 5)] {
            for (theme_index, theme) in [
                Theme::terminal_native(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
                Theme::orchbee_dark().quantize(ColorLevel::Ansi256),
                Theme::orchbee_light().quantize(ColorLevel::Ansi16),
                Theme::terminal_native()
                    .quantize(ColorLevel::Monochrome)
                    .with_glyphs(GlyphSet::ASCII),
            ]
            .into_iter()
            .enumerate()
            {
                let mut snapshot = grouped();
                let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
                snapshot.terminal_contexts[0].tabs[0].pinned = true;
                let (mut ui, _, _, _) = ready_snapshot(snapshot);
                ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                for collapsed in [false, true] {
                    if collapsed {
                        ui.apply_client_action(ClientAction::ToggleTabGroup(group.clone()));
                    }
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let hits = hits.finish();
                    let header = (0..height).flat_map(|y| (0..width).map(move |x| (x,y))).find(|(x,y)| matches!(hits.hit_target_at(*x,*y), Some(HitTarget::TabGroup { group: id, .. }) if id == &group));
                    if width >= 80 {
                        assert!(
                            header.is_some(),
                            "whole screen must expose the group header"
                        );
                        let buffer = terminal.backend().buffer();
                        assert!(rendered_text(buffer).contains("源组"));
                        // Pinned Tabs end at a vertical rule instead of a text annotation.
                        let (header_x, header_y) = header.unwrap();
                        let first_tab = (0..header_x)
                            .find(|x| {
                                matches!(hits.hit_target_at(*x, header_y), Some(HitTarget::Tab(_)))
                            })
                            .unwrap();
                        assert!(
                            (first_tab..header_x).any(|x| {
                                buffer[(x, header_y)].symbol() == theme.glyphs.vertical_rule
                            }),
                            "a vertical rule separates pinned Tabs"
                        );
                    }
                    if collapsed {
                        let member_hit = (0..height).any(|y| {
                            (0..width).any(|x| {
                                matches!(hits.hit_target_at(x, y), Some(HitTarget::Tab(id)) if id != ui.state.as_ref().unwrap().cache().tabs()[0].tab_id())
                            })
                        });
                        assert!(!member_hit);
                    }
                    if theme_index == 0 && [(120, 40), (80, 20)].contains(&(width, height)) {
                        crate::tui_render::test_capture::capture_screen(
                            &mut terminal,
                            &format!(
                                "{locale:?}-{width}x{height}-{}",
                                if collapsed { "collapsed" } else { "expanded" }
                            ),
                            locale,
                            &theme,
                        );
                    }
                }
                if theme_index == 0 && [(120, 40), (80, 20)].contains(&(width, height)) {
                    let tab_id = ui.state.as_ref().unwrap().cache().tabs()[1]
                        .tab_id()
                        .clone();
                    assert!(ui.open_action_menu(
                        MenuTarget::TabGroups {
                            tab_id,
                            colors: false
                        },
                        None,
                        false
                    ));
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    crate::tui_render::test_capture::capture_screen(
                        &mut terminal,
                        &format!("{locale:?}-{width}x{height}-menu"),
                        locale,
                        &theme,
                    );
                    let hits = hits.finish();
                    assert!((0..height).any(|y| (0..width).any(|x| matches!(
                        hits.hit_target_at(x, y),
                        Some(HitTarget::ActionMenuRow { .. })
                    ))));
                }
            }
        }
    }
}

fn rendered_text(buffer: &ratatui::buffer::Buffer) -> String {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
        }
        text.push('\n');
    }
    text
}

#[test]
fn tab_group_strip_reserves_quick_add_paint_and_hits_after_expanded_or_collapsed_tokens() {
    use crate::{
        localization::Locale,
        tui_hit_map::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [80, 120] {
            for collapsed in [false, true] {
                for long_label in [false, true] {
                    let mut snapshot = grouped();
                    snapshot.terminal_contexts[0].tabs[0].pinned = true;
                    if long_label {
                        snapshot.terminal_contexts[0].groups[0].label =
                            "很长的标签组 🦀 ".repeat(12);
                    }
                    let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
                    let (mut ui, _, _, _) = ready_snapshot(snapshot);
                    ui.observe_geometry(Geometry::new(width, 24, 8, 16).unwrap());
                    if collapsed {
                        ui.apply_client_action(ClientAction::ToggleTabGroup(group));
                    }
                    let expected = ui.view().tab_create_target().unwrap();
                    let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                &crate::client_view::ClientView::new(width),
                                locale,
                                Some(ui.view()),
                                &Theme::terminal_native(),
                                &mut hits,
                            );
                        })
                        .unwrap();
                    let hits = hits.finish();
                    let mut create_cells = Vec::new();
                    let mut token_cells = Vec::new();
                    let mut saw_header = false;
                    for y in 0..24 {
                        for x in 0..width {
                            match hits.hit_target_at(x, y) {
                                Some(HitTarget::TabCreate(target)) => {
                                    assert_eq!(target, &expected);
                                    if hits.interaction_at(x, y).is_some_and(|fingerprint| {
                                        fingerprint.spec.primary
                                            == crate::tui_hit_map::PrimaryContract::ActivateOnRelease
                                    }) {
                                        create_cells.push((x, y));
                                    }
                                }
                                Some(HitTarget::TabGroup { .. }) => {
                                    saw_header = true;
                                    token_cells.push((x, y));
                                }
                                Some(HitTarget::Tab(_)) => token_cells.push((x, y)),
                                _ => {}
                            }
                        }
                    }
                    assert!(saw_header, "group header must coexist with quick add");
                    assert_eq!(create_cells.len(), 3);
                    let (create_x, create_y) = create_cells[0];
                    assert_eq!(
                        create_cells,
                        [
                            (create_x, create_y),
                            (create_x + 1, create_y),
                            (create_x + 2, create_y)
                        ]
                    );
                    assert!(
                        token_cells
                            .iter()
                            .all(|(x, y)| *y == create_y && *x < create_x)
                    );
                    let paint: String = create_cells
                        .iter()
                        .map(|point| terminal.backend().buffer()[*point].symbol())
                        .collect();
                    assert_eq!(
                        paint, "[+]",
                        "group clipping must not overwrite the existing control"
                    );
                    assert_eq!(
                        ui.tab_create_action(&expected),
                        Some(ClientAction::CreateTab)
                    );
                }
            }
        }
    }
}

#[test]
fn selected_tab_group_composed_screen_keeps_set_markers_and_scrollable_menu_routes() {
    use crate::{
        localization::Locale,
        tui_hit_map::{HitMapBuilder, HitTarget},
        tui_theme::Theme,
    };
    use ratatui::{Terminal, backend::TestBackend};
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (80, 20), (80, 10), (40, 10)] {
            let mut snapshot = grouped();
            snapshot.terminal_contexts[0].tabs[0].pinned = true;
            let tab_id = snapshot.terminal_contexts[0].tabs[0].tab_id.clone();
            let group = snapshot.terminal_contexts[0].groups[0].group_id.clone();
            let (mut ui, mailbox, _, _) = ready_snapshot(snapshot);
            ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
            ui.apply_client_action(ClientAction::TabMenu {
                action: MenuAction::SelectAllTabs,
                tab_id: None,
            });
            if width <= 80 {
                ui.apply_client_action(ClientAction::ToggleTabGroup(group));
            }
            assert!(ui.open_action_menu(MenuTarget::Tab(tab_id), None, false));
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let theme = Theme::terminal_native();
            let mut receipt = None;
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    receipt = Some(crate::tui_render::render_terminal_shell(
                        frame,
                        &crate::client_view::ClientView::new(width),
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut hits,
                    ));
                })
                .unwrap();
            let text = rendered_text(terminal.backend().buffer());
            if width >= 80 && height >= 20 {
                // Checked Tabs, and a collapsed group holding them, share the selection surface.
                let buffer = terminal.backend().buffer();
                let selection = theme.color(crate::tui_theme::Role::SurfaceSelection);
                let strip: Vec<&str> = (0..width).map(|x| buffer[(x, 0)].symbol()).collect();
                let labels: &[&str] = if width <= 80 {
                    &["Tab 0", "源"]
                } else {
                    &["Tab 0", "Tab 1", "Tab 2"]
                };
                for label in labels {
                    let cells: Vec<String> = label.chars().map(String::from).collect();
                    let x = strip
                        .windows(cells.len())
                        .position(|window| window.iter().zip(&cells).all(|(a, b)| *a == b))
                        .unwrap_or_else(|| panic!("{label}: {text}"));
                    assert_eq!(
                        buffer[(u16::try_from(x).unwrap(), 0)].bg,
                        selection,
                        "{label} uses the selection surface: {text}"
                    );
                }
                assert!(text.contains(&locale.menu_tab_action(
                    &MenuAction::PinTab,
                    TabMenuScope::Selected(NonZeroUsize::new(3).unwrap()),
                )));
                crate::tui_render::test_capture::capture_screen(
                    &mut terminal,
                    &format!("{locale:?}-{width}x{height}-selected-menu"),
                    locale,
                    &theme,
                );
            }
            // A low-height admitted menu still scrolls even when the strip is hidden.
            ui.after_close_draw(receipt.take().unwrap());
            ui.cancel_action_menu();
            assert!(ui.open_action_menu(MenuTarget::Global, None, false));
            // Keyboard traversal scrolls to the global movement rows; paint, then pointer invokes
            // one.
            let target = ui
                .view()
                .action_menu()
                .unwrap()
                .actions
                .iter()
                .position(|action| *action == MenuAction::MoveTabLast)
                .unwrap();
            for _ in 0..target {
                present_action_menu(&mut ui);
                ui.capture_action_menu_key(crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::Down,
                    crate::client_view::Modifiers::NONE,
                ));
            }
            let view = ui.view();
            let menu = view.action_menu().unwrap();
            let source = menu.source.clone();
            let token = menu.token;
            let selected = menu.selected;
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    receipt = Some(crate::tui_render::render_terminal_shell(
                        frame,
                        &crate::client_view::ClientView::new(width),
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut hits,
                    ));
                })
                .unwrap();
            let hits = hits.finish();
            let (painted_width, painted_height) = (width, height);
            assert!(
                (0..painted_height).any(|y| (0..painted_width).any(|x| matches!(
                    hits.hit_target_at(x, y),
                    Some(HitTarget::ActionMenuRow { token: painted, row })
                        if *painted == token && *row == target
                ))),
                "the movement row must be painted: {locale:?} {width}x{height}, selected {}, row {target}, source {source:?}\n{}",
                selected,
                rendered_text(terminal.backend().buffer()),
            );
            ui.after_close_draw(receipt.take().unwrap());
            ui.activate_action_menu_row(&source, token, target);
            let action = ui.take_action_menu_action(false).unwrap();
            assert!(matches!(
                action.intent(),
                ClientAction::TabMenu {
                    action: MenuAction::MoveTabLast,
                    ..
                }
            ));
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "selection and menu traversal do not mutate canonical state"
            );
        }
    }
}
