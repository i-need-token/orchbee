use super::Locale;

/// Already-admitted completion facts; reminder timing and notification policy remain outside text.
pub(crate) struct CompletionReminder {
    pub(crate) title: String,
    pub(crate) duration_milliseconds: u64,
    pub(crate) exit_code: u8,
}

/// Host-reported settlement, independent of the retained terminal frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalEnd {
    Exited(orchbee_terminal_protocol::ExitResult),
    Failed,
}

/// One action of a Panel a restore held because its saved directory was unavailable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HeldRestoreAction {
    Retry,
    StartInHome,
    Close,
}

impl Locale {
    pub(crate) const fn held_restore_title(self) -> &'static str {
        match self {
            Self::EnUs => "Directory unavailable",
            Self::ZhCn => "目录不可用",
        }
    }
    pub(crate) const fn held_restore_action(self, action: HeldRestoreAction) -> &'static str {
        match (self, action) {
            (Self::EnUs, HeldRestoreAction::Retry) => "Retry",
            (Self::ZhCn, HeldRestoreAction::Retry) => "重试",
            (Self::EnUs, HeldRestoreAction::StartInHome) => "Start in home folder",
            (Self::ZhCn, HeldRestoreAction::StartInHome) => "改在主目录启动",
            (Self::EnUs, HeldRestoreAction::Close) => "Close",
            (Self::ZhCn, HeldRestoreAction::Close) => "关闭",
        }
    }
    pub(crate) fn launch_failure_actions(self, settings: &str, copy: &str) -> String {
        match self {
            Self::EnUs => format!("Settings: {settings}\nCopy diagnostics: {copy}"),
            Self::ZhCn => format!("设置：{settings}\n复制诊断：{copy}"),
        }
    }
    pub(crate) fn terminal_end(self, end: crate::localization::TerminalEnd) -> String {
        use crate::localization::TerminalEnd;
        use orchbee_terminal_protocol::ExitResult;
        match (self, end) {
            (Self::EnUs, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
                let code = orchbee_terminal_protocol::format_exit_code(code);
                format!("Shell exited · code {code} · output retained")
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
                let code = orchbee_terminal_protocol::format_exit_code(code);
                format!("Shell 已结束 · 退出码 {code} · 输出已保留")
            }
            (Self::EnUs, TerminalEnd::Exited(ExitResult::Signal(signal))) => {
                format!("Shell ended · signal {signal} · output retained")
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::Signal(signal))) => {
                format!("Shell 已结束 · 信号 {signal} · 输出已保留")
            }
            (Self::EnUs, TerminalEnd::Exited(ExitResult::Unknown(_))) => {
                "Shell ended · exit code unavailable · output retained".into()
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::Unknown(_))) => {
                "Shell 已结束 · 退出码未知 · 输出已保留".into()
            }
            (Self::EnUs, TerminalEnd::Failed) => {
                "Terminal stopped after a runtime failure · output retained".into()
            }
            (Self::ZhCn, TerminalEnd::Failed) => "终端因运行故障停止 · 输出已保留".into(),
        }
    }

    /// Keeps the exit result intact when the retained-output detail cannot fit.
    pub(crate) fn terminal_end_compact(self, end: crate::localization::TerminalEnd) -> String {
        use crate::localization::TerminalEnd;
        use orchbee_terminal_protocol::ExitResult;
        match (self, end) {
            (Self::EnUs, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
                let code = orchbee_terminal_protocol::format_exit_code(code);
                format!("Exited {code}")
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
                let code = orchbee_terminal_protocol::format_exit_code(code);
                format!("已退出 {code}")
            }
            (Self::EnUs, TerminalEnd::Exited(ExitResult::Signal(signal))) => {
                format!("Signal {signal}")
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::Signal(signal))) => {
                format!("信号 {signal}")
            }
            (Self::EnUs, TerminalEnd::Exited(ExitResult::Unknown(_))) => "Exit code unknown".into(),
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::Unknown(_))) => "退出码未知".into(),
            (Self::EnUs, TerminalEnd::Failed) => "Terminal failed".into(),
            (Self::ZhCn, TerminalEnd::Failed) => "终端运行故障".into(),
        }
    }

    pub(crate) fn terminal_launch_failure(
        self,
        failure: crate::localization::LaunchFailure<'_>,
    ) -> String {
        use crate::localization::LaunchProfile;
        let name = match (self, failure.profile) {
            (_, LaunchProfile::Named(name)) => crate::terminal_text::sanitize_terminal_text(name),
            (Self::EnUs, LaunchProfile::SystemDefault) => "System default".into(),
            (Self::ZhCn, LaunchProfile::SystemDefault) => "系统默认".into(),
            (Self::EnUs, LaunchProfile::Unknown) => "Unknown".into(),
            (Self::ZhCn, LaunchProfile::Unknown) => "未知".into(),
        };
        let reason = match (self, failure.executable) {
            (Self::EnUs, Some(path)) => format!("The executable path could not be started. Check that the file exists and is accessible.\n{}", crate::terminal_text::sanitize_terminal_text(path)),
            (Self::ZhCn, Some(path)) => format!("可执行文件路径不可用，无法启动。请检查文件是否存在且可访问。\n{}", crate::terminal_text::sanitize_terminal_text(path)),
            (Self::EnUs, None) => "The configured shell could not be started. Check its launch profile and working directory.".into(),
            (Self::ZhCn, None) => "无法启动所配置的 Shell。请检查启动配置和工作目录。".into(),
        };
        match self {
            Self::EnUs => format!(
                "Terminal creation failed\nLaunch profile: {name}\n\n{reason}\n\nOpen Settings → Launch profiles to repair or choose a profile. Create a new tab with that profile, or close this failed tab."
            ),
            Self::ZhCn => format!(
                "终端创建失败\n启动配置：{name}\n\n{reason}\n\n打开设置 → 启动配置，修复或选择要使用的配置，再用该配置新建标签页。也可以关闭此失败标签页。"
            ),
        }
    }
    pub(crate) fn command_completion_reminder(self, reminder: &CompletionReminder) -> String {
        let seconds = reminder.duration_milliseconds / 1000;
        let milliseconds = reminder.duration_milliseconds % 1000;
        let title = &reminder.title;
        let exit = reminder.exit_code;
        match self {
            Self::EnUs => {
                format!("{title}: command finished · {seconds}.{milliseconds:03} s · exit {exit}")
            }
            Self::ZhCn => {
                format!("{title}：命令已完成 · {seconds}.{milliseconds:03} 秒 · 退出码 {exit}")
            }
        }
    }
}
