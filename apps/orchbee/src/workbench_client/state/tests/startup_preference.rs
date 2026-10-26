use super::*;
use crate::workbench_client::{
    ClientContextLocation,
    startup_preference::{Position, resolve},
    workbench_projection::{preferred_workspace_location, project_native_context_tabs},
};

#[test]
fn startup_preference_restores_exact_nonfirst_member_after_reorder_and_new_session_boot() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    let location = ClientContextLocation::Workspace {
        workspace_id: snapshot.navigation.workspaces[1].workspace_id.clone(),
        terminal_context_id: snapshot.terminal_contexts[1].terminal_context_id.clone(),
    };
    let cache = project_native_context_tabs(&snapshot, &location).unwrap();
    let tab = &cache.tabs()[1];
    let member = super::super::super::workbench_projection::ClientMemberTarget {
        tab: tab.tab_id().clone(),
        panel: tab.panel(0).unwrap().panel_id().clone(),
        presentation: tab.panel(0).unwrap().presentation_id().clone(),
    };
    let saved = Position {
        location: location.clone(),
        member: Some(member.clone()),
    };
    snapshot.session_id = SessionId::parse(&id(999)).unwrap();
    snapshot.terminal_contexts[1].tabs.reverse();
    let (cache, selected, fallback) = resolve(&snapshot, &saved).unwrap();
    assert_eq!(cache.location(), &location);
    assert_eq!(selected, Some(member));
    assert!(!fallback);
}

#[test]
fn startup_preference_removed_tab_falls_back_in_same_location_without_launching() {
    let snapshot = super::context_selection::contexts(2, 1);
    let workspace = &snapshot.navigation.workspaces[1];
    let cache = project_native_context_tabs(
        &snapshot,
        &ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
    )
    .unwrap();
    let position = Position {
        location: cache.location().clone(),
        member: Some(
            super::super::super::workbench_projection::ClientMemberTarget {
                tab: WorkbenchTabId::parse(&id(999)).unwrap(),
                panel: WorkbenchPanelId::parse(&id(998)).unwrap(),
                presentation: TerminalPresentationId::parse(&id(997)).unwrap(),
            },
        ),
    };
    let (restored, selected, fallback) = resolve(&snapshot, &position).unwrap();
    assert_eq!(restored, cache);
    assert!(fallback);
    assert_eq!(selected.unwrap().tab, cache.tabs()[0].tab_id().clone());
}

#[test]
fn startup_preference_removed_location_falls_back_without_inventing_a_context() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    let workspace = &snapshot.navigation.workspaces[2];
    let position = Position {
        location: ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
        member: None,
    };
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert!(cache.tabs().is_empty());
    assert!(member.is_none());
    assert!(!fallback);
    snapshot.navigation.workspaces.remove(2);
    snapshot.terminal_contexts.remove(2);
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert_eq!(
        cache.location(),
        &preferred_workspace_location(&snapshot).unwrap()
    );
    assert_eq!(member.unwrap().tab, cache.tabs()[0].tab_id().clone());
    assert!(fallback);
    snapshot.navigation.workspaces.clear();
    snapshot.navigation.workspace_order.clear();
    snapshot.terminal_contexts.clear();
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert_eq!(cache.location(), &ClientContextLocation::NoWorkspace);
    assert!(cache.terminal_context_id().is_none());
    assert!(member.is_none());
    assert!(fallback);
}

#[test]
fn startup_preference_preserves_an_existing_empty_workspace() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    snapshot.terminal_contexts[0].tabs.clear();
    let saved = Position {
        location: project_preferred_workspace(&snapshot)
            .unwrap()
            .location()
            .clone(),
        member: None,
    };
    let (cache, member, fallback) = resolve(&snapshot, &saved).unwrap();
    assert_eq!(cache.location(), &saved.location);
    assert!(member.is_none());
    assert!(!fallback);
}
