//! Discovers effective shortcut help from Client bindings and active feature modes.
//! Stable identity, availability, contextual selection, deduplication and filtering stay here;
//! localization receives only selected semantic labels and groups.

use crate::{
    action_projection::{ActionCategory, ActionIdentity, ActionMetadata},
    client_view::{
        ClientAction, ClientView, DiscoveryEntry, KeyLabelGlyphs, LabelStyle, Overlay,
        PrefixAction, format_binding_with_glyphs,
    },
    localization::{
        ActionSearchAlias, Locale, PrefixDesc, ShortcutContext, ShortcutHelpGroup, ShortcutLabel,
    },
};

/// Render-ready discovery row with private stable identity for merging, never for execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutHelpEntry {
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) group: ShortcutHelpGroup,
    pub(crate) search_text: String,
    pub(crate) disabled_reason: Option<String>,
    identity: String,
}

impl ShortcutHelpEntry {
    pub(crate) fn common(&self) -> bool {
        self.group == ShortcutHelpGroup::Common || self.identity.starts_with("context.")
    }
}

/// The stable feature-owned order shared by discovery and filtered rendering.
pub(crate) const fn group_order(group: ShortcutHelpGroup) -> u8 {
    match group {
        ShortcutHelpGroup::Common => 0,
        ShortcutHelpGroup::Workspace => 1,
        ShortcutHelpGroup::Tab => 2,
        ShortcutHelpGroup::Panel => 3,
        ShortcutHelpGroup::Agent => 4,
        ShortcutHelpGroup::Client => 5,
        ShortcutHelpGroup::Other => 6,
    }
}

/// Discover current bindings and modes without executing an action.
pub(crate) fn entries(
    locale: Locale,
    view: &ClientView,
    session_available: bool,
    glyphs: KeyLabelGlyphs,
) -> Vec<ShortcutHelpEntry> {
    let mut entries: Vec<ShortcutHelpEntry> = Vec::new();
    for binding in view.bindings().discovery_entries(glyphs) {
        if let PrefixAction::Client(action) = &binding.action
            && crate::tui_capabilities::for_action(action)
                .is_some_and(|capability| !view.capabilities().supports(capability))
        {
            continue;
        }
        if matches!(
            binding.action,
            PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext
        ) {
            continue;
        }
        // A withheld welcome card is hidden, so help never offers to dismiss it (ORC-402).
        if matches!(
            binding.action,
            PrefixAction::Client(ClientAction::DismissOnboarding)
        ) && !crate::onboarding::offered()
        {
            continue;
        }
        let group = if common_action(&binding.action) {
            ShortcutHelpGroup::Common
        } else {
            help_group(binding.metadata)
        };
        let title = shortcut_action_title(locale, view, &binding);
        let aliases = binding
            .metadata
            .search_aliases
            .iter()
            .map(|alias| locale.shortcut_search_alias(*alias))
            .collect::<Vec<_>>()
            .join(" ");
        let disabled_reason = (!session_available && requires_session(&binding.action))
            .then(|| locale.shortcut_requires_session().to_owned());
        let search_text = format!(
            "{} {} {} {} {}",
            title,
            binding.key,
            aliases,
            locale.shortcut_group_name(group),
            disabled_reason.as_deref().unwrap_or_default()
        );
        merge_help_entry(
            &mut entries,
            ShortcutHelpEntry {
                key: binding.key,
                title,
                group,
                search_text,
                disabled_reason,
                identity: help_identity(&binding.action, binding.metadata.identity),
            },
        );
    }
    append_contextual_entries(locale, view, glyphs, &mut entries);
    entries.sort_by_key(|entry| group_order(entry.group));
    entries
}

fn shortcut_action_title(
    locale: Locale,
    view: &ClientView,
    binding: &DiscoveryEntry<'_>,
) -> String {
    if view.overlay() == Some(Overlay::ShortcutHelp)
        && matches!(
            binding.action,
            PrefixAction::ShowHelp | PrefixAction::Client(ClientAction::OpenShortcutHelp)
        )
    {
        return locale.shortcut_close_label().to_owned();
    }
    if let Some(name) = view.bindings().configured_name(binding.description) {
        return name.to_owned();
    }

    if matches!(
        binding.action,
        PrefixAction::Client(
            ClientAction::RequestWorkspaceRename
                | ClientAction::RequestProjectRename
                | ClientAction::RequestRenameTab
                | ClientAction::ClosePanel
                | ClientAction::ToggleZoom
                | ClientAction::RequestControlTakeover
                | ClientAction::Exit
                | ClientAction::MovePanelFocus(_)
                | ClientAction::SplitPanel(_)
        )
    ) {
        return local_action_title(locale, &binding.action);
    }

    if let Some(action) = shared_menu_action(binding) {
        return locale.menu_action(&action).to_owned();
    }

    local_action_title(locale, &binding.action)
}

fn append_contextual_entries(
    locale: Locale,
    view: &ClientView,
    glyphs: KeyLabelGlyphs,
    entries: &mut Vec<ShortcutHelpEntry>,
) {
    use crate::machine_picker::{SIDEBAR_BINDINGS, SidebarCommand};

    let shell = locale.shell();
    if view.machines.sidebar_navigation_active() {
        for (binding, command) in SIDEBAR_BINDINGS {
            if *command == SidebarCommand::Exit
                || (*command == SidebarCommand::Search && !view.capabilities().offers_search())
                || (*command == SidebarCommand::NewWorkspace
                    && !view
                        .capabilities()
                        .supports(crate::tui_capabilities::Capability::DirectoryInput))
            {
                continue;
            }
            let key = format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs);
            let (title, identity) = match command {
                SidebarCommand::Help => (
                    locale.shortcut_close_label().to_owned(),
                    semantic_identity(PrefixAction::ShowHelp),
                ),
                SidebarCommand::NewWorkspace => {
                    let action = PrefixAction::Client(ClientAction::RequestQuickWorkspace);
                    (
                        local_action_title(locale, &action),
                        semantic_identity(action),
                    )
                }
                _ => (
                    locale.sidebar_navigation_action(*command).to_owned(),
                    format!("context.sidebar.{command:?}"),
                ),
            };
            let group = if *command == SidebarCommand::NewWorkspace {
                ShortcutHelpGroup::Workspace
            } else {
                ShortcutHelpGroup::Other
            };
            merge_context_entry(
                locale,
                entries,
                key,
                &title,
                group,
                identity,
                locale.shortcut_context(ShortcutContext::Sidebar),
            );
        }
        for binding in view
            .bindings()
            .discovery_entries(glyphs)
            .into_iter()
            .filter(|binding| {
                matches!(
                    binding.action,
                    PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext
                )
            })
        {
            merge_context_entry(
                locale,
                entries,
                binding.key,
                locale.shortcut_scroll_label(),
                ShortcutHelpGroup::Other,
                identity_key(binding.metadata.identity),
                locale.shortcut_context(ShortcutContext::Sidebar),
            );
        }
    } else if view.copy_mode_active() {
        for (key, description) in view.copy_help_rows() {
            if description == PrefixDesc::CopyExit {
                continue;
            }
            let (title, identity, group) = match description {
                PrefixDesc::ShowHelp => (
                    locale.shortcut_close_label(),
                    semantic_identity(PrefixAction::ShowHelp),
                    ShortcutHelpGroup::Other,
                ),
                PrefixDesc::CopyPage => (
                    locale.shortcut_scroll_label(),
                    semantic_identity(PrefixAction::HelpPageNext),
                    ShortcutHelpGroup::Other,
                ),
                _ => (
                    (shell.prefix_desc)(description),
                    format!("context.copy.{description:?}"),
                    ShortcutHelpGroup::Panel,
                ),
            };
            merge_context_entry(
                locale,
                entries,
                key,
                title,
                group,
                identity,
                locale.shortcut_context(ShortcutContext::Copy),
            );
        }
    } else if view.resize_mode() {
        for (key, description) in view.resize_help_rows_with_glyphs(glyphs) {
            if description == PrefixDesc::ResizeExit {
                continue;
            }
            let (title, identity, group) = match description {
                PrefixDesc::ShowHelp => (
                    locale.shortcut_close_label(),
                    semantic_identity(PrefixAction::ShowHelp),
                    ShortcutHelpGroup::Other,
                ),
                PrefixDesc::HelpPage => (
                    locale.shortcut_scroll_label(),
                    semantic_identity(PrefixAction::HelpPageNext),
                    ShortcutHelpGroup::Other,
                ),
                _ => (
                    (shell.prefix_desc)(description),
                    format!("context.resize.{description:?}"),
                    ShortcutHelpGroup::Panel,
                ),
            };
            merge_context_entry(
                locale,
                entries,
                key,
                title,
                group,
                identity,
                locale.shortcut_context(ShortcutContext::Resize),
            );
        }
    }
}

fn merge_context_entry(
    locale: Locale,
    entries: &mut Vec<ShortcutHelpEntry>,
    key: String,
    title: &str,
    group: ShortcutHelpGroup,
    identity: String,
    scope: &str,
) {
    let search_text = format!(
        "{title} {key} {} {scope}",
        locale.shortcut_group_name(group)
    );
    merge_help_entry(
        entries,
        ShortcutHelpEntry {
            key,
            title: title.to_owned(),
            group,
            search_text,
            disabled_reason: None,
            identity,
        },
    );
}

fn local_action_title(locale: Locale, action: &PrefixAction) -> String {
    use ClientAction as A;
    let label = match action {
        PrefixAction::Client(A::MovePanelFocus(direction)) => ShortcutLabel::FocusPanel(*direction),
        PrefixAction::Client(A::SplitPanel(direction)) => ShortcutLabel::SplitPanel(*direction),
        PrefixAction::Client(A::MovePanel(direction)) => ShortcutLabel::MovePanel(*direction),
        PrefixAction::Client(A::RequestWorkspaceCreate | A::RequestQuickWorkspace) => {
            return locale
                .menu_action(&crate::action_menu::MenuAction::NewWorkspace)
                .to_owned();
        }
        PrefixAction::Client(A::DismissOnboarding) => {
            return locale
                .onboarding_label(crate::onboarding::Action::Dismiss)
                .to_owned();
        }
        PrefixAction::Client(A::FocusBusinessSidebar) => {
            return (locale.shell().prefix_desc)(PrefixDesc::SidebarFocus).to_owned();
        }
        PrefixAction::Client(A::OpenSettings) => ShortcutLabel::Settings,
        PrefixAction::Client(A::ReloadConfiguration) => ShortcutLabel::ReloadSettings,
        PrefixAction::Client(A::OpenActionMenu) => ShortcutLabel::ActionMenu,
        PrefixAction::Client(A::EnterResizeMode | A::ResizePane { .. }) => {
            ShortcutLabel::ResizePanel
        }
        PrefixAction::Client(A::EnterCopyMode) => ShortcutLabel::CopyText,
        PrefixAction::Client(A::ConfiguredCommand(_)) => ShortcutLabel::ConfiguredCommand,
        PrefixAction::Client(A::RequestWorkspaceClose) => ShortcutLabel::CloseWorkspace,
        PrefixAction::Client(A::RequestWorkspaceAtDirectory) => ShortcutLabel::WorkspaceAtDirectory,
        PrefixAction::Client(A::RequestWorkspaceDirectory) => ShortcutLabel::WorkspaceDirectory,
        PrefixAction::Client(A::RequestWorktreeCreate) => ShortcutLabel::NewWorktree,
        PrefixAction::Client(A::RequestWorktreeOpen) => ShortcutLabel::OpenWorktree,
        PrefixAction::Client(A::RequestWorkspaceRename) => ShortcutLabel::RenameWorkspace,
        PrefixAction::Client(A::RequestProjectRename) => ShortcutLabel::RenameProject,
        PrefixAction::Client(A::RequestRenameTab) => ShortcutLabel::RenameTab,
        PrefixAction::Client(A::CreateTab) => ShortcutLabel::NewTab,
        PrefixAction::Client(A::RequestCloseTab) => ShortcutLabel::CloseTab,
        PrefixAction::Client(A::MovePreviousTab) => ShortcutLabel::MoveTabLeft,
        PrefixAction::Client(A::MoveNextTab) => ShortcutLabel::MoveTabRight,
        PrefixAction::Client(A::SelectNextTab) => ShortcutLabel::NextTab,
        PrefixAction::Client(A::SelectPreviousTab) => ShortcutLabel::PreviousTab,
        PrefixAction::Client(A::SelectPreviousContext) => ShortcutLabel::PreviousWorkspace,
        PrefixAction::Client(A::SelectNextContext) => ShortcutLabel::NextWorkspace,
        PrefixAction::Client(A::PreviousPresentation) => ShortcutLabel::PreviousTerminal,
        PrefixAction::Client(A::NextPresentation) => ShortcutLabel::NextTerminal,
        PrefixAction::Client(A::ForwardLiteralPrefix) => ShortcutLabel::LiteralPrefix,
        PrefixAction::Client(A::DismissPrefix) => ShortcutLabel::CancelPrefix,
        PrefixAction::Client(A::TogglePointerCapture) => ShortcutLabel::PointerCapture,
        PrefixAction::Client(A::Exit) => ShortcutLabel::Exit,
        PrefixAction::Client(A::RequestControlTakeover) => ShortcutLabel::ControlTakeover,
        PrefixAction::Client(A::OpenMachines) => ShortcutLabel::Machines,
        PrefixAction::Client(A::PreviousAgent) => ShortcutLabel::PreviousAgent,
        PrefixAction::Client(A::NextAgent) => ShortcutLabel::NextAgent,
        PrefixAction::Client(A::ToggleZoom) => ShortcutLabel::ZoomPanel,
        PrefixAction::Client(A::ClosePanel) => ShortcutLabel::ClosePanel,
        PrefixAction::Client(A::ToggleBusinessSidebar) | PrefixAction::SidebarToggle => {
            ShortcutLabel::Sidebar
        }
        PrefixAction::NavigatorToggle | PrefixAction::Client(A::OpenWorkbenchNavigator) => {
            ShortcutLabel::Navigator
        }
        PrefixAction::BoardToggle | PrefixAction::Client(A::ToggleAgentBoard) => {
            ShortcutLabel::AgentBoard
        }
        PrefixAction::ShowHelp | PrefixAction::Client(A::OpenShortcutHelp) => ShortcutLabel::Help,
        PrefixAction::SelectTabDigit => ShortcutLabel::TabNumber,
        PrefixAction::SelectAgentDigit => ShortcutLabel::AgentNumber,
        _ => ShortcutLabel::Other,
    };
    locale.shortcut_label(label)
}

pub(crate) fn shortcut_filter_matches(query: &str, target: &str) -> bool {
    let words: Vec<_> = target
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .all(|term| {
            words.iter().any(|word| {
                word.starts_with(&term.to_lowercase())
                    || (!term.is_ascii() && word.contains(&term.to_lowercase()))
            })
        })
}

fn common_action(action: &PrefixAction) -> bool {
    use ClientAction as A;
    if matches!(action, PrefixAction::Client(A::FocusBusinessSidebar)) {
        return true;
    }
    matches!(
        action,
        PrefixAction::Client(
            A::CreateTab
                | A::RequestQuickWorkspace
                | A::RequestWorkspaceCreate
                | A::SplitPanel(_)
                | A::SelectNextTab
                | A::SelectNextContext
                | A::ToggleZoom
                | A::ToggleBusinessSidebar
                | A::RequestCloseTab
        ) | PrefixAction::SidebarToggle
            | PrefixAction::NavigatorToggle
    )
}

fn merge_help_entry(entries: &mut Vec<ShortcutHelpEntry>, incoming: ShortcutHelpEntry) {
    if let Some(existing) = entries.iter_mut().find(|existing| {
        existing.identity == incoming.identity
            && existing.group == incoming.group
            && existing.title == incoming.title
            && existing.disabled_reason == incoming.disabled_reason
    }) {
        if !existing.key.split(" / ").any(|key| key == incoming.key) {
            existing.key.push_str(" / ");
            existing.key.push_str(&incoming.key);
        }
        existing.search_text.push(' ');
        existing.search_text.push_str(&incoming.search_text);
    } else {
        entries.push(incoming);
    }
}

fn help_group(metadata: ActionMetadata<'_>) -> ShortcutHelpGroup {
    use ActionCategory as C;
    match metadata.category {
        C::Workspace | C::Project => ShortcutHelpGroup::Workspace,
        C::Tab => ShortcutHelpGroup::Tab,
        C::Pane => ShortcutHelpGroup::Panel,
        C::Agent => ShortcutHelpGroup::Agent,
        C::Client => ShortcutHelpGroup::Client,
        C::Navigation => match metadata.search_aliases.first() {
            Some(ActionSearchAlias::Workspace) => ShortcutHelpGroup::Workspace,
            Some(ActionSearchAlias::Tab) => ShortcutHelpGroup::Tab,
            Some(ActionSearchAlias::Agent) => ShortcutHelpGroup::Agent,
            Some(ActionSearchAlias::Pane) => ShortcutHelpGroup::Panel,
            _ => ShortcutHelpGroup::Other,
        },
        C::Application | C::Input => ShortcutHelpGroup::Other,
    }
}

fn identity_key(identity: ActionIdentity<'_>) -> String {
    match identity {
        ActionIdentity::BuiltIn(id) => id.to_owned(),
        ActionIdentity::ConfiguredCommand(id) => format!("command:{id}"),
    }
}

fn help_identity(action: &PrefixAction, identity: ActionIdentity<'_>) -> String {
    if matches!(
        action,
        PrefixAction::Client(
            ClientAction::RequestWorkspaceCreate | ClientAction::RequestQuickWorkspace
        )
    ) {
        "workbench.workspace.create".to_owned()
    } else {
        identity_key(identity)
    }
}

fn semantic_identity(action: PrefixAction) -> String {
    crate::action_projection::binding_metadata(&action)
        .map(|metadata| help_identity(&action, metadata.identity))
        .unwrap_or_else(|| format!("contextual:{action:?}"))
}

fn requires_session(action: &PrefixAction) -> bool {
    use ClientAction as A;
    matches!(
        action,
        PrefixAction::Client(
            A::CreateTab
                | A::SelectNextTab
                | A::SelectPreviousTab
                | A::SelectTab(_)
                | A::SelectPreviousContext
                | A::SelectNextContext
                | A::PreviousPresentation
                | A::NextPresentation
                | A::MovePanelFocus(_)
                | A::ToggleZoom
                | A::SplitPanel(_)
                | A::ClosePanel
                | A::MovePanel(_)
                | A::FocusLastPanel
                | A::RequestRenamePanel
                | A::ClearTerminal
                | A::EditScrollback
                | A::RequestEarlierLayouts
        ) | PrefixAction::SelectTabDigit
    )
}

fn shared_menu_action(binding: &DiscoveryEntry<'_>) -> Option<crate::action_menu::MenuAction> {
    use crate::{
        action_menu::{
            GLOBAL_ACTIONS, MenuAction, PANE_ACTIONS, PROJECT_ACTIONS, TAB_ACTIONS,
            WORKSPACE_ACTIONS,
        },
        action_projection::menu_metadata,
    };

    let preferred = match binding.action {
        PrefixAction::Client(ClientAction::CreateTab) => Some(MenuAction::NewTab),
        PrefixAction::Client(
            ClientAction::RequestWorkspaceCreate | ClientAction::RequestQuickWorkspace,
        ) => Some(MenuAction::NewWorkspace),
        PrefixAction::Client(ClientAction::MovePreviousTab) => Some(MenuAction::MoveTabPrevious),
        PrefixAction::Client(ClientAction::MoveNextTab) => Some(MenuAction::MoveTabNext),
        PrefixAction::Client(ClientAction::RequestCloseTab) => Some(MenuAction::CloseTab),
        _ => None,
    };
    if let Some(preferred) = preferred {
        return Some(preferred);
    }
    let collections: [&[MenuAction]; 5] = [
        GLOBAL_ACTIONS,
        PANE_ACTIONS,
        TAB_ACTIONS,
        WORKSPACE_ACTIONS,
        PROJECT_ACTIONS,
    ];
    collections
        .iter()
        .flat_map(|actions| actions.iter())
        .find(|action| {
            menu_metadata((*action).clone())
                .action
                .same_discovery_entry(&binding.metadata)
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_view::{ClientActionDisposition, ClientView};

    fn view_with_help() -> ClientView {
        let mut view = ClientView::new(100);
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('b'),
                crate::client_view::Modifiers::CTRL,
            ),
        ));
        view.commit(transition, ClientActionDisposition::KeepNavigator);
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('?'),
                crate::client_view::Modifiers::NONE,
            ),
        ));
        view.commit(transition, ClientActionDisposition::KeepNavigator);
        view
    }

    #[test]
    fn help_entries_merge_workspace_creation_aliases_and_localize_groups() {
        let view = view_with_help();
        let entries = super::entries(Locale::EnUs, &view, true, KeyLabelGlyphs::Unicode);
        let workspace = entries
            .iter()
            .filter(|entry| entry.title == "New Workspace")
            .collect::<Vec<_>>();
        assert!(!workspace.is_empty(), "workspace creation is discoverable");
        assert!(workspace.iter().any(|entry| entry.key.contains("Ctrl+b a")));
        {
            assert_eq!(
                workspace.len(),
                1,
                "two input bindings for one workspace operation share a row"
            );
            assert!(workspace[0].key.contains("Ctrl+b Shift+N"));
            assert_eq!(workspace[0].identity, "workbench.workspace.create");
            assert_eq!(
                workspace[0].title,
                Locale::EnUs.menu_action(&crate::action_menu::MenuAction::NewWorkspace)
            );
        }

        let entries_zh = super::entries(Locale::ZhCn, &view, true, KeyLabelGlyphs::Unicode);
        let rename_workspace = entries
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b r"))
            .expect("Workspace rename keeps its owning binding");
        let rename_project = entries
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b Shift+R"))
            .expect("Project rename keeps its owning binding");
        assert_ne!(rename_workspace.identity, rename_project.identity);
        let rename_workspace_zh = entries_zh
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b r"))
            .expect("localized Workspace rename keeps its owning binding");
        let rename_project_zh = entries_zh
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b Shift+R"))
            .expect("localized Project rename keeps its owning binding");
        assert_eq!(rename_workspace.identity, rename_workspace_zh.identity);
        assert_eq!(rename_project.identity, rename_project_zh.identity);
        {
            assert_eq!(
                rename_workspace.title,
                Locale::EnUs.menu_action(&crate::action_menu::MenuAction::RenameWorkspace)
            );
            assert_eq!(
                rename_project.title,
                Locale::EnUs.menu_action(&crate::action_menu::MenuAction::RenameProject)
            );
            assert_eq!(
                rename_workspace_zh.title,
                Locale::ZhCn.menu_action(&crate::action_menu::MenuAction::RenameWorkspace)
            );
            assert_eq!(
                rename_project_zh.title,
                Locale::ZhCn.menu_action(&crate::action_menu::MenuAction::RenameProject)
            );
        }

        assert!(!entries.iter().any(|entry| entry.key.contains("Unbound")));
        assert!(entries.iter().all(|entry| entry.title != "Shortcut"));
        let workspace_zh = entries_zh
            .iter()
            .filter(|entry| entry.title == "新建工作区")
            .collect::<Vec<_>>();
        assert_eq!(workspace_zh.len(), 1);
        assert!(workspace_zh[0].key.contains("Ctrl+b a"));
        {
            assert!(workspace_zh[0].key.contains("Ctrl+b Shift+N"));
            assert_eq!(
                workspace_zh[0].title,
                Locale::ZhCn.menu_action(&crate::action_menu::MenuAction::NewWorkspace)
            );
        }
        let create_tab = entries
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b c"))
            .expect("Tab creation keeps its semantic name");
        assert_eq!(create_tab.title, "New Tab");
        let create_tab_zh = entries_zh
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b c"))
            .expect("localized Tab creation keeps its semantic name");
        assert_eq!(create_tab_zh.title, "新建标签页");
        {
            assert_eq!(
                create_tab.title,
                Locale::EnUs.menu_action(&crate::action_menu::MenuAction::NewTab)
            );
            assert_eq!(
                create_tab_zh.title,
                Locale::ZhCn.menu_action(&crate::action_menu::MenuAction::NewTab)
            );
            for entries in [&entries, &entries_zh] {
                assert!(
                    entries.iter().any(|entry| entry.key.contains("Shift+B")),
                    "sidebar focus keeps its owning binding in either locale"
                );
            }
        }
        assert!(
            entries
                .iter()
                .any(|entry| entry.group == ShortcutHelpGroup::Workspace)
        );
        assert_eq!(
            Locale::ZhCn.shortcut_group_name(ShortcutHelpGroup::Workspace),
            "工作区"
        );
    }

    #[test]
    fn help_entry_merge_keeps_same_titles_for_distinct_action_identities() {
        let actions = [
            PrefixAction::Client(ClientAction::RequestWorkspaceRename),
            PrefixAction::Client(ClientAction::RequestProjectRename),
        ];
        let mut entries = Vec::new();
        for (key, action) in ["Ctrl+b r", "Ctrl+b Shift+R"].into_iter().zip(actions) {
            let metadata = crate::action_projection::binding_metadata(&action)
                .expect("rename actions have semantic identities");
            merge_help_entry(
                &mut entries,
                ShortcutHelpEntry {
                    key: key.to_owned(),
                    title: "Rename".to_owned(),
                    group: ShortcutHelpGroup::Workspace,
                    search_text: "Rename workspace project".to_owned(),
                    disabled_reason: None,
                    identity: identity_key(metadata.identity),
                },
            );
        }
        assert_eq!(entries.len(), 2);
        assert_ne!(entries[0].identity, entries[1].identity);
        assert!(entries.iter().any(|entry| entry.key == "Ctrl+b r"));
        assert!(entries.iter().any(|entry| entry.key == "Ctrl+b Shift+R"));
    }

    #[test]
    fn multiple_bindings_merge_when_their_semantic_identity_matches() {
        use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

        let chord = |key: &str| KeyChord {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
        };
        let mut view = ClientView::new(100);
        view.configure_keys(&KeySettings {
            prefix: KeyChord {
                key: "b".into(),
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
            },
            overrides: vec![KeyOverride {
                action: "new_workspace",
                triggers: vec![
                    KeyTrigger {
                        prefix: true,
                        chord: chord("x"),
                    },
                    KeyTrigger {
                        prefix: true,
                        chord: chord("z"),
                    },
                ],
            }],
            commands: Vec::new(),
        });
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('b'),
                crate::client_view::Modifiers::CTRL,
            ),
        ));
        view.commit(transition, ClientActionDisposition::KeepNavigator);
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('?'),
                crate::client_view::Modifiers::NONE,
            ),
        ));
        view.commit(transition, ClientActionDisposition::KeepNavigator);

        let entries = super::entries(Locale::EnUs, &view, true, KeyLabelGlyphs::Unicode);
        let workspace = entries
            .iter()
            .filter(|entry| entry.title == "New Workspace")
            .collect::<Vec<_>>();
        assert_eq!(workspace.len(), 1);
        assert!(workspace[0].key.contains("Ctrl+b x"));
        assert!(workspace[0].key.contains("Ctrl+b z"));
        assert!(workspace[0].key.contains("Ctrl+b Shift+N"));
        assert!(!workspace[0].key.contains("Ctrl+b a"));
        assert_eq!(workspace[0].identity, "workbench.workspace.create");
    }

    #[test]
    fn active_local_modes_contribute_their_owner_bindings_without_exit_keys() {
        fn press(
            view: &mut ClientView,
            code: crate::client_view::KeyCode,
            mods: crate::client_view::Modifiers,
        ) {
            let transition = view.decide(crate::client_view::PrefixInput::Key(
                crate::client_view::KeyEvent::press(code, mods),
            ));
            view.commit(transition, ClientActionDisposition::KeepNavigator);
        }

        fn open_help(view: &mut ClientView) {
            use crate::client_view::{KeyCode, Modifiers};
            press(view, KeyCode::Char('b'), Modifiers::CTRL);
            press(view, KeyCode::Char('?'), Modifiers::NONE);
        }

        use crate::{
            client_view::{KeyCode, KeyLabelGlyphs, Modifiers},
            machine_picker::{SidebarCommand, sidebar_binding_label},
        };

        let mut copy = ClientView::new(100);
        copy.settle_copy_mode(true);
        open_help(&mut copy);
        assert!(copy.copy_mode_active());
        assert_eq!(
            copy.overlay(),
            Some(crate::client_view::Overlay::ShortcutHelp)
        );
        let copy_rows = copy.copy_help_rows();
        let copy_entries = super::entries(Locale::EnUs, &copy, true, KeyLabelGlyphs::Unicode);
        let move_key = copy_rows
            .iter()
            .find(|(_, description)| *description == PrefixDesc::CopyMove)
            .unwrap()
            .0
            .as_str();
        assert!(copy_entries.iter().any(|entry| entry.key == move_key));
        assert!(!copy_entries.iter().any(|entry| entry.key == "Esc"));
        let copy_entries_zh = super::entries(Locale::ZhCn, &copy, true, KeyLabelGlyphs::Unicode);
        assert!(
            copy_entries_zh
                .iter()
                .any(|entry| { entry.key == move_key && entry.title == "移动复制光标" })
        );

        let mut resize = ClientView::new(100);
        press(&mut resize, KeyCode::Char('b'), Modifiers::CTRL);
        press(&mut resize, KeyCode::Char('P'), Modifiers::NONE);
        press(&mut resize, KeyCode::Char('?'), Modifiers::NONE);
        assert!(resize.resize_mode());
        let resize_rows = resize.resize_help_rows_with_glyphs(KeyLabelGlyphs::Unicode);
        let resize_entries = super::entries(Locale::EnUs, &resize, true, KeyLabelGlyphs::Unicode);
        let coarse_key = resize_rows
            .iter()
            .find(|(_, description)| *description == PrefixDesc::ResizeCoarse)
            .unwrap()
            .0
            .as_str();
        assert!(resize_entries.iter().any(|entry| entry.key == coarse_key));
        assert!(!resize_entries.iter().any(|entry| entry.key == "Esc"));

        let mut sidebar = ClientView::new(100);
        sidebar.machines.available = true;
        assert!(sidebar.machines.enter_sidebar_navigation());
        open_help(&mut sidebar);
        assert!(sidebar.machines.sidebar_navigation_active());
        let sidebar_entries = super::entries(Locale::EnUs, &sidebar, true, KeyLabelGlyphs::Unicode);
        let search_key =
            sidebar_binding_label(SidebarCommand::Search, KeyLabelGlyphs::Unicode).unwrap();
        assert!(sidebar_entries.iter().any(|entry| entry.key == search_key));
        assert!(!sidebar_entries.iter().any(|entry| entry.key == "Esc"));
        let help_key =
            sidebar_binding_label(SidebarCommand::Help, KeyLabelGlyphs::Unicode).unwrap();
        assert!(sidebar_entries.iter().any(|entry| {
            entry.key.split(" / ").any(|key| key == help_key)
                && entry.title == Locale::EnUs.shortcut_close_label()
        }));
    }

    #[test]
    fn shared_machine_action_uses_the_action_menu_name() {
        let view = view_with_help();
        let entry = super::entries(Locale::EnUs, &view, true, KeyLabelGlyphs::Unicode)
            .into_iter()
            .find(|entry| {
                entry.title == Locale::EnUs.menu_action(&crate::action_menu::MenuAction::Machines)
            })
            .expect("machine discovery should reuse its menu action name");
        assert!(entry.key.contains("Shift+M"));
    }

    #[test]
    fn disconnected_session_actions_explain_their_unavailability() {
        let view = view_with_help();
        let entries = super::entries(Locale::EnUs, &view, false, KeyLabelGlyphs::Unicode);
        let create_tab = entries
            .iter()
            .find(|entry| entry.key.contains("Ctrl+b c"))
            .expect("Tab creation remains discoverable");
        assert_eq!(
            create_tab.disabled_reason.as_deref(),
            Some("Connect to an Orchbee session")
        );
        assert!(create_tab.search_text.contains("Orchbee session"));
    }

    #[test]
    fn shortcut_filter_matches_fuzzy_names_keys_and_multiple_terms() {
        assert!(shortcut_filter_matches(
            "new",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(shortcut_filter_matches(
            "ctrl b",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(shortcut_filter_matches(
            "workspace ne",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(!shortcut_filter_matches(
            "panel",
            "New Workspace Ctrl+b a workspace"
        ));
    }
}
