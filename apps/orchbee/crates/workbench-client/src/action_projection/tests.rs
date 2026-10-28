use super::*;
use crate::client_view::{PREFIX_GROUPS, PrefixAction};

#[test]
fn every_shipped_prefix_member_has_stable_metadata() {
    for group in PREFIX_GROUPS {
        for member in group.members {
            let metadata = binding_metadata(&member.action)
                .unwrap_or_else(|| panic!("missing metadata for {:?}", member.action));
            assert!(!metadata.identity.stable_id().is_empty());
            assert!(
                !metadata.search_aliases.contains(&ActionSearchAlias::Detach)
                    || metadata.category == ActionCategory::Client
            );
        }
    }
}

#[test]
fn workbench_binding_identities_match_the_owning_catalog_contracts() {
    let cases = [
        (
            PrefixAction::Client(ClientAction::CreateTab),
            "workbench.terminal.create",
        ),
        (
            PrefixAction::Client(ClientAction::SelectNextTab),
            "workbench.tab.select-next",
        ),
        (
            PrefixAction::Client(ClientAction::SelectPreviousTab),
            "workbench.tab.select-previous",
        ),
        (
            PrefixAction::SelectTabDigit,
            "workbench.tab.select-by-position",
        ),
        (
            PrefixAction::Client(ClientAction::RequestWorkspaceClose),
            "workbench.workspace.delete",
        ),
    ];
    for (action, expected) in cases {
        assert_eq!(
            binding_metadata(&action).unwrap().identity.stable_id(),
            expected
        );
    }
    assert_eq!(
        binding_metadata(&PrefixAction::Client(ClientAction::FocusBusinessSidebar))
            .unwrap()
            .identity
            .stable_id(),
        "workbench.focus.sidebar"
    );
}

#[test]
fn direction_is_typed_input_and_does_not_mint_resize_identity() {
    let left = PrefixAction::Client(ClientAction::ResizePane {
        direction: PanelDirection::Left,
        fine: false,
    });
    let fine_right = PrefixAction::Client(ClientAction::ResizePane {
        direction: PanelDirection::Right,
        fine: true,
    });
    assert_eq!(
        binding_metadata(&left).unwrap().identity,
        binding_metadata(&fine_right).unwrap().identity
    );
}

#[test]
fn configured_command_identity_preserves_the_opaque_id() {
    let id = "user.action/日本語/$opaque".to_owned();
    let action = PrefixAction::Client(ClientAction::ConfiguredCommand(id.clone()));
    let identity = binding_metadata(&action).unwrap().identity;
    assert_eq!(identity, ActionIdentity::ConfiguredCommand(id.as_str()));
    assert_eq!(identity.stable_id(), id);
    assert_ne!(
        ActionIdentity::ConfiguredCommand("workbench.settings.open"),
        ActionIdentity::BuiltIn("workbench.settings.open")
    );
}

#[test]
fn menu_and_binding_presenters_share_common_action_identity() {
    let cases = [
        (
            MenuAction::SplitPanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Right)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Down)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Left)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Up)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Left)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Down)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Up)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Right)),
        ),
        (
            MenuAction::Zoom,
            PrefixAction::Client(ClientAction::ToggleZoom),
        ),
        (
            MenuAction::Resize,
            PrefixAction::Client(ClientAction::EnterResizeMode),
        ),
        (
            MenuAction::Copy,
            PrefixAction::Client(ClientAction::EnterCopyMode),
        ),
        (
            MenuAction::Takeover,
            PrefixAction::Client(ClientAction::RequestControlTakeover),
        ),
        (
            MenuAction::ClosePane,
            PrefixAction::Client(ClientAction::ClosePanel),
        ),
        (
            MenuAction::Settings,
            PrefixAction::Client(ClientAction::OpenSettings),
        ),
        (
            MenuAction::NewWorkspace,
            PrefixAction::Client(ClientAction::RequestWorkspaceCreate),
        ),
        (
            MenuAction::QuickWorkspace,
            PrefixAction::Client(ClientAction::RequestQuickWorkspace),
        ),
        (
            MenuAction::WorkspaceDirectory,
            PrefixAction::Client(ClientAction::RequestWorkspaceDirectory),
        ),
        (
            MenuAction::RenameWorkspace,
            PrefixAction::Client(ClientAction::RequestWorkspaceRename),
        ),
        (
            MenuAction::RenameProject,
            PrefixAction::Client(ClientAction::RequestProjectRename),
        ),
        (
            MenuAction::CloseWorkspace,
            PrefixAction::Client(ClientAction::RequestWorkspaceClose),
        ),
        (
            MenuAction::NewWorktree,
            PrefixAction::Client(ClientAction::RequestWorktreeCreate),
        ),
        (
            MenuAction::OpenWorktree,
            PrefixAction::Client(ClientAction::RequestWorktreeOpen),
        ),
        (
            MenuAction::NewTab,
            PrefixAction::Client(ClientAction::CreateTab),
        ),
        (
            MenuAction::RenameTab,
            PrefixAction::Client(ClientAction::RequestRenameTab),
        ),
        (
            MenuAction::MoveTabPrevious,
            PrefixAction::Client(ClientAction::MovePreviousTab),
        ),
        (
            MenuAction::MoveTabNext,
            PrefixAction::Client(ClientAction::MoveNextTab),
        ),
        (
            MenuAction::CloseSelectedTabs,
            PrefixAction::Client(ClientAction::RequestCloseTab),
        ),
        (
            MenuAction::Reload,
            PrefixAction::Client(ClientAction::ReloadConfiguration),
        ),
        (
            MenuAction::Machines,
            PrefixAction::Client(ClientAction::OpenMachines),
        ),
        (MenuAction::Sidebar, PrefixAction::SidebarToggle),
        (MenuAction::Navigator, PrefixAction::NavigatorToggle),
        (MenuAction::AgentBoard, PrefixAction::BoardToggle),
        (MenuAction::Help, PrefixAction::ShowHelp),
        (
            MenuAction::ReleasePointer,
            PrefixAction::Client(ClientAction::TogglePointerCapture),
        ),
        (MenuAction::Detach, PrefixAction::Client(ClientAction::Exit)),
    ];
    for (menu, binding) in cases {
        assert_eq!(
            menu_metadata(menu.clone()).action,
            binding_metadata(&binding).unwrap(),
            "{menu:?}"
        );
    }
}

#[test]
fn dynamic_menu_input_keeps_one_semantic_identity() {
    use orchbee_workbench_protocol::TabGroupColor;
    for candidate in [
        MenuAction::NewTabToRight,
        MenuAction::NewTabInGroup,
        MenuAction::NewTabInRecentGroup,
    ] {
        assert_eq!(
            menu_metadata(MenuAction::NewTab).action.identity,
            menu_metadata(candidate).action.identity
        );
    }
    assert_eq!(
        menu_metadata(MenuAction::GroupColor(TabGroupColor::Blue))
            .action
            .identity,
        menu_metadata(MenuAction::GroupColor(TabGroupColor::Red))
            .action
            .identity
    );
    assert_eq!(
        menu_metadata(MenuAction::JoinGroup(0)).action.identity,
        menu_metadata(MenuAction::JoinGroup(u16::MAX))
            .action
            .identity
    );
    assert_eq!(
        menu_metadata(MenuAction::MoveTabPrevious).action.identity,
        menu_metadata(MenuAction::MoveTabLast).action.identity
    );
    assert_eq!(
        menu_metadata(MenuAction::CollapseGroup).action.identity,
        menu_metadata(MenuAction::ExpandGroup).action.identity
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
            menu_metadata(action.clone()).tone == super::ActionTone::Destructive,
            matches!(
                action,
                MenuAction::DeleteWorkspace
                    | MenuAction::DeleteProject
                    | MenuAction::CloseSelectedTabs
                    | MenuAction::CloseOtherTabs
                    | MenuAction::CloseTrailingTabs
            ),
            "{action:?}",
        );
    }
    for action in [MenuAction::CloseGroup, MenuAction::ResumeTabClose] {
        assert_eq!(menu_metadata(action).tone, super::ActionTone::Destructive);
    }
    for legacy in [
        MenuAction::CloseWorkspace,
        MenuAction::RenameProject,
        MenuAction::CloseProjectWorkspaces,
        MenuAction::Ungroup,
        MenuAction::GroupColor(orchbee_workbench_protocol::TabGroupColor::Red),
    ] {
        assert_eq!(menu_metadata(legacy).tone, super::ActionTone::Ordinary);
    }
}

#[test]
fn menu_binding_projection_requires_the_same_typed_target_semantics() {
    use crate::action_menu::MenuTarget;

    let global = MenuTarget::Global;
    let cases = [
        (
            MenuAction::SplitPanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Up)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Down)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Left)),
        ),
        (
            MenuAction::SplitPanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Right)),
        ),
        (
            MenuAction::ClosePane,
            PrefixAction::Client(ClientAction::ClosePanel),
        ),
        (
            MenuAction::Zoom,
            PrefixAction::Client(ClientAction::ToggleZoom),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Up),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Up)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Down),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Down)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Left),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Left)),
        ),
        (
            MenuAction::MovePanel(PanelDirection::Right),
            PrefixAction::Client(ClientAction::MovePanel(PanelDirection::Right)),
        ),
        (
            MenuAction::Copy,
            PrefixAction::Client(ClientAction::EnterCopyMode),
        ),
        (
            MenuAction::Resize,
            PrefixAction::Client(ClientAction::EnterResizeMode),
        ),
        (
            MenuAction::RenameTab,
            PrefixAction::Client(ClientAction::RequestRenameTab),
        ),
        (
            MenuAction::Settings,
            PrefixAction::Client(ClientAction::OpenSettings),
        ),
        (MenuAction::Help, PrefixAction::ShowHelp),
        (MenuAction::Sidebar, PrefixAction::SidebarToggle),
        (MenuAction::Detach, PrefixAction::Client(ClientAction::Exit)),
    ];
    for (menu, expected) in cases {
        assert_eq!(menu_binding_action(&global, menu), Some(expected));
    }

    let tab = MenuTarget::Tab(
        orchbee_workbench_protocol::WorkbenchTabId::parse("00000000000000000000000000000001")
            .unwrap(),
    );
    assert_eq!(
        menu_metadata(MenuAction::RenameTab).action,
        binding_metadata(&PrefixAction::Client(ClientAction::RequestRenameTab)).unwrap()
    );
    assert_eq!(menu_binding_action(&tab, MenuAction::RenameTab), None);
    assert_eq!(
        menu_binding_action(&MenuTarget::Create(None), MenuAction::NewWorkspace),
        Some(PrefixAction::Client(ClientAction::RequestWorkspaceCreate))
    );
    assert_eq!(
        menu_binding_action(&MenuTarget::Create(None), MenuAction::Settings),
        None
    );
    assert_eq!(
        menu_binding_action(&tab, MenuAction::SplitPanel(PanelDirection::Up)),
        None,
        "a global current-pane shortcut is not advertised for an exact background target"
    );
}

#[test]
fn native_only_tab_operations_do_not_claim_narrower_gui_contracts() {
    assert_eq!(
        menu_metadata(MenuAction::SelectAllTabs).action.identity,
        ActionIdentity::BuiltIn("orchbee.native.tab-selection.all")
    );
    assert_eq!(
        menu_metadata(MenuAction::ClearTabSelection).action.identity,
        ActionIdentity::BuiltIn("orchbee.native.tab-selection.clear")
    );
    assert_ne!(
        menu_metadata(MenuAction::SelectAllTabs).action.identity,
        menu_metadata(MenuAction::SelectTab).action.identity
    );
    assert_eq!(
        menu_metadata(MenuAction::ResumeTabClose).action.identity,
        ActionIdentity::BuiltIn("orchbee.native.tab-close.resume")
    );
    assert_ne!(
        menu_metadata(MenuAction::ResumeTabClose).action.identity,
        menu_metadata(MenuAction::CloseSelectedTabs).action.identity
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
fn reused_cross_client_ids_are_present_in_their_owning_catalogs() {
    let workbench = include_str!(
        "../../../../../../packages/workbench/src/workbench/actions/workbench-action-ids.ts"
    );
    let tabs = include_str!(
        "../../../../../../packages/tab-strip/src/tab-strip/actions/tab-strip-action-contracts.ts"
    );
    for id in [
        "workbench.settings.open",
        "workbench.focus.sidebar",
        "workbench.terminal.active.close",
        "workbench.terminal.create",
        "workbench.workspace.create",
        "workbench.workspace.delete",
        "workbench.workspace.set-project",
        "workbench.project.close-all",
        "workbench.project.dissolve",
        "workbench.project.rename",
        "workbench.panel.active.focus-down",
        "workbench.panel.active.focus-left",
        "workbench.panel.active.focus-right",
        "workbench.panel.active.focus-up",
        "workbench.panel.active.move-down",
        "workbench.panel.active.move-left",
        "workbench.panel.active.move-right",
        "workbench.panel.active.move-up",
        "workbench.panel.active.resize",
        "workbench.panel.active.maximization.toggle",
        "workbench.panel.active.split-below",
        "workbench.panel.active.split-right",
        "workbench.sidebar.toggle",
        "workbench.tab.title.set",
        "workbench.tab.select-by-position",
        "workbench.tab.select-next",
        "workbench.tab.select-previous",
    ] {
        assert!(
            workbench.contains(&format!("'{id}'")),
            "missing GUI action ID {id}"
        );
    }
    for id in [
        "tabs.group.collapsed.set",
        "tabs.group.create",
        "tabs.group.create-with-tab",
        "tabs.group.move",
        "tabs.group.ungroup",
        "tabs.group.update",
        "tabs.tab.close",
        "tabs.tab.duplicate",
        "tabs.tab.move",
        "tabs.tab.move-to-group",
        "tabs.tab.move-to-window",
        "tabs.tab.pinned.set",
        "tabs.tab.select",
    ] {
        assert!(
            tabs.contains(&format!("'{id}'")),
            "missing tab-strip action ID {id}"
        );
    }
}
