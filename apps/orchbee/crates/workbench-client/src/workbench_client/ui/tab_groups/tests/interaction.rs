use super::*;
use crate::client_view::{KeyCode, KeyEvent, Modifiers};

#[test]
fn tab_group_and_creation_menus_have_distinct_scopes_and_submenu_back_requires_new_paint() {
    let (mut ui, mailbox, _, snapshot) = ready_snapshot(grouped());
    let target = snapshot.terminal_contexts[0].tabs[1].tab_id.clone();
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    assert!(ui.open_action_menu(MenuTarget::Tab(target.clone()), None, false));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(
        menu.actions,
        &[
            MenuAction::RenameTab,
            MenuAction::PinTab,
            MenuAction::ManageGroups,
            MenuAction::RemoveFromGroup,
            MenuAction::CloseSelectedTabs,
            MenuAction::CloseOtherTabs,
        ]
    );
    assert_eq!(menu.unavailable.len(), menu.actions.len());
    // Sections: name and pin, group membership, closing.
    for row in 1..menu.actions.len() {
        assert_eq!(
            menu.result_separator_before(menu.result_actions, row),
            matches!(row, 2 | 4),
            "row {row}"
        );
    }
    choose(&mut ui, MenuAction::ManageGroups);
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        &[MenuAction::CreateGroup]
    );
    present_action_menu(&mut ui);
    let view = ui.view();
    let child = view.action_menu().unwrap();
    let child_source = child.source.clone();
    let child_token = child.token;
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Left, Modifiers::NONE));
    assert_eq!(
        ui.view().action_menu().unwrap().actions[2],
        MenuAction::ManageGroups
    );
    assert_eq!(ui.view().action_menu().unwrap().selected, 2);
    ui.activate_action_menu_row(&child_source, child_token, 0);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    ui.cancel_action_menu();
    assert!(ui.open_action_menu(
        MenuTarget::TabGroups {
            tab_id: target,
            colors: false
        },
        None,
        false
    ));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert_eq!(menu.title, "源组");
    assert_eq!(
        menu.actions,
        &[
            MenuAction::NewTabInGroup,
            MenuAction::RenameGroup,
            MenuAction::ChooseGroupColor,
            MenuAction::Ungroup,
            MenuAction::CloseGroup
        ]
    );
    assert!(!menu.actions.contains(&MenuAction::CreateGroup));
    choose(&mut ui, MenuAction::ChooseGroupColor);
    assert_eq!(ui.view().action_menu().unwrap().actions.len(), 9);
    let source = {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        assert!(menu.context_menu && menu.has_parent);
        menu.source.clone()
    };
    let stale = present_action_menu(&mut ui);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
    ui.activate_action_menu_control(&source, stale, crate::action_menu::MenuControl::Back);
    assert!(matches!(
        ui.view().action_menu().unwrap().source,
        MenuTarget::TabGroups { colors: true, .. }
    ));
    let token = present_action_menu(&mut ui);
    ui.activate_action_menu_control(&source, token, crate::action_menu::MenuControl::Back);
    assert_eq!(ui.view().action_menu().unwrap().selected, 2);
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "Back cannot write a candidate color"
    );
    choose(&mut ui, MenuAction::ChooseGroupColor);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert_eq!(ui.view().action_menu().unwrap().selected, 2);
    ui.cancel_action_menu();
    let target = ui.view().tab_create_target().unwrap();
    assert!(ui.open_action_menu(MenuTarget::TabCreate(target), None, false));
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        &[
            MenuAction::NewTab,
            MenuAction::NewTabInRecentGroup,
            MenuAction::NewGroupWithTab,
            MenuAction::NewTabWithProfile(Box::new(
                orchbee_workbench_protocol::UserLaunchProfile {
                    id: "system-default".into(),
                    name: None,
                    command: orchbee_workbench_protocol::UserLaunchCommand::SystemShell {
                        login: false,
                    },
                }
            ))
        ]
    );
    assert_eq!(
        ui.view().action_menu().unwrap().labels[1].as_deref(),
        Some("源组")
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn strip_paging_requires_the_painted_source_and_follows_its_pointer_selection() {
    for exact in [false, true] {
        let (mut ui, _mailbox, _, _) =
            ready_snapshot(super::super::super::tests::membership::snapshot(6, 1));
        let source = ui.tab_drag_source().unwrap();
        let mut stale = source.drawn.clone();
        stale.source.revision = WorkbenchRevision::new(0);
        let painted = if exact { &source.drawn } else { &stale };
        let page = crate::workbench_state::TabStripStart {
            tab: source.tabs[3].id.clone(),
            header: false,
        };
        ui.page_drawn_tab_strip(painted, &page);
        assert_eq!(ui.view().tab_strip_start(), exact.then(|| page.clone()));
        // Only a click on the painted strip carries its window across the selection it makes.
        ui.page_drawn_tab_strip(&source.drawn, &page);
        ui.follow_drawn_tab_strip(painted, &source.tabs[4].id);
        ui.apply_client_action(ClientAction::SelectTab(5));
        ui.prepare_draw(
            ratatui::layout::Rect::new(0, 0, 80, 24),
            crate::client_view::SidebarSurface::Hidden,
            false,
        );
        assert_eq!(ui.view().tab_strip_start(), exact.then_some(page));
    }
}

#[test]
fn drag_uses_existing_atomic_receipt_and_stale_source_cannot_queue() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(grouped());
    let original = snapshot.clone();
    let source = ui.tab_drag_source().unwrap();
    let moving = source.tabs[2].id.clone();
    let operation = TabConfiguration::MoveTabsByDrag {
        tab_ids: vec![moving.clone()].try_into().unwrap(),
        insert_index: orchbee_workbench_protocol::WorkbenchTabInsertIndex::new(0).unwrap(),
        group_id: None,
    };
    let mut stale = source.drawn.clone();
    stale.source.revision = WorkbenchRevision::new(0);
    ui.commit_tab_drag(&stale, operation.clone());
    assert!(mailbox.try_recv().unwrap().is_none());
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    ui.commit_tab_drag(&source.drawn, operation.clone());
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &original
    );
    assert!(
        ui.tab_drag_source().is_none(),
        "one configuration owns admission"
    );
    assert_eq!(
        accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None),
        operation
    );
    assert_eq!(snapshot.terminal_contexts[0].tabs[0].tab_id, moving);
    assert!(snapshot.terminal_contexts[0].tabs[0].group_id.is_none());
    for tab in &snapshot.terminal_contexts[0].tabs {
        assert_eq!(
            tab.root,
            original.terminal_contexts[0]
                .tabs
                .iter()
                .find(|before| before.tab_id == tab.tab_id)
                .unwrap()
                .root
        );
    }
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    ui.commit_tab_drag(&source.drawn, operation);
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "old painted source never retries"
    );
}
