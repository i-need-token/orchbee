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
            MenuAction::NewTabToRight,
            MenuAction::RenameTab,
            MenuAction::ManageGroups,
            MenuAction::RemoveFromGroup,
            MenuAction::MoveToNewWindow,
            MenuAction::DuplicateTab,
            MenuAction::PinTab,
            MenuAction::CloseSelectedTabs,
            MenuAction::CloseOtherTabs,
            MenuAction::CloseTrailingTabs,
            MenuAction::TabTools,
        ]
    );
    assert_eq!(
        menu.unavailable[4],
        Some(crate::action_menu::MenuUnavailable::UnsupportedHost)
    );
    assert!(menu.separator_before(5));
    assert!(menu.separator_before(7));
    choose(&mut ui, MenuAction::ManageGroups);
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        &[MenuAction::CreateGroup]
    );
    let child_token = ui.view().action_menu().unwrap().token;
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Left, Modifiers::NONE));
    assert_eq!(
        ui.view().action_menu().unwrap().actions[2],
        MenuAction::ManageGroups
    );
    assert_eq!(ui.view().action_menu().unwrap().selected, 2);
    ui.click_action_menu(child_token, 0);
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
        &menu.actions[..5],
        &[
            MenuAction::RenameGroup,
            MenuAction::ChooseGroupColor,
            MenuAction::NewTabInGroup,
            MenuAction::Ungroup,
            MenuAction::CloseGroup
        ]
    );
    assert!(!menu.actions.contains(&MenuAction::CreateGroup));
    choose(&mut ui, MenuAction::ChooseGroupColor);
    assert_eq!(ui.view().action_menu().unwrap().actions.len(), 9);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert_eq!(ui.view().action_menu().unwrap().selected, 1);
    ui.cancel_action_menu();
    let target = ui.view().tab_create_target().unwrap();
    assert!(ui.open_action_menu(MenuTarget::TabCreate(target), None, false));
    assert_eq!(
        ui.view().action_menu().unwrap().actions,
        &[
            MenuAction::NewTab,
            MenuAction::NewTabInRecentGroup,
            MenuAction::NewGroupWithTab
        ]
    );
    assert_eq!(
        ui.view().action_menu().unwrap().labels[1].as_deref(),
        Some("源组")
    );
    assert!(mailbox.try_recv().unwrap().is_none());
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
