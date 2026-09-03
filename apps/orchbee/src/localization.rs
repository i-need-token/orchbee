use std::ffi::OsString;

use crate::client_view::{BackendAction, Notice};

const EN_US_HELP: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee --help\n  orchbee --version\n\nModes:\n  orchbee            Select the interactive TUI (unavailable in this development slice)\n  orchbee --help     Show this help\n  orchbee --version  Show the version\n";
const ZH_CN_HELP: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee --help\n  orchbee --version\n\n模式：\n  orchbee            选择交互式 TUI（本开发切片尚未提供）\n  orchbee --help     显示此帮助\n  orchbee --version  显示版本\n";
const EN_US_TUI_UNAVAILABLE: &str = "The Orchbee TUI is unavailable in this development slice.\n";
const ZH_CN_TUI_UNAVAILABLE: &str = "Orchbee TUI 在本开发切片中尚不可用。\n";
const EN_US_USAGE_FAILURE: &str = "Invalid invocation. Run 'orchbee --help'.\n";
const ZH_CN_USAGE_FAILURE: &str = "调用方式无效。请运行 ‘orchbee --help’。\n";

const EN_US_SHORTCUTS: &[&str] = &[
    "Ctrl+B c       Create a Tab (requires Site)",
    "Ctrl+B n / p   Select next / previous Tab (requires Site)",
    "Ctrl+B 1 … 9   Select ordered Tab (requires Site)",
    "Ctrl+B h/j/k/l Move between Panels (requires Site)",
    "Ctrl+B b       Toggle business sidebar",
    "Ctrl+B w       Open Workbench navigator",
    "Ctrl+B ?       Open this shortcut help",
    "Ctrl+B q / d   Close this shell",
    "Ctrl+B Ctrl+B  Send literal Ctrl+B to the terminal input port",
    "Other keys     Send to the terminal input port",
];
const ZH_CN_SHORTCUTS: &[&str] = &[
    "Ctrl+B c       创建 Tab（需要 Site）",
    "Ctrl+B n / p   选择下一个 / 上一个 Tab（需要 Site）",
    "Ctrl+B 1 … 9   选择对应顺序的 Tab（需要 Site）",
    "Ctrl+B h/j/k/l 在 Panel 之间移动（需要 Site）",
    "Ctrl+B b       切换业务侧边栏",
    "Ctrl+B w       打开 Workbench 导航器",
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
