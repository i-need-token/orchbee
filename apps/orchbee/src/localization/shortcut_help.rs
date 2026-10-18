use super::Locale;
use crate::{
    action_projection::{ActionCategory, ActionIdentity, ActionMetadata, ActionSearchAlias},
    client_view::{
        ClientAction, ClientView, DiscoveryEntry, KeyLabelGlyphs, PanelDirection, PrefixAction,
    },
    terminal_text::sanitize_terminal_text,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ShortcutHelpGroup {
    Workspace,
    Tab,
    Panel,
    Agent,
    Session,
    Other,
}

impl ShortcutHelpGroup {
    const fn order(self) -> u8 {
        match self {
            Self::Workspace => 0,
            Self::Tab => 1,
            Self::Panel => 2,
            Self::Agent => 3,
            Self::Session => 4,
            Self::Other => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutHelpEntry {
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) group: ShortcutHelpGroup,
    pub(crate) search_text: String,
    pub(crate) disabled_reason: Option<String>,
    identity: String,
    custom: bool,
}

impl Locale {
    pub(crate) fn shortcut_help_entries(
        self,
        view: &ClientView,
        session_available: bool,
        glyphs: KeyLabelGlyphs,
    ) -> Vec<ShortcutHelpEntry> {
        let mut entries: Vec<ShortcutHelpEntry> = Vec::new();
        for binding in view.bindings().discovery_entries(glyphs) {
            if matches!(
                binding.action,
                PrefixAction::HelpPagePrevious | PrefixAction::HelpPageNext
            ) {
                continue;
            }
            let group = help_group(binding.metadata);
            let title = self.shortcut_action_title(view, &binding);
            let aliases = binding
                .metadata
                .search_aliases
                .iter()
                .map(|alias| self.shortcut_search_alias(*alias))
                .collect::<Vec<_>>()
                .join(" ");
            let disabled_reason = (!session_available && requires_session(&binding.action))
                .then(|| self.shortcut_requires_session().to_owned());
            let search_text = format!(
                "{} {} {} {} {}",
                title,
                binding.key,
                aliases,
                self.shortcut_group_name(group),
                disabled_reason.as_deref().unwrap_or_default()
            );
            if let Some(existing) = entries.iter_mut().find(|existing| {
                existing.group == group
                    && existing.title == title
                    && existing.disabled_reason == disabled_reason
                    && if existing.custom {
                        existing.identity == identity_key(binding.metadata.identity)
                    } else {
                        !is_configured_action(binding.metadata)
                    }
            }) {
                if !existing.key.split(" / ").any(|key| key == binding.key) {
                    existing.key.push_str(" / ");
                    existing.key.push_str(&binding.key);
                }
                existing.search_text.push(' ');
                existing.search_text.push_str(&search_text);
            } else {
                entries.push(ShortcutHelpEntry {
                    key: binding.key,
                    title,
                    group,
                    search_text,
                    disabled_reason,
                    identity: identity_key(binding.metadata.identity),
                    custom: is_configured_action(binding.metadata),
                });
            }
        }
        entries.sort_by_key(|entry| entry.group.order());
        entries
    }

    pub(crate) const fn shortcut_group_name(self, group: ShortcutHelpGroup) -> &'static str {
        match (self, group) {
            (Self::EnUs, ShortcutHelpGroup::Workspace) => "Workspace",
            (Self::ZhCn, ShortcutHelpGroup::Workspace) => "工作区",
            (Self::EnUs, ShortcutHelpGroup::Tab) => "Tab",
            (Self::ZhCn, ShortcutHelpGroup::Tab) => "标签页",
            (Self::EnUs, ShortcutHelpGroup::Panel) => "Panel",
            (Self::ZhCn, ShortcutHelpGroup::Panel) => "面板",
            (Self::EnUs, ShortcutHelpGroup::Agent) => "Agent",
            (Self::ZhCn, ShortcutHelpGroup::Agent) => "Agent",
            (Self::EnUs, ShortcutHelpGroup::Session) => "Session",
            (Self::ZhCn, ShortcutHelpGroup::Session) => "会话",
            (Self::EnUs, ShortcutHelpGroup::Other) => "Other",
            (Self::ZhCn, ShortcutHelpGroup::Other) => "其他",
        }
    }

    pub(crate) const fn shortcut_search_label(self) -> &'static str {
        match self {
            Self::EnUs => "Filter",
            Self::ZhCn => "筛选",
        }
    }

    pub(crate) const fn shortcut_filter_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Type to filter · Backspace erase · Ctrl+U clear",
            Self::ZhCn => "输入筛选 · Backspace 删除 · Ctrl+U 清空",
        }
    }

    pub(crate) const fn shortcut_scroll_hint(self) -> &'static str {
        match self {
            Self::EnUs => "↑/↓ scroll · PgUp/PgDn page · Esc close",
            Self::ZhCn => "↑/↓ 滚动 · PgUp/PgDn 翻屏 · Esc 关闭",
        }
    }

    pub(crate) const fn shortcut_no_matches(self) -> &'static str {
        match self {
            Self::EnUs => "No matching shortcuts",
            Self::ZhCn => "没有匹配的快捷键",
        }
    }

    pub(crate) const fn shortcut_requires_session(self) -> &'static str {
        match self {
            Self::EnUs => "Connect to an Orchbee session",
            Self::ZhCn => "请先连接 Orchbee 会话",
        }
    }

    fn shortcut_action_title(self, view: &ClientView, binding: &DiscoveryEntry<'_>) -> String {
        #[cfg(windows)]
        if let Some(description) = view.bindings().configured_description(binding.description) {
            return sanitize_terminal_text(description).into_owned();
        }
        #[cfg(not(windows))]
        let _ = (view, binding.description);

        #[cfg(windows)]
        if let Some(action) = shared_menu_action(binding) {
            return self.menu_action(&action).to_owned();
        }

        self.shortcut_local_action_title(&binding.action)
    }

    fn shortcut_local_action_title(self, action: &PrefixAction) -> String {
        use ClientAction as A;
        let title = match action {
            PrefixAction::Client(A::OpenSettings) => Some(match self {
                Self::EnUs => "Settings",
                Self::ZhCn => "设置",
            }),
            PrefixAction::Client(A::ReloadConfiguration) => Some(match self {
                Self::EnUs => "Reload settings",
                Self::ZhCn => "重新加载设置",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::OpenActionMenu) => Some(match self {
                Self::EnUs => "Open action menu",
                Self::ZhCn => "打开动作菜单",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::EnterResizeMode | A::ResizePane { .. }) => Some(match self {
                Self::EnUs => "Resize panel",
                Self::ZhCn => "调整面板大小",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::EnterCopyMode) => Some(match self {
                Self::EnUs => "Copy text",
                Self::ZhCn => "复制文本",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::ConfiguredCommand(_)) => Some(match self {
                Self::EnUs => "Run configured command",
                Self::ZhCn => "运行已配置的命令",
            }),
            PrefixAction::Client(A::RequestWorkspaceClose) => Some(match self {
                Self::EnUs => "Close Workspace",
                Self::ZhCn => "关闭工作区",
            }),
            PrefixAction::Client(A::RequestWorkspaceCreate | A::RequestQuickWorkspace) => {
                Some(match self {
                    Self::EnUs => "New Workspace",
                    Self::ZhCn => "新建工作区",
                })
            }
            PrefixAction::Client(A::RequestWorkspaceAtDirectory(_)) => Some(match self {
                Self::EnUs => "New Workspace in directory",
                Self::ZhCn => "在指定目录新建工作区",
            }),
            PrefixAction::Client(A::RequestWorkspaceDirectory) => Some(match self {
                Self::EnUs => "Set Workspace directory",
                Self::ZhCn => "设置工作区目录",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::RequestWorktreeCreate) => Some(match self {
                Self::EnUs => "New Worktree",
                Self::ZhCn => "新建 Worktree",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::RequestWorktreeOpen) => Some(match self {
                Self::EnUs => "Open Worktree",
                Self::ZhCn => "打开 Worktree",
            }),
            PrefixAction::Client(A::RequestWorkspaceRename) => Some(match self {
                Self::EnUs => "Rename Workspace",
                Self::ZhCn => "重命名工作区",
            }),
            PrefixAction::Client(A::RequestProjectRename) => Some(match self {
                Self::EnUs => "Rename Project",
                Self::ZhCn => "重命名项目",
            }),
            PrefixAction::Client(A::RequestRenameTab) => Some(match self {
                Self::EnUs => "Rename Tab",
                Self::ZhCn => "重命名标签页",
            }),
            PrefixAction::Client(A::RequestCloseTab) => Some(match self {
                Self::EnUs => "Close Tab",
                Self::ZhCn => "关闭标签页",
            }),
            PrefixAction::Client(A::MovePreviousTab) => Some(match self {
                Self::EnUs => "Move Tab left",
                Self::ZhCn => "向左移动标签页",
            }),
            PrefixAction::Client(A::MoveNextTab) => Some(match self {
                Self::EnUs => "Move Tab right",
                Self::ZhCn => "向右移动标签页",
            }),
            PrefixAction::Client(A::SelectNextTab) => Some(match self {
                Self::EnUs => "Select next Tab",
                Self::ZhCn => "选择下一个标签页",
            }),
            PrefixAction::Client(A::SelectPreviousTab) => Some(match self {
                Self::EnUs => "Select previous Tab",
                Self::ZhCn => "选择上一个标签页",
            }),
            PrefixAction::Client(A::SelectPreviousContext) => Some(match self {
                Self::EnUs => "Select previous Workspace",
                Self::ZhCn => "选择上一个工作区",
            }),
            PrefixAction::Client(A::SelectNextContext) => Some(match self {
                Self::EnUs => "Select next Workspace",
                Self::ZhCn => "选择下一个工作区",
            }),
            PrefixAction::Client(A::MovePanelFocus(direction)) => {
                Some(self.panel_direction_title("Focus panel", "聚焦面板", *direction))
            }
            PrefixAction::Client(A::PreviousPresentation) => Some(match self {
                Self::EnUs => "Select previous terminal",
                Self::ZhCn => "选择上一个终端",
            }),
            PrefixAction::Client(A::NextPresentation) => Some(match self {
                Self::EnUs => "Select next terminal",
                Self::ZhCn => "选择下一个终端",
            }),
            PrefixAction::Client(A::SplitPanel(direction)) => {
                Some(self.panel_direction_title("Split panel", "分割面板", *direction))
            }
            PrefixAction::Client(A::MovePanel(direction)) => {
                Some(self.panel_direction_title("Move panel", "移动面板", *direction))
            }
            PrefixAction::Client(A::ForwardLiteralPrefix) => Some(match self {
                Self::EnUs => "Send prefix to terminal",
                Self::ZhCn => "向终端发送前缀",
            }),
            PrefixAction::Client(A::DismissPrefix) => Some(match self {
                Self::EnUs => "Cancel prefix",
                Self::ZhCn => "取消前缀",
            }),
            PrefixAction::Client(A::TogglePointerCapture) => Some(match self {
                Self::EnUs => "Toggle mouse capture",
                Self::ZhCn => "切换鼠标捕获",
            }),
            PrefixAction::Client(A::Exit) => Some(match self {
                Self::EnUs => "Detach",
                Self::ZhCn => "分离",
            }),
            PrefixAction::Client(A::RequestControlTakeover) => Some(match self {
                Self::EnUs => "Take control of this panel",
                Self::ZhCn => "接管此面板",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::OpenMachines) => Some(match self {
                Self::EnUs => "Search machines, Workspaces and Agents",
                Self::ZhCn => "搜索机器、工作区和 Agent",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::PreviousAgent) => Some(match self {
                Self::EnUs => "Select previous Agent",
                Self::ZhCn => "选择上一个 Agent",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::NextAgent) => Some(match self {
                Self::EnUs => "Select next Agent",
                Self::ZhCn => "选择下一个 Agent",
            }),
            PrefixAction::Client(A::ToggleBusinessSidebar) | PrefixAction::SidebarToggle => {
                Some(match self {
                    Self::EnUs => "Toggle sidebar",
                    Self::ZhCn => "切换侧栏",
                })
            }
            PrefixAction::NavigatorToggle | PrefixAction::Client(A::OpenWorkbenchNavigator) => {
                Some(match self {
                    Self::EnUs => "Workspace navigator",
                    Self::ZhCn => "工作区导航器",
                })
            }
            PrefixAction::BoardToggle | PrefixAction::Client(A::ToggleAgentBoard) => {
                Some(match self {
                    Self::EnUs => "Agent board",
                    Self::ZhCn => "Agent 看板",
                })
            }
            PrefixAction::ShowHelp | PrefixAction::Client(A::OpenShortcutHelp) => {
                Some(match self {
                    Self::EnUs => "Shortcut help",
                    Self::ZhCn => "快捷键帮助",
                })
            }
            PrefixAction::Client(A::ToggleZoom) => Some(match self {
                Self::EnUs => "Zoom panel",
                Self::ZhCn => "缩放面板",
            }),
            PrefixAction::Client(A::ClosePanel) => Some(match self {
                Self::EnUs => "Close panel",
                Self::ZhCn => "关闭面板",
            }),
            PrefixAction::SelectTabDigit => Some(match self {
                Self::EnUs => "Select Tab by number",
                Self::ZhCn => "按编号选择标签页",
            }),
            #[cfg(windows)]
            PrefixAction::SelectAgentDigit => Some(match self {
                Self::EnUs => "Select Agent by number",
                Self::ZhCn => "按编号选择 Agent",
            }),
            _ => None,
        };
        title.map(str::to_owned).unwrap_or_else(|| {
            match self {
                Self::EnUs => "Shortcut",
                Self::ZhCn => "快捷键",
            }
            .to_owned()
        })
    }

    fn panel_direction_title(
        self,
        english: &'static str,
        chinese: &'static str,
        direction: PanelDirection,
    ) -> String {
        let (verb, direction) = match (self, direction) {
            (Self::EnUs, PanelDirection::Left) => (english, "left"),
            (Self::EnUs, PanelDirection::Down) => (english, "down"),
            (Self::EnUs, PanelDirection::Up) => (english, "up"),
            (Self::EnUs, PanelDirection::Right) => (english, "right"),
            (Self::ZhCn, PanelDirection::Left) => (chinese, "向左"),
            (Self::ZhCn, PanelDirection::Down) => (chinese, "向下"),
            (Self::ZhCn, PanelDirection::Up) => (chinese, "向上"),
            (Self::ZhCn, PanelDirection::Right) => (chinese, "向右"),
        };
        match self {
            Self::EnUs => format!("{verb} {direction}"),
            Self::ZhCn => format!("{direction}{verb}"),
        }
    }

    fn shortcut_search_alias(self, alias: ActionSearchAlias) -> &'static str {
        use ActionSearchAlias as A;
        match (self, alias) {
            #[cfg(windows)]
            (Self::EnUs, A::Actions) => "actions",
            #[cfg(windows)]
            (Self::ZhCn, A::Actions) => "动作",
            #[cfg(windows)]
            (Self::EnUs, A::Commands) => "commands",
            #[cfg(windows)]
            (Self::ZhCn, A::Commands) => "命令",
            (Self::EnUs, A::Preferences) => "preferences",
            (Self::ZhCn, A::Preferences) => "偏好设置",
            (Self::EnUs, A::Workspace) => "workspace",
            (Self::ZhCn, A::Workspace) => "工作区",
            #[cfg(windows)]
            (Self::EnUs, A::Worktree) => "worktree",
            #[cfg(windows)]
            (Self::ZhCn, A::Worktree) => "Worktree",
            (Self::EnUs, A::Project) => "project",
            (Self::ZhCn, A::Project) => "项目",
            (Self::EnUs, A::Tab) => "tab",
            (Self::ZhCn, A::Tab) => "标签页",
            (Self::EnUs, A::Pane) => "pane panel",
            (Self::ZhCn, A::Pane) => "面板",
            (Self::EnUs, A::Split) => "split",
            (Self::ZhCn, A::Split) => "分屏",
            #[cfg(windows)]
            (Self::EnUs, A::Resize) => "resize",
            #[cfg(windows)]
            (Self::ZhCn, A::Resize) => "调整大小",
            #[cfg(windows)]
            (Self::EnUs, A::Copy) => "copy",
            #[cfg(windows)]
            (Self::ZhCn, A::Copy) => "复制",
            (Self::EnUs, A::Close) => "close",
            (Self::ZhCn, A::Close) => "关闭",
            (Self::EnUs, A::Rename) => "rename",
            (Self::ZhCn, A::Rename) => "重命名",
            (Self::EnUs, A::Move) => "move",
            (Self::ZhCn, A::Move) => "移动",
            (Self::EnUs, A::Select) => "select",
            (Self::ZhCn, A::Select) => "选择",
            (Self::EnUs, A::Sidebar) => "sidebar",
            (Self::ZhCn, A::Sidebar) => "侧栏",
            (Self::EnUs, A::Navigator) => "navigate",
            (Self::ZhCn, A::Navigator) => "导航",
            (Self::EnUs, A::Agent) => "agent",
            (Self::ZhCn, A::Agent) => "Agent",
            (Self::EnUs, A::Help) => "help",
            (Self::ZhCn, A::Help) => "帮助",
            (Self::EnUs, A::Mouse) => "mouse pointer",
            (Self::ZhCn, A::Mouse) => "鼠标指针",
            (Self::EnUs, A::Detach) => "detach",
            (Self::ZhCn, A::Detach) => "分离",
        }
    }
}

pub(crate) fn shortcut_filter_matches(query: &str, target: &str) -> bool {
    let target = searchable(target);
    query
        .split_whitespace()
        .map(searchable)
        .filter(|term| !term.is_empty())
        .all(|term| fuzzy_subsequence(&term, &target))
}

fn searchable(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn fuzzy_subsequence(query: &str, target: &str) -> bool {
    let mut query = query.chars();
    let Some(mut expected) = query.next() else {
        return true;
    };
    for character in target.chars() {
        if character == expected {
            match query.next() {
                Some(next) => expected = next,
                None => return true,
            }
        }
    }
    false
}

fn help_group(metadata: ActionMetadata<'_>) -> ShortcutHelpGroup {
    use ActionCategory as C;
    match metadata.category {
        C::Workspace | C::Project => ShortcutHelpGroup::Workspace,
        C::Tab => ShortcutHelpGroup::Tab,
        C::Pane => ShortcutHelpGroup::Panel,
        C::Agent => ShortcutHelpGroup::Agent,
        C::Session => ShortcutHelpGroup::Session,
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

fn is_configured_action(metadata: ActionMetadata<'_>) -> bool {
    #[cfg(windows)]
    {
        matches!(metadata.identity, ActionIdentity::ConfiguredCommand(_))
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn identity_key(identity: ActionIdentity<'_>) -> String {
    match identity {
        ActionIdentity::BuiltIn(id) => id.to_owned(),
        #[cfg(windows)]
        ActionIdentity::ConfiguredCommand(id) => format!("command:{id}"),
    }
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
        ) | PrefixAction::SelectTabDigit
    )
}

#[cfg(windows)]
fn shared_menu_action(binding: &DiscoveryEntry<'_>) -> Option<crate::action_menu::MenuAction> {
    use crate::{
        action_menu::{
            GLOBAL_ACTIONS, LEGACY_WORKSPACE_ACTIONS, MenuAction, PANE_ACTIONS, PROJECT_ACTIONS,
            TAB_ACTIONS, WORKSPACE_ACTIONS,
        },
        action_projection::menu_metadata,
    };

    let preferred = match binding.action {
        PrefixAction::Client(ClientAction::MovePreviousTab) => Some(MenuAction::MoveTabPrevious),
        PrefixAction::Client(ClientAction::MoveNextTab) => Some(MenuAction::MoveTabNext),
        _ => None,
    };
    if let Some(preferred) = preferred {
        return Some(preferred);
    }
    let collections: [&[MenuAction]; 6] = [
        GLOBAL_ACTIONS,
        PANE_ACTIONS,
        TAB_ACTIONS,
        WORKSPACE_ACTIONS,
        PROJECT_ACTIONS,
        LEGACY_WORKSPACE_ACTIONS,
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
    use crate::client_view::{ClientView, SessionActionDisposition};

    fn view_with_help() -> ClientView {
        let mut view = ClientView::new(100);
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('b'),
                crate::client_view::Modifiers::CTRL,
            ),
        ));
        view.commit(transition, SessionActionDisposition::KeepNavigator);
        let transition = view.decide(crate::client_view::PrefixInput::Key(
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('?'),
                crate::client_view::Modifiers::NONE,
            ),
        ));
        view.commit(transition, SessionActionDisposition::KeepNavigator);
        view
    }

    #[test]
    fn help_entries_merge_workspace_aliases_hide_unbound_and_localize_groups() {
        let view = view_with_help();
        let entries = Locale::EnUs.shortcut_help_entries(&view, true, KeyLabelGlyphs::Unicode);
        let workspace = entries
            .iter()
            .find(|entry| entry.title == "New Workspace")
            .expect("workspace creation is discoverable");
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.title == "New Workspace")
                .count(),
            1,
            "workspace aliases produce one semantic help row"
        );
        assert!(workspace.key.contains("Ctrl+b a"), "{}", workspace.key);
        assert!(workspace.key.contains("Shift+N"), "{}", workspace.key);
        assert!(!entries.iter().any(|entry| entry.key.contains("Unbound")));
        assert!(entries.iter().all(|entry| entry.title != "Shortcut"));
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

    #[cfg(windows)]
    #[test]
    fn shared_machine_action_uses_the_action_menu_name() {
        let view = view_with_help();
        let entry = Locale::EnUs
            .shortcut_help_entries(&view, true, KeyLabelGlyphs::Unicode)
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
        let entries = Locale::EnUs.shortcut_help_entries(&view, false, KeyLabelGlyphs::Unicode);
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
            "nwb",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(shortcut_filter_matches(
            "ctrl b",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(shortcut_filter_matches(
            "workspace nw",
            "New Workspace Ctrl+b a workspace"
        ));
        assert!(!shortcut_filter_matches(
            "panel",
            "New Workspace Ctrl+b a workspace"
        ));
    }
}
