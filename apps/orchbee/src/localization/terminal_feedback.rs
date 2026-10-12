use super::Locale;
use crate::terminal_session::CompletionReminder;

impl Locale {
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
