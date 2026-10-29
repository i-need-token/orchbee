use super::*;
use crate::client_view::{ClientKey, KeyCode, KeyEvent, Modifiers};
use crate::workbench_client::state::TabSelectionChange;

fn scopes_snapshot() -> Snapshot {
    let mut snapshot = super::super::super::tests::membership::snapshot(5, 1);
    let groups = [
        TabGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 2600)).unwrap(),
            label: "Work".into(),
            color: TabGroupColor::Green,
        },
        TabGroup {
            group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 2601)).unwrap(),
            label: "Build group".into(),
            color: TabGroupColor::Red,
        },
    ];
    let context = &mut snapshot.terminal_contexts[0];
    for (index, tab) in context.tabs.iter_mut().enumerate() {
        tab.label = ["work", "Review", "Test", "Build", "Logs"][index].into();
        tab.group_id = groups.get(index / 2).map(|group| group.group_id.clone());
    }
    context.groups = groups.into();
    snapshot
}

fn row_scope(ui: &WorkbenchClientUi, action: MenuAction) -> TabMenuScope {
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let row = menu
        .actions
        .iter()
        .position(|candidate| *candidate == action)
        .unwrap();
    menu.tabs.unwrap().rows[row]
}

#[test]
fn cross_group_selection_and_unselected_target_present_the_same_scope_as_close_confirmation() {
    for (target_index, expected) in [(3, vec![0, 3]), (4, vec![4])] {
        let (mut ui, mailbox, _, snapshot) = ready_snapshot(scopes_snapshot());
        let tabs = &snapshot.terminal_contexts[0].tabs;
        ui.state
            .as_mut()
            .unwrap()
            .mark_tabs(&tabs[3].tab_id, TabSelectionChange::Toggle)
            .unwrap();
        let desired = ui.state.as_ref().unwrap().desired().cloned();
        let marks = ui.state.as_ref().unwrap().selected_tab_ids();
        // The global menu's Tab section addresses the current Tab, which belongs to the marks.
        assert!(ui.open_action_menu(MenuTarget::Global, None, false));
        assert_eq!(
            row_scope(&ui, MenuAction::DuplicateTab),
            TabMenuScope::Selected(NonZeroUsize::new(2).unwrap())
        );
        for action in [MenuAction::RenameTab, MenuAction::NewTabToRight] {
            assert_eq!(row_scope(&ui, action), TabMenuScope::Addressed);
        }
        ui.cancel_action_menu();
        assert!(ui.open_action_menu(
            MenuTarget::Tab(tabs[target_index].tab_id.clone()),
            None,
            false
        ));
        let selected = if target_index == 3 {
            TabMenuScope::Selected(NonZeroUsize::new(2).unwrap())
        } else {
            TabMenuScope::Addressed
        };
        for action in [
            MenuAction::PinTab,
            MenuAction::ManageGroups,
            MenuAction::CloseSelectedTabs,
        ] {
            assert_eq!(row_scope(&ui, action), selected);
        }
        assert_eq!(
            row_scope(&ui, MenuAction::RenameTab),
            TabMenuScope::Addressed
        );
        assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
        assert_eq!(ui.state.as_ref().unwrap().selected_tab_ids(), marks);
        choose(&mut ui, MenuAction::CloseSelectedTabs);
        let expected_labels: Vec<_> = expected
            .iter()
            .map(|index| (index + 1, tabs[*index].label.clone()))
            .collect();
        let view = ui.view();
        let prompt = view.close_confirmation().unwrap();
        assert_eq!(prompt.set_count(), Some(expected.len()));
        assert_eq!(prompt.set_labels().unwrap(), expected_labels);
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "opening a scoped decision never starts Close"
        );
        assert!(ui.capture_confirmation_key(ClientKey::Escape));
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
        assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
        assert_eq!(ui.state.as_ref().unwrap().selected_tab_ids(), marks);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

#[test]
fn relative_close_scope_protects_pinned_tabs_and_starts_after_rightmost_selected_tab() {
    let mut snapshot = scopes_snapshot();
    // Pinned tabs form an ungrouped prefix; the other fixture cases keep
    // two real groups so their cross-group selection remains canonical.
    for tab in &mut snapshot.terminal_contexts[0].tabs[..2] {
        tab.pinned = true;
        tab.group_id = None;
    }
    snapshot.terminal_contexts[0].groups.remove(0);
    let (mut ui, mailbox, _, snapshot) = ready_snapshot(snapshot);
    let tabs = &snapshot.terminal_contexts[0].tabs;
    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&tabs[3].tab_id, TabSelectionChange::Toggle)
        .unwrap();
    assert!(ui.open_action_menu(MenuTarget::Tab(tabs[0].tab_id.clone()), None, false));
    assert_eq!(
        row_scope(&ui, MenuAction::CloseOtherTabs),
        TabMenuScope::Other(NonZeroUsize::new(2).unwrap())
    );
    ui.cancel_action_menu();
    // The global row acts on the current, first Tab.
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    assert_eq!(
        row_scope(&ui, MenuAction::CloseTrailingTabs),
        TabMenuScope::Trailing(NonZeroUsize::new(1).unwrap())
    );
    choose(&mut ui, MenuAction::CloseTrailingTabs);
    let view = ui.view();
    assert_eq!(
        view.close_confirmation().unwrap().set_labels().unwrap(),
        &[(5, "Logs".into())]
    );
    assert!(ui.capture_confirmation_key(ClientKey::Escape));
    assert!(mailbox.try_recv().unwrap().is_none());

    let mut one = super::super::super::tests::membership::snapshot(1, 1);
    one.terminal_contexts[0].tabs[0].pinned = true;
    let (mut ui, mailbox, _, snapshot) = ready_snapshot(one);
    assert!(ui.open_action_menu(
        MenuTarget::Tab(snapshot.terminal_contexts[0].tabs[0].tab_id.clone()),
        None,
        false
    ));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    assert!(!menu.actions.contains(&MenuAction::CloseOtherTabs));
    assert!(
        menu.actions.contains(&MenuAction::CloseSelectedTabs),
        "direct Close still includes a pinned Tab"
    );
    ui.cancel_action_menu();
    // Without a Tab to the right, the global row stays in place with its reason.
    assert!(ui.open_action_menu(MenuTarget::Global, None, false));
    let view = ui.view();
    let menu = view.action_menu().unwrap();
    let row = menu
        .actions
        .iter()
        .position(|action| *action == MenuAction::CloseTrailingTabs)
        .unwrap();
    assert_eq!(
        menu.unavailable[row],
        Some(crate::action_menu::MenuUnavailable::NoMatchingTabs)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn direct_group_menu_and_saved_color_are_independent_of_cross_group_marks_and_candidate() {
    let (mut ui, mailbox, clock, mut snapshot) = ready_snapshot(scopes_snapshot());
    let target = snapshot.terminal_contexts[0].tabs[3].tab_id.clone();
    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&target, TabSelectionChange::Toggle)
        .unwrap();
    let marks = ui.state.as_ref().unwrap().selected_tab_ids();
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    assert!(ui.open_action_menu(
        MenuTarget::TabGroups {
            tab_id: target.clone(),
            colors: false
        },
        None,
        false
    ));
    assert_eq!(
        row_scope(&ui, MenuAction::CloseGroup),
        TabMenuScope::Group(NonZeroUsize::new(2).unwrap())
    );
    choose(&mut ui, MenuAction::ChooseGroupColor);
    let color_facts = |ui: &WorkbenchClientUi| {
        let view = ui.view();
        let menu = view.action_menu().unwrap();
        let checked: Vec<_> = menu
            .actions
            .iter()
            .filter(|action| menu.tabs.unwrap().checked(action))
            .cloned()
            .collect();
        (menu.actions[menu.selected].clone(), checked)
    };
    assert_eq!(
        color_facts(&ui),
        (
            MenuAction::GroupColor(TabGroupColor::Red),
            vec![MenuAction::GroupColor(TabGroupColor::Red)]
        )
    );
    present_action_menu(&mut ui);
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
    assert_eq!(
        color_facts(&ui),
        (
            MenuAction::GroupColor(TabGroupColor::Blue),
            vec![MenuAction::GroupColor(TabGroupColor::Red)]
        )
    );
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &snapshot
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "candidate changes are never color writes"
    );
    ui.capture_action_menu_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(matches!(
        ui.view().action_menu().unwrap().source,
        MenuTarget::TabGroups { colors: false, .. }
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    choose(&mut ui, MenuAction::ChooseGroupColor);
    choose(&mut ui, MenuAction::GroupColor(TabGroupColor::Yellow));
    let operation = accept_configuration(&mut ui, &mailbox, &clock, &mut snapshot, None);
    assert!(matches!(
        operation,
        TabConfiguration::UpdateGroup {
            color: TabGroupColor::Yellow,
            ..
        }
    ));
    assert_eq!(
        snapshot.terminal_contexts[0].groups[0].color,
        TabGroupColor::Green
    );
    assert_eq!(
        snapshot.terminal_contexts[0].groups[1].color,
        TabGroupColor::Yellow
    );
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
    assert_eq!(ui.state.as_ref().unwrap().selected_tab_ids(), marks);
    assert!(ui.open_action_menu(
        MenuTarget::TabGroups {
            tab_id: target,
            colors: true
        },
        None,
        false
    ));
    assert_eq!(
        color_facts(&ui),
        (
            MenuAction::GroupColor(TabGroupColor::Yellow),
            vec![MenuAction::GroupColor(TabGroupColor::Yellow)]
        )
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "reopening saved state never repeats the update"
    );
}
