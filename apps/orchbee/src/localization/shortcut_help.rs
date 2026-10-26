use super::Locale;
use crate::{
    action_projection::{ActionCategory, ActionIdentity, ActionMetadata, ActionSearchAlias},
    client_view::{
        ClientAction, ClientView, DiscoveryEntry, KeyLabelGlyphs, Overlay, PanelDirection,
        PrefixAction,
    },
    terminal_text::sanitize_terminal_text,
};

#[cfg(windows)]
use crate::client_view::{LabelStyle, PrefixDesc, format_binding_with_glyphs};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ShortcutHelpGroup {
    Common,
    Workspace,
    Tab,
    Panel,
    Agent,
    Client,
    Other,
}

impl ShortcutHelpGroup {
    pub(crate) const fn order(self) -> u8 {
        match self {
            Self::Common => 0,
            Self::Workspace => 1,
            Self::Tab => 2,
            Self::Panel => 3,
            Self::Agent => 4,
            Self::Client => 5,
            Self::Other => 6,
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
}

impl ShortcutHelpEntry {
    pub(crate) fn common(&self) -> bool {
        self.group == ShortcutHelpGroup::Common || self.identity.starts_with("context.")
    }
}

impl Locale {
    #[cfg(windows)]
    pub(crate) fn sidebar_marker_label(self, marker: &str) -> &'static str {
        match (self, marker) {
            (Self::EnUs, "workspace") => "Workspace",
            (Self::ZhCn, "workspace") => "工作区",
            (Self::EnUs, "current") => "Current",
            (Self::ZhCn, "current") => "当前",
            (Self::EnUs, "unread") => "Unseen change",
            (Self::ZhCn, "unread") => "有未读变化",
            (Self::EnUs, "expanded") => "Expanded",
            (Self::ZhCn, "expanded") => "已展开",
            (Self::EnUs, "collapsed") => "Collapsed",
            (Self::ZhCn, "collapsed") => "已折叠",
            (Self::EnUs, "help") => "F1: symbol legend",
            (Self::ZhCn, "help") => "F1：符号说明",
            (Self::EnUs, _) => "Sidebar symbols",
            (Self::ZhCn, _) => "侧栏符号",
        }
    }
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
            let group = if common_action(&binding.action) {
                ShortcutHelpGroup::Common
            } else {
                help_group(binding.metadata)
            };
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
        #[cfg(windows)]
        self.append_contextual_entries(view, glyphs, &mut entries);
        entries.sort_by_key(|entry| entry.group.order());
        entries
    }

    pub(crate) const fn shortcut_group_name(self, group: ShortcutHelpGroup) -> &'static str {
        match (self, group) {
            (Self::EnUs, ShortcutHelpGroup::Common) => "Common tasks",
            (Self::ZhCn, ShortcutHelpGroup::Common) => "常用操作",
            (Self::EnUs, ShortcutHelpGroup::Workspace) => "Workspace",
            (Self::ZhCn, ShortcutHelpGroup::Workspace) => "工作区",
            (Self::EnUs, ShortcutHelpGroup::Tab) => "Tab",
            (Self::ZhCn, ShortcutHelpGroup::Tab) => "标签页",
            (Self::EnUs, ShortcutHelpGroup::Panel) => "Panel",
            (Self::ZhCn, ShortcutHelpGroup::Panel) => "面板",
            (Self::EnUs, ShortcutHelpGroup::Agent) => "Agent",
            (Self::ZhCn, ShortcutHelpGroup::Agent) => "智能体",
            (Self::EnUs, ShortcutHelpGroup::Client) => "Client",
            (Self::ZhCn, ShortcutHelpGroup::Client) => "客户端",
            (Self::EnUs, ShortcutHelpGroup::Other) => "Other",
            (Self::ZhCn, ShortcutHelpGroup::Other) => "其他",
        }
    }

    pub(crate) const fn shortcut_scope_label(self, expanded: bool) -> &'static str {
        match (self, expanded) {
            (Self::EnUs, false) => "[Tab: All + symbols]",
            (Self::ZhCn, false) => "[标签页: 全部及符号]",
            (Self::EnUs, true) => "[Tab: Common]",
            (Self::ZhCn, true) => "[标签页: 常用]",
        }
    }
    pub(crate) const fn shortcut_buttons_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Buttons: </> page · x close",
            Self::ZhCn => "按钮：</> 翻页 · x 关闭",
        }
    }
    pub(crate) const fn shortcut_search_label(self) -> &'static str {
        match self {
            Self::EnUs => "Filter",
            Self::ZhCn => "筛选",
        }
    }

    pub(crate) const fn shortcut_close_action(self) -> &'static str {
        match self {
            Self::EnUs => "close",
            Self::ZhCn => "关闭",
        }
    }

    pub(crate) const fn shortcut_scroll_action(self) -> &'static str {
        match self {
            Self::EnUs => "scroll",
            Self::ZhCn => "滚动",
        }
    }

    pub(crate) const fn shortcut_page_action(self) -> &'static str {
        match self {
            Self::EnUs => "page",
            Self::ZhCn => "翻屏",
        }
    }

    pub(crate) const fn shortcut_erase_action(self) -> &'static str {
        match self {
            Self::EnUs => "erase",
            Self::ZhCn => "删除",
        }
    }

    pub(crate) const fn shortcut_clear_action(self) -> &'static str {
        match self {
            Self::EnUs => "clear",
            Self::ZhCn => "清空",
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
        if view.overlay() == Some(Overlay::ShortcutHelp)
            && matches!(
                binding.action,
                PrefixAction::ShowHelp | PrefixAction::Client(ClientAction::OpenShortcutHelp)
            )
        {
            return self.shortcut_close_label().to_owned();
        }
        #[cfg(windows)]
        if let Some(description) = view.bindings().configured_description(binding.description) {
            return sanitize_terminal_text(description).into_owned();
        }
        #[cfg(not(windows))]
        let _ = (view, binding.description);

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
            return self.shortcut_local_action_title(&binding.action);
        }

        #[cfg(windows)]
        if let Some(action) = shared_menu_action(binding) {
            return self.menu_action(&action).to_owned();
        }

        self.shortcut_local_action_title(&binding.action)
    }

    pub(crate) const fn shortcut_close_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close help",
            Self::ZhCn => "关闭帮助",
        }
    }

    pub(crate) const fn shortcut_scroll_label(self) -> &'static str {
        match self {
            Self::EnUs => "Scroll Help",
            Self::ZhCn => "滚动帮助",
        }
    }

    const fn shortcut_context_alias(
        self,
        english: &'static str,
        chinese: &'static str,
    ) -> &'static str {
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
    }

    #[cfg(windows)]
    fn append_contextual_entries(
        self,
        view: &ClientView,
        glyphs: KeyLabelGlyphs,
        entries: &mut Vec<ShortcutHelpEntry>,
    ) {
        use crate::machine_picker::{SIDEBAR_BINDINGS, SidebarCommand};

        let shell = self.shell();
        if view.machines.sidebar_navigation_active() {
            for (binding, command) in SIDEBAR_BINDINGS {
                if *command == SidebarCommand::Exit {
                    continue;
                }
                let key = format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs);
                let (title, identity) = match command {
                    SidebarCommand::Help => (
                        self.shortcut_close_label().to_owned(),
                        semantic_identity(PrefixAction::ShowHelp),
                    ),
                    SidebarCommand::NewWorkspace => {
                        let action = PrefixAction::Client(ClientAction::RequestQuickWorkspace);
                        (
                            self.shortcut_local_action_title(&action),
                            semantic_identity(action),
                        )
                    }
                    _ => (
                        self.sidebar_navigation_action(*command).to_owned(),
                        format!("context.sidebar.{command:?}"),
                    ),
                };
                let group = if *command == SidebarCommand::NewWorkspace {
                    ShortcutHelpGroup::Workspace
                } else {
                    ShortcutHelpGroup::Other
                };
                self.merge_context_entry(
                    entries,
                    key,
                    &title,
                    group,
                    identity,
                    self.shortcut_context_alias("sidebar navigation", "侧栏导航"),
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
                self.merge_context_entry(
                    entries,
                    binding.key,
                    self.shortcut_scroll_label(),
                    ShortcutHelpGroup::Other,
                    identity_key(binding.metadata.identity),
                    self.shortcut_context_alias("sidebar navigation", "侧栏导航"),
                );
            }
        } else if view.copy_mode_active() {
            for (key, description) in view.copy_help_rows() {
                if description == PrefixDesc::CopyExit {
                    continue;
                }
                let (title, identity, group) = match description {
                    PrefixDesc::ShowHelp => (
                        self.shortcut_close_label(),
                        semantic_identity(PrefixAction::ShowHelp),
                        ShortcutHelpGroup::Other,
                    ),
                    PrefixDesc::CopyPage => (
                        self.shortcut_scroll_label(),
                        semantic_identity(PrefixAction::HelpPageNext),
                        ShortcutHelpGroup::Other,
                    ),
                    _ => (
                        (shell.prefix_desc)(description),
                        format!("context.copy.{description:?}"),
                        ShortcutHelpGroup::Panel,
                    ),
                };
                self.merge_context_entry(
                    entries,
                    key,
                    title,
                    group,
                    identity,
                    self.shortcut_context_alias("copy mode", "复制模式"),
                );
            }
        } else if view.resize_mode() {
            for (key, description) in view.resize_help_rows_with_glyphs(glyphs) {
                if description == PrefixDesc::ResizeExit {
                    continue;
                }
                let (title, identity, group) = match description {
                    PrefixDesc::ShowHelp => (
                        self.shortcut_close_label(),
                        semantic_identity(PrefixAction::ShowHelp),
                        ShortcutHelpGroup::Other,
                    ),
                    PrefixDesc::HelpPage => (
                        self.shortcut_scroll_label(),
                        semantic_identity(PrefixAction::HelpPageNext),
                        ShortcutHelpGroup::Other,
                    ),
                    _ => (
                        (shell.prefix_desc)(description),
                        format!("context.resize.{description:?}"),
                        ShortcutHelpGroup::Panel,
                    ),
                };
                self.merge_context_entry(
                    entries,
                    key,
                    title,
                    group,
                    identity,
                    self.shortcut_context_alias("resize mode", "调整大小模式"),
                );
            }
        }
    }

    #[cfg(windows)]
    fn merge_context_entry(
        self,
        entries: &mut Vec<ShortcutHelpEntry>,
        key: String,
        title: &str,
        group: ShortcutHelpGroup,
        identity: String,
        scope: &str,
    ) {
        let search_text = format!("{title} {key} {} {scope}", self.shortcut_group_name(group));
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

    fn shortcut_local_action_title(self, action: &PrefixAction) -> String {
        use ClientAction as A;

        match action {
            PrefixAction::Client(A::MovePanelFocus(direction)) => {
                return self.panel_direction_title("Focus panel", "聚焦面板", *direction);
            }
            PrefixAction::Client(A::SplitPanel(direction)) => {
                return self.panel_direction_title("Split panel", "分割面板", *direction);
            }
            PrefixAction::Client(A::MovePanel(direction)) => {
                return self.panel_direction_title("Move panel", "移动面板", *direction);
            }
            _ => {}
        }

        #[cfg(windows)]
        if matches!(
            action,
            PrefixAction::Client(A::RequestWorkspaceCreate | A::RequestQuickWorkspace)
        ) {
            return self
                .menu_action(&crate::action_menu::MenuAction::NewWorkspace)
                .to_owned();
        }

        let title = match action {
            #[cfg(windows)]
            PrefixAction::Client(A::DismissOnboarding) => {
                Some(self.onboarding_label(crate::onboarding::Action::Dismiss))
            }
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
                Self::ZhCn => "打开操作菜单",
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
                Self::ZhCn => "新建工作树",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::RequestWorktreeOpen) => Some(match self {
                Self::EnUs => "Open Worktree",
                Self::ZhCn => "打开工作树",
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
            PrefixAction::Client(A::CreateTab) => Some(match self {
                Self::EnUs => "New Tab",
                Self::ZhCn => "新建标签页",
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
            PrefixAction::Client(A::PreviousPresentation) => Some(match self {
                Self::EnUs => "Select previous terminal in panel",
                Self::ZhCn => "选择面板内上一个终端",
            }),
            PrefixAction::Client(A::NextPresentation) => Some(match self {
                Self::EnUs => "Select next terminal in panel",
                Self::ZhCn => "选择面板内下一个终端",
            }),
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
                Self::EnUs => "Leave view (keep terminals running)",
                Self::ZhCn => "离开界面（终端继续运行）",
            }),
            PrefixAction::Client(A::RequestControlTakeover) => Some(match self {
                Self::EnUs => "Enable typing in this panel",
                Self::ZhCn => "获取此面板的输入权限",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::FocusBusinessSidebar) => {
                Some((self.shell().prefix_desc)(PrefixDesc::SidebarFocus))
            }
            #[cfg(windows)]
            PrefixAction::Client(A::OpenMachines) => Some(match self {
                Self::EnUs => "Search machines, Workspaces and Agents",
                Self::ZhCn => "搜索机器、工作区和智能体",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::PreviousAgent) => Some(match self {
                Self::EnUs => "Select previous Agent",
                Self::ZhCn => "选择上一个智能体",
            }),
            #[cfg(windows)]
            PrefixAction::Client(A::NextAgent) => Some(match self {
                Self::EnUs => "Select next Agent",
                Self::ZhCn => "选择下一个智能体",
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
                    Self::ZhCn => "智能体看板",
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
                Self::ZhCn => "按编号选择智能体",
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
            (Self::ZhCn, A::Worktree) => "工作树",
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
            (Self::ZhCn, A::Agent) => "智能体",
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
    #[cfg(windows)]
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
        #[cfg(windows)]
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

#[cfg(windows)]
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
        ) | PrefixAction::SelectTabDigit
    )
}

#[cfg(windows)]
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
        let entries = Locale::EnUs.shortcut_help_entries(&view, true, KeyLabelGlyphs::Unicode);
        let workspace = entries
            .iter()
            .filter(|entry| entry.title == "New Workspace")
            .collect::<Vec<_>>();
        assert!(!workspace.is_empty(), "workspace creation is discoverable");
        assert!(workspace.iter().any(|entry| entry.key.contains("Ctrl+b a")));
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        assert_eq!(workspace.len(), 1);

        let entries_zh = Locale::ZhCn.shortcut_help_entries(&view, true, KeyLabelGlyphs::Unicode);
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
        #[cfg(windows)]
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
        #[cfg(not(windows))]
        {
            assert_eq!(rename_workspace.title, "Rename Workspace");
            assert_eq!(rename_project.title, "Rename Project");
            assert_eq!(rename_workspace_zh.title, "重命名工作区");
            assert_eq!(rename_project_zh.title, "重命名项目");
        }

        assert!(!entries.iter().any(|entry| entry.key.contains("Unbound")));
        assert!(entries.iter().all(|entry| entry.title != "Shortcut"));
        let workspace_zh = entries_zh
            .iter()
            .filter(|entry| entry.title == "新建工作区")
            .collect::<Vec<_>>();
        assert_eq!(workspace_zh.len(), 1);
        assert!(workspace_zh[0].key.contains("Ctrl+b a"));
        #[cfg(windows)]
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
        #[cfg(windows)]
        {
            assert_eq!(
                create_tab.title,
                Locale::EnUs.menu_action(&crate::action_menu::MenuAction::NewTab)
            );
            assert_eq!(
                create_tab_zh.title,
                Locale::ZhCn.menu_action(&crate::action_menu::MenuAction::NewTab)
            );
            let focus_sidebar = entries
                .iter()
                .find(|entry| entry.key.contains("Shift+B"))
                .expect("sidebar focus keeps its owning binding");
            assert_eq!(focus_sidebar.title, "Switch sidebar / terminal focus");
            let focus_sidebar_zh = entries_zh
                .iter()
                .find(|entry| entry.key.contains("Shift+B"))
                .expect("localized sidebar focus keeps its owning binding");
            assert_eq!(focus_sidebar_zh.title, "切换侧栏／终端焦点");
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

    #[cfg(windows)]
    #[test]
    fn multiple_bindings_merge_when_their_semantic_identity_matches() {
        use orchbee_local_site_runtime::{
            WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings, WindowsKeyTrigger,
        };

        let chord = |key: &str| WindowsKeyChord {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            super_key: false,
        };
        let mut view = ClientView::new(100);
        view.configure_keys(&WindowsKeySettings {
            prefix: WindowsKeyChord {
                key: "b".into(),
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
            },
            overrides: vec![WindowsKeyOverride {
                action: "new_workspace",
                triggers: vec![
                    WindowsKeyTrigger {
                        prefix: true,
                        chord: chord("x"),
                    },
                    WindowsKeyTrigger {
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

        let entries = Locale::EnUs.shortcut_help_entries(&view, true, KeyLabelGlyphs::Unicode);
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

    #[cfg(windows)]
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
        let copy_entries = Locale::EnUs.shortcut_help_entries(&copy, true, KeyLabelGlyphs::Unicode);
        let move_key = copy_rows
            .iter()
            .find(|(_, description)| *description == PrefixDesc::CopyMove)
            .unwrap()
            .0
            .as_str();
        assert!(copy_entries.iter().any(|entry| entry.key == move_key));
        assert!(!copy_entries.iter().any(|entry| entry.key == "Esc"));
        let copy_entries_zh =
            Locale::ZhCn.shortcut_help_entries(&copy, true, KeyLabelGlyphs::Unicode);
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
        let resize_entries =
            Locale::EnUs.shortcut_help_entries(&resize, true, KeyLabelGlyphs::Unicode);
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
        let sidebar_entries =
            Locale::EnUs.shortcut_help_entries(&sidebar, true, KeyLabelGlyphs::Unicode);
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
