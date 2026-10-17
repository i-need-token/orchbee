use super::Locale;
use crate::terminal_session::CompletionReminder;

impl Locale {
    #[cfg(windows)]
    pub(crate) fn launch_failure_actions(self, settings: &str, copy: &str) -> String {
        match self {
            Self::EnUs => format!("Settings: {settings}\nCopy diagnostics: {copy}"),
            Self::ZhCn => format!("设置：{settings}\n复制诊断：{copy}"),
        }
    }
    pub(crate) fn terminal_end(self, end: crate::terminal_projection::TerminalEnd) -> String {
        use crate::terminal_projection::TerminalEnd;
        use orchbee_terminal_protocol::ExitResult;
        match (self, end) {
            (Self::EnUs, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
                format!("Shell exited · code {code} · output retained")
            }
            (Self::ZhCn, TerminalEnd::Exited(ExitResult::ExitCode(code))) => {
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

    pub(crate) fn terminal_launch_failure(
        self,
        failure: crate::terminal_session::NativeLaunchFailure<'_>,
    ) -> String {
        use crate::terminal_session::NativeLaunchProfile;
        let name = match (self, failure.profile) {
            (_, NativeLaunchProfile::Named(name)) => {
                crate::terminal_text::sanitize_terminal_text(name)
            }
            (Self::EnUs, NativeLaunchProfile::SystemDefault) => "System default".into(),
            (Self::ZhCn, NativeLaunchProfile::SystemDefault) => "系统默认".into(),
            (Self::EnUs, NativeLaunchProfile::Unknown) => "Unknown".into(),
            (Self::ZhCn, NativeLaunchProfile::Unknown) => "未知".into(),
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
