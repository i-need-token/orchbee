use std::ffi::OsString;

use crate::client_view::{BackendAction, Notice};

#[cfg(unix)]
mod tab_close;

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee --help\n  orchbee --version\n\nModes:\n  orchbee            Open the local terminal (Linux development cohort)\n  orchbee --help     Show this help\n  orchbee --version  Show the version\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee --help\n  orchbee --version\n\n模式：\n  orchbee            打开本地终端（Linux 开发 cohort）\n  orchbee --help     显示此帮助\n  orchbee --version  显示版本\n";
const EN_US_TUI_UNAVAILABLE: &str = "The Orchbee TUI is unavailable in this development slice.\n";
const ZH_CN_TUI_UNAVAILABLE: &str = "Orchbee TUI 在本开发切片中尚不可用。\n";
const EN_US_USAGE_FAILURE: &str = "Invalid invocation. Run 'orchbee --help'.\n";
const ZH_CN_USAGE_FAILURE: &str = "调用方式无效。请运行 ‘orchbee --help’。\n";

const EN_US_SHORTCUTS: &[&str] = &[
    "Ctrl+B x       Close current Tab / retry Close (confirms for all Clients)",
    "Ctrl+B c       Create a Tab (requires Site)",
    "Ctrl+B < / >   Move current Tab left / right (wraps)",
    "Ctrl+B n / p   Select next / previous Tab (requires Site)",
    "Ctrl+B 1 … 9   Select ordered Tab (requires Site)",
    "Ctrl+B h/j/k/l Move between Panels (requires Site)",
    "Ctrl+B b       Toggle business sidebar",
    "Ctrl+B w       Toggle Workbench navigator",
    "Ctrl+B ?       Open this shortcut help",
    "Ctrl+B q / d   Close this shell",
    "Ctrl+B Ctrl+B  Send literal Ctrl+B to the terminal input port",
    "Other keys     Send to the terminal input port",
];
const ZH_CN_SHORTCUTS: &[&str] = &[
    "Ctrl+B x       关闭当前 Tab / 重试关闭（确认后影响所有 Client）",
    "Ctrl+B c       创建 Tab（需要 Site）",
    "Ctrl+B < / >   向左 / 右移动当前 Tab（首尾循环）",
    "Ctrl+B n / p   选择下一个 / 上一个 Tab（需要 Site）",
    "Ctrl+B 1 … 9   选择对应顺序的 Tab（需要 Site）",
    "Ctrl+B h/j/k/l 在 Panel 之间移动（需要 Site）",
    "Ctrl+B b       切换业务侧边栏",
    "Ctrl+B w       切换 Workbench 导航器",
    "Ctrl+B ?       打开此快捷键帮助",
    "Ctrl+B q / d   关闭此 shell",
    "Ctrl+B Ctrl+B  向终端输入端口发送字面 Ctrl+B",
    "其他按键       发送到终端输入端口",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShellText {
    pub(crate) agents: &'static str,
    pub(crate) app_title: &'static str,
    pub(crate) backend_status: &'static str,
    pub(crate) business_section: &'static str,
    pub(crate) overlay_input_hint: &'static str,
    pub(crate) navigator_detail: &'static str,
    pub(crate) navigator_title: &'static str,
    pub(crate) no_client_session: &'static str,
    pub(crate) panel_title: &'static str,
    pub(crate) prefix_active: &'static str,
    pub(crate) prefix_idle: &'static str,
    pub(crate) projects: &'static str,
    pub(crate) shell_only_detail: &'static str,
    pub(crate) shortcut_title: &'static str,
    pub(crate) shortcuts: &'static [&'static str],
    pub(crate) sidebar_hint: &'static str,
    pub(crate) site_section: &'static str,
    pub(crate) standalone: &'static str,
    pub(crate) tabs_unavailable: &'static str,
    pub(crate) terminal_unavailable: &'static str,
    pub(crate) workbench_title: &'static str,
    pub(crate) workspaces: &'static str,
}

const EN_US_SHELL: ShellText = ShellText {
    agents: "Agents",
    app_title: "ORCHBEE",
    backend_status: "Backend unavailable",
    business_section: "BUSINESS",
    navigator_detail: "A Site connection is required before Workbench locations can be shown.",
    navigator_title: "Workbench navigator",
    no_client_session: "No Site or Client Session is connected.",
    overlay_input_hint: "Terminal input continues while this overlay is visible.",
    panel_title: "Panel · Terminal",
    prefix_active: "PREFIX Ctrl+B",
    prefix_idle: "Ctrl+B · shortcuts",
    projects: "Projects",
    shell_only_detail: "This development slice renders only the Client-local shell.",
    shortcut_title: "Keyboard shortcuts",
    shortcuts: EN_US_SHORTCUTS,
    sidebar_hint: "Ctrl+B b · sidebar",
    site_section: "SITE",
    standalone: "Standalone terminal",
    tabs_unavailable: "No shared Tabs · Site unavailable",
    terminal_unavailable: "Terminal unavailable",
    workbench_title: "WORKBENCH",
    workspaces: "Workspaces",
};

const ZH_CN_SHELL: ShellText = ShellText {
    agents: "智能体",
    app_title: "ORCHBEE",
    backend_status: "后端不可用",
    business_section: "业务",
    navigator_detail: "连接 Site 后才能显示 Workbench 位置。",
    navigator_title: "Workbench 导航器",
    no_client_session: "当前未连接 Site 或 Client Session。",
    overlay_input_hint: "此浮层显示时，终端输入仍会继续传递。",
    panel_title: "Panel · 终端",
    prefix_active: "前缀 Ctrl+B",
    prefix_idle: "Ctrl+B · 快捷键",
    projects: "项目",
    shell_only_detail: "此开发切片只渲染 Client-local shell。",
    shortcut_title: "键盘快捷键",
    shortcuts: ZH_CN_SHORTCUTS,
    sidebar_hint: "Ctrl+B b · 侧边栏",
    site_section: "站点",
    standalone: "独立终端",
    tabs_unavailable: "无共享 Tab · Site 不可用",
    terminal_unavailable: "终端不可用",
    workbench_title: "WORKBENCH",
    workspaces: "工作区",
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    pub(crate) const fn help(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_HELP,
            Self::ZhCn => ZH_CN_HELP,
        }
    }

    pub(crate) const fn shell(self) -> ShellText {
        match self {
            Self::EnUs => EN_US_SHELL,
            Self::ZhCn => ZH_CN_SHELL,
        }
    }

    pub(crate) const fn shell_notice(self, notice: Option<Notice>) -> &'static str {
        match (self, notice) {
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "Close unavailable · no command has been sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CloseTab))) => {
                "关闭不可用 · 未发送命令"
            }
            (Self::EnUs, None) => "Site unavailable · no command has been sent",
            (Self::ZhCn, None) => "Site 不可用 · 未发送任何命令",
            (Self::EnUs, Some(Notice::TerminalInputUnavailable)) => {
                "Terminal input unavailable · input was not sent"
            }
            (Self::ZhCn, Some(Notice::TerminalInputUnavailable)) => "终端输入不可用 · 未发送输入",
            (Self::EnUs, Some(Notice::UnknownPrefix)) => {
                "Unknown prefix key · press Ctrl+B then ? for shortcuts"
            }
            (Self::ZhCn, Some(Notice::UnknownPrefix)) => {
                "未知前缀键 · 按 Ctrl+B 后再按 ? 查看快捷键"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "Tab creation unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "无法创建 Tab · 未发送命令"
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
            ) => "无法移动 Tab · 未发送命令",
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
            ) => "Tab 导航不可用 · 未发送命令",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::MovePanelFocus(_)))) => {
                "Panel navigation unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::MovePanelFocus(_)))) => {
                "Panel 导航不可用 · 未发送命令"
            }
        }
    }

    pub(crate) const fn tui_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_TUI_UNAVAILABLE,
            Self::ZhCn => ZH_CN_TUI_UNAVAILABLE,
        }
    }

    pub(crate) const fn usage_failure(self) -> &'static str {
        match self {
            Self::EnUs => EN_US_USAGE_FAILURE,
            Self::ZhCn => ZH_CN_USAGE_FAILURE,
        }
    }
}

pub(crate) fn select_locale(mut read_environment: impl FnMut(&str) -> Option<OsString>) -> Locale {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = read_environment(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return locale_from_value(&value);
    }
    Locale::EnUs
}

fn locale_from_value(value: &OsString) -> Locale {
    let Some(value) = value.to_str() else {
        return Locale::EnUs;
    };
    let bytes = value.as_bytes();
    match (bytes.first(), bytes.get(1)) {
        (Some(first), Some(second))
            if first.eq_ignore_ascii_case(&b'z') && second.eq_ignore_ascii_case(&b'h') =>
        {
            Locale::ZhCn
        }
        _ => Locale::EnUs,
    }
}

#[cfg(test)]
mod tests;

#[cfg(unix)]
impl Locale {
    pub(crate) const fn terminal_status(
        self,
        status: crate::terminal_session::TerminalStatus,
    ) -> &'static str {
        use crate::terminal_session::TerminalStatus;
        match (self, status) {
            (Self::EnUs, TerminalStatus::Connecting) => "Connecting terminal…",
            (Self::ZhCn, TerminalStatus::Connecting) => "正在连接终端…",
            (Self::EnUs, TerminalStatus::Controlling) => "Terminal connected",
            (Self::ZhCn, TerminalStatus::Controlling) => "终端已连接",
            (Self::EnUs, TerminalStatus::ReadOnly) => {
                "Viewing only · another Client controls this terminal"
            }
            (Self::ZhCn, TerminalStatus::ReadOnly) => "仅查看 · 其他 Client 正在控制此终端",
            (Self::EnUs, TerminalStatus::Blocked) => {
                "Terminal display paused · waiting for a fresh frame"
            }
            (Self::ZhCn, TerminalStatus::Blocked) => "终端显示已暂停 · 正在等待新帧",
            (Self::EnUs, TerminalStatus::Unavailable) => {
                "Terminal disconnected · Ctrl+B then q to exit"
            }
            (Self::ZhCn, TerminalStatus::Unavailable) => "终端已断连 · 按 Ctrl+B 后按 q 退出",
            (Self::EnUs, TerminalStatus::OutcomeUnknown) => {
                "Terminal operation result unknown · no replay · Ctrl+B then q to exit"
            }
            (Self::ZhCn, TerminalStatus::OutcomeUnknown) => {
                "终端操作结果未知 · 不会重放 · 按 Ctrl+B 后按 q 退出"
            }
        }
    }

    pub(crate) fn native_status(
        self,
        view: &crate::terminal_session::NativeSessionView<'_>,
    ) -> String {
        if view.retirement().is_some() {
            return self.native_phase_status(view);
        }
        view.notice().map_or_else(
            || self.native_phase_status(view),
            |notice| self.native_notice(notice),
        )
    }

    pub(crate) fn native_phase_status(
        self,
        view: &crate::terminal_session::NativeSessionView<'_>,
    ) -> String {
        self.native_phase_status_values(
            view.phase(),
            view.active_position(),
            view.controller(),
            view.selected_provenance(),
            view.projection_status(),
            view.retirement(),
        )
    }

    fn native_phase_status_values(
        self,
        phase: crate::terminal_session::NativeDisplayPhase,
        position: Option<usize>,
        controller: Option<crate::terminal_session::NativeDisplayController>,
        provenance: Option<crate::terminal_session::NativeDisplayProvenance>,
        projection: crate::terminal_session::NativeProjectionStatus,
        retirement: Option<crate::terminal_session::NativeDisplayRetirement>,
    ) -> String {
        use crate::terminal_session::{
            NativeDisplayController as Controller, NativeDisplayPhase as Phase,
            NativeDisplayProvenance as Provenance, NativeDisplayRetirement as Retirement,
        };

        if let Some(retirement) = retirement {
            return match retirement {
                Retirement::OutcomeUnknown => self
                    .terminal_status(crate::terminal_session::TerminalStatus::OutcomeUnknown)
                    .to_owned(),
                Retirement::Unavailable => self
                    .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                    .to_owned(),
            };
        }

        match phase {
            Phase::Empty => match self {
                Self::EnUs => "No terminal tabs · Ctrl+B c to create a Tab".to_owned(),
                Self::ZhCn => "没有终端标签 · 按 Ctrl+B 后按 c 创建 Tab".to_owned(),
            },
            Phase::Starting => self.native_starting_status(position, projection),
            Phase::Switching => self.native_switching_status(position, projection),
            Phase::Ready => {
                let Some(position) = position else {
                    return self
                        .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match controller {
                    Some(Controller::Controlling) => self.native_ready_controlling(position),
                    Some(Controller::ReadOnly) => self.native_ready_read_only(position),
                    None => self.native_terminal_unavailable(position),
                }
            }
            Phase::Creating
            | Phase::CreateCommittedNeedDetach
            | Phase::AwaitingCommittedProjection => self.native_creating().to_owned(),
            Phase::ProjectionUnavailable => self.native_committed_refresh_required().to_owned(),
            Phase::MoveProjectionUnavailable => match self {
                Self::EnUs => "Tab moved · Ctrl+B w to refresh tab order".to_owned(),
                Self::ZhCn => "标签已移动 · Ctrl+B w 刷新标签顺序".to_owned(),
            },
            Phase::SelectedWithoutAttachment => {
                let Some(position) = position else {
                    return self
                        .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match provenance {
                    Some(Provenance::Pending) => self.native_terminal_pending(position),
                    Some(Provenance::Unavailable) => self.native_terminal_unavailable(position),
                    Some(Provenance::Failed) => self.native_terminal_failed(position),
                    Some(Provenance::OutcomeUnknown) => {
                        self.native_terminal_outcome_unknown(position)
                    }
                    Some(Provenance::Started | Provenance::BindingOnly) | None => {
                        self.native_terminal_unavailable(position)
                    }
                }
            }
            Phase::RetryableAttachUnavailable => position.map_or_else(
                || {
                    self.terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned()
                },
                |position| self.native_terminal_unavailable(position),
            ),
            Phase::Retired => self
                .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                .to_owned(),
        }
    }

    pub(crate) fn native_refresh_status(
        self,
        status: crate::terminal_session::NativeRefreshStatus,
    ) -> String {
        use crate::terminal_session::NativeRefreshStatus;
        match status {
            NativeRefreshStatus::Refreshing => self.native_refreshing().to_owned(),
            NativeRefreshStatus::Success(revision) => self.native_refresh_success(revision),
            NativeRefreshStatus::Failure(revision) => self.native_refresh_failure(revision),
        }
    }

    pub(crate) fn native_notice(
        self,
        notice: crate::terminal_session::NativeSessionNotice,
    ) -> String {
        use crate::terminal_session::NativeSessionNotice;
        match notice {
            NativeSessionNotice::LocalBusy => self.native_local_busy().to_owned(),
            NativeSessionNotice::NoTabAtPosition(position) => self.native_no_tab(position),
            NativeSessionNotice::CreateNotCreated => self.native_create_not_created().to_owned(),
            NativeSessionNotice::CloseRequestNotApplied => match self {
                Self::EnUs => "Close request was not applied".to_owned(),
                Self::ZhCn => "关闭请求未执行".to_owned(),
            },
            NativeSessionNotice::MoveNotMoved => match self {
                Self::EnUs => "Tab was not moved".to_owned(),
                Self::ZhCn => "标签未移动".to_owned(),
            },
        }
    }

    fn native_starting_status(
        self,
        position: Option<usize>,
        projection: crate::terminal_session::NativeProjectionStatus,
    ) -> String {
        self.native_connection_status(
            position,
            projection,
            crate::terminal_session::TerminalStatus::Connecting,
        )
    }

    fn native_switching_status(
        self,
        position: Option<usize>,
        projection: crate::terminal_session::NativeProjectionStatus,
    ) -> String {
        self.native_connection_status(
            position,
            projection,
            crate::terminal_session::TerminalStatus::Unavailable,
        )
    }

    fn native_connection_status(
        self,
        position: Option<usize>,
        projection: crate::terminal_session::NativeProjectionStatus,
        missing_position: crate::terminal_session::TerminalStatus,
    ) -> String {
        use crate::terminal_session::NativeProjectionStatus;

        let Some(position) = position else {
            return self.terminal_status(missing_position).to_owned();
        };
        match projection {
            NativeProjectionStatus::WaitingForFullFrame => self.native_waiting_for_frame(position),
            NativeProjectionStatus::Blocked => self.native_display_blocked(position),
            NativeProjectionStatus::Empty | NativeProjectionStatus::FrameAvailable => {
                self.native_connecting(position)
            }
        }
    }

    const fn native_creating(self) -> &'static str {
        match self {
            Self::EnUs => "Creating Tab…",
            Self::ZhCn => "正在创建 Tab…",
        }
    }

    const fn native_create_not_created(self) -> &'static str {
        match self {
            Self::EnUs => "Tab not created · previous Tab unchanged",
            Self::ZhCn => "未创建 Tab · 原 Tab 未更改",
        }
    }

    const fn native_refreshing(self) -> &'static str {
        match self {
            Self::EnUs => "Refreshing Tabs…",
            Self::ZhCn => "正在刷新 Tab…",
        }
    }

    fn native_refresh_success(self, revision: u64) -> String {
        match self {
            Self::EnUs => format!("Tabs refreshed · revision {revision}"),
            Self::ZhCn => format!("Tab 已刷新 · 修订 {revision}"),
        }
    }

    fn native_refresh_failure(self, revision: u64) -> String {
        match self {
            Self::EnUs => format!("Tab refresh failed · showing revision {revision}"),
            Self::ZhCn => format!("Tab 刷新失败 · 正在显示修订 {revision}"),
        }
    }

    const fn native_local_busy(self) -> &'static str {
        match self {
            Self::EnUs => "Client busy · no command sent",
            Self::ZhCn => "Client 正忙 · 未发送命令",
        }
    }

    fn native_no_tab(self, position: u8) -> String {
        match self {
            Self::EnUs => format!("No Tab at position {position}"),
            Self::ZhCn => format!("第 {position} 个 Tab 不存在"),
        }
    }

    fn native_connecting(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · Connecting…"),
            Self::ZhCn => format!("Tab {position} · 正在连接…"),
        }
    }

    fn native_waiting_for_frame(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · Connecting · waiting for full frame"),
            Self::ZhCn => format!("Tab {position} · 正在连接 · 等待完整帧"),
        }
    }

    fn native_display_blocked(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · Connecting · display blocked"),
            Self::ZhCn => format!("Tab {position} · 正在连接 · 显示已阻塞"),
        }
    }

    fn native_ready_controlling(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · Ready · controlling"),
            Self::ZhCn => format!("Tab {position} · 已就绪 · 可控制"),
        }
    }

    fn native_ready_read_only(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · Ready · view only"),
            Self::ZhCn => format!("Tab {position} · 已就绪 · 仅查看"),
        }
    }

    const fn native_committed_refresh_required(self) -> &'static str {
        match self {
            Self::EnUs => "Created Tab committed · refresh required",
            Self::ZhCn => "已提交创建的 Tab · 需要刷新",
        }
    }

    fn native_terminal_pending(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · terminal creation pending · no replay"),
            Self::ZhCn => format!("Tab {position} · 终端创建等待中 · 不会重放"),
        }
    }

    fn native_terminal_unavailable(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · terminal unavailable"),
            Self::ZhCn => format!("Tab {position} · 终端不可用"),
        }
    }

    fn native_terminal_failed(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · terminal creation failed"),
            Self::ZhCn => format!("Tab {position} · 终端创建失败"),
        }
    }

    fn native_terminal_outcome_unknown(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position} · terminal result unknown · no replay"),
            Self::ZhCn => format!("Tab {position} · 终端结果未知 · 不会重放"),
        }
    }
}

#[cfg(target_os = "linux")]
impl Locale {
    pub(crate) const fn startup_progress(self) -> &'static str {
        match self {
            Self::EnUs => "Starting the local terminal…\n",
            Self::ZhCn => "正在启动本地终端…\n",
        }
    }
    pub(crate) const fn startup_failure(
        self,
        failure: crate::local_startup::StartupFailure,
    ) -> &'static str {
        use crate::local_startup::StartupFailure;
        match (self, failure) {
            (Self::EnUs, StartupFailure::Interface) => {
                "The terminal interface failed. Existing Site processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::Interface) => "终端界面运行失败。已有 Site 进程已保留。\n",
            (Self::EnUs, StartupFailure::Terminal) => {
                "Orchbee requires the same interactive terminal on standard input and output, with valid dimensions.\n"
            }
            (Self::ZhCn, StartupFailure::Terminal) => {
                "Orchbee 的标准输入和输出需要连接同一交互终端，并提供有效尺寸。\n"
            }
            (Self::EnUs, StartupFailure::Cohort) => {
                "The native development cohort is missing, changed, or invalid. Stage all four binaries together before running orchbee.\n"
            }
            (Self::ZhCn, StartupFailure::Cohort) => {
                "原生开发 cohort 缺失、已修改或无效。请将四个二进制一起暂存后再运行 orchbee。\n"
            }
            (Self::EnUs, StartupFailure::Runtime) => {
                "The local Site runtime failed its ownership or permission checks.\n"
            }
            (Self::ZhCn, StartupFailure::Runtime) => "本地 Site 运行时未通过所有者或权限检查。\n",
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                "The local Site requires explicit recovery. No replacement process was started.\n"
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                "本地 Site 需要显式恢复。未启动替代进程。\n"
            }
            (Self::EnUs, StartupFailure::Incompatible) => {
                "The running local Site is incompatible with this development cohort.\n"
            }
            (Self::ZhCn, StartupFailure::Incompatible) => {
                "正在运行的本地 Site 与当前开发 cohort 不兼容。\n"
            }
            (Self::EnUs, StartupFailure::Admission) => {
                "The local Site could not admit this Client connection.\n"
            }
            (Self::ZhCn, StartupFailure::Admission) => "本地 Site 无法准入此 Client 连接。\n",
            (Self::EnUs, StartupFailure::TimedOut) => {
                "Local terminal startup timed out. Existing Site processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::TimedOut) => "本地终端启动超时。已有 Site 进程已保留。\n",
            (Self::EnUs, StartupFailure::TerminalUnavailable) => {
                "The local terminal is unavailable. No terminal creation was retried.\n"
            }
            (Self::ZhCn, StartupFailure::TerminalUnavailable) => {
                "本地终端不可用。未重试终端创建。\n"
            }
            (Self::EnUs, StartupFailure::OutcomeUnknown) => {
                "The terminal operation outcome is unknown. It was not replayed.\n"
            }
            (Self::ZhCn, StartupFailure::OutcomeUnknown) => "终端操作结果未知。未重放该操作。\n",
        }
    }
}
