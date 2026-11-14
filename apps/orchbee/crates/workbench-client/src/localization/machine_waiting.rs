//! The TUI's notice of a command login offered to it, and the status that names the Machine while
//! another overlay holds the keyboard.

use super::Locale;

/// The notice's own words, beside the login overlay's title.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitingLabel {
    /// Fails the waiting commands and stops asking for this Machine.
    Decline,
    LogIn,
    /// The person accepted: the login's question follows.
    Starting,
}

impl Locale {
    pub(crate) const fn machine_waiting_label(self, label: WaitingLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, WaitingLabel::Decline) => "Not now",
            (Self::EnUs, WaitingLabel::LogIn) => "Log in",
            (Self::EnUs, WaitingLabel::Starting) => "Logging in…",
            (Self::ZhCn, WaitingLabel::Decline) => "暂不登录",
            (Self::ZhCn, WaitingLabel::LogIn) => "登录",
            (Self::ZhCn, WaitingLabel::Starting) => "正在登录…",
        }
    }

    /// How many commands wait on the login.
    pub(crate) fn machine_waiting_count(self, waiters: u64) -> String {
        match (self, waiters) {
            (Self::EnUs, 1) => "1 command is waiting to log in to this machine.".to_owned(),
            (Self::EnUs, count) => {
                format!("{count} commands are waiting to log in to this machine.")
            }
            (Self::ZhCn, count) => format!("有 {count} 条命令在等待登录这台机器。"),
        }
    }

    /// What declining does to the waiting commands.
    pub(crate) const fn machine_waiting_consequence(self, several: bool) -> &'static str {
        match (self, several) {
            (Self::EnUs, false) => "If you don't log in now, this command will fail.",
            (Self::EnUs, true) => "If you don't log in now, these commands will fail.",
            (Self::ZhCn, false) => "暂不登录的话，这条命令会失败。",
            (Self::ZhCn, true) => "暂不登录的话，这些命令会失败。",
        }
    }

    /// The status while the notice for the Machine named `label` waits for the keyboard.
    pub(crate) fn machine_waiting_deferred(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} needs login"),
            Self::ZhCn => format!("{label} 需要登录"),
        }
    }
}
