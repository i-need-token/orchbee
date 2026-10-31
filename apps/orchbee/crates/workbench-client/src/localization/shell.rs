//! Localized shell chrome, notices, and effective shortcut labels; does not dispatch actions.

use super::Locale;
use super::PrefixDesc;
use super::agent_board::AgentBoardText;
use super::prefix::{en_us_prefix_desc, zh_cn_prefix_desc};
use super::{BackendAction, Notice};

const EN_US_TUI_UNAVAILABLE: &str = "The Orchbee TUI is unavailable in this development slice.\n";

const ZH_CN_TUI_UNAVAILABLE: &str = "Orchbee TUI 在本开发切片中尚不可用。\n";

const EN_US_USAGE_FAILURE: &str = "Invalid invocation. Run 'orchbee --help'.\n";

const ZH_CN_USAGE_FAILURE: &str = "调用方式无效。请运行 ‘orchbee --help’。\n";

#[derive(Clone, Copy, Debug)]
pub struct ShellText<'a> {
    pub(crate) agents: &'static str,
    pub(crate) app_title: &'static str,
    pub(crate) board: AgentBoardText,
    pub(crate) backend_status: &'static str,
    pub(crate) business_section: &'static str,
    pub(crate) exit_hint: &'a str,
    pub(crate) hint_short: &'a str,
    pub(crate) binding_unbound: &'static str,
    pub(crate) overlay_refused: &'static str,
    pub(crate) navigator_title: &'static str,
    pub(crate) no_client_connection: &'static str,
    pub(crate) panel_title: &'static str,
    pub(crate) pointer_released: &'a str,
    pub(crate) prefix_active: &'a str,
    pub(crate) prefix_desc: fn(PrefixDesc) -> &'static str,
    pub(crate) prefix_idle: &'a str,
    pub(crate) projects: &'static str,
    pub(crate) shell_only_detail: &'static str,
    pub(crate) shortcut_title: &'static str,
    pub(crate) sidebar_hint: &'a str,
    pub(crate) session_section: &'static str,
    pub(crate) no_workspace: &'static str,
    pub(crate) tabs_unavailable: &'static str,
    pub(crate) terminal_unavailable: &'static str,
    pub(crate) too_small_notice: &'static str,
    pub(crate) workbench_title: &'static str,
    pub(crate) pinned_tab: &'static str,
    pub(crate) unnamed_tab_group: &'static str,
    pub(crate) workspaces: &'static str,
    pub(crate) zoom_badge: &'a str,
}

const EN_US_SHELL: ShellText<'static> = ShellText {
    agents: "Agents",
    app_title: "ORCHBEE",
    board: Locale::EnUs.agent_board_text(),
    backend_status: "Backend unavailable",
    business_section: "BUSINESS",
    exit_hint: "Ctrl+B q · exit",
    hint_short: "Ctrl+B ? · help",
    overlay_refused: "Not enough room to show this view · resize the terminal",
    navigator_title: "Workbench navigator",
    no_client_connection: "No Orchbee session is connected.",
    binding_unbound: "Unbound",
    panel_title: "Terminal",
    pointer_released: "mouse released · Ctrl+B m restores",
    prefix_active: "PREFIX Ctrl+B",
    prefix_desc: en_us_prefix_desc,
    prefix_idle: "Ctrl+B · shortcuts",
    projects: "Projects",
    shell_only_detail: "Connect to a session to use terminals and agents.",
    shortcut_title: "Keyboard shortcuts",
    sidebar_hint: "Ctrl+B b · sidebar",
    session_section: "SESSION",
    no_workspace: "No workspace",
    tabs_unavailable: "No shared Tabs · session unavailable",
    terminal_unavailable: "Terminal unavailable",
    too_small_notice: "Too small · resize to continue",
    workbench_title: "WORKBENCH",
    pinned_tab: "Pinned",
    unnamed_tab_group: "Unnamed group",
    workspaces: "Workspaces",
    zoom_badge: "zoomed · Ctrl+B z restores",
};

const ZH_CN_SHELL: ShellText<'static> = ShellText {
    agents: "智能体",
    app_title: "ORCHBEE",
    board: Locale::ZhCn.agent_board_text(),
    backend_status: "后端不可用",
    business_section: "业务",
    exit_hint: "Ctrl+B q · 退出",
    hint_short: "Ctrl+B ? · 帮助",
    overlay_refused: "空间不足，无法显示此视图 · 请调整终端大小",
    navigator_title: "Workbench 导航器",
    no_client_connection: "当前未连接 Orchbee 会话。",
    binding_unbound: "未绑定",
    panel_title: "终端",
    pointer_released: "鼠标已释放 · Ctrl+B m 恢复",
    prefix_active: "前缀 Ctrl+B",
    prefix_desc: zh_cn_prefix_desc,
    prefix_idle: "Ctrl+B · 快捷键",
    projects: "项目",
    shell_only_detail: "连接会话后即可使用终端和智能体。",
    shortcut_title: "键盘快捷键",
    sidebar_hint: "Ctrl+B b · 侧边栏",
    session_section: "会话",
    no_workspace: "未打开工作区",
    tabs_unavailable: "无共享标签页 · 会话不可用",
    terminal_unavailable: "终端不可用",
    too_small_notice: "窗口太小 · 请调整大小后继续",
    workbench_title: "WORKBENCH",
    pinned_tab: "固定",
    unnamed_tab_group: "未命名分组",
    workspaces: "工作区",
    zoom_badge: "已放大 · Ctrl+B z 恢复",
};

/// Effective labels selected by the binding owner; text only substitutes these frozen strings.
#[derive(Clone, Debug)]
pub(crate) struct ShellBindings {
    pub(crate) prefix: String,
    pub(crate) help: String,
    pub(crate) exit: String,
    pub(crate) sidebar: String,
    pub(crate) zoom: String,
    pub(crate) navigator: String,
    pub(crate) create_tab: String,
    pub(crate) close_tab: String,
}

/// Temporary render-owned strings; the borrowed shell text remains cheap to copy.
pub(crate) struct BoundShellText<'a> {
    base: ShellText<'a>,
    hints: [String; 7],
}

impl BoundShellText<'_> {
    pub(crate) fn text(&self) -> ShellText<'_> {
        ShellText {
            exit_hint: &self.hints[0],
            hint_short: &self.hints[1],
            pointer_released: &self.hints[2],
            prefix_active: &self.hints[3],
            prefix_idle: &self.hints[4],
            sidebar_hint: &self.hints[5],
            zoom_badge: &self.hints[6],
            ..self.base
        }
    }
}

impl<'a> ShellText<'a> {
    pub(crate) fn bind(self, bindings: &ShellBindings) -> BoundShellText<'a> {
        let hints = [
            self.exit_hint,
            self.hint_short,
            self.pointer_released,
            self.prefix_active,
            self.prefix_idle,
            self.sidebar_hint,
            self.zoom_badge,
        ]
        .map(|hint| self.bound_hint(bindings, hint));
        BoundShellText { base: self, hints }
    }

    pub(crate) fn bound_hint(&self, bindings: &ShellBindings, hint: &str) -> String {
        let mut result = hint.to_owned();
        // Substitute complete default chords before the standalone prefix. The replacement
        // values come from the same resolved entries dispatch uses, including disabled actions.
        let replacements = [
            ("Ctrl+B ?", bindings.help.as_str()),
            ("Ctrl+B q", bindings.exit.as_str()),
            ("Ctrl+B b", bindings.sidebar.as_str()),
            ("Ctrl+B z", bindings.zoom.as_str()),
            ("Ctrl+B w", bindings.navigator.as_str()),
            ("Ctrl+B c", bindings.create_tab.as_str()),
            ("Ctrl+B x", bindings.close_tab.as_str()),
            ("Ctrl+B then q", bindings.exit.as_str()),
            ("Ctrl+B 后按 q", bindings.exit.as_str()),
            ("Ctrl+B 后按 c", bindings.create_tab.as_str()),
            ("Ctrl+B 后按 x", bindings.close_tab.as_str()),
            ("Ctrl+B 后按 w", bindings.navigator.as_str()),
        ];
        // Placeholders prevent one configured chord from being rewritten by a later replacement.
        for (index, (old, _)) in replacements.iter().enumerate() {
            result = result.replace(old, &format!("\u{e000}{index}\u{e001}"));
        }
        result = result.replace("Ctrl+B", &bindings.prefix);
        for (index, (_, actual)) in replacements.iter().enumerate() {
            result = result.replace(&format!("\u{e000}{index}\u{e001}"), actual);
        }
        result
    }
}

impl Locale {
    pub fn tui_detached(self, return_command: &str) -> String {
        match self {
            Self::EnUs => format!(
                "Detached from Orchbee. Terminals keep running; run \"{return_command}\" to return."
            ),
            Self::ZhCn => {
                format!("已离开 Orchbee，终端程序仍在后台运行；输入 {return_command} 返回。")
            }
        }
    }

    pub const fn tui_measurement_path_invalid(self) -> &'static str {
        match self {
            Self::EnUs => "ORCHBEE_TUI_MEASUREMENTS must name a new absolute file path",
            Self::ZhCn => "ORCHBEE_TUI_MEASUREMENTS 必须指定新的绝对文件路径",
        }
    }

    pub const fn tui_measurement_also_failed(self) -> &'static str {
        match self {
            Self::EnUs => "TUI timing diagnostics also failed",
            Self::ZhCn => "TUI 计时诊断也失败",
        }
    }

    pub const fn tui_restoration_also_failed(self) -> &'static str {
        match self {
            Self::EnUs => "terminal cleanup also failed",
            Self::ZhCn => "终端清理也失败",
        }
    }

    pub(crate) const fn shell(self) -> ShellText<'static> {
        match self {
            Self::EnUs => EN_US_SHELL,
            Self::ZhCn => ZH_CN_SHELL,
        }
    }

    pub(crate) const fn navigator_columns(self) -> (&'static str, &'static str) {
        match self {
            Self::EnUs => ("Workspaces / terminals", "Tabs in this location"),
            Self::ZhCn => ("工作区 / 终端", "当前位置的标签页"),
        }
    }

    pub(crate) const fn resize_done_label(self) -> &'static str {
        match self {
            Self::EnUs => "Done",
            Self::ZhCn => "完成",
        }
    }

    pub const fn shell_notice(self, notice: Option<Notice>) -> &'static str {
        match (self, notice) {
            (Self::EnUs, Some(Notice::NotificationUnsupported)) => {
                "Host notification delivery is not supported on this platform"
            }
            (Self::ZhCn, Some(Notice::NotificationUnsupported)) => "此平台尚不支持宿主通知投递",
            (Self::EnUs, Some(Notice::WindowTitleUnsupported)) => {
                "Setting the host window title is not supported on this platform"
            }
            (Self::ZhCn, Some(Notice::WindowTitleUnsupported)) => "此平台尚不支持设置宿主窗口标题",

            (Self::EnUs, Some(Notice::OnboardingSaveFailed)) => {
                "Guide closed here; saving failed, so it may return next time"
            }
            (Self::ZhCn, Some(Notice::OnboardingSaveFailed)) => {
                "本次引导已关闭；保存失败，下次可能再次显示"
            }
            (Self::EnUs, Some(Notice::ResizeBusy)) => "Resize pending; input was not queued",
            (Self::ZhCn, Some(Notice::ResizeBusy)) => "大小调整待定；输入未排队",
            (Self::EnUs, Some(Notice::ResizeFailed)) => "Resize failed; inspect the current layout",
            (Self::ZhCn, Some(Notice::ResizeFailed)) => "大小调整失败；请检查当前布局",
            (Self::EnUs, Some(Notice::ResizeUnknown)) => {
                "Resize outcome unknown; inspect layout before trying again"
            }
            (Self::ZhCn, Some(Notice::ResizeUnknown)) => "大小调整结果不确定；请先检查布局再操作",
            (Self::EnUs, Some(Notice::ResizeNoSplit)) => "No divider on this axis",
            (Self::ZhCn, Some(Notice::ResizeNoSplit)) => "该方向没有分隔线",
            (Self::EnUs, Some(Notice::ResizeLimit)) => "Divider is at its size limit",
            (Self::ZhCn, Some(Notice::ResizeLimit)) => "分隔线已达到大小边界",
            (Self::EnUs, Some(Notice::CheckpointRecovered)) => {
                "Unreadable workspace snapshots retained; restored a valid snapshot"
            }
            (Self::ZhCn, Some(Notice::CheckpointRecovered)) => {
                "部分工作区存档不可读；已使用有效存档恢复，原始文件已保留"
            }
            (Self::EnUs, Some(Notice::CheckpointEmpty)) => {
                "Workspace snapshots unreadable; opened an empty workspace and retained originals"
            }
            (Self::ZhCn, Some(Notice::CheckpointEmpty)) => {
                "工作区存档均不可读；已打开空工作区，原始文件已保留"
            }
            (Self::EnUs, Some(Notice::ConfigurationPublished)) => {
                "Configuration published; keys updated"
            }
            (Self::ZhCn, Some(Notice::ConfigurationPublished)) => "配置已发布；键位已更新",
            (Self::EnUs, Some(Notice::ConfigurationWarning)) => {
                "Configuration loaded with warnings; run orchbee config check"
            }
            (Self::ZhCn, Some(Notice::ConfigurationWarning)) => {
                "配置已加载，但有警告；请运行 orchbee config check"
            }
            (Self::EnUs, Some(Notice::ConfigurationUnknown)) => {
                "Reload outcome unknown; inspect server config"
            }
            (Self::ZhCn, Some(Notice::ConfigurationUnknown)) => {
                "重载结果未知；请查看 server config"
            }
            (Self::EnUs, Some(Notice::NotificationUnavailable)) => {
                "Notification transport unavailable; terminal remains usable"
            }
            (Self::ZhCn, Some(Notice::NotificationUnavailable)) => {
                "通知投递方式不可用；终端仍可正常使用"
            }
            (Self::EnUs, Some(Notice::WindowTitleUnavailable)) => {
                "Window title unavailable; terminal remains usable"
            }
            (Self::ZhCn, Some(Notice::WindowTitleUnavailable)) => {
                "窗口标题不可用；终端仍可正常使用"
            }
            (
                Self::EnUs,
                Some(
                    Notice::ConfigurationFailed
                    | Notice::BackendUnavailable(BackendAction::ReloadConfiguration),
                ),
            ) => "Reload failed; previous keys retained",
            (
                Self::ZhCn,
                Some(
                    Notice::ConfigurationFailed
                    | Notice::BackendUnavailable(BackendAction::ReloadConfiguration),
                ),
            ) => "重载失败；保留此前键位",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameWorkspace))) => {
                "Workspace rename unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameWorkspace))) => {
                "工作区重命名不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "Project rename unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "项目重命名不可用 · 未发送命令"
            }
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorkspace
                    | BackendAction::QuickCreateWorkspace
                    | BackendAction::ConfigureWorkspaceDirectory,
                )),
            ) => "Workspace setup unavailable · no command was sent",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorkspace
                    | BackendAction::QuickCreateWorkspace
                    | BackendAction::ConfigureWorkspaceDirectory,
                )),
            ) => "工作区设置不可用 · 未发送命令",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "Worktree unavailable · no command was sent",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "工作树不可用 · 未发送命令",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CloseWorkspace))) => {
                "Workspace Close unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CloseWorkspace))) => {
                "工作区关闭不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameTab))) => {
                "Rename unavailable · no command has been sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameTab))) => {
                "重命名不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "Close unavailable · no command has been sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "关闭不可用 · 未发送命令"
            }
            (Self::EnUs, None) => "Client connection unavailable · no command has been sent",
            (Self::ZhCn, None) => "客户端连接不可用 · 未发送任何命令",
            (Self::EnUs, Some(Notice::TerminalInputUnavailable)) => {
                "Terminal input unavailable · input was not sent"
            }
            (Self::ZhCn, Some(Notice::TerminalInputUnavailable)) => "终端输入不可用 · 未发送输入",
            (Self::EnUs, Some(Notice::NavigationInputHeld)) => "Switching · input was not sent",
            (Self::ZhCn, Some(Notice::NavigationInputHeld)) => "正在切换 · 未发送输入",
            (Self::EnUs, Some(Notice::WorkspaceNavigationUnavailable)) => {
                "Could not switch · select the destination again to retry"
            }
            (Self::ZhCn, Some(Notice::WorkspaceNavigationUnavailable)) => {
                "无法切换 · 请再次选择目标以重试"
            }
            (Self::EnUs, Some(Notice::PointerInputDropped)) => {
                "Pointer input dropped · focus switch did not settle in time"
            }
            (Self::ZhCn, Some(Notice::PointerInputDropped)) => {
                "指针输入被丢弃 · 焦点切换未及时就绪"
            }
            (Self::EnUs, Some(Notice::NavigatorChanged)) => {
                "Navigator closed · location or target changed"
            }
            (Self::ZhCn, Some(Notice::NavigatorChanged)) => "导航器已关闭 · 位置或目标已改变",
            (_, Some(Notice::NavigatorTooSmall)) => self.shell().overlay_refused,
            (Self::EnUs, Some(Notice::UnknownPrefix)) => "Unknown prefix key",
            (Self::ZhCn, Some(Notice::UnknownPrefix)) => "未知前缀键",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "Tab creation unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "无法创建标签页 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::SplitPanel))) => {
                "Panel split unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::SplitPanel))) => {
                "面板拆分不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::ClosePanel))) => {
                "Panel close unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::ClosePanel))) => {
                "面板关闭不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::MovePanel))) => {
                "Panel move unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::MovePanel))) => {
                "面板移动不可用 · 未发送命令"
            }
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::MovePreviousTab | BackendAction::MoveNextTab,
                )),
            ) => "Tab move unavailable · no command was sent",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::MovePreviousTab | BackendAction::MoveNextTab,
                )),
            ) => "无法移动标签页 · 未发送命令",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectNextTab
                    | BackendAction::SelectPreviousTab
                    | BackendAction::SelectTab(_),
                )),
            ) => "Tab navigation unavailable · no command was sent",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectNextTab
                    | BackendAction::SelectPreviousTab
                    | BackendAction::SelectTab(_),
                )),
            ) => "标签页导航不可用 · 未发送命令",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectPreviousContext | BackendAction::SelectNextContext,
                )),
            ) => "Workbench location navigation unavailable · no command was sent",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectPreviousContext | BackendAction::SelectNextContext,
                )),
            ) => "Workbench 位置导航不可用 · 未发送命令",
        }
    }

    pub const fn tui_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_TUI_UNAVAILABLE,
            Self::ZhCn => ZH_CN_TUI_UNAVAILABLE,
        }
    }

    pub const fn usage_failure(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_USAGE_FAILURE,
            Self::ZhCn => ZH_CN_USAGE_FAILURE,
        }
    }
}
