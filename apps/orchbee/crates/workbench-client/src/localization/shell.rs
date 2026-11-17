//! Localized shell chrome, notices, and effective shortcut labels; does not dispatch actions.

use super::Locale;
use super::PrefixDesc;
use super::agent_board::AgentBoardText;
use super::prefix::{en_us_prefix_desc, zh_cn_prefix_desc};
use super::{BackendAction, Notice};
use crate::machine_refusal::Refusal;

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
    pub(crate) binding_unbound: &'static str,
    pub(crate) overlay_refused: &'static str,
    /// The status line's shorter form of `overlay_refused`, without the advice.
    pub(crate) overlay_refused_short: &'static str,
    pub(crate) navigator_title: &'static str,
    pub(crate) no_client_connection: &'static str,
    pub(crate) panel_title: &'static str,
    pub(crate) pointer_released: &'a str,
    pub(crate) prefix_active: &'a str,
    pub(crate) prefix_desc: fn(PrefixDesc) -> Option<&'static str>,
    pub(crate) projects: &'static str,
    pub(crate) shell_only_detail: &'static str,
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
}

const EN_US_SHELL: ShellText<'static> = ShellText {
    agents: "Agents",
    app_title: "ORCHBEE",
    board: Locale::EnUs.agent_board_text(),
    backend_status: "Backend unavailable",
    business_section: "BROWSE",
    exit_hint: "{exit} · exit",
    overlay_refused: "Not enough room to show this view · resize the terminal",
    overlay_refused_short: "Not enough room to show this view",
    navigator_title: "Workbench navigator",
    no_client_connection: "No Orchbee session is connected.",
    binding_unbound: "Unbound",
    panel_title: "Terminal",
    pointer_released: "mouse released · {pointer} restores",
    prefix_active: "PREFIX {prefix}",
    prefix_desc: en_us_prefix_desc,
    projects: "Projects",
    shell_only_detail: "Connect to a session to use terminals and agents.",
    sidebar_hint: "{sidebar} · sidebar",
    session_section: "SESSION",
    no_workspace: "No workspace",
    tabs_unavailable: "No shared Tabs · session unavailable",
    terminal_unavailable: "Terminal unavailable",
    too_small_notice: "Too small · resize to continue",
    workbench_title: "WORKBENCH",
    pinned_tab: "Pinned",
    unnamed_tab_group: "Unnamed group",
    workspaces: "Workspaces",
};

const ZH_CN_SHELL: ShellText<'static> = ShellText {
    agents: "智能体",
    app_title: "ORCHBEE",
    board: Locale::ZhCn.agent_board_text(),
    backend_status: "后端不可用",
    business_section: "浏览",
    exit_hint: "{exit} · 退出",
    overlay_refused: "空间不足，无法显示此视图 · 请调整终端大小",
    overlay_refused_short: "空间不足，无法显示此视图",
    navigator_title: "Workbench 导航器",
    no_client_connection: "当前未连接 Orchbee 会话。",
    binding_unbound: "未绑定",
    panel_title: "终端",
    pointer_released: "鼠标已释放 · {pointer} 恢复",
    prefix_active: "前缀 {prefix}",
    prefix_desc: zh_cn_prefix_desc,
    projects: "项目",
    shell_only_detail: "连接会话后即可使用终端和智能体。",
    sidebar_hint: "{sidebar} · 边栏",
    session_section: "会话",
    no_workspace: "未打开工作区",
    tabs_unavailable: "无共享标签页 · 会话不可用",
    terminal_unavailable: "终端不可用",
    too_small_notice: "窗口太小 · 请调整大小后继续",
    workbench_title: "WORKBENCH",
    pinned_tab: "固定",
    unnamed_tab_group: "未命名分组",
    workspaces: "工作区",
};

/// Effective key labels selected by the binding owner. Localized text names a key only through
/// one of these slots, so every surface shows the label dispatch resolves, in one spelling.
#[derive(Clone, Debug)]
pub(crate) struct ShellBindings {
    pub(crate) prefix: String,
    pub(crate) exit: String,
    pub(crate) sidebar: String,
    pub(crate) pointer: String,
    pub(crate) start_agent: String,
}

impl ShellBindings {
    /// Fills the key slot a status notice carries, for the texts the status line draws.
    pub(crate) fn fill_notice(&self, notice: &mut String) {
        if notice.contains(START_AGENT_KEY_SLOT) {
            *notice = notice.replace(START_AGENT_KEY_SLOT, &self.start_agent);
        }
        if notice.contains(EXIT_KEY_SLOT) {
            *notice = notice.replace(EXIT_KEY_SLOT, &self.exit);
        }
    }
}

/// The slot a notice built without the binding owner leaves for the Start Agent picker's key;
/// [`ShellBindings::fill_notice`] fills it when the status line is drawn. Private-use characters
/// keep it apart from any text a person typed.
pub(crate) const START_AGENT_KEY_SLOT: &str = "\u{e000}start-agent\u{e001}";
/// The slot for the key that leaves the view, which [`ShellBindings::fill_notice`] fills.
pub(crate) const EXIT_KEY_SLOT: &str = "\u{e000}exit\u{e001}";

/// Temporary render-owned strings; the borrowed shell text remains cheap to copy.
pub(crate) struct BoundShellText<'a> {
    base: ShellText<'a>,
    hints: [String; 4],
}

impl BoundShellText<'_> {
    pub(crate) fn text(&self) -> ShellText<'_> {
        ShellText {
            exit_hint: &self.hints[0],
            pointer_released: &self.hints[1],
            prefix_active: &self.hints[2],
            sidebar_hint: &self.hints[3],
            ..self.base
        }
    }
}

impl<'a> ShellText<'a> {
    pub(crate) fn bind(self, bindings: &ShellBindings) -> BoundShellText<'a> {
        let hints = [
            self.exit_hint,
            self.pointer_released,
            self.prefix_active,
            self.sidebar_hint,
        ]
        .map(|hint| self.bound_hint(bindings, hint));
        BoundShellText { base: self, hints }
    }

    /// Fills the key slots of `hint` with the labels of the entries dispatch resolves, including
    /// the unbound label of a disabled action. Text without a slot is returned as it is.
    pub(crate) fn bound_hint(&self, bindings: &ShellBindings, hint: &str) -> String {
        [
            ("{prefix}", &bindings.prefix),
            ("{exit}", &bindings.exit),
            ("{sidebar}", &bindings.sidebar),
            ("{pointer}", &bindings.pointer),
        ]
        .into_iter()
        .fold(hint.to_owned(), |text, (slot, label)| {
            text.replace(slot, label)
        })
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

    pub const fn shell_notice(self, notice: Option<Notice>) -> &'static str {
        use crate::host_capabilities::Capability;
        match (self, notice) {
            (Self::EnUs, Some(Notice::NotificationUnsupported))
                if Capability::SystemNotification.session_dependent() =>
            {
                "Host notification delivery isn't available in this session"
            }
            (Self::ZhCn, Some(Notice::NotificationUnsupported))
                if Capability::SystemNotification.session_dependent() =>
            {
                "当前会话不支持宿主通知投递"
            }
            (Self::EnUs, Some(Notice::WindowTitleUnsupported))
                if Capability::WindowTitle.session_dependent() =>
            {
                "Setting the host window title isn't available in this session"
            }
            (Self::ZhCn, Some(Notice::WindowTitleUnsupported))
                if Capability::WindowTitle.session_dependent() =>
            {
                "当前会话不支持设置宿主窗口标题"
            }
            (Self::EnUs, Some(Notice::NotificationUnsupported)) => {
                "Host notification delivery is not supported on this platform"
            }
            (Self::ZhCn, Some(Notice::NotificationUnsupported)) => "此平台尚不支持宿主通知投递",
            (Self::EnUs, Some(Notice::WindowTitleUnsupported)) => {
                "Setting the host window title is not supported on this platform"
            }
            (Self::ZhCn, Some(Notice::WindowTitleUnsupported)) => "此平台尚不支持设置宿主窗口标题",

            (
                Self::EnUs,
                Some(Notice::ResizeBusy | Notice::ResizeFailed | Notice::ResizeContended),
            ) => "Couldn't resize the Panel.",
            (
                Self::ZhCn,
                Some(Notice::ResizeBusy | Notice::ResizeFailed | Notice::ResizeContended),
            ) => "没能调整面板大小。",
            (_, Some(Notice::ResizeUnknown)) => self.outcome_unknown(),
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
            (Self::EnUs, Some(Notice::SettingsFileUnusable)) => {
                "Settings file can't be used · keeping last valid settings · run orchbee config check"
            }
            (Self::ZhCn, Some(Notice::SettingsFileUnusable)) => {
                "无法使用设置文件 · 继续使用上次有效的设置 · 请运行 orchbee config check"
            }
            (Self::EnUs, Some(Notice::ConfigurationWarning)) => {
                "Configuration loaded with warnings; run orchbee config check"
            }
            (Self::ZhCn, Some(Notice::ConfigurationWarning)) => {
                "配置已加载，但有警告；请运行 orchbee config check"
            }
            (Self::EnUs, Some(Notice::NotificationUnavailable)) => "Couldn't send notifications.",
            (Self::ZhCn, Some(Notice::NotificationUnavailable)) => "没能发送通知。",
            (Self::EnUs, Some(Notice::WindowTitleUnavailable)) => "Couldn't set the window title.",
            (Self::ZhCn, Some(Notice::WindowTitleUnavailable)) => "没能设置窗口标题。",
            (Self::EnUs, Some(Notice::ConfigurationFailed)) => "Couldn't reload the key bindings.",
            (Self::ZhCn, Some(Notice::ConfigurationFailed)) => "没能重新加载键位。",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameWorkspace))) => {
                "Couldn't rename the Workspace."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameWorkspace))) => {
                "没能重命名工作区。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "Couldn't rename the Project."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "没能重命名项目。"
            }
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorkspace | BackendAction::QuickCreateWorkspace,
                )),
            ) => "Couldn't create the Workspace.",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorkspace | BackendAction::QuickCreateWorkspace,
                )),
            ) => "没能创建工作区。",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "Couldn't open the worktree.",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "没能打开工作树。",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CloseWorkspace))) => {
                "Couldn't close the Workspace."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CloseWorkspace))) => {
                "没能关闭工作区。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameTab))) => {
                "Couldn't rename the Tab."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameTab))) => {
                "没能重命名标签页。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "Couldn't close the Tab."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "没能关闭标签页。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "Couldn't create the Tab."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "没能创建标签页。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::SplitPanel))) => {
                "Couldn't split the Panel."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::SplitPanel))) => {
                "没能拆分面板。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::ClosePanel))) => {
                "Couldn't close the Panel."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::ClosePanel))) => {
                "没能关闭面板。"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::MovePanel))) => {
                "Couldn't move the Panel."
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::MovePanel))) => {
                "没能移动面板。"
            }
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::MovePreviousTab | BackendAction::MoveNextTab,
                )),
            ) => "Couldn't move the Tab.",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::MovePreviousTab | BackendAction::MoveNextTab,
                )),
            ) => "没能移动标签页。",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectNextTab
                    | BackendAction::SelectPreviousTab
                    | BackendAction::SelectTab(_),
                )),
            ) => "Couldn't switch Tabs.",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectNextTab
                    | BackendAction::SelectPreviousTab
                    | BackendAction::SelectTab(_),
                )),
            ) => "没能切换标签页。",
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectPreviousContext | BackendAction::SelectNextContext,
                )),
            ) => "Couldn't switch the location.",
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::SelectPreviousContext | BackendAction::SelectNextContext,
                )),
            ) => "没能切换位置。",
            (Self::EnUs, None) => "Couldn't connect to Orchbee.",
            (Self::ZhCn, None) => "没能连接 Orchbee。",
            (_, Some(Notice::TerminalInputUnavailable | Notice::NavigationInputHeld)) => {
                self.terminal_input_not_sent()
            }
            (Self::EnUs, Some(Notice::WorkspaceNavigationUnavailable)) => "Couldn't switch there.",
            (Self::ZhCn, Some(Notice::WorkspaceNavigationUnavailable)) => "没能切换过去。",
            (Self::EnUs, Some(Notice::MachineRefused(refusal))) => match refusal {
                Refusal::OtherEnvironment | Refusal::DevelopmentBuildMismatch => {
                    "Couldn't switch. That machine runs a different installation."
                }
                Refusal::ClientTooOld => "Couldn't switch. Update Orchbee on this machine.",
                Refusal::SessionTooOld => "Couldn't switch. Update Orchbee on that machine.",
            },
            (Self::ZhCn, Some(Notice::MachineRefused(refusal))) => match refusal {
                Refusal::OtherEnvironment | Refusal::DevelopmentBuildMismatch => {
                    "没能切换。那台机器运行的是另一个安装。"
                }
                Refusal::ClientTooOld => "没能切换。请更新这台机器上的 Orchbee。",
                Refusal::SessionTooOld => "没能切换。请更新那台机器上的 Orchbee。",
            },
            (Self::EnUs, Some(Notice::PasswordNotSaved)) => "Couldn't save the password.",
            (Self::ZhCn, Some(Notice::PasswordNotSaved)) => "没能保存密码。",
            (Self::EnUs, Some(Notice::NoNotificationTarget)) => "No Agent notification",
            (Self::ZhCn, Some(Notice::NoNotificationTarget)) => "没有智能体通知",
            (Self::EnUs, Some(Notice::UpdateWaiting)) => {
                "This update applies when no terminal is running."
            }
            (Self::ZhCn, Some(Notice::UpdateWaiting)) => "此更新将在没有终端运行时生效。",
            (Self::EnUs, Some(Notice::UpdateUnavailable)) => "No update to switch to.",
            (Self::ZhCn, Some(Notice::UpdateUnavailable)) => "没有可切换的更新。",
            (Self::EnUs, Some(Notice::PointerInputDropped)) => "Couldn't handle the click.",
            (Self::ZhCn, Some(Notice::PointerInputDropped)) => "没能处理这次点击。",
            (Self::EnUs, Some(Notice::NavigatorChanged)) => "Couldn't go there.",
            (Self::ZhCn, Some(Notice::NavigatorChanged)) => "没能前往那里。",
            (_, Some(Notice::NavigatorTooSmall)) => self.shell().overlay_refused,
            (Self::EnUs, Some(Notice::UnknownPrefix)) => "Unknown prefix key",
            (Self::ZhCn, Some(Notice::UnknownPrefix)) => "未知前缀键",
        }
    }

    /// The status line's shorter form of a Client notice too wide for it at the Compact minimum
    /// width, for the notices that can be; it keeps the outcome and drops the advice.
    pub(crate) const fn shell_notice_shorter(self, notice: Notice) -> Option<&'static str> {
        let (english, chinese) = match notice {
            Notice::CheckpointRecovered => (
                "Workspace snapshot restored; some unreadable",
                "已使用有效存档恢复；部分工作区存档不可读",
            ),
            Notice::CheckpointEmpty => (
                "Workspace snapshots unreadable; empty workspace opened",
                "工作区存档均不可读；已打开空工作区",
            ),
            Notice::ConfigurationWarning => {
                ("Configuration loaded with warnings", "配置已加载，但有警告")
            }
            Notice::SettingsFileUnusable => (
                "Settings file can't be used · using last values",
                "无法使用设置文件 · 继续使用上次有效的设置",
            ),
            Notice::WindowTitleUnsupported
                if crate::host_capabilities::Capability::WindowTitle.session_dependent() =>
            {
                (
                    "Setting the host window title unavailable",
                    "无法设置宿主窗口标题",
                )
            }
            Notice::WindowTitleUnsupported => (
                "Setting the host window title not supported",
                "不支持设置宿主窗口标题",
            ),
            Notice::NotificationUnsupported
                if crate::host_capabilities::Capability::SystemNotification.session_dependent() =>
            {
                ("Host notification delivery unavailable", "无法投递宿主通知")
            }
            Notice::NotificationUnsupported => (
                "Host notification delivery not supported",
                "不支持宿主通知投递",
            ),
            Notice::NavigatorTooSmall => return Some(self.shell().overlay_refused_short),
            Notice::MachineRefused(
                Refusal::OtherEnvironment | Refusal::DevelopmentBuildMismatch,
            ) => (
                "Couldn't switch: another installation.",
                "没能切换：另一个安装。",
            ),
            Notice::ResizeUnknown
            | Notice::NotificationUnavailable
            | Notice::PointerInputDropped
            | Notice::BackendUnavailable(
                BackendAction::SelectPreviousContext
                | BackendAction::SelectNextContext
                | BackendAction::CreateWorkspace
                | BackendAction::QuickCreateWorkspace
                | BackendAction::CreateWorktree
                | BackendAction::OpenWorktree
                | BackendAction::CloseWorkspace
                | BackendAction::RenameWorkspace
                | BackendAction::RenameProject
                | BackendAction::RenameTab
                | BackendAction::CloseTab
                | BackendAction::MovePreviousTab
                | BackendAction::MoveNextTab
                | BackendAction::CreateTab
                | BackendAction::SelectNextTab
                | BackendAction::SelectPreviousTab
                | BackendAction::SelectTab(_)
                | BackendAction::SplitPanel
                | BackendAction::ClosePanel
                | BackendAction::MovePanel,
            )
            | Notice::WorkspaceNavigationUnavailable
            | Notice::MachineRefused(_)
            | Notice::PasswordNotSaved
            | Notice::NoNotificationTarget
            | Notice::UpdateWaiting
            | Notice::UpdateUnavailable
            | Notice::NavigatorChanged
            | Notice::ResizeBusy
            | Notice::ResizeFailed
            | Notice::ResizeContended
            | Notice::ResizeNoSplit
            | Notice::ResizeLimit
            | Notice::ConfigurationFailed
            | Notice::WindowTitleUnavailable
            | Notice::TerminalInputUnavailable
            | Notice::NavigationInputHeld
            | Notice::UnknownPrefix => return None,
        };
        Some(match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        })
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
