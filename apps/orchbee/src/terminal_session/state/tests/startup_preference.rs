use super::*;
use crate::terminal_session::{
    NativeContextLocation,
    startup_preference::{Position, resolve},
    workbench_projection::project_native_context_tabs,
};

#[test]
fn startup_preference_restores_exact_nonfirst_member_after_reorder_and_new_site_boot() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    let location = NativeContextLocation::Workspace {
        workspace_id: snapshot.navigation.workspaces[0].workspace_id.clone(),
        terminal_context_id: snapshot.terminal_contexts[1].terminal_context_id.clone(),
    };
    let cache = project_native_context_tabs(&snapshot, &location).unwrap();
    let tab = &cache.tabs()[1];
    let member = super::super::super::workbench_projection::NativeMemberTarget {
        tab: tab.tab_id().clone(),
        panel: tab.panel(0).unwrap().panel_id().clone(),
        presentation: tab.panel(0).unwrap().presentation_id().clone(),
    };
    let saved = Position {
        location: location.clone(),
        member: Some(member.clone()),
    };
    snapshot.site_id = SiteId::parse(&id(999)).unwrap();
    snapshot.terminal_contexts[1].tabs.reverse();
    let (cache, selected, fallback) = resolve(&snapshot, &saved).unwrap();
    assert_eq!(cache.location(), &location);
    assert_eq!(selected, Some(member));
    assert!(!fallback);
}

#[test]
fn startup_preference_removed_tab_falls_back_in_same_location_without_launching() {
    let snapshot = super::context_selection::contexts(2, 1);
    let workspace = &snapshot.navigation.workspaces[0];
    let cache = project_native_context_tabs(
        &snapshot,
        &NativeContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
    )
    .unwrap();
    let position = Position {
        location: cache.location().clone(),
        member: Some(
            super::super::super::workbench_projection::NativeMemberTarget {
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
fn startup_preference_removed_location_falls_back_and_empty_location_has_no_invented_member() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    let workspace = &snapshot.navigation.workspaces[1];
    let position = Position {
        location: NativeContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        },
        member: None,
    };
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert!(cache.tabs().is_empty());
    assert_eq!(member, None);
    assert!(!fallback);
    snapshot.navigation.projects.clear();
    snapshot.navigation.workspaces.clear();
    snapshot.terminal_contexts.truncate(1);
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert!(matches!(
        cache.location(),
        NativeContextLocation::Standalone(_)
    ));
    assert_eq!(member.unwrap().tab, cache.tabs()[0].tab_id().clone());
    assert!(fallback);
    snapshot.terminal_contexts[0].tabs.clear();
    let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
    assert!(cache.tabs().is_empty());
    assert_eq!(member, None);
    assert!(fallback);
}

#[test]
fn startup_preference_empty_compatibility_position_yields_to_existing_workspace() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    snapshot.terminal_contexts[0].tabs.clear();
    let saved = Position {
        location: NativeContextLocation::Standalone(
            snapshot.navigation.standalone_terminal_context_id.clone(),
        ),
        member: None,
    };
    let (cache, member, fallback) = resolve(&snapshot, &saved).unwrap();
    assert!(matches!(
        cache.location(),
        NativeContextLocation::Workspace { workspace_id, terminal_context_id }
            if workspace_id == &snapshot.navigation.workspaces[0].workspace_id
                && terminal_context_id == &snapshot.navigation.workspaces[0].terminal_context_id
    ));
    assert_eq!(
        member.unwrap().tab,
        snapshot.terminal_contexts[1].tabs[0].tab_id
    );
    assert!(fallback);
}

#[test]
fn startup_preference_canonical_empty_workspace_precedes_legacy_and_removed_positions() {
    let mut snapshot = super::context_selection::contexts(2, 1);
    let legacy = project_native_standalone_tabs(&snapshot).unwrap();
    let last = &legacy.tabs()[1];
    let saved = Position {
        location: legacy.location().clone(),
        member: Some(crate::terminal_session::NativeMemberTarget {
            tab: last.tab_id().clone(),
            panel: last.panel(0).unwrap().panel_id().clone(),
            presentation: last.panel(0).unwrap().presentation_id().clone(),
        }),
    };
    snapshot.terminal_contexts[1].tabs.clear();
    for position in [
        saved.clone(),
        Position {
            location: NativeContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(&id(999))
                    .unwrap(),
                terminal_context_id: context_id(998),
            },
            member: saved.member.clone(),
        },
    ] {
        let (cache, member, fallback) = resolve(&snapshot, &position).unwrap();
        assert!(
            matches!(cache.location(), NativeContextLocation::Workspace { workspace_id, .. }
            if workspace_id == &snapshot.navigation.workspaces[0].workspace_id)
        );
        assert!(cache.tabs().is_empty());
        assert!(member.is_none());
        assert!(fallback);
    }
    snapshot.navigation.workspaces.clear();
    snapshot.navigation.workspace_order.clear();
    snapshot.terminal_contexts.truncate(1);
    let (cache, member, fallback) = resolve(&snapshot, &saved).unwrap();
    assert_eq!(cache.location(), &saved.location);
    assert_eq!(member, saved.member);
    assert!(!fallback);
}
