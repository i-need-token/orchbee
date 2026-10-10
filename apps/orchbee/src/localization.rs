#[cfg(windows)]
mod action_menu;
mod control_takeover;
#[cfg(windows)]
mod directory_startup;
#[cfg(windows)]
mod machines;
mod notification;
#[cfg(windows)]
mod plugins;
mod runtime_configuration;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod update;
mod window_title;
#[cfg(test)]
use crate::client_view::{PREFIX_CHORD, PREFIX_GROUPS, PrefixGroup};
use std::ffi::OsString;

use crate::client_view::{BackendAction, LabelStyle, Notice, PrefixDesc, format_binding};

mod agent_manifests;
mod cli;
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
pub(crate) use cli::CliLabels;
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
        #[cfg(windows)]
        PrefixDesc::AgentCycle => "Select previous / next Agent in the filtered machine view",
        #[cfg(windows)]
        PrefixDesc::AgentIndex => "Focus numbered Agent in the filtered machine view",
        PrefixDesc::ReloadConfiguration => "Reload Site configuration",
        PrefixDesc::QuickWorkspaceCreate => "Create a Workspace and enter its first terminal",
        #[cfg(windows)]
        PrefixDesc::WorktreeCreate => "Create a Git worktree and enter its terminal",
        #[cfg(windows)]
        PrefixDesc::WorktreeOpen => "Open an existing Git checkout",
        PrefixDesc::ControlTakeover => "Take control of the current pane; use this window's size",
        PrefixDesc::WorkspaceClose => {
            "Close current Workspace / worktree group (asks for confirmation)"
        }
        PrefixDesc::WorkspaceCreate => "Create a Workspace in an existing or new Project",
        PrefixDesc::WorkspaceDirectory => "Configure an empty Workspace's first directory",
        PrefixDesc::WorkspaceRename => "Rename current Workspace",
        PrefixDesc::ProjectRename => "Rename current Workspace's Project",
        PrefixDesc::RenameTab => "Rename current Tab",
        PrefixDesc::CloseTab => "Close current Tab / retry Close (confirms for all Clients)",
        PrefixDesc::CreateTab => "Create a Tab (requires Site)",
        PrefixDesc::MoveTab => "Move current Tab left / right (wraps)",
        PrefixDesc::SelectTabWrap => "Select next / previous Tab (requires Site)",
        PrefixDesc::SelectContext => {
            "Select previous / next Workbench location (wraps; requires Site)"
        }
        PrefixDesc::SelectTabDigit => "Select ordered Tab (requires Site)",
        PrefixDesc::PanelFocus => "Move between Panels (requires Site)",
        PrefixDesc::PresentationSelect => "Select previous / next terminal in this Panel",
        PrefixDesc::PanelZoom => "Zoom the focused Panel (requires Site)",
        PrefixDesc::SplitPanel => "Split focused Panel below/above/left/right (requires Site)",
        PrefixDesc::ClosePanel => "Close focused Panel (requires Site)",
        PrefixDesc::MovePanel => "Move focused Panel to that side (requires Site)",
        PrefixDesc::SidebarToggle => "Toggle business sidebar",
        #[cfg(windows)]
        PrefixDesc::SidebarFocus => "Focus sidebar navigation",
        PrefixDesc::NavigatorToggle => "Toggle Workbench navigator",
        PrefixDesc::BoardToggle => "Toggle the Agent board",
        PrefixDesc::ShowHelp => "Open or close this shortcut help",
        PrefixDesc::HelpPage => "Page this shortcut help while it is open",
        PrefixDesc::PointerCapture => "Release or restore mouse capture",
        PrefixDesc::Exit => "Close this shell",
        PrefixDesc::LiteralPrefix => "Send the literal prefix to the terminal input port",
        PrefixDesc::DismissPrefix => "Dismiss the prefix without an action",
        PrefixDesc::OtherKeys => "Send to the terminal input port",
    }
}

fn zh_cn_prefix_desc(desc: PrefixDesc) -> &'static str {
    match desc {
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
        PrefixDesc::ActionMenu => "打开动作菜单",
        #[cfg(windows)]
        PrefixDesc::AgentCycle => "按机器视图的筛选顺序切换上一个 / 下一个 Agent",
        #[cfg(windows)]
        PrefixDesc::AgentIndex => "按机器视图的筛选顺序聚焦编号 Agent",
        PrefixDesc::ReloadConfiguration => "重载 Site 配置",
        PrefixDesc::QuickWorkspaceCreate => "创建工作区并进入第一个终端",
        #[cfg(windows)]
        PrefixDesc::WorktreeCreate => "创建 Git worktree 并进入终端",
        #[cfg(windows)]
        PrefixDesc::WorktreeOpen => "打开已有 Git checkout",
        PrefixDesc::ControlTakeover => "接管当前 pane；使用本窗口尺寸",
        PrefixDesc::WorkspaceClose => "关闭当前工作区 / worktree 组（需要确认）",
        PrefixDesc::WorkspaceCreate => "在现有或新建 Project 中创建 Workspace",
        PrefixDesc::WorkspaceDirectory => "配置空 Workspace 的首个目录",
        PrefixDesc::WorkspaceRename => "重命名当前 Workspace",
        PrefixDesc::ProjectRename => "重命名当前 Workspace 所属的 Project",
        PrefixDesc::RenameTab => "重命名当前 Tab",
        PrefixDesc::CloseTab => "关闭当前 Tab / 重试关闭（确认后影响所有 Client）",
        PrefixDesc::CreateTab => "创建 Tab（需要 Site）",
        PrefixDesc::MoveTab => "向左 / 右移动当前 Tab（首尾循环）",
        PrefixDesc::SelectTabWrap => "选择下一个 / 上一个 Tab（需要 Site）",
        PrefixDesc::SelectContext => "选择上一个 / 下一个 Workbench 位置（首尾循环；需要 Site）",
        PrefixDesc::SelectTabDigit => "选择对应顺序的 Tab（需要 Site）",
        PrefixDesc::PanelFocus => "在 Panel 之间移动（需要 Site）",
        PrefixDesc::PresentationSelect => "选择当前 Panel 中的上一个 / 下一个终端",
        PrefixDesc::PanelZoom => "放大焦点 Panel（需要 Site）",
        PrefixDesc::SplitPanel => "将焦点 Panel 向下/上/左/右分屏（需要 Site）",
        PrefixDesc::ClosePanel => "关闭焦点 Panel（需要 Site）",
        PrefixDesc::MovePanel => "将焦点 Panel 移向该侧（需要 Site）",
        PrefixDesc::SidebarToggle => "切换业务侧边栏",
        #[cfg(windows)]
        PrefixDesc::SidebarFocus => "聚焦侧栏导航",
        PrefixDesc::NavigatorToggle => "切换 Workbench 导航器",
        PrefixDesc::BoardToggle => "切换智能体看板",
        PrefixDesc::ShowHelp => "打开或关闭此快捷键帮助",
        PrefixDesc::HelpPage => "在帮助打开时翻看此快捷键帮助的页面",
        PrefixDesc::PointerCapture => "释放或恢复鼠标捕获",
        PrefixDesc::Exit => "关闭此 shell",
        PrefixDesc::LiteralPrefix => "向终端输入端口发送字面前缀",
        PrefixDesc::DismissPrefix => "取消前缀而不执行动作",
        PrefixDesc::OtherKeys => "发送到终端输入端口",
    }
}

/// The chord column for one binding group: the prefix label plus the member
/// key labels, joined by the group separator. Derived from the registry, so
/// help can never drift from dispatch.
#[cfg(test)]
fn group_chord(group: &PrefixGroup) -> String {
    let style = LabelStyle::for_host();
    let prefix = format_binding(&PREFIX_CHORD, style);
    let labels: Vec<String> = group
        .members
        .iter()
        .map(|member| member.keys.label(style))
        .collect();
    let separator = if labels.len() == 2 {
        " / "
    } else if labels
        .iter()
        .any(|label| matches!(label.as_str(), "\\" | "|" | "/"))
    {
        " "
    } else {
        "/"
    };
    format!(
        "{prefix} {}",
        crate::client_view::join_key_labels(&labels, style, separator)
    )
}

/// The Agent board's localized copy. Read-state lines, jump-refusal
/// notices, and the input hint all live behind this struct so the
/// renderer never hard-codes product copy.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentBoardText {
    pub(crate) title: &'static str,
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
    pub(crate) jumping: &'static str,
    pub(crate) footer: &'static str,
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
    pub(crate) other_keys_label: &'static str,
    pub(crate) binding_unbound: &'static str,
    #[cfg(windows)]
    history_previous: &'static str,
    #[cfg(windows)]
    help_next: &'static str,
    #[cfg(not(windows))]
    pub(crate) overlay_input_hint: &'static str,
    pub(crate) help_dismiss_hint: &'a str,
    pub(crate) help_dismiss_hint_short: &'a str,
    pub(crate) help_paging_hint: &'a str,
    pub(crate) overlay_refused: &'static str,
    #[cfg(not(windows))]
    pub(crate) navigator_detail: &'static str,
    pub(crate) navigator_title: &'static str,
    pub(crate) no_client_session: &'static str,
    pub(crate) panel_title: &'static str,
    pub(crate) pointer_released: &'a str,
    pub(crate) prefix_active: &'a str,
    pub(crate) prefix_cancel_hint: &'static str,
    pub(crate) prefix_desc: fn(PrefixDesc) -> &'static str,
    #[cfg(windows)]
    sidebar_action: fn(crate::machine_picker::SidebarCommand) -> &'static str,
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

impl ShellText<'_> {
    /// Builds the help rows from the same prefix binding registry that
    /// dispatch matches: `(padded chord, localized description)` pairs, with
    /// the localized "other keys" passthrough row appended last. The chord is
    /// padded to a uniform column so callers can align descriptions without
    /// re-deriving the split — chords may themselves contain spaces.
    #[cfg(test)]
    pub(crate) fn shortcut_rows(&self) -> Vec<(String, String)> {
        let chords: Vec<String> = PREFIX_GROUPS.iter().map(group_chord).collect();
        let width = chords
            .iter()
            .map(|chord| chord.chars().count())
            .max()
            .unwrap_or(0)
            + 1;
        let mut rows: Vec<(String, String)> = PREFIX_GROUPS
            .iter()
            .zip(&chords)
            .map(|(group, chord)| {
                (
                    format!("{chord:<width$}"),
                    (self.prefix_desc)(group.desc).to_owned(),
                )
            })
            .collect();
        rows.push((
            format!("{:<width$}", self.other_keys_label),
            (self.prefix_desc)(PrefixDesc::OtherKeys).to_owned(),
        ));
        rows
    }
}

const EN_US_BOARD: AgentBoardText = AgentBoardText {
    title: "Agent board",
    unread: "No completed Agent read yet · press r to read",
    in_flight: "Reading Agent sessions…",
    incomplete: "listing incomplete",
    read_denied: "Agent read denied",
    read_unavailable: "Agent runtime unavailable",
    read_transport: "Agent read failed · transport",
    lane_unavailable: "Agent board unavailable · no read lane",
    empty: "No Agent sessions",
    jumping: "Jumping…",
    footer: "↑/k ↓/j select · Enter jump · r refresh · Esc close",
    stale: "stale",
    disconnected: "disconnected",
    standalone: "Standalone",
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
    navigator_detail: "A Site connection is required before Workbench locations can be shown.",
    navigator_title: "Workbench navigator",
    no_client_session: "No Site or Client Session is connected.",
    other_keys_label: "Other keys",
    binding_unbound: "Unbound",
    #[cfg(windows)]
    history_previous: "Enter retained history when Help is closed; page back while Help is open",
    #[cfg(windows)]
    help_next: "Page forward while this shortcut help is open",
    #[cfg(not(windows))]
    overlay_input_hint: "Terminal input continues while this overlay is visible.",
    help_dismiss_hint: "Ctrl+B ? closes help · typing still reaches the terminal",
    help_dismiss_hint_short: "Ctrl+B ? closes · typing continues",
    help_paging_hint: "Ctrl+B PgUp/PgDn turns pages",
    panel_title: "Terminal",
    pointer_released: "mouse released · Ctrl+B m restores",
    prefix_active: "PREFIX Ctrl+B",
    prefix_cancel_hint: "Esc cancel",
    prefix_desc: en_us_prefix_desc,
    #[cfg(windows)]
    sidebar_action: |command| Locale::EnUs.sidebar_navigation_action(command),
    prefix_idle: "Ctrl+B · shortcuts",
    projects: "Projects",
    shell_only_detail: "This development slice renders only the Client-local shell.",
    shortcut_title: "Keyboard shortcuts",
    sidebar_hint: "Ctrl+B b · sidebar",
    site_section: "SITE",
    standalone: "Standalone terminal",
    tabs_unavailable: "No shared Tabs · Site unavailable",
    terminal_unavailable: "Terminal unavailable",
    too_small_notice: "Too small · resize to continue",
    workbench_title: "WORKBENCH",
    pinned_tab: "Pinned",
    unnamed_tab_group: "Unnamed group",
    workspaces: "Workspaces",
    zoom_badge: "zoomed · Ctrl+B z restores",
};

const ZH_CN_BOARD: AgentBoardText = AgentBoardText {
    title: "智能体看板",
    unread: "尚未完成智能体读取 · 按 r 读取",
    in_flight: "正在读取智能体会话…",
    incomplete: "列表不完整",
    read_denied: "智能体读取被拒绝",
    read_unavailable: "智能体运行时不可用",
    read_transport: "智能体读取失败 · 传输错误",
    lane_unavailable: "看板不可用 · 无读取通道",
    empty: "没有智能体会话",
    jumping: "正在跳转…",
    footer: "↑/k ↓/j 选择 · Enter 跳转 · r 刷新 · Esc 关闭",
    stale: "已过期",
    disconnected: "已断连",
    standalone: "独立终端",
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
    navigator_detail: "连接 Site 后才能显示 Workbench 位置。",
    navigator_title: "Workbench 导航器",
    no_client_session: "当前未连接 Site 或 Client Session。",
    other_keys_label: "其他按键",
    binding_unbound: "未绑定",
    #[cfg(windows)]
    history_previous: "帮助关闭时进入保留历史；帮助打开时向前翻页",
    #[cfg(windows)]
    help_next: "在此快捷键帮助打开时向后翻页",
    #[cfg(not(windows))]
    overlay_input_hint: "此浮层显示时，终端输入仍会继续传递。",
    help_dismiss_hint: "Ctrl+B ? 关闭此帮助 · 输入仍会传递到终端",
    help_dismiss_hint_short: "Ctrl+B ? 关闭 · 输入仍传递到终端",
    help_paging_hint: "Ctrl+B PgUp/PgDn 翻页",
    panel_title: "终端",
    pointer_released: "鼠标已释放 · Ctrl+B m 恢复",
    prefix_active: "前缀 Ctrl+B",
    prefix_cancel_hint: "Esc 取消",
    prefix_desc: zh_cn_prefix_desc,
    #[cfg(windows)]
    sidebar_action: |command| Locale::ZhCn.sidebar_navigation_action(command),
    prefix_idle: "Ctrl+B · 快捷键",
    projects: "项目",
    shell_only_detail: "此开发切片只渲染 Client-local shell。",
    shortcut_title: "键盘快捷键",
    sidebar_hint: "Ctrl+B b · 侧边栏",
    site_section: "站点",
    standalone: "独立终端",
    tabs_unavailable: "无共享 Tab · Site 不可用",
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
    #[cfg(windows)]
    pub(crate) fn sidebar_navigation_hint(self, full: bool) -> String {
        use crate::machine_picker::{SIDEBAR_BINDINGS, SidebarCommand};
        let mut hints = vec![self.machine_picker_text("sidebar-navigation").to_owned()];
        hints.extend(
            SIDEBAR_BINDINGS
                .iter()
                .filter(|(_, command)| {
                    matches!(command, SidebarCommand::Exit)
                        || (full
                            && matches!(
                                command,
                                SidebarCommand::Activate
                                    | SidebarCommand::OpenMenu
                                    | SidebarCommand::Help
                            ))
                })
                .map(|(binding, command)| {
                    format!(
                        "{} {}",
                        format_binding(binding, LabelStyle::for_host()),
                        self.sidebar_navigation_action(*command)
                    )
                }),
        );
        hints.join(" · ")
    }

    pub(crate) const fn help(self) -> &'static str {
        self.cli_root_help()
    }

    pub(crate) const fn shell(self) -> ShellText<'static> {
        match self {
            Self::EnUs => EN_US_SHELL,
            Self::ZhCn => ZH_CN_SHELL,
        }
    }

    #[cfg(windows)]
    pub(crate) fn navigator_hint(self, full: bool) -> String {
        use crate::client_view::navigator::{Command, label};
        if full {
            return format!(
                "{} · {} / {} · {} / {}",
                self.navigator_movement(),
                label(Command::First),
                label(Command::Last),
                label(Command::PagePrevious),
                label(Command::PageNext)
            );
        }
        let (exit, open) = (label(Command::Exit), label(Command::Open));
        match self {
            Self::EnUs => format!("{exit} return · {open} open"),
            Self::ZhCn => format!("{exit} 返回 · {open} 打开"),
        }
    }

    #[cfg(windows)]
    pub(crate) fn navigator_movement(self) -> String {
        use crate::client_view::navigator::{Command, label};
        let (previous, next, column) = (
            label(Command::Previous),
            label(Command::Next),
            label(Command::Column),
        );
        let (left, right) = (label(Command::Locations), label(Command::Tabs));
        match self {
            Self::EnUs => format!("{previous}/{next} select · {column}/{left}/{right} column"),
            Self::ZhCn => format!("{previous}/{next} 选择 · {column}/{left}/{right} 切换列"),
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
    pub(crate) const fn resize_hint(self, full: bool, pending: bool) -> &'static str {
        match (self, full, pending) {
            (Self::EnUs, true, false) => "RESIZE · arrows 5% · Shift 1% · ? · Esc exit",
            (Self::EnUs, true, true) => "RESIZE pending · Esc exit",
            (Self::EnUs, false, _) => "RESIZE · Esc exit",
            (Self::ZhCn, true, false) => "调整 · 方向键 5% · Shift 1% · ? · Esc 退出",
            (Self::ZhCn, true, true) => "调整待定 · Esc 退出",
            (Self::ZhCn, false, _) => "调整 · Esc 退出",
        }
    }

    /// A release-only divider gesture's persistent status hint.
    #[cfg(windows)]
    pub(crate) const fn divider_drag_hint(self, full: bool) -> &'static str {
        match (self, full) {
            (Self::EnUs, true) => "Divider · release to apply · Esc cancel",
            (Self::EnUs, false) => "Divider · release / Esc",
            (Self::ZhCn, true) => "分隔线 · 松手应用 · Esc 取消",
            (Self::ZhCn, false) => "分隔线 · 松手 / Esc",
        }
    }

    /// The local adjustment remains pending after the pointer preview ends.
    #[cfg(windows)]
    pub(crate) const fn resize_pending_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Resizing…",
            Self::ZhCn => "正在调整…",
        }
    }

    pub(crate) const fn shell_notice(self, notice: Option<Notice>) -> &'static str {
        match (self, notice) {
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
                "Keys updated; some Site settings need a restart"
            }
            (Self::ZhCn, Some(Notice::ConfigurationRestart)) => {
                "键位已更新；部分 Site 设置需要重启"
            }
            (Self::EnUs, Some(Notice::ConfigurationUnknown)) => {
                "Reload outcome unknown; inspect site config"
            }
            (Self::ZhCn, Some(Notice::ConfigurationUnknown)) => "重载结果未知；请查看 site config",
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
                "Workspace 重命名不可用 · 未发送命令"
            }
            (Self::EnUs, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "Project rename unavailable · no command was sent"
            }
            (Self::ZhCn, Some(Notice::BackendUnavailable(BackendAction::RenameProject))) => {
                "Project 重命名不可用 · 未发送命令"
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
            ) => "Workspace 设置不可用 · 未发送命令",
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
            ) => "Worktree 不可用 · 未发送命令",
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
            (Self::EnUs, None) => "Site unavailable · no command has been sent",
            (Self::ZhCn, None) => "Site 不可用 · 未发送任何命令",
            (Self::EnUs, Some(Notice::TerminalInputUnavailable)) => {
                "Terminal input unavailable · input was not sent"
            }
            (Self::ZhCn, Some(Notice::TerminalInputUnavailable)) => "终端输入不可用 · 未发送输入",
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
        self.native_phase_status_values(
            view.phase(),
            view.active_position(),
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
            Phase::Creating | Phase::CreateCommittedNeedDetach => self.native_creating().to_owned(),
            Phase::ProjectionUnavailable => self.native_committed_refresh_required().to_owned(),
            Phase::MoveProjectionUnavailable => match self {
                Self::EnUs => "Tab moved · Ctrl+B w to refresh tab order".to_owned(),
                Self::ZhCn => "标签已移动 · Ctrl+B w 刷新标签顺序".to_owned(),
            },
            Phase::PanelProjectionUnavailable => match self {
                Self::EnUs => "Panel operation committed · Ctrl+B w to refresh".to_owned(),
                Self::ZhCn => "面板操作已提交 · Ctrl+B w 刷新".to_owned(),
            },
            Phase::RenameProjectionUnavailable => match self {
                Self::EnUs => "Rename committed · Ctrl+B w to refresh".to_owned(),
                Self::ZhCn => "重命名已提交 · Ctrl+B w 刷新".to_owned(),
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
    #[cfg(windows)]
    pub(crate) fn agent_unseen_badge(self, count: usize) -> String {
        if count == 0 {
            return String::new();
        }
        match self {
            Self::EnUs => format!(" [{count} unseen]"),
            Self::ZhCn => format!(" [{count} 未读]"),
        }
    }

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

    pub(crate) fn native_notice(
        self,
        notice: crate::terminal_session::NativeSessionNotice,
    ) -> String {
        use crate::terminal_session::NativeSessionNotice;
        match notice {
            #[cfg(windows)]
            NativeSessionNotice::Settings(notice) => self.settings_notice(notice).to_owned(),
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
                Self::EnUs => "Select a Workspace to close; Standalone stays open".to_owned(),
                Self::ZhCn => "请先选择要关闭的工作区；独立终端保持打开".to_owned(),
            },
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
                Self::EnUs => "History position lost · returning to live".to_owned(),
                Self::ZhCn => "历史位置已失效 · 正在返回实时".to_owned(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandAccepted => match self {
                Self::EnUs => "Command accepted".into(),
                Self::ZhCn => "命令已接受".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandFailed => match self {
                Self::EnUs => "Command or focus failed · check the command catalog".into(),
                Self::ZhCn => "命令或聚焦失败 · 请检查命令目录".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CommandUnknown => match self {
                Self::EnUs => "Command outcome unknown · inspect before running again".into(),
                Self::ZhCn => "命令结果不确定 · 请检查后再决定是否执行".into(),
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
                    "Popup connection or cleanup uncertain · restart the Site before reopening"
                        .to_owned()
                }
                Self::ZhCn => "popup 连接或清理状态不确定 · 重启 Site 后再打开".to_owned(),
            },
            NativeSessionNotice::HistoryUnavailable => match self {
                Self::EnUs => "History is not available on this pane".to_owned(),
                Self::ZhCn => "此窗格历史不可用".to_owned(),
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
            #[cfg(windows)]
            NativeSessionNotice::CopyModeUnavailable => match self {
                Self::EnUs => "Copy mode is not available on this pane".into(),
                Self::ZhCn => "此窗格暂时无法进入复制模式".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CopyModeBusy => match self {
                Self::EnUs => "Copy mode is busy; wait before trying again".into(),
                Self::ZhCn => "复制模式正在处理，请稍后再操作".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CopyModeInvalidated => match self {
                Self::EnUs => "Copy mode ended because its content or pane changed".into(),
                Self::ZhCn => "内容或窗格已改变，复制模式已退出".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CopyModeQueryFailed => match self {
                Self::EnUs => {
                    "Could not read matching content; try the motion or search again".into()
                }
                Self::ZhCn => "无法读取对应内容，请重新移动或搜索".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CopyModeNoMatch => match self {
                Self::EnUs => "No search match".into(),
                Self::ZhCn => "没有搜索匹配项".into(),
            },
            #[cfg(windows)]
            NativeSessionNotice::CopyModeSearchLimit => match self {
                Self::EnUs => "Rejected search: invalid text or more than 512 characters".into(),
                Self::ZhCn => "拒绝搜索：文本无效或超过 512 个字符".into(),
            },
            #[cfg(windows)]
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
                    "The saved position was removed; showing the first available member.".into()
                }
                Self::ZhCn => "上次位置已被删除；已打开首个可用成员。".into(),
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
            Self::EnUs => format!(
                "Tab {position} · Ready · view only{}",
                if cfg!(windows) {
                    " · Ctrl+B t to take control"
                } else {
                    ""
                }
            ),
            Self::ZhCn => format!(
                "Tab {position} · 已就绪 · 仅查看{}",
                if cfg!(windows) {
                    " · Ctrl+B t 接管"
                } else {
                    ""
                }
            ),
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

#[cfg(any(target_os = "linux", windows))]
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
            (Self::EnUs, StartupFailure::Configuration) => {
                "Invalid Windows configuration. Run orchbee config check for details, correct the setting and retry.\n"
            }
            #[cfg(windows)]
            (Self::ZhCn, StartupFailure::Configuration) => {
                "Windows 配置无效。运行 orchbee config check 查看详情，修正后重试。\n"
            }
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
            (_, StartupFailure::SocketPathTooLong) => self.cli_socket_path_too_long(),
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "Local startup could not confirm existing ownership or startup completion. The checkpoint is retained. Run orchbee site diagnose with the same --site selection; use orchbee --site recovery-work site start for another session. Live tasks require Host adoption; incomplete or legacy records need diagnosis.\n"
                } else {
                    "The local Site requires explicit recovery. No replacement process was started.\n"
                }
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "无法确认既有进程归属或启动完成情况，已保留检查点。请用相同 --site 选择运行 orchbee site diagnose；可执行 orchbee --site recovery-work site start 另开会话。存活任务仍需 Host 接管；不完整或旧版记录需要诊断。\n"
                } else {
                    "本地 Site 需要显式恢复。未启动替代进程。\n"
                }
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

/// Temporary render-owned strings; the borrowed shell text remains cheap to copy.
pub(crate) struct BoundShellText {
    base: ShellText<'static>,
    hints: [String; 10],
}

impl BoundShellText {
    pub(crate) fn text(&self) -> ShellText<'_> {
        ShellText {
            exit_hint: &self.hints[0],
            hint_short: &self.hints[1],
            help_dismiss_hint: &self.hints[2],
            help_dismiss_hint_short: &self.hints[3],
            help_paging_hint: &self.hints[4],
            pointer_released: &self.hints[5],
            prefix_active: &self.hints[6],
            prefix_idle: &self.hints[7],
            sidebar_hint: &self.hints[8],
            zoom_badge: &self.hints[9],
            ..self.base
        }
    }
}

impl ShellText<'static> {
    pub(crate) fn bind(self, view: &crate::client_view::ClientView) -> BoundShellText {
        let hints = [
            self.exit_hint,
            self.hint_short,
            self.help_dismiss_hint,
            self.help_dismiss_hint_short,
            self.help_paging_hint,
            self.pointer_released,
            self.prefix_active,
            self.prefix_idle,
            self.sidebar_hint,
            self.zoom_badge,
        ]
        .map(|hint| self.bound_hint(view, hint));
        #[cfg(windows)]
        let hints = {
            let mut hints = hints;
            if view.machines.sidebar_navigation_active() {
                use crate::machine_picker::{SIDEBAR_BINDINGS, SidebarCommand};
                hints[2] = SIDEBAR_BINDINGS
                    .iter()
                    .filter(|(_, command)| {
                        matches!(command, SidebarCommand::Help | SidebarCommand::Exit)
                    })
                    .map(|(binding, _)| format_binding(binding, LabelStyle::for_host()))
                    .collect::<Vec<_>>()
                    .join(" / ");
                hints[3] = SIDEBAR_BINDINGS
                    .iter()
                    .filter(|(_, command)| *command == SidebarCommand::Exit)
                    .map(|(binding, _)| format_binding(binding, LabelStyle::for_host()))
                    .collect::<Vec<_>>()
                    .join(" / ");
                hints[4] = crate::client_view::ClientView::sidebar_help_page_label();
            } else if view.copy_mode_active() {
                hints[2] = format!(
                    "F1 · {} · Esc · {}",
                    (self.prefix_desc)(PrefixDesc::ShowHelp),
                    (self.prefix_desc)(PrefixDesc::CopyExit)
                );
                hints[3] = format!("Esc · {}", (self.prefix_desc)(PrefixDesc::CopyExit));
                hints[4] = "PageUp / PageDown".into();
            } else if view.resize_mode() {
                hints[2] = format!(
                    "? · {} · Esc · {}",
                    (self.prefix_desc)(PrefixDesc::ShowHelp),
                    (self.prefix_desc)(PrefixDesc::ResizeExit)
                );
                hints[3] = format!("Esc · {}", (self.prefix_desc)(PrefixDesc::ResizeExit));
                hints[4] = view
                    .resize_help_rows()
                    .into_iter()
                    .find(|(_, desc)| *desc == PrefixDesc::HelpPage)
                    .expect("resize registry includes help paging")
                    .0;
            }
            hints
        };
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

impl ShellText<'_> {
    pub(crate) fn configured_shortcut_rows(
        &self,
        view: &crate::client_view::ClientView,
    ) -> Vec<(String, String)> {
        #[cfg(windows)]
        if view.machines.sidebar_navigation_active() {
            use crate::machine_picker::{SIDEBAR_BINDINGS, SidebarCommand};
            let mut rows: Vec<_> = SIDEBAR_BINDINGS
                .iter()
                .filter(|(_, command)| *command != SidebarCommand::Exit)
                .map(|(binding, command)| {
                    (
                        format!("{} ", format_binding(binding, LabelStyle::for_host())),
                        (self.sidebar_action)(*command).to_owned(),
                    )
                })
                .collect();
            rows.push((
                format!(
                    "{} ",
                    crate::client_view::ClientView::sidebar_help_page_label()
                ),
                (self.prefix_desc)(PrefixDesc::HelpPage).to_owned(),
            ));
            rows.extend(
                SIDEBAR_BINDINGS
                    .iter()
                    .filter(|(_, command)| *command == SidebarCommand::Exit)
                    .map(|(binding, command)| {
                        (
                            format!("{} ", format_binding(binding, LabelStyle::for_host())),
                            (self.sidebar_action)(*command).to_owned(),
                        )
                    }),
            );
            return rows;
        }
        #[cfg(windows)]
        if view.copy_mode_active() {
            return view
                .copy_help_rows()
                .into_iter()
                .map(|(key, desc)| (format!("{key} "), (self.prefix_desc)(desc).to_owned()))
                .collect();
        }
        #[cfg(windows)]
        if view.resize_mode() {
            return view
                .resize_help_rows()
                .into_iter()
                .map(|(key, desc)| (format!("{key} "), (self.prefix_desc)(desc).to_owned()))
                .collect();
        }
        let mut rows: Vec<_> = view
            .bindings()
            .descriptions()
            .into_iter()
            .flat_map(|desc| {
                #[cfg(windows)]
                if desc == PrefixDesc::HelpPage {
                    use crate::client_view::PrefixAction;
                    return [
                        (PrefixAction::HelpPagePrevious, self.history_previous),
                        (PrefixAction::HelpPageNext, self.help_next),
                    ]
                    .into_iter()
                    .map(|(action, description)| {
                        let labels = view.bindings().action_labels(action);
                        (
                            if labels.is_empty() {
                                self.binding_unbound.into()
                            } else {
                                labels.join(" / ")
                            },
                            description.to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                }
                let label = view
                    .bindings()
                    .group_label(desc, self.binding_unbound)
                    .unwrap_or_else(|| self.binding_unbound.into());
                let description = (self.prefix_desc)(desc);
                #[cfg(windows)]
                let description = view
                    .bindings()
                    .configured_description(desc)
                    .unwrap_or(description);
                vec![(
                    label,
                    crate::terminal_text::sanitize_terminal_text(description).into_owned(),
                )]
            })
            .collect();
        let width = rows
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap_or(0)
            + 1;
        for (key, _) in &mut rows {
            *key = format!("{key:<width$}");
        }
        rows.push((
            format!("{:<width$}", self.other_keys_label),
            (self.prefix_desc)(PrefixDesc::OtherKeys).to_owned(),
        ));
        rows
    }
}
