//! The TUI's notice of a command login offered to it, why an accepted one ended before its
//! question, and the status that names the Machine while another overlay holds the keyboard.

use super::Locale;
use crate::machine_login::{Ending, Withdrawn};
use orchbee_machine_protocol::hub::LoginEnd;

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

    /// Why a login the person accepted ended before its question, in a few words.
    pub(crate) const fn machine_waiting_ended(self, why: Withdrawn) -> &'static str {
        match (self, why) {
            (Self::EnUs, Withdrawn::Taken) => "Another Orchbee window took this login.",
            (Self::ZhCn, Withdrawn::Taken) => "另一个 Orchbee 窗口接手了这次登录。",
            (Self::EnUs, Withdrawn::Ended(Ending::Hub(LoginEnd::Succeeded))) => "Logged in.",
            (Self::ZhCn, Withdrawn::Ended(Ending::Hub(LoginEnd::Succeeded))) => "已登录。",
            (Self::EnUs, Withdrawn::Ended(Ending::Hub(LoginEnd::Failed))) => "Login failed.",
            (Self::ZhCn, Withdrawn::Ended(Ending::Hub(LoginEnd::Failed))) => "登录失败。",
            (Self::EnUs, Withdrawn::Ended(Ending::Hub(LoginEnd::Dismissed))) => "Login cancelled.",
            (Self::ZhCn, Withdrawn::Ended(Ending::Hub(LoginEnd::Dismissed))) => "登录已取消。",
            (Self::EnUs, Withdrawn::Ended(Ending::Hub(LoginEnd::Unavailable))) => {
                "Connection failed."
            }
            (Self::ZhCn, Withdrawn::Ended(Ending::Hub(LoginEnd::Unavailable))) => "连接失败。",
            (Self::EnUs, Withdrawn::Ended(Ending::Lost)) => "Machine connections stopped.",
            (Self::ZhCn, Withdrawn::Ended(Ending::Lost)) => "机器连接已中断。",
            // Nobody remained to answer, this client let it go, or an end this build does not know.
            (Self::EnUs, Withdrawn::Ended(_)) => "Login ended.",
            (Self::ZhCn, Withdrawn::Ended(_)) => "登录已结束。",
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
