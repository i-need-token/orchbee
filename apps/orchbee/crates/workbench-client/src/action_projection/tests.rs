use super::*;
use crate::client_actions::MenuAction;

#[test]
fn dynamic_menu_input_keeps_one_semantic_identity() {
    use orchbee_workbench_protocol::TabGroupColor;
    for candidate in [MenuAction::NewTabToRight, MenuAction::NewTabInGroup] {
        assert_eq!(
            MenuAction::NewTab.declaration().action.identity,
            candidate.declaration().action.identity
        );
    }
    assert_eq!(
        MenuAction::GroupColor(TabGroupColor::Blue)
            .declaration()
            .action
            .identity,
        MenuAction::GroupColor(TabGroupColor::Red)
            .declaration()
            .action
            .identity
    );
    assert_eq!(
        MenuAction::JoinGroup(0).declaration().action.identity,
        MenuAction::JoinGroup(u16::MAX)
            .declaration()
            .action
            .identity
    );
    assert_eq!(
        MenuAction::MoveTabPrevious.declaration().action.identity,
        MenuAction::MoveTabLast.declaration().action.identity
    );
}

#[test]
fn sidebar_delete_and_tab_close_intents_share_destructive_tone_without_status_roles() {
    for action in crate::action_menu::WORKSPACE_ACTIONS
        .iter()
        .chain(crate::action_menu::PROJECT_ACTIONS)
        .chain(crate::action_menu::PANE_ACTIONS)
        .chain(crate::action_menu::TAB_ACTIONS)
    {
        assert_eq!(
            action.declaration().tone == super::ActionTone::Destructive,
            // Dissolving a Project keeps every Workspace, so it is not destructive.
            matches!(
                action,
                MenuAction::DeleteWorkspace
                    | MenuAction::ClosePane
                    | MenuAction::CloseSelectedTabs
                    | MenuAction::CloseOtherTabs
                    | MenuAction::CloseTrailingTabs
            ),
            "{action:?}",
        );
    }
    assert_eq!(
        MenuAction::CloseGroup.declaration().tone,
        super::ActionTone::Destructive
    );
    for legacy in [
        MenuAction::CloseWorkspace,
        MenuAction::RenameProject,
        MenuAction::CloseProjectWorkspaces,
        MenuAction::Ungroup,
        MenuAction::GroupColor(orchbee_workbench_protocol::TabGroupColor::Red),
    ] {
        assert_eq!(legacy.declaration().tone, super::ActionTone::Ordinary);
    }
}

#[test]
fn native_only_tab_operations_do_not_claim_narrower_gui_contracts() {
    assert_eq!(
        MenuAction::SelectAllTabs.declaration().action.identity,
        ActionIdentity::BuiltIn("orchbee.native.tab-selection.all")
    );
    assert_eq!(
        MenuAction::ClearTabSelection.declaration().action.identity,
        ActionIdentity::BuiltIn("orchbee.native.tab-selection.clear")
    );
    assert_ne!(
        MenuAction::SelectAllTabs.declaration().action.identity,
        MenuAction::SelectTab.declaration().action.identity
    );
}

#[test]
fn availability_keeps_hidden_distinct_from_disabled() {
    let enabled = ActionAvailability::<&str>::Enabled;
    let disabled = ActionAvailability::Disabled("busy");
    let hidden = ActionAvailability::<&str>::Hidden;
    assert_eq!(
        ActionAvailability::from_disabled_reason(Some("busy")),
        disabled
    );
    assert_eq!(ActionAvailability::from_disabled_reason(None), enabled);
    assert!(enabled.is_enabled());
    assert!(!disabled.is_enabled());
    assert!(!disabled.is_hidden());
    assert!(hidden.is_hidden());
}

#[test]
fn reused_cross_client_ids_match_the_shared_action_identity_registry() {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Registry {
        contract_version: u32,
        catalogs: Catalogs,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Catalogs {
        workbench: Vec<String>,
        tabs: Vec<String>,
    }
    let registry: Registry = serde_json::from_str(include_str!(
        "../../../../../../schemas/workbench/v1/workbench-action-identities-v1.json"
    ))
    .unwrap();
    assert_eq!(registry.contract_version, 1);
    let mut registered = std::collections::BTreeSet::new();
    for (namespace, ids) in [
        ("workbench.", &registry.catalogs.workbench),
        ("tabs.", &registry.catalogs.tabs),
    ] {
        for id in ids {
            assert!(
                id.starts_with(namespace),
                "{id} is outside its owning catalog"
            );
            assert!(registered.insert(id.as_str()), "duplicate registry ID {id}");
        }
    }
    // Every GUI-namespace literal the projection mints reuses a registered GUI identity, and
    // the registry names no identity the native projection has stopped reusing.
    let projected: std::collections::BTreeSet<_> = include_str!("menu.rs")
        .split('"')
        .filter(|segment| {
            (segment.starts_with("workbench.") || segment.starts_with("tabs."))
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-".contains(&byte)
                })
        })
        .collect();
    assert_eq!(projected, registered);
}
