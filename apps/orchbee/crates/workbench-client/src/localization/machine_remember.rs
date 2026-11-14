//! The words of remembered login answers: the login overlay's box and its lines, and the
//! picker's forget action and its outcomes.

use super::Locale;

/// One sentence or label about remembering an answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RememberText {
    /// The box beside a password or passphrase field.
    Remember,
    /// The box when the store keeps answers only until the computer restarts.
    RememberUntilRestart,
    /// The line in the box's place when no store can be used in this login.
    Unavailable,
    /// The line in the box's place when the local `ssh` is older than OpenSSH 8.5.
    NeedsNewerSsh,
    /// Where the answer goes while the box is ticked.
    Privacy,
    /// `ssh` refused the remembered answer, which was deleted.
    Rejected,
    /// The picker's action that deletes a Machine's remembered answers.
    Forget,
    Forgotten,
    NothingSaved,
    ForgetFailed,
}

impl Locale {
    pub(crate) const fn machine_remember_text(self, text: RememberText) -> &'static str {
        match (self, text) {
            (Self::EnUs, RememberText::Remember) => "Remember password",
            (Self::ZhCn, RememberText::Remember) => "记住密码",
            (Self::EnUs, RememberText::RememberUntilRestart) => {
                "Remember password until this computer restarts"
            }
            (Self::ZhCn, RememberText::RememberUntilRestart) => "记住密码，直到这台电脑重启",
            (Self::EnUs, RememberText::Unavailable) => {
                "Passwords can't be remembered in this kind of login."
            }
            (Self::ZhCn, RememberText::Unavailable) => "在这种登录方式下不能记住密码。",
            (Self::EnUs, RememberText::NeedsNewerSsh) => {
                "Passwords can be remembered with OpenSSH 8.5 or later."
            }
            (Self::ZhCn, RememberText::NeedsNewerSsh) => {
                "需要 OpenSSH 8.5 或更高版本才能记住密码。"
            }
            (Self::EnUs, RememberText::Privacy) => {
                "Goes to ssh, then to the system password store once logged in."
            }
            (Self::ZhCn, RememberText::Privacy) => "交给 ssh，登录成功后存入系统的密码存放处。",
            (Self::EnUs, RememberText::Rejected) => "The saved password no longer works.",
            (Self::ZhCn, RememberText::Rejected) => "保存的密码已失效。",
            (Self::EnUs, RememberText::Forget) => "Forget password",
            (Self::ZhCn, RememberText::Forget) => "忘记密码",
            (Self::EnUs, RememberText::Forgotten) => "Saved password forgotten.",
            (Self::ZhCn, RememberText::Forgotten) => "已忘记保存的密码。",
            (Self::EnUs, RememberText::NothingSaved) => "No saved password for this machine.",
            (Self::ZhCn, RememberText::NothingSaved) => "这台机器没有保存的密码。",
            (Self::EnUs, RememberText::ForgetFailed) => "Couldn't forget the saved password.",
            (Self::ZhCn, RememberText::ForgetFailed) => "未能忘记保存的密码。",
        }
    }
}
