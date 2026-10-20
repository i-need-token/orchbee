#[cfg(windows)]
mod action_menu;
#[cfg(windows)]
mod agent_projection;
mod control_takeover;
mod directory_startup;
#[cfg(windows)]
mod machines;
#[cfg(windows)]
mod navigator;
mod notification;
mod operation;
mod presentation;
#[cfg(windows)]
mod tab_menu;
pub(crate) use presentation::RecoveryVerb;
#[cfg(windows)]
mod plugins;
mod shortcut_help;
pub(crate) use shortcut_help::{ShortcutHelpEntry, shortcut_filter_matches};
mod runtime_configuration;
mod settings;
mod status;
mod terminal_feedback;
#[cfg(windows)]
mod update;
mod window_title;
use std::ffi::OsString;

use crate::client_view::{BackendAction, LabelStyle, Notice, PrefixDesc, format_binding};

mod agent_manifests;
mod cli;
pub(crate) use cli::{HelpDetails, HelpText};
#[cfg(windows)]
mod configured_commands;
#[cfg(windows)]
mod copy_mode;
#[cfg(windows)]
mod events;
#[cfg(windows)]
mod pane_graphics;
#[cfg(windows)]
mod panel_reports;
pub(crate) use cli::{CliLabels, DetailLabel};
#[cfg(any(unix, windows))]
mod location_labels;
#[cfg(any(unix, windows))]
mod panel_close;
#[cfg(any(unix, windows))]
mod tab_close;
#[cfg(any(unix, windows))]
mod tab_rename;
#[cfg(any(unix, windows))]
mod workspace_close;
#[cfg(any(unix, windows))]
mod workspace_setup;
#[cfg(windows)]
mod worktree_launch;

const EN_US_TUI_UNAVAILABLE: &str = "The Orchbee TUI is unavailable in this development slice.\n";
const ZH_CN_TUI_UNAVAILABLE: &str = "Orchbee TUI 在本开发切片中尚不可用。\n";
const EN_US_USAGE_FAILURE: &str = "Invalid invocation. Run 'orchbee --help'.\n";
const ZH_CN_USAGE_FAILURE: &str = "调用方式无效。请运行 ‘orchbee --help’。\n";

fn en_us_prefix_desc(desc: PrefixDesc) -> &'static str {
    match desc {
        #[cfg(windows)]
        PrefixDesc::DismissOnboarding => "Dismiss the welcome guide",
        #[cfg(windows)]
        PrefixDesc::ConfiguredCommand(_) => "Run configured command",
        #[cfg(windows)]
        PrefixDesc::ResizeMode => "Enter pane resize mode",
        #[cfg(windows)]
        PrefixDesc::CopyMode => "Enter keyboard copy mode",
        #[cfg(windows)]
        PrefixDesc::CopyMove => "Move the copy cursor",
        #[cfg(windows)]
        PrefixDesc::CopyWord => "Move by word / whitespace-delimited word",
        #[cfg(windows)]
        PrefixDesc::CopyParagraph => "Move to previous / next paragraph",
        #[cfg(windows)]
        PrefixDesc::CopyPage => "Page older / newer content (pages help while open)",
        #[cfg(windows)]
        PrefixDesc::CopyEnds => "Go to oldest / newest content",
        #[cfg(windows)]
        PrefixDesc::CopySearch => "Search forward / backward; Enter confirms",
        #[cfg(windows)]
        PrefixDesc::CopyRepeatSearch => "Repeat search / reverse direction",
        #[cfg(windows)]
        PrefixDesc::CopyReverseSearch => "Reverse the search draft direction",
        #[cfg(windows)]
        PrefixDesc::CopySelection => "Start / cancel selection",
        #[cfg(windows)]
        PrefixDesc::CopySubmit => "Copy selection; return live after success",
        #[cfg(windows)]
        PrefixDesc::CopyExit => "Leave copy mode (Esc also cancels search)",
        #[cfg(windows)]
        PrefixDesc::ResizeLeft => "Move pane divider left by 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeRight => "Move pane divider right by 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeUp => "Move pane divider up by 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeDown => "Move pane divider down by 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeCoarse => "Move divider by 5 percentage points",
        #[cfg(windows)]
        PrefixDesc::ResizeFine => "Move divider by 1 percentage point",
        #[cfg(windows)]
        PrefixDesc::ResizeExit => "Leave resize mode",
        #[cfg(windows)]
        PrefixDesc::Machines => "Choose local or saved SSH machine",
        #[cfg(windows)]
        PrefixDesc::ActionMenu => "Open action menu",
        PrefixDesc::Settings => "Open settings",
        #[cfg(windows)]
        PrefixDesc::AgentCycle => "Select previous / next Agent in the filtered machine view",
        #[cfg(windows)]
        PrefixDesc::AgentIndex => "Focus numbered Agent in the filtered machine view",
        PrefixDesc::ReloadConfiguration => "Reload session configuration",
        PrefixDesc::QuickWorkspaceCreate => "Create a Workspace and enter its first terminal",
        #[cfg(windows)]
        PrefixDesc::WorktreeCreate => "Create a Git worktree and enter its terminal",
        #[cfg(windows)]
        PrefixDesc::WorktreeOpen => "Open an existing Git checkout",
        PrefixDesc::ControlTakeover => "Take control of the current pane; use this window's size",
        PrefixDesc::WorkspaceClose => {
            "Close current Workspace and its terminals (asks for confirmation)"
        }
        PrefixDesc::WorkspaceCreate => "Create a Workspace and enter its first terminal",
        PrefixDesc::WorkspaceDirectory => "Configure an empty Workspace's first directory",
        PrefixDesc::WorkspaceRename => "Rename current Workspace",
        PrefixDesc::ProjectRename => "Rename current Workspace's Project",
        PrefixDesc::RenameTab => "Rename current Tab",
        PrefixDesc::CloseTab => "Close current Tab / retry Close (confirms for all Clients)",
        PrefixDesc::CreateTab => "Create a Tab (requires session)",
        PrefixDesc::MoveTab => "Move current Tab left / right (wraps)",
        PrefixDesc::SelectTabWrap => "Select next / previous Tab (requires session)",
        PrefixDesc::SelectContext => {
            "Select previous / next Workbench location (wraps; requires session)"
        }
        PrefixDesc::SelectTabDigit => "Select ordered Tab (requires session)",
        PrefixDesc::PanelFocus => "Move between Panels (requires session)",
        PrefixDesc::PresentationSelect => "Select previous / next terminal in this Panel",
        PrefixDesc::PanelZoom => "Zoom the focused Panel (requires session)",
        PrefixDesc::SplitPanel => "Split focused Panel below/above/left/right (requires session)",
        PrefixDesc::ClosePanel => "Close focused Panel (requires session)",
        PrefixDesc::MovePanel => "Move focused Panel to that side (requires session)",
        PrefixDesc::SidebarToggle => "Toggle business sidebar",
        #[cfg(windows)]
        PrefixDesc::SidebarFocus => "Switch sidebar / terminal focus",
        PrefixDesc::NavigatorToggle => "Toggle Workbench navigator",
        PrefixDesc::BoardToggle => "Toggle the Agent board",
        PrefixDesc::ShowHelp => "Open or close this shortcut help",
        PrefixDesc::HelpPage => "Page this shortcut help while it is open",
        PrefixDesc::PointerCapture => "Release or restore mouse capture",
        PrefixDesc::Exit => Locale::EnUs.client_detach_label(),
        PrefixDesc::LiteralPrefix => "Send the literal prefix to the terminal input port",
        PrefixDesc::DismissPrefix => "Dismiss the prefix without an action",
    }
}

fn zh_cn_prefix_desc(desc: PrefixDesc) -> &'static str {
    match desc {
        #[cfg(windows)]
        PrefixDesc::DismissOnboarding => "关闭首次运行引导",
        #[cfg(windows)]
        PrefixDesc::ConfiguredCommand(_) => "运行配置命令",
        #[cfg(windows)]
        PrefixDesc::ResizeMode => "进入 pane 大小调整模式",
        #[cfg(windows)]
        PrefixDesc::CopyMode => "进入键盘复制模式",
        #[cfg(windows)]
        PrefixDesc::CopyMove => "移动复制光标",
        #[cfg(windows)]
        PrefixDesc::CopyWord => "按词 / 空白分隔的大词移动",
        #[cfg(windows)]
        PrefixDesc::CopyParagraph => "移动到上一个 / 下一个段落",
        #[cfg(windows)]
        PrefixDesc::CopyPage => "翻看更早 / 更新内容（帮助打开时翻页）",
        #[cfg(windows)]
        PrefixDesc::CopyEnds => "跳到最旧 / 最新内容",
        #[cfg(windows)]
        PrefixDesc::CopySearch => "向前 / 向后搜索；Enter 确认",
        #[cfg(windows)]
        PrefixDesc::CopyRepeatSearch => "重复搜索 / 反向跳转",
        #[cfg(windows)]
        PrefixDesc::CopyReverseSearch => "反转搜索草稿方向",
        #[cfg(windows)]
        PrefixDesc::CopySelection => "开始 / 取消选择",
        #[cfg(windows)]
        PrefixDesc::CopySubmit => "复制选区；成功后回实时显示",
        #[cfg(windows)]
        PrefixDesc::CopyExit => "退出复制模式（Esc 也取消搜索）",
        #[cfg(windows)]
        PrefixDesc::ResizeLeft => "分隔线向左移动 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeRight => "分隔线向右移动 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeUp => "分隔线向上移动 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeDown => "分隔线向下移动 5%",
        #[cfg(windows)]
        PrefixDesc::ResizeCoarse => "分隔线移动 5 个百分点",
        #[cfg(windows)]
        PrefixDesc::ResizeFine => "分隔线移动 1 个百分点",
        #[cfg(windows)]
        PrefixDesc::ResizeExit => "退出大小调整模式",
        #[cfg(windows)]
        PrefixDesc::Machines => "选择本地或已保存的 SSH 机器",
        #[cfg(windows)]
        PrefixDesc::ActionMenu => "打开操作菜单",
        PrefixDesc::Settings => "打开设置",
        #[cfg(windows)]
        PrefixDesc::AgentCycle => "按机器视图的筛选顺序切换上一个 / 下一个 Agent",
        #[cfg(windows)]
        PrefixDesc::AgentIndex => "按机器视图的筛选顺序聚焦编号 Agent",
        PrefixDesc::ReloadConfiguration => "重载会话配置",
        PrefixDesc::QuickWorkspaceCreate => "创建工作区并进入第一个终端",
        #[cfg(windows)]
        PrefixDesc::工作树Create => "创建 Git 工作树 并进入终端",
        #[cfg(windows)]
        PrefixDesc::工作树Open => "打开已有 Git 检出目录",
        PrefixDesc::ControlTakeover => "接管当前 pane；使用本窗口尺寸",
        PrefixDesc::WorkspaceClose => "关闭当前工作区及其终端（需要确认）",
        PrefixDesc::WorkspaceCreate => "创建工作区并进入第一个终端",
        PrefixDesc::WorkspaceDirectory => "配置空工作区的首个目录",
        PrefixDesc::WorkspaceRename => "重命名当前工作区",
        PrefixDesc::ProjectRename => "重命名当前工作区所属的项目",
        PrefixDesc::RenameTab => "重命名当前 Tab",
        PrefixDesc::CloseTab => "关闭当前 Tab / 重试关闭（确认后影响所有 Client）",
        PrefixDesc::CreateTab => "创建 Tab（需要会话）",
        PrefixDesc::MoveTab => "向左 / 右移动当前 Tab（首尾循环）",
        PrefixDesc::SelectTabWrap => "选择下一个 / 上一个 Tab（需要会话）",
        PrefixDesc::SelectContext => "选择上一个 / 下一个 Workbench 位置（首尾循环；需要会话）",
        PrefixDesc::SelectTabDigit => "选择对应顺序的 Tab（需要会话）",
        PrefixDesc::PanelFocus => "在 Panel 之间移动（需要会话）",
        PrefixDesc::PresentationSelect => "选择当前 Panel 中的上一个 / 下一个终端",
        PrefixDesc::PanelZoom => "放大焦点 Panel（需要会话）",
        PrefixDesc::SplitPanel => "将焦点 Panel 向下/上/左/右分屏（需要会话）",
        PrefixDesc::ClosePanel => "关闭焦点 Panel（需要会话）",
        PrefixDesc::MovePanel => "将焦点 Panel 移向该侧（需要会话）",
        PrefixDesc::SidebarToggle => "切换业务侧边栏",
        #[cfg(windows)]
        PrefixDesc::SidebarFocus => "切换侧栏／终端焦点",
        PrefixDesc::NavigatorToggle => "切换 Workbench 导航器",
        PrefixDesc::BoardToggle => "切换智能体看板",
        PrefixDesc::ShowHelp => "打开或关闭此快捷键帮助",
        PrefixDesc::HelpPage => "在帮助打开时翻看此快捷键帮助的页面",
        PrefixDesc::PointerCapture => "释放或恢复鼠标捕获",
        PrefixDesc::Exit => Locale::ZhCn.client_detach_label(),
        PrefixDesc::LiteralPrefix => "向终端输入端口发送字面前缀",
        PrefixDesc::DismissPrefix => "取消前缀而不执行动作",
    }
}

/// The Agent board's localized copy. Read-state lines, jump-refusal notices,
/// action labels and shortcut verbs live here; the Board key map supplies the
/// separately formatted effective key labels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentBoardText {
    pub(crate) title: &'static str,
    pub(crate) open_button: &'static str,
    pub(crate) refresh_button: &'static str,
    /// No completed read yet — never rendered as "no Agents".
    pub(crate) unread: &'static str,
    pub(crate) in_flight: &'static str,
    /// The page walk exhausted its bound — shown, never hidden.
    pub(crate) incomplete: &'static str,
    pub(crate) read_denied: &'static str,
    pub(crate) read_unavailable: &'static str,
    pub(crate) read_transport: &'static str,
    pub(crate) lane_unavailable: &'static str,
    /// A completed read found zero sessions.
    pub(crate) empty: &'static str,
    /// The next step shown when the last completed Agent read has no rows.
    pub(crate) empty_start: &'static str,
    /// CLI syntax used to start an Agent from the current idle terminal.
    pub(crate) empty_start_command: &'static str,
    /// A partial empty read is not proof that there are no sessions.
    pub(crate) empty_partial: &'static str,
    /// PATH-backed startup form copy.
    pub(crate) start_checking: &'static str,
    pub(crate) start_catalog_unavailable: &'static str,
    pub(crate) start_none_found: &'static str,
    pub(crate) start_providers: &'static str,
    pub(crate) start_name: &'static str,
    pub(crate) start_name_placeholder: &'static str,
    pub(crate) start_name_invalid: &'static str,
    pub(crate) start_button: &'static str,
    pub(crate) start_cancel_button: &'static str,
    pub(crate) start_starting: &'static str,
    pub(crate) start_waiting_input: &'static str,
    pub(crate) start_unconfirmed: &'static str,
    pub(crate) start_cancelling: &'static str,
    pub(crate) start_started: &'static str,
    pub(crate) start_failed: &'static str,
    pub(crate) start_unknown: &'static str,
    pub(crate) start_cancelled: &'static str,
    pub(crate) start_provider_hint: &'static str,
    pub(crate) start_edit_hint: &'static str,
    pub(crate) start_cancel_hint: &'static str,
    pub(crate) jumping: &'static str,
    pub(crate) close_hint: &'static str,
    pub(crate) select_hint: &'static str,
    pub(crate) jump_hint: &'static str,
    pub(crate) refresh_hint: &'static str,
    pub(crate) stale: &'static str,
    pub(crate) disconnected: &'static str,
    pub(crate) standalone: &'static str,
    pub(crate) jump_gone: &'static str,
    pub(crate) jump_unavailable: &'static str,
    pub(crate) jump_denied: &'static str,
    pub(crate) jump_ended: &'static str,
    pub(crate) jump_no_placement: &'static str,
    pub(crate) jump_expired: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShellText<'a> {
    pub(crate) agents: &'static str,
    pub(crate) app_title: &'static str,
    pub(crate) board: AgentBoardText,
    pub(crate) backend_status: &'static str,
    pub(crate) business_section: &'static str,
    pub(crate) exit_hint: &'a str,
    pub(crate) hint_short: &'a str,
    pub(crate) binding_unbound: &'static str,
    #[cfg(not(windows))]
    pub(crate) overlay_input_hint: &'static str,
    #[cfg(test)]
    pub(crate) help_close_label: &'static str,
    #[cfg(test)]
    pub(crate) help_previous_label: &'static str,
    #[cfg(test)]
    pub(crate) help_next_label: &'static str,
    #[cfg(test)]
    pub(crate) help_pages_label: &'static str,
    #[cfg(test)]
    pub(crate) help_escape_close_hint: &'static str,
    pub(crate) overlay_refused: &'static str,
    #[cfg(not(windows))]
    pub(crate) navigator_detail: &'static str,
    pub(crate) navigator_title: &'static str,
    pub(crate) no_client_session: &'static str,
    pub(crate) panel_title: &'static str,
    pub(crate) pointer_released: &'a str,
    pub(crate) prefix_active: &'a str,
    pub(crate) prefix_desc: fn(PrefixDesc) -> &'static str,
    pub(crate) prefix_idle: &'a str,
    pub(crate) projects: &'static str,
    pub(crate) shell_only_detail: &'static str,
    pub(crate) shortcut_title: &'static str,
    pub(crate) sidebar_hint: &'a str,
    pub(crate) site_section: &'static str,
    pub(crate) standalone: &'static str,
    pub(crate) tabs_unavailable: &'static str,
    pub(crate) terminal_unavailable: &'static str,
    pub(crate) too_small_notice: &'static str,
    pub(crate) workbench_title: &'static str,
    pub(crate) pinned_tab: &'static str,
    pub(crate) unnamed_tab_group: &'static str,
    pub(crate) workspaces: &'static str,
    pub(crate) zoom_badge: &'a str,
}

const EN_US_BOARD: AgentBoardText = AgentBoardText {
    title: "Agent board",
    open_button: "Open",
    refresh_button: "Refresh",
    unread: "No completed Agent read yet",
    in_flight: "Reading Agent sessions…",
    incomplete: "listing incomplete",
    read_denied: "Agent read denied",
    read_unavailable: "Agent runtime unavailable",
    read_transport: "Agent read failed: transport",
    lane_unavailable: "Agent board unavailable: no read lane",
    empty: "No Agent sessions",
    empty_start: "Choose an installed Agent. Its name is ready to use or edit.",
    empty_start_command: "orchbee agent start <name> --provider <provider> --current",
    empty_partial: "No sessions in this partial read.",
    start_checking: "Looking for installed commands; you can choose now.",
    start_catalog_unavailable: "Detection incomplete. Choose an Agent installed in this terminal.",
    start_none_found: "Nothing detected. You can still choose an installed Agent.",
    start_providers: "Found on PATH",
    start_name: "Agent name",
    start_name_placeholder: "choose a name",
    start_name_invalid: "Name: 1–32 lowercase letters, digits, _ or -; start with a letter.",
    start_button: "Start Agent",
    start_cancel_button: "Cancel start",
    start_starting: "Starting Agent — continue in the terminal.",
    start_waiting_input: "Your input is needed in the terminal to continue startup.",
    start_unconfirmed: "Command sent. Check the terminal prompts; readiness is not confirmed. Do not start again.",
    start_cancelling: "Cancelling startup…",
    start_started: "Agent started. Refreshing sessions…",
    start_failed: "Agent did not start. Check the current panel and try again.",
    start_unknown: "Startup outcome is unknown. Refresh sessions before retrying.",
    start_cancelled: "Startup canceled. The Agent process may still be running.",
    start_provider_hint: "more Agents",
    start_edit_hint: "edit name",
    start_cancel_hint: "cancel start",
    jumping: "Jumping…",
    close_hint: "close",
    select_hint: "select",
    jump_hint: "jump",
    refresh_hint: "refresh",
    stale: "stale",
    disconnected: "disconnected",
    standalone: "Legacy workspace",
    jump_gone: "Session or its panel is gone",
    jump_unavailable: "Cannot verify session · read unavailable",
    jump_denied: "Session read denied",
    jump_ended: "Session has ended",
    jump_no_placement: "Session has no navigable panel",
    jump_expired: "Jump timed out",
};

const EN_US_SHELL: ShellText<'static> = ShellText {
    agents: "Agents",
    app_title: "ORCHBEE",
    board: EN_US_BOARD,
    backend_status: "Backend unavailable",
    business_section: "BUSINESS",
    exit_hint: "Ctrl+B q · exit",
    hint_short: "Ctrl+B ? · help",
    overlay_refused: "Not enough room to show this view · resize the terminal",
    #[cfg(not(windows))]
    navigator_detail: "A session connection is required before Workbench locations can be shown.",
    navigator_title: "Workbench navigator",
    no_client_session: "No Orchbee session is connected.",
    binding_unbound: "Unbound",
    #[cfg(not(windows))]
    overlay_input_hint: "Terminal input continues while this overlay is visible.",
    #[cfg(test)]
    help_close_label: "Close help",
    #[cfg(test)]
    help_previous_label: "Previous page",
    #[cfg(test)]
    help_next_label: "Next page",
    #[cfg(test)]
    help_pages_label: "Turn pages",
    #[cfg(test)]
    help_escape_close_hint: "Esc close",
    panel_title: "Terminal",
    pointer_released: "mouse released · Ctrl+B m restores",
    prefix_active: "PREFIX Ctrl+B",
    prefix_desc: en_us_prefix_desc,
    prefix_idle: "Ctrl+B · shortcuts",
    projects: "Projects",
    shell_only_detail: "Connect to a session to use terminals and agents.",
    shortcut_title: "Keyboard shortcuts",
    sidebar_hint: "Ctrl+B b · sidebar",
    site_section: "SITE",
    standalone: "Legacy workspace",
    tabs_unavailable: "No shared Tabs · session unavailable",
    terminal_unavailable: "Terminal unavailable",
    too_small_notice: "Too small · resize to continue",
    workbench_title: "WORKBENCH",
    pinned_tab: "Pinned",
    unnamed_tab_group: "Unnamed group",
    workspaces: "Workspaces",
    zoom_badge: "zoomed · Ctrl+B z restores",
};

const ZH_CN_BOARD: AgentBoardText = AgentBoardText {
    open_button: "打开",
    refresh_button: "刷新",
    title: "智能体看板",
    unread: "尚未完成智能体读取",
    in_flight: "正在读取智能体会话…",
    incomplete: "列表不完整",
    read_denied: "智能体读取被拒绝",
    read_unavailable: "智能体运行时不可用",
    read_transport: "智能体读取失败：传输错误",
    lane_unavailable: "看板不可用：无读取通道",
    empty: "没有智能体会话",
    empty_start: "选择已安装的智能体，名称已填好，也可修改。",
    empty_start_command: "orchbee agent start <name> --provider <provider> --current",
    empty_partial: "本次不完整读取暂未返回会话。",
    start_checking: "正在查找已安装的命令；也可直接选择。",
    start_catalog_unavailable: "检测未完成，请选择此终端中已安装的智能体。",
    start_none_found: "未检测到命令，仍可手动选择已安装的智能体。",
    start_providers: "已在 PATH 中找到",
    start_name: "智能体名称",
    start_name_placeholder: "请输入名称",
    start_name_invalid: "请以小写字母开头，后接字母、数字、_ 或 -，最多 32 个字符。",
    start_button: "启动智能体",
    start_cancel_button: "取消启动",
    start_starting: "正在启动智能体，请在终端中继续。",
    start_waiting_input: "请在终端中确认或输入，以继续启动。",
    start_unconfirmed: "命令已发送，请检查终端提示；尚未确认就绪，请勿重复启动。",
    start_cancelling: "正在取消启动…",
    start_started: "智能体已启动，正在刷新会话…",
    start_failed: "智能体未启动，请检查当前面板后重试。",
    start_unknown: "启动结果未知，请先刷新会话再决定是否重试。",
    start_cancelled: "已取消启动；智能体进程仍可能在运行。",
    start_provider_hint: "更多智能体",
    start_edit_hint: "编辑名称",
    start_cancel_hint: "取消启动",
    jumping: "正在跳转…",
    close_hint: "关闭",
    select_hint: "选择",
    jump_hint: "跳转",
    refresh_hint: "刷新",
    stale: "已过期",
    disconnected: "已断连",
    standalone: "原有工作区",
    jump_gone: "会话或其面板已不存在",
    jump_unavailable: "无法验证会话 · 读取不可用",
    jump_denied: "会话读取被拒绝",
    jump_ended: "会话已结束",
    jump_no_placement: "会话没有可跳转的面板",
    jump_expired: "跳转超时",
};

const ZH_CN_SHELL: ShellText<'static> = ShellText {
    agents: "智能体",
    app_title: "ORCHBEE",
    board: ZH_CN_BOARD,
    backend_status: "后端不可用",
    business_section: "业务",
    exit_hint: "Ctrl+B q · 退出",
    hint_short: "Ctrl+B ? · 帮助",
    overlay_refused: "空间不足，无法显示此视图 · 请调整终端大小",
    #[cfg(not(windows))]
    navigator_detail: "连接会话后才能显示 Workbench 位置。",
    navigator_title: "Workbench 导航器",
    no_client_session: "当前未连接 Orchbee 会话。",
    binding_unbound: "未绑定",
    #[cfg(not(windows))]
    overlay_input_hint: "此浮层显示时，终端输入仍会继续传递。",
    #[cfg(test)]
    help_close_label: "关闭帮助",
    #[cfg(test)]
    help_previous_label: "上一页",
    #[cfg(test)]
    help_next_label: "下一页",
    #[cfg(test)]
    help_pages_label: "翻页",
    #[cfg(test)]
    help_escape_close_hint: "Esc 关闭帮助",
    panel_title: "终端",
    pointer_released: "鼠标已释放 · Ctrl+B m 恢复",
    prefix_active: "前缀 Ctrl+B",
    prefix_desc: zh_cn_prefix_desc,
    prefix_idle: "Ctrl+B · 快捷键",
    projects: "项目",
    shell_only_detail: "连接会话后即可使用终端和智能体。",
    shortcut_title: "键盘快捷键",
    sidebar_hint: "Ctrl+B b · 侧边栏",
    site_section: "站点",
    standalone: "原有工作区",
    tabs_unavailable: "无共享 Tab · 会话不可用",
    terminal_unavailable: "终端不可用",
    too_small_notice: "窗口太小 · 请调整大小后继续",
    workbench_title: "WORKBENCH",
    pinned_tab: "固定",
    unnamed_tab_group: "未命名分组",
    workspaces: "工作区",
    zoom_badge: "已放大 · Ctrl+B z 恢复",
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    pub(crate) fn tui_detached(self, return_command: &str) -> String {
        match self {
            Self::EnUs => format!(
                "Detached from Orchbee. Terminals keep running; run \"{return_command}\" to return."
            ),
            Self::ZhCn => {
                format!("已离开 Orchbee，终端程序仍在后台运行；输入 {return_command} 返回。")
            }
        }
    }

    /// Help and the action menu share the current Client's detach lifecycle wording.
    const fn client_detach_label(self) -> &'static str {
        match self {
            Self::EnUs => "Leave view; terminals keep running",
            Self::ZhCn => "离开界面；终端继续运行",
        }
    }

    pub(crate) const fn tui_measurement_path_invalid(self) -> &'static str {
        match self {
            Self::EnUs => "ORCHBEE_TUI_MEASUREMENTS must name a new absolute file path",
            Self::ZhCn => "ORCHBEE_TUI_MEASUREMENTS 必须指定新的绝对文件路径",
        }
    }

    pub(crate) const fn tui_measurement_also_failed(self) -> &'static str {
        match self {
            Self::EnUs => "TUI timing diagnostics also failed",
            Self::ZhCn => "TUI 计时诊断也失败",
        }
    }

    pub(crate) const fn tui_restoration_also_failed(self) -> &'static str {
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

    #[cfg(windows)]
    pub(crate) const fn navigator_columns(self) -> (&'static str, &'static str) {
        match self {
            Self::EnUs => ("Workspaces / terminals", "Tabs in this location"),
            Self::ZhCn => ("工作区 / 终端", "当前位置的 Tab"),
        }
    }

    #[cfg(windows)]
    pub(crate) const fn resize_done_label(self) -> &'static str {
        match self {
            Self::EnUs => "Done",
            Self::ZhCn => "完成",
        }
    }

    pub(crate) const fn shell_notice(self, notice: Option<Notice>) -> &'static str {
        match (self, notice) {
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::OnboardingSaveFailed)) => {
                "Guide closed here; saving failed, so it may return next time"
            }
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::OnboardingSaveFailed)) => {
                "本次引导已关闭；保存失败，下次可能再次显示"
            }
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::ResizeBusy)) => "Resize pending; input was not queued",
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::ResizeBusy)) => "大小调整待定；输入未排队",
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::ResizeFailed)) => "Resize failed; inspect the current layout",
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::ResizeFailed)) => "大小调整失败；请检查当前布局",
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::ResizeUnknown)) => {
                "Resize outcome unknown; inspect layout before trying again"
            }
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::ResizeUnknown)) => "大小调整结果不确定；请先检查布局再操作",
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::ResizeNoSplit)) => "No divider on this axis",
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::ResizeNoSplit)) => "该方向没有分隔线",
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::ResizeLimit)) => "Divider is at its size limit",
            #[cfg(windows)]
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
            (Self::EnUs, Some(Notice::ConfigurationRestart)) => {
                "Keys updated; some session settings need a restart"
            }
            (Self::ZhCn, Some(Notice::ConfigurationRestart)) => "键位已更新；部分会话设置需要重启",
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
            #[cfg(windows)]
            (
                Self::EnUs,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "Worktree unavailable · no command was sent",
            #[cfg(windows)]
            (
                Self::ZhCn,
                Some(Notice::BackendUnavailable(
                    BackendAction::CreateWorktree | BackendAction::OpenWorktree,
                )),
            ) => "工作树 不可用 · 未发送命令",
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
            (Self::EnUs, None) => "Session unavailable · no command has been sent",
            (Self::ZhCn, None) => "会话不可用 · 未发送任何命令",
            (Self::EnUs, Some(Notice::TerminalInputUnavailable)) => {
                "Terminal input unavailable · input was not sent"
            }
            (Self::ZhCn, Some(Notice::TerminalInputUnavailable)) => "终端输入不可用 · 未发送输入",
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::WorkspaceNavigationUnavailable)) => {
                "Could not switch · select the destination again to retry"
            }
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::WorkspaceNavigationUnavailable)) => {
                "无法切换 · 请再次选择目标以重试"
            }
            (Self::EnUs, Some(Notice::PointerInputDropped)) => {
                "Pointer input dropped · focus switch did not settle in time"
            }
            (Self::ZhCn, Some(Notice::PointerInputDropped)) => {
                "指针输入被丢弃 · 焦点切换未及时就绪"
            }
            #[cfg(windows)]
            (Self::EnUs, Some(Notice::NavigatorChanged)) => {
                "Navigator closed · location or target changed"
            }
            #[cfg(windows)]
            (Self::ZhCn, Some(Notice::NavigatorChanged)) => "导航器已关闭 · 位置或目标已改变",
            #[cfg(windows)]
            (_, Some(Notice::NavigatorTooSmall)) => self.shell().overlay_refused,
            (Self::EnUs, Some(Notice::UnknownPrefix)) => "Unknown prefix key",
            (Self::ZhCn, Some(Notice::UnknownPrefix)) => "未知前缀键",
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "Tab creation unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::CreateTab))) => {
                "无法创建 Tab · 未发送命令"
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

/// Effective locale plus the variable that selected it (`default` when none),
/// reading only LC_ALL/LC_MESSAGES/LANG.
pub(crate) fn locale_source() -> (&'static str, &'static str) {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = std::env::var_os(name) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        return (locale_name(locale_from_value(&value)), name);
    }
    ("en-US", "default")
}

pub(crate) const fn locale_name(locale: Locale) -> &'static str {
    match locale {
        Locale::EnUs => "en-US",
        Locale::ZhCn => "zh-CN",
    }
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

#[cfg(any(unix, windows))]
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
            (Self::EnUs, TerminalStatus::Unavailable) => "Terminal disconnected",
            (Self::ZhCn, TerminalStatus::Unavailable) => "终端已断连",
            (Self::EnUs, TerminalStatus::OutcomeUnknown) => {
                "Terminal operation result unknown · no replay"
            }
            (Self::ZhCn, TerminalStatus::OutcomeUnknown) => "终端操作结果未知 · 不会重放",
        }
    }

    pub(crate) fn native_status(
        self,
        view: &crate::terminal_session::NativeSessionView<'_>,
        columns: u16,
    ) -> String {
        #[cfg(windows)]
        if let Some(notice) = view.worktree_notice() {
            return self.worktree_status_line(notice, view.worktree().map(|v| v.stage()));
        }
        if let Some((status, id)) = view.takeover_status() {
            return self.takeover_status(status, id);
        }
        if let Some(status) = view.workspace_close_status() {
            return self.workspace_close_status(status);
        }
        if view.retirement().is_none()
            && (view.phase() == crate::terminal_session::NativeDisplayPhase::Ready
                && view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
                    .is_some()
                || view.phase()
                    == crate::terminal_session::NativeDisplayPhase::SelectedWithoutAttachment
                    && view.selected_provenance()
                        == Some(crate::terminal_session::NativeDisplayProvenance::Failed))
        {
            let status = self.native_phase_status(view);
            if crate::terminal_text::display_width(&status) > usize::from(columns)
                && let Some(end) = view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
            {
                return self.terminal_end_compact(end);
            }
            return status;
        }
        #[cfg(windows)]
        if view.notice().is_none()
            && view.retirement().is_none()
            && view.setup_status().is_none()
            && let Some(progress) = view.duplicate_progress()
        {
            use crate::terminal_session::state::DuplicateStop;
            let status = match (self, progress.stop, progress.complete) {
                (Self::EnUs, _, true) => "Duplication complete",
                (Self::ZhCn, _, true) => "复制完成",
                (Self::EnUs, None, false) => "Duplicating tabs",
                (Self::ZhCn, None, false) => "正在复制标签页",
                (Self::EnUs, Some(DuplicateStop::Changed), _) => {
                    "Duplication stopped: selection or workspace changed"
                }
                (Self::ZhCn, Some(DuplicateStop::Changed), _) => "复制已停止：选择或工作区已改变",
                (Self::EnUs, Some(DuplicateStop::Deadline), _) => "Duplication stopped: time limit",
                (Self::ZhCn, Some(DuplicateStop::Deadline), _) => "复制已停止：已到时限",
                (Self::EnUs, Some(DuplicateStop::NotCreated), _) => {
                    "Duplication stopped: tab not created"
                }
                (Self::ZhCn, Some(DuplicateStop::NotCreated), _) => "复制已停止：标签页未创建",
                (
                    Self::EnUs,
                    Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery),
                    _,
                ) => "Duplication stopped: check pending, failed or unknown result; no replay",
                (
                    Self::ZhCn,
                    Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery),
                    _,
                ) => "复制已停止：请检查待定、失败或未知结果；不会重放",
            };
            return match self {
                Self::EnUs => format!(
                    "{status} · {}/{} submitted · {} tabs created · {} terminals started",
                    progress.submitted, progress.total, progress.committed, progress.started
                ),
                Self::ZhCn => format!(
                    "{status} · 已提交 {}/{} · 已创建 {} 个标签页 · 已启动 {} 个终端",
                    progress.submitted, progress.total, progress.committed, progress.started
                ),
            };
        }
        self.native_status_with_setup(
            self.native_phase_status(view),
            view.retirement().is_some(),
            view.notice(),
            view.setup_status(),
            view.phase() == crate::terminal_session::NativeDisplayPhase::Empty
                && view.workspace_directory_unconfigured(),
        )
    }

    pub(crate) fn native_phase_status(
        self,
        view: &crate::terminal_session::NativeSessionView<'_>,
    ) -> String {
        if view.retirement().is_none()
            && view.phase() == crate::terminal_session::NativeDisplayPhase::Ready
            && let Some(end) = view
                .focus_panel_id()
                .and_then(|panel| view.terminal_end(panel))
        {
            return self.terminal_end(end);
        }
        if let Some(recovery) = view.quick_workspace_recovery() {
            return self.quick_workspace_failure(
                recovery.project,
                recovery.workspace,
                recovery.unknown,
            );
        }
        if view.phase() == crate::terminal_session::NativeDisplayPhase::SelectedWithoutAttachment
            && view.selected_is_unbound()
            && view.retirement().is_none()
        {
            return self.native_unbound_terminal().to_owned();
        }
        let tab = view
            .active_position()
            .and_then(|position| view.tab(position - 1));
        let label = tab.map(|tab| tab.display_label(self));
        let safe_label = label
            .as_deref()
            .map(crate::terminal_text::sanitize_terminal_text);
        self.native_phase_status_values(
            view.phase(),
            safe_label.as_deref(),
            view.controller(),
            view.selected_provenance(),
            view.projection_status(),
            view.retirement(),
        )
    }

    pub(crate) const fn native_unbound_terminal(self) -> &'static str {
        match self {
            Self::EnUs => "Empty terminal slot · Use panel create-terminal to start a terminal",
            Self::ZhCn => "空终端位 · 使用 panel create-terminal 启动终端",
        }
    }

    fn native_phase_status_values(
        self,
        phase: crate::terminal_session::NativeDisplayPhase,
        label: Option<&str>,
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
                Self::EnUs => "No terminal tabs".to_owned(),
                Self::ZhCn => "没有终端标签".to_owned(),
            },
            Phase::Starting => self.native_starting_status(label, projection),
            Phase::Switching => self.native_switching_status(label, projection),
            Phase::Ready => {
                let Some(label) = label else {
                    return self
                        .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match controller {
                    Some(Controller::Controlling) => self.native_ready_controlling(label),
                    Some(Controller::ReadOnly) => self.native_ready_read_only(label),
                    None => self.native_terminal_unavailable(label),
                }
            }
            Phase::Creating | Phase::CreateCommittedNeedDetach => self.native_creating().to_owned(),
            Phase::ProjectionUnavailable => self.native_committed_refresh_required().to_owned(),
            Phase::MoveProjectionUnavailable => match self {
                Self::EnUs => "Tab moved · refresh required".to_owned(),
                Self::ZhCn => "标签已移动 · 需要刷新".to_owned(),
            },
            Phase::PanelProjectionUnavailable => match self {
                Self::EnUs => "Panel operation committed · refresh required".to_owned(),
                Self::ZhCn => "面板操作已提交 · 需要刷新".to_owned(),
            },
            Phase::RenameProjectionUnavailable => match self {
                Self::EnUs => "Rename committed · refresh required".to_owned(),
                Self::ZhCn => "重命名已提交 · 需要刷新".to_owned(),
            },
            Phase::SelectedWithoutAttachment => {
                let Some(label) = label else {
                    return self
                        .terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match provenance {
                    Some(Provenance::Pending) => self.native_terminal_pending(label),
                    Some(Provenance::Unavailable) => self.native_terminal_unavailable(label),
                    Some(Provenance::Failed) => self.native_terminal_failed(label),
                    Some(Provenance::OutcomeUnknown) => self.native_terminal_outcome_unknown(label),
                    Some(Provenance::Started | Provenance::BindingOnly) | None => {
                        self.native_terminal_unavailable(label)
                    }
                }
            }
            Phase::RetryableAttachUnavailable => label.map_or_else(
                || {
                    self.terminal_status(crate::terminal_session::TerminalStatus::Unavailable)
                        .to_owned()
                },
                |label| self.native_terminal_unavailable(label),
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

    /// One board row's canonical semantic status, rendered verbatim —
    /// the board localizes the server's verdict, never reinterprets it.
    #[cfg(any(unix, windows))]
    pub(crate) fn agent_status(
        self,
        status: &orchbee_agent_observation_protocol::AgentSemanticStatus,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentSemanticStatus;
        match status {
            AgentSemanticStatus::NeedsAttention { count } => match self {
                Self::EnUs => format!("needs attention · {}", count.value()),
                Self::ZhCn => format!("需要关注 · {}", count.value()),
            },
            AgentSemanticStatus::Failed {} => match self {
                Self::EnUs => "failed".to_owned(),
                Self::ZhCn => "失败".to_owned(),
            },
            AgentSemanticStatus::Succeeded {} => match self {
                Self::EnUs => "succeeded".to_owned(),
                Self::ZhCn => "已完成".to_owned(),
            },
            AgentSemanticStatus::Canceled {} => match self {
                Self::EnUs => "canceled".to_owned(),
                Self::ZhCn => "已取消".to_owned(),
            },
            AgentSemanticStatus::Interrupted {} => match self {
                Self::EnUs => "interrupted".to_owned(),
                Self::ZhCn => "已中断".to_owned(),
            },
            AgentSemanticStatus::Working { .. } => match self {
                Self::EnUs => "working".to_owned(),
                Self::ZhCn => "工作中".to_owned(),
            },
            AgentSemanticStatus::Idle {} => match self {
                Self::EnUs => "idle".to_owned(),
                Self::ZhCn => "空闲".to_owned(),
            },
            AgentSemanticStatus::Unknown { .. } => match self {
                Self::EnUs => "unknown".to_owned(),
                Self::ZhCn => "未知".to_owned(),
            },
            AgentSemanticStatus::Ended {} => match self {
                Self::EnUs => "ended".to_owned(),
                Self::ZhCn => "已结束".to_owned(),
            },
        }
    }

    /// The board body's line for a non-ready read state.
    #[cfg(any(unix, windows))]
    pub(crate) fn agent_board_read_line(
        self,
        read: crate::terminal_session::AgentBoardRead,
    ) -> &'static str {
        use crate::terminal_session::{AgentBoardRead, AgentBoardReadFailure};
        let board = self.shell().board;
        match read {
            AgentBoardRead::Unread => board.unread,
            AgentBoardRead::InFlight => board.in_flight,
            AgentBoardRead::Failed(AgentBoardReadFailure::Denied) => board.read_denied,
            AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable) => board.read_unavailable,
            AgentBoardRead::Failed(AgentBoardReadFailure::Transport) => board.read_transport,
            AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable) => {
                board.lane_unavailable
            }
            AgentBoardRead::Ready { .. } => board.empty,
        }
    }

    /// A jump refusal's board notice.
    #[cfg(any(unix, windows))]
    pub(crate) fn agent_board_notice(
        self,
        failure: crate::terminal_session::JumpFailure,
    ) -> &'static str {
        use crate::terminal_session::JumpFailure;
        let board = self.shell().board;
        match failure {
            JumpFailure::Gone => board.jump_gone,
            JumpFailure::Unavailable => board.jump_unavailable,
            JumpFailure::Denied => board.jump_denied,
            JumpFailure::Ended => board.jump_ended,
            JumpFailure::NoPlacement => board.jump_no_placement,
            JumpFailure::Expired => board.jump_expired,
        }
    }

    /// The sidebar's one-line Agent summary: the last *completed* read's
    /// count plus its completeness and age marks — `None` input means no
    /// completed read ever ran and the line must not exist at all.
    #[cfg(any(unix, windows))]
    pub(crate) fn agent_sidebar_line(
        self,
        summary: crate::terminal_session::AgentBoardSummary,
        elapsed: std::time::Duration,
    ) -> String {
        let seconds = elapsed.as_secs();
        let mark = if summary.complete { "" } else { "+" };
        match self {
            Self::EnUs => {
                format!(
                    "  {} {} · {}s ago",
                    summary.sessions,
                    if summary.sessions == 1 {
                        "agent"
                    } else {
                        "agents"
                    },
                    seconds,
                ) + mark
            }
            Self::ZhCn => format!("  智能体 {} · {}秒前", summary.sessions, seconds) + mark,
        }
    }

    /// Adds only the matching operation's sanitized display context to a localized notice.
    pub(crate) fn native_notice_with_context(
        self,
        notice: crate::terminal_session::NativeSessionNotice,
        native: &crate::terminal_session::NativeSessionView<'_>,
    ) -> String {
        let message = self.native_notice(notice);
        #[cfg(windows)]
        if let Some(context) = native.notice_context() {
            use crate::terminal_session::NativeNoticeContext;
            return match context {
                NativeNoticeContext::LaunchProfile(name) => {
                    format!("{message} · {}", self.launch_profile_label(name.as_deref()))
                }
                NativeNoticeContext::ConfiguredCommand(id) => format!(
                    "{} · {message}",
                    crate::terminal_text::sanitize_terminal_text(id)
                ),
            };
        }
        #[cfg(not(windows))]
        let _ = native;
        message
    }

    pub(crate) fn native_notice(
        self,
        notice: crate::terminal_session::NativeSessionNotice,
    ) -> String {
        use crate::terminal_session::NativeSessionNotice;
        match notice {
            NativeSessionNotice::Settings(notice) => self.settings_notice(notice).to_owned(),
            #[cfg(windows)]
            NativeSessionNotice::AgentStart(phase) => self.agent_start_phase(phase).to_owned(),
            NativeSessionNotice::LocalBusy => self.native_local_busy().to_owned(),
            #[cfg(windows)]
            NativeSessionNotice::MenuUnavailable => match self {
                Self::EnUs => "Menu action cancelled or unavailable · open the menu again".into(),
                Self::ZhCn => "菜单动作已取消或不可用 · 请重新打开菜单".into(),
            },
            NativeSessionNotice::NoTabAtPosition(position) => self.native_no_tab(position),
            NativeSessionNotice::CreateNotCreated => self.native_create_not_created().to_owned(),
            NativeSessionNotice::CreateOrderConflict => match self {
                Self::EnUs => "Cannot create an unpinned tab inside the pinned tabs".to_owned(),
                Self::ZhCn => "无法在固定标签页之间创建未固定的标签页".to_owned(),
            },
            NativeSessionNotice::DuplicatePending => match self {
                Self::EnUs => "Duplicate tab added; terminal start is pending".into(),
                Self::ZhCn => "已添加复制标签页；终端仍在启动中".into(),
            },
            NativeSessionNotice::DuplicateNotStarted => match self {
                Self::EnUs => "Duplicate tab added; terminal did not start".into(),
                Self::ZhCn => "已添加复制标签页；终端未能启动".into(),
            },
            NativeSessionNotice::DuplicateOutcomeUnknown => match self {
                Self::EnUs => "Duplicate tab added; terminal start outcome is unknown".into(),
                Self::ZhCn => "已添加复制标签页；终端启动结果未知".into(),
            },
            NativeSessionNotice::CloseRequestNotApplied => match self {
                Self::EnUs => "Close request was not applied".to_owned(),
                Self::ZhCn => "关闭请求未执行".to_owned(),
            },
            NativeSessionNotice::WorkspaceCloseUnavailable => match self {
                Self::EnUs => {
                    "Select a workspace to close; legacy workspace tabs close individually"
                        .to_owned()
                }
                Self::ZhCn => "请选择要关闭的工作区；原有工作区请逐个关闭 Tab".to_owned(),
            },
            NativeSessionNotice::LegacyWorkspace => {
                self.legacy_workspace_action_unavailable().to_owned()
            }
            NativeSessionNotice::NoWorkspace => self.no_current_workspace().to_owned(),
            NativeSessionNotice::RenameNotRenamed => match self {
                Self::EnUs => "Tab not renamed".to_owned(),
                Self::ZhCn => "标签页未重命名".to_owned(),
            },
            NativeSessionNotice::MoveNotMoved => match self {
                Self::EnUs => "Tab was not moved".to_owned(),
                Self::ZhCn => "标签未移动".to_owned(),
            },
            NativeSessionNotice::MoveOrderConflict => match self {
                Self::EnUs => {
                    "Tab not moved: preserve pinned Tabs and contiguous groups".to_owned()
                }
                Self::ZhCn => "标签未移动：需保留固定标签前缀和分组连续性".to_owned(),
            },
            NativeSessionNotice::PanelNotApplied => match self {
                Self::EnUs => "Panel operation was not applied".to_owned(),
                Self::ZhCn => "面板操作未执行".to_owned(),
            },
            NativeSessionNotice::ConfirmationCancelled => match self {
                Self::EnUs => "Close confirmation cancelled".to_owned(),
                Self::ZhCn => "关闭确认已取消".to_owned(),
            },
            NativeSessionNotice::DraftCancelled => match self {
                Self::EnUs => "Draft cancelled".to_owned(),
                Self::ZhCn => "草稿已取消".to_owned(),
            },
            NativeSessionNotice::ModalOwnsInput => match self {
                Self::EnUs => "Close the active dialog first".to_owned(),
                Self::ZhCn => "请先关闭当前对话框".to_owned(),
            },
            NativeSessionNotice::HistoryRecovering => match self {
                Self::EnUs => "Showing the latest output".to_owned(),
                Self::ZhCn => "正在显示最新输出".to_owned(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandAccepted => match self {
                Self::EnUs => "Command accepted".into(),
                Self::ZhCn => "命令已接受".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandFailed => match self {
                Self::EnUs => "Command or focus failed; inspect result".into(),
                Self::ZhCn => "命令或聚焦失败；请检查结果".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandUnknown => match self {
                Self::EnUs => "Outcome unknown; inspect before retry".into(),
                Self::ZhCn => "结果不确定；请检查后再决定是否重试".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandRefused(reason) => {
                use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
                match (self, reason) {
                    (Self::EnUs, F::Denied) => "Denied by Session permissions",
                    (Self::ZhCn, F::Denied) => "Session 权限拒绝；请检查权限",
                    (Self::EnUs, F::InvalidIntent) => "Invalid request; reload configuration",
                    (Self::ZhCn, F::InvalidIntent) => "请求无效；请重载配置",
                    (Self::EnUs, F::SiteMismatch) => "Session changed; reconnect",
                    (Self::ZhCn, F::SiteMismatch) => "Session 已变化；请重新连接",
                    (Self::EnUs, F::Unavailable) => "Unavailable; check Session and retry",
                    (Self::ZhCn, F::Unavailable) => "服务不可用；请检查 Session 后重试",
                    (Self::EnUs, F::Busy) => "Busy; wait and retry",
                    (Self::ZhCn, F::Busy) => "正在忙；请稍后重试",
                    (Self::EnUs, F::Conflict) => "Configuration changed; reload",
                    (Self::ZhCn, F::Conflict) => "配置已变化；请重载配置",
                    (Self::EnUs, F::NotFound) => "Command changed; reload configuration",
                    (Self::ZhCn, F::NotFound) => "命令已变化；请重载配置",
                    (Self::EnUs, F::StaleSource) => "Source changed; return live and retry",
                    (Self::ZhCn, F::StaleSource) => "来源已变化；请返回实时终端后重试",
                    (Self::EnUs, F::PhysicalUnknown) => "Outcome unknown; inspect before retry",
                    (Self::ZhCn, F::PhysicalUnknown) => "结果不确定；请检查后再决定是否重试",
                }
                .into()
            }
            #[cfg(windows)]
            NativeSessionNotice::LaunchProfileSelected => match self {
                Self::EnUs => "Launch profile".into(),
                Self::ZhCn => "启动配置".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LaunchProfileUnavailable => match self {
                Self::EnUs => "Terminal not created · check profile in Settings".into(),
                Self::ZhCn => "终端未创建 · 请在设置中检查启动配置".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkOpened => match self {
                Self::EnUs => "Link opened".into(),
                Self::ZhCn => "链接已打开".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkHandled => match self {
                Self::EnUs => "Link sent to a plugin".into(),
                Self::ZhCn => "链接已交给插件处理".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkNoLink => match self {
                Self::EnUs => "No link at that cell".into(),
                Self::ZhCn => "该单元格没有链接".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkBusy => match self {
                Self::EnUs => "Another link activation is still running".into(),
                Self::ZhCn => "另一个链接仍在处理中".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkFailed => match self {
                Self::EnUs => "Link activation failed".into(),
                Self::ZhCn => "链接打开失败".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::LinkUnknown => match self {
                Self::EnUs => {
                    "Link activation outcome unknown · inspect before trying again".into()
                }
                Self::ZhCn => "链接处理结果不确定 · 请检查后再决定是否重试".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PopupUnavailable => match self {
                Self::EnUs => {
                    "Popup connection or cleanup uncertain · restart the session before reopening"
                        .to_owned()
                }
                Self::ZhCn => "popup 连接或清理状态不确定 · 重启会话后再打开".to_owned(),
            },
            NativeSessionNotice::HistoryUnavailable => match self {
                Self::EnUs => "History is not available on this pane".to_owned(),
                Self::ZhCn => "此窗格历史不可用".to_owned(),
            },
            NativeSessionNotice::HistoryPositionLost => match self {
                Self::EnUs => {
                    "Earlier output is no longer saved; showing the latest output".to_owned()
                }
                Self::ZhCn => "先前输出已不在保留范围，正在显示最新输出".to_owned(),
            },
            NativeSessionNotice::SelectionEvicted => match self {
                Self::EnUs => "Selected rows are no longer available".to_owned(),
                Self::ZhCn => "所选行已不可用".to_owned(),
            },
            NativeSessionNotice::CopyFailed => match self {
                Self::EnUs => "Could not copy to the clipboard".to_owned(),
                Self::ZhCn => "未能复制到剪贴板".to_owned(),
            },
            NativeSessionNotice::CopyUnknown => match self {
                Self::EnUs => "Clipboard delivery could not be confirmed".to_owned(),
                Self::ZhCn => "无法确认剪贴板投递结果".to_owned(),
            },
            NativeSessionNotice::CopyModeUnavailable => match self {
                Self::EnUs => "Copy mode is not available on this pane".into(),
                Self::ZhCn => "此窗格暂时无法进入复制模式".into(),
            },
            NativeSessionNotice::CopyModeBusy => match self {
                Self::EnUs => "Copy mode is busy; wait before trying again".into(),
                Self::ZhCn => "复制模式正在处理，请稍后再操作".into(),
            },
            NativeSessionNotice::CopyModeInvalidated => match self {
                Self::EnUs => "Copy mode ended because its content or pane changed".into(),
                Self::ZhCn => "内容或窗格已改变，复制模式已退出".into(),
            },
            NativeSessionNotice::CopyModeQueryFailed => match self {
                Self::EnUs => {
                    "Could not read matching content; try the motion or search again".into()
                }
                Self::ZhCn => "无法读取对应内容，请重新移动或搜索".into(),
            },
            NativeSessionNotice::CopyModeNoMatch => match self {
                Self::EnUs => "No search match".into(),
                Self::ZhCn => "没有搜索匹配项".into(),
            },
            NativeSessionNotice::CopyModeSearchLimit => match self {
                Self::EnUs => "Rejected search: invalid text or more than 512 characters".into(),
                Self::ZhCn => "拒绝搜索：文本无效或超过 512 个字符".into(),
            },
            NativeSessionNotice::CopyModeNoSelection => match self {
                Self::EnUs => "Start a selection with v or Space before copying".into(),
                Self::ZhCn => "请先用 v 或空格开始选择，再复制".into(),
            },
            NativeSessionNotice::BellFailed => match self {
                Self::EnUs => "Could not forward the terminal bell".to_owned(),
                Self::ZhCn => "未能转发终端响铃".to_owned(),
            },
            NativeSessionNotice::BellUnknown => match self {
                Self::EnUs => "Terminal bell delivery could not be confirmed".to_owned(),
                Self::ZhCn => "无法确认终端响铃投递结果".to_owned(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PositionChanged => match self {
                Self::EnUs => {
                    "The saved Tab or pane is unavailable; showing the first available member.".into()
                }
                Self::ZhCn => "上次的标签页或面板已不可用；已打开首个可用成员。".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PositionChangedEmpty => match self {
                Self::EnUs => "The saved Tab is unavailable; showing the empty Workspace.".into(),
                Self::ZhCn => "上次的标签页已不可用；已打开原工作区的空视图。".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PositionLocationChanged => match self {
                Self::EnUs => "The saved location could not be restored; showing the first available Workspace.".into(),
                Self::ZhCn => "无法恢复上次的位置；已打开首个可用工作区。".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PositionLocationChangedEmpty => match self {
                Self::EnUs => "The saved location could not be restored; showing an empty Workspace.".into(),
                Self::ZhCn => "无法恢复上次的位置；已打开空工作区。".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::PositionUnavailable => match self {
                Self::EnUs => {
                    "Position memory is unavailable; terminal tasks are still running.".into()
                }
                Self::ZhCn => "位置记忆不可用；终端任务仍继续运行。".into(),
            },
        }
    }

    /// The right-edge badge a pane carries while its retained-history core
    /// owns the leaf.
    pub(crate) const fn terminal_history_badge(self) -> &'static str {
        match self {
            Self::EnUs => "HISTORY",
            Self::ZhCn => "历史",
        }
    }

    /// The loading indicator while entry or a row window is still in
    /// flight — no retained content may show yet.
    pub(crate) const fn terminal_history_loading(self) -> &'static str {
        match self {
            Self::EnUs => "Loading history…",
            Self::ZhCn => "正在加载历史…",
        }
    }

    /// The recovering indicator while the core returns the pane to live.
    pub(crate) const fn terminal_history_recovering(self) -> &'static str {
        match self {
            Self::EnUs => "Returning to live…",
            Self::ZhCn => "正在返回实时…",
        }
    }

    /// The retained-window position readout: the window's first retained
    /// row index out of the total retained rows the host reported.
    pub(crate) fn terminal_history_position(self, start_row: u64, total_rows: u64) -> String {
        match self {
            Self::EnUs => format!("Row {start_row} of {total_rows}"),
            Self::ZhCn => format!("第 {start_row} 行 · 共 {total_rows} 行"),
        }
    }

    /// The persistent bottom-line hint for leaving history and returning
    /// to live output.
    pub(crate) const fn terminal_history_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Esc back to live",
            Self::ZhCn => "Esc 返回实时",
        }
    }

    /// Live viewing is clipped locally; the physical terminal keeps its controller's size.
    pub(crate) const fn terminal_live_crop(self) -> &'static str {
        match self {
            Self::EnUs => "Live crop",
            Self::ZhCn => "实时裁切",
        }
    }

    /// Complete compact clipping and history-entry instruction for a narrow live viewer.
    pub(crate) const fn terminal_live_crop_compact(self) -> &'static str {
        match self {
            Self::EnUs => "Crop: scroll up",
            Self::ZhCn => "裁切：向上滚动",
        }
    }

    /// Short action label paired with the effective history-entry binding.
    pub(crate) const fn terminal_view_history(self) -> &'static str {
        match self {
            Self::EnUs => "History",
            Self::ZhCn => "历史",
        }
    }

    fn native_starting_status(
        self,
        label: Option<&str>,
        projection: crate::terminal_session::NativeProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::terminal_session::TerminalStatus::Connecting,
        )
    }

    fn native_switching_status(
        self,
        label: Option<&str>,
        projection: crate::terminal_session::NativeProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::terminal_session::TerminalStatus::Unavailable,
        )
    }

    fn native_connection_status(
        self,
        label: Option<&str>,
        projection: crate::terminal_session::NativeProjectionStatus,
        missing_position: crate::terminal_session::TerminalStatus,
    ) -> String {
        use crate::terminal_session::NativeProjectionStatus;

        let Some(label) = label else {
            return self.terminal_status(missing_position).to_owned();
        };
        match projection {
            NativeProjectionStatus::WaitingForFullFrame => self.native_waiting_for_frame(label),
            NativeProjectionStatus::Blocked => self.native_display_blocked(label),
            NativeProjectionStatus::Empty | NativeProjectionStatus::FrameAvailable => {
                self.native_connecting(label)
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

    /// Labels a one-based automatic Tab position without changing its canonical name.
    pub(crate) fn automatic_tab_label(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position}"),
            Self::ZhCn => format!("标签页 {position}"),
        }
    }

    fn native_connecting(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting…"),
            Self::ZhCn => format!("{label} · 正在连接…"),
        }
    }

    fn native_waiting_for_frame(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting · waiting for full frame"),
            Self::ZhCn => format!("{label} · 正在连接 · 等待完整帧"),
        }
    }

    fn native_display_blocked(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting · display blocked"),
            Self::ZhCn => format!("{label} · 正在连接 · 显示已阻塞"),
        }
    }

    fn native_ready_controlling(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Ready"),
            Self::ZhCn => format!("{label} · 已就绪"),
        }
    }

    fn native_ready_read_only(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Ready · view only"),
            Self::ZhCn => format!("{label} · 已就绪 · 仅查看"),
        }
    }

    const fn native_committed_refresh_required(self) -> &'static str {
        match self {
            Self::EnUs => "Created Tab committed · refresh required",
            Self::ZhCn => "已提交创建的 Tab · 需要刷新",
        }
    }

    fn native_terminal_pending(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal creation pending · no replay"),
            Self::ZhCn => format!("{label} · 终端创建等待中 · 不会重放"),
        }
    }

    fn native_terminal_unavailable(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal unavailable"),
            Self::ZhCn => format!("{label} · 终端不可用"),
        }
    }

    fn native_terminal_failed(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal creation failed"),
            Self::ZhCn => format!("{label} · 终端创建失败"),
        }
    }

    fn native_terminal_outcome_unknown(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal result unknown · no replay"),
            Self::ZhCn => format!("{label} · 终端结果未知 · 不会重放"),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
impl Locale {
    pub(crate) const fn startup_progress(self) -> &'static str {
        match self {
            Self::EnUs => "Starting the local terminal…\n",
            Self::ZhCn => "正在启动本地终端…\n",
        }
    }
    pub(crate) const fn startup_wait_notice(self) -> &'static str {
        match self {
            Self::EnUs => "The terminal window is too small. Resize it to continue.\n",
            Self::ZhCn => "终端窗口太小。请调整大小后继续。\n",
        }
    }
    pub(crate) const fn startup_wait_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Press Ctrl+B then q to exit.\n",
            Self::ZhCn => "按 Ctrl+B 再按 q 退出。\n",
        }
    }
    pub(crate) const fn startup_failure(
        self,
        failure: crate::local_activation::StartupFailure,
    ) -> &'static str {
        use crate::local_activation::StartupFailure;
        match (self, failure) {
            #[cfg(windows)]
            (Self::EnUs, StartupFailure::AlreadyInSession) => {
                "You are already inside this Session. Return to the existing Client to navigate, or use --session <name> to open another Session. CLI subcommands remain available.\n"
            }
            #[cfg(windows)]
            (Self::ZhCn, StartupFailure::AlreadyInSession) => {
                "当前终端已在此会话内。请返回现有 Client 切换位置，或使用 --session <name> 进入其他会话。CLI 子命令仍可正常使用。\n"
            }
            #[cfg(windows)]
            (Self::EnUs, StartupFailure::Configuration) => {
                "Invalid Windows configuration. Run orchbee config check for details, correct the setting and retry.\n"
            }
            #[cfg(windows)]
            (Self::ZhCn, StartupFailure::Configuration) => {
                "Windows 配置无效。运行 orchbee config check 查看详情，修正后重试。\n"
            }
            (Self::EnUs, StartupFailure::Interface) => {
                "The terminal interface failed. Existing session processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::Interface) => "终端界面运行失败。已有会话进程已保留。\n",
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
                "The local session runtime failed its ownership or permission checks.\n"
            }
            (Self::ZhCn, StartupFailure::Runtime) => "本地会话运行时未通过所有者或权限检查。\n",
            (_, StartupFailure::SocketPathTooLong) => self.cli_socket_path_too_long(),
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "Local startup could not confirm existing ownership or startup completion. The checkpoint is retained. Run orchbee server diagnose with the same --session selection; use orchbee --session recovery-work server start for another session. Live tasks require Host adoption; incomplete or legacy records need diagnosis.\n"
                } else {
                    "The local session requires explicit recovery. No replacement process was started.\n"
                }
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "无法确认既有进程归属或启动完成情况，已保留检查点。请用相同 --session 选择运行 orchbee server diagnose；可执行 orchbee --session recovery-work server start 另开会话。存活任务仍需 Host 接管；不完整或旧版记录需要诊断。\n"
                } else {
                    "本地会话需要显式恢复。未启动替代进程。\n"
                }
            }
            (Self::EnUs, StartupFailure::Incompatible) => {
                "The running local session is incompatible with this development cohort.\n"
            }
            (Self::ZhCn, StartupFailure::Incompatible) => {
                "正在运行的本地会话与当前开发 cohort 不兼容。\n"
            }
            (Self::EnUs, StartupFailure::Admission) => {
                "The local session could not admit this Client connection.\n"
            }
            (Self::ZhCn, StartupFailure::Admission) => "本地会话无法准入此 Client 连接。\n",
            (Self::EnUs, StartupFailure::TimedOut) => {
                "Local terminal startup timed out. Existing session processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::TimedOut) => "本地终端启动超时。已有会话进程已保留。\n",
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
    pub(crate) fn bind(self, view: &crate::client_view::ClientView) -> BoundShellText<'a> {
        let hints = [
            self.exit_hint,
            self.hint_short,
            self.pointer_released,
            self.prefix_active,
            self.prefix_idle,
            self.sidebar_hint,
            self.zoom_badge,
        ]
        .map(|hint| self.bound_hint(view, hint));
        BoundShellText { base: self, hints }
    }

    pub(crate) fn bound_hint(&self, view: &crate::client_view::ClientView, hint: &str) -> String {
        let bindings = view.bindings();
        let mut result = hint.to_owned();
        // Substitute complete default chords before the standalone prefix. The replacement
        // values come from the same resolved entries dispatch uses, including disabled actions.
        let replacements = [
            ("Ctrl+B ?", PrefixDesc::ShowHelp),
            ("Ctrl+B q", PrefixDesc::Exit),
            ("Ctrl+B b", PrefixDesc::SidebarToggle),
            ("Ctrl+B z", PrefixDesc::PanelZoom),
            ("Ctrl+B w", PrefixDesc::NavigatorToggle),
            ("Ctrl+B c", PrefixDesc::CreateTab),
            ("Ctrl+B x", PrefixDesc::CloseTab),
            ("Ctrl+B then q", PrefixDesc::Exit),
            ("Ctrl+B 后按 q", PrefixDesc::Exit),
            ("Ctrl+B 后按 c", PrefixDesc::CreateTab),
            ("Ctrl+B 后按 x", PrefixDesc::CloseTab),
            ("Ctrl+B 后按 w", PrefixDesc::NavigatorToggle),
        ];
        // Placeholders prevent one configured chord from being rewritten by a later replacement.
        for (index, (old, _)) in replacements.iter().enumerate() {
            result = result.replace(old, &format!("\u{e000}{index}\u{e001}"));
        }
        result = result.replace(
            "Ctrl+B",
            &format_binding(&bindings.prefix, LabelStyle::for_host()),
        );
        for (index, (_, desc)) in replacements.iter().enumerate() {
            let actual = bindings
                .labels(*desc)
                .into_iter()
                .next()
                .unwrap_or_else(|| self.binding_unbound.into());
            result = result.replace(&format!("\u{e000}{index}\u{e001}"), &actual);
        }
        result
    }
}

#[cfg(windows)]
impl Locale {
    #[cfg(windows)]
    pub(crate) fn agent_start_phase(
        self,
        phase: crate::terminal_session::AgentStartPhase,
    ) -> &'static str {
        use crate::terminal_session::AgentStartPhase as Phase;
        let text = self.shell().board;
        match phase {
            Phase::Starting => text.start_starting,
            Phase::WaitingInput => text.start_waiting_input,
            Phase::Unconfirmed => text.start_unconfirmed,
            Phase::Cancelling => text.start_cancelling,
            Phase::Started => text.start_started,
            Phase::Failed => text.start_failed,
            Phase::Unknown => text.start_unknown,
            Phase::Cancelled => text.start_cancelled,
        }
    }

    pub(crate) fn onboarding_title(self) -> &'static str {
        match self {
            Self::EnUs => "Welcome to Orchbee",
            Self::ZhCn => "欢迎使用 Orchbee",
        }
    }
    pub(crate) fn onboarding_status(self, failed: bool) -> &'static str {
        match (self, failed) {
            (Self::EnUs, false) => "Type in your terminal; choose an action when ready.",
            (Self::ZhCn, false) => "终端可以继续输入；需要时选择下列操作。",
            (Self::EnUs, true) => "Guide state unavailable; existing data is preserved.",
            (Self::ZhCn, true) => "无法读取引导状态；已保留现有数据。",
        }
    }
    pub(crate) fn onboarding_label(self, action: crate::onboarding::Action) -> &'static str {
        use crate::onboarding::Action::*;
        match (self, action) {
            (Self::EnUs, Workspace) => "New workspace",
            (Self::ZhCn, Workspace) => "新建工作区",
            (Self::EnUs, Agent) => "Choose and start an Agent",
            (Self::ZhCn, Agent) => "选择并启动 Agent",
            (Self::EnUs, Menu) => "Actions and commands",
            (Self::ZhCn, Menu) => "操作与命令",
            (Self::EnUs, Help) => "Help",
            (Self::ZhCn, Help) => "帮助",
            (Self::EnUs, Dismiss) => "Dismiss guide",
            (Self::ZhCn, Dismiss) => "关闭引导",
        }
    }
}
