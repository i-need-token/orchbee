use super::*;
use crate::client_view::{ClientKey, KeyCode, KeyEvent, Modifiers};
use crate::workbench_client::state::PreparedRefresh;
use crate::workbench_client::ui::tests::{membership::snapshot, workspace_setup::ready_snapshot};
use orchbee_workbench_protocol::{
    Snapshot, TabGroup, TabGroupColor, WorkbenchRevision, WorkbenchSnapshotOutcome,
    WorkbenchTabGroupId,
};

fn key(ui: &mut WorkbenchClientUi, code: KeyCode) {
    ui.capture_action_menu_key(KeyEvent::press(code, Modifiers::NONE));
}

fn select(ui: &mut WorkbenchClientUi, action: MenuAction) -> (MenuToken, MenuTarget, usize) {
    arm(ui);
    key(ui, KeyCode::Home);
    let row = action_row(ui, action);
    for _ in 0..row {
        key(ui, KeyCode::Down);
    }
    let token = arm(ui);
    let source = ui.view().action_menu().unwrap().source.clone();
    (token, source, row)
}

/// The real state refresh policy installs a peer Append without opening another UI surface.
fn peer_append(ui: &mut WorkbenchClientUi, snapshot: Snapshot) {
    let state = ui.state.as_mut().unwrap();
    let desired = state.desired().cloned();
    let focus = state.focus_pane_ref();
    assert!(state.start_pending_refresh(50));
    let prepared = state.prepare_refresh(50, WorkbenchSnapshotOutcome::Available { snapshot });
    assert!(matches!(prepared, PreparedRefresh::Append { .. }));
    assert_eq!(
        state.apply_prepared_refresh(prepared, Instant::now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.desired(), desired.as_ref());
    assert_eq!(state.focus_pane_ref(), focus);
    assert!(state.invariant_holds());
}

#[test]
fn automatic_and_explicit_tab_headings_share_zero_based_name_owner_without_selecting_target() {
    for automatic in [true, false] {
        let mut snapshot = snapshot(3, 1);
        for (index, tab) in snapshot.terminal_contexts[0].tabs.iter_mut().enumerate() {
            tab.automatic_label = automatic;
            tab.label = format!("Named {index}");
        }
        let (mut ui, mailbox, _, snapshot) = ready_snapshot(snapshot);
        let desired = ui.state.as_ref().unwrap().desired().cloned();
        let focus = ui.state.as_ref().unwrap().focus_pane_ref();
        for (index, tab) in snapshot.terminal_contexts[0].tabs.iter().enumerate() {
            for target in [
                MenuTarget::Tab(tab.tab_id.clone()),
                MenuTarget::TabGroupChoices(tab.tab_id.clone()),
            ] {
                assert!(
                    ui.open_action_menu(target.clone(), None, false),
                    "{target:?}"
                );
                let view = ui.view();
                let menu = view.action_menu().unwrap();
                assert_eq!(menu.source, &target);
                assert_eq!(
                    menu.title,
                    if automatic {
                        (index + 1).to_string()
                    } else {
                        tab.label.clone()
                    }
                );
                assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());
                assert_eq!(ui.state.as_ref().unwrap().focus_pane_ref(), focus);
                ui.cancel_action_menu();
            }
        }
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

fn scope_drift_retires(
    old: Snapshot,
    mut peer: Snapshot,
    action: MenuAction,
    expected_indices: &[usize],
) {
    peer.revision = WorkbenchRevision::new(2);
    let target = old.terminal_contexts[0].tabs[0].tab_id.clone();
    let expected_ids: Vec<_> = expected_indices
        .iter()
        .map(|i| peer.terminal_contexts[0].tabs[*i].tab_id.clone())
        .collect();
    let expected_labels: Vec<_> = expected_indices
        .iter()
        .map(|i| (i + 1, peer.terminal_contexts[0].tabs[*i].label.clone()))
        .collect();
    let (mut ui, mailbox, _, _) = ready_snapshot(old);
    let desired = ui.state.as_ref().unwrap().desired().cloned();
    // Close group belongs to the group header's menu.
    let root = if action == MenuAction::CloseGroup {
        MenuTarget::TabGroups {
            tab_id: target.clone(),
            colors: false,
        }
    } else {
        MenuTarget::Tab(target.clone())
    };
    assert!(ui.open_action_menu(root.clone(), None, false));
    let (old_token, old_source, old_row) = select(&mut ui, action.clone());
    let old_input = ui.action_menu_input_context().unwrap();
    peer_append(&mut ui, peer.clone());
    // A previously captured Enter must be consumed without offering a recomputed target set.
    ui.capture_action_menu_key_for(old_input, KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    if let Some(action) = ui.take_action_menu_action(false) {
        ui.apply_client_action(action);
    }
    assert!(
        ui.view().close_confirmation().is_none(),
        "old painted scope must not hand off a different set"
    );
    assert!(
        !ui.action_menu_active(),
        "changed resolver inputs retire the old menu"
    );
    ui.activate_action_menu_row(&old_source, old_token, old_row);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.state.as_ref().unwrap().desired(), desired.as_ref());

    assert!(ui.open_action_menu(root, None, false));
    let fresh_root = ui.view().action_menu().unwrap().token;
    assert_ne!(fresh_root, old_token);
    key(&mut ui, KeyCode::Enter);
    assert!(
        ui.take_action_menu_action(false).is_none(),
        "a new token still requires paint"
    );
    assert_eq!(
        super::super::super::tab_groups::close_tab_ids(
            ui.state.as_ref().unwrap(),
            &target,
            action.clone()
        ),
        Some(expected_ids)
    );
    let (token, _, row) = select(&mut ui, action);
    activate_row(&mut ui, token, row);
    let handoff = ui
        .take_action_menu_action(false)
        .expect("fresh reviewed scope");
    ui.apply_client_action(handoff);
    let view = ui.view();
    let prompt = view
        .close_confirmation()
        .expect("existing CloseSet owner remains mandatory");
    assert_eq!(prompt.set_count(), Some(expected_indices.len()));
    assert_eq!(prompt.captured_set_labels().unwrap(), expected_labels);
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "a decision is not Close execution"
    );
    assert!(ui.capture_confirmation_key(ClientKey::Escape));
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &peer
    );
}

#[test]
fn appended_unselected_tab_retires_painted_other_scope_before_enter_handoff() {
    scope_drift_retires(
        snapshot(2, 1),
        snapshot(3, 2),
        MenuAction::CloseOtherTabs,
        &[1, 2],
    );
}

#[test]
fn same_count_different_unselected_id_retires_painted_other_scope_before_enter_handoff() {
    let old = snapshot(2, 1);
    let mut peer = snapshot(3, 2);
    peer.terminal_contexts[0].tabs.remove(1);
    scope_drift_retires(old, peer, MenuAction::CloseOtherTabs, &[1]);
}

#[test]
fn unselected_order_and_pin_changes_retire_relative_scope() {
    let old = snapshot(3, 1);
    let mut peer = old.clone();
    peer.terminal_contexts[0].tabs.swap(1, 2);
    scope_drift_retires(old, peer, MenuAction::CloseOtherTabs, &[1, 2]);
    let mut old = snapshot(3, 1);
    old.terminal_contexts[0].tabs[0].pinned = true;
    let mut peer = old.clone();
    peer.terminal_contexts[0].tabs[1].pinned = true;
    scope_drift_retires(old, peer, MenuAction::CloseOtherTabs, &[2]);
}

#[test]
fn group_join_and_same_count_member_replacement_retire_group_scope() {
    let mut old = snapshot(3, 1);
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 2900)).unwrap(),
        label: "Stable group".into(),
        color: TabGroupColor::Blue,
    };
    old.terminal_contexts[0].groups.push(group.clone());
    for tab in &mut old.terminal_contexts[0].tabs[..2] {
        tab.group_id = Some(group.group_id.clone());
    }
    let mut peer = old.clone();
    peer.terminal_contexts[0].tabs[2].group_id = Some(group.group_id.clone());
    scope_drift_retires(old.clone(), peer, MenuAction::CloseGroup, &[0, 1, 2]);
    let mut peer = old.clone();
    peer.terminal_contexts[0].tabs[1].group_id = None;
    peer.terminal_contexts[0].tabs[2].group_id = Some(group.group_id);
    peer.terminal_contexts[0].tabs.swap(1, 2);
    scope_drift_retires(old, peer, MenuAction::CloseGroup, &[0, 1]);
}

#[test]
fn unchanged_resolver_source_keeps_painted_token_and_exact_other_scope() {
    let old = snapshot(2, 1);
    let mut peer = old.clone();
    peer.revision = WorkbenchRevision::new(2);
    let target = old.terminal_contexts[0].tabs[0].tab_id.clone();
    let (mut ui, mailbox, _, _) = ready_snapshot(old);
    assert!(ui.open_action_menu(MenuTarget::Tab(target), None, false));
    let (token, _, _) = select(&mut ui, MenuAction::CloseOtherTabs);
    peer_append(&mut ui, peer);
    assert!(!ui.drive_action_menu());
    assert_eq!(ui.view().action_menu().unwrap().token, token);
    key(&mut ui, KeyCode::Enter);
    let action = ui.take_action_menu_action(false).unwrap();
    ui.apply_client_action(action);
    assert_eq!(
        ui.view()
            .close_confirmation()
            .unwrap()
            .captured_set_labels()
            .unwrap(),
        &[(2, "Tab 1".into())]
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn drift_after_enter_offer_is_revalidated_before_handoff_without_replay() {
    let old = snapshot(2, 1);
    let target = old.terminal_contexts[0].tabs[0].tab_id.clone();
    let (mut ui, mailbox, _, _) = ready_snapshot(old);
    assert!(ui.open_action_menu(MenuTarget::Tab(target), None, false));
    let (token, source, row) = select(&mut ui, MenuAction::CloseOtherTabs);
    key(&mut ui, KeyCode::Enter);
    assert!(ui.menus.menu.confirmed().is_some());
    peer_append(&mut ui, snapshot(3, 2));
    assert!(ui.take_action_menu_action(false).is_none());
    ui.activate_action_menu_row(&source, token, row);
    assert!(ui.take_action_menu_action(false).is_none());
    assert!(ui.view().close_confirmation().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}
