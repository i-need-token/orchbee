//! Login question wording every interface shares, and the TUI's login overlay, picker details and
//! lost-link copy. `machine connect` and the TUI word a question from the same sentences; `ssh`'s
//! own prompt is shown beside them, escaped and bounded by `machine_question`.

use super::Locale;
use crate::machine_discovery::LoginState;
use crate::machine_login::Ending;
use crate::terminal_text::escape;
use orchbee_machine_protocol::{
    HostKey,
    hub::{LoginEnd, LoginFailedCause, NeedsLoginCause, QuestionKind},
};

/// What a login question asks, as its wording names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineQuestion<'a> {
    Password,
    KeyPassphrase,
    /// Whether to trust a new host, with its offered key.
    HostKey {
        algorithm: &'a str,
        fingerprint: &'a str,
    },
    /// Anything else: a server's question, a jump host's, or one of `ssh`'s this build does not
    /// read.
    Other,
}

impl<'a> MachineQuestion<'a> {
    /// The wording of a question of `kind`, as the client read it.
    pub fn of(kind: &'a QuestionKind) -> Self {
        match kind {
            QuestionKind::Password => Self::Password,
            QuestionKind::KeyPassphrase => Self::KeyPassphrase,
            QuestionKind::HostKey { host_key } => Self::HostKey {
                algorithm: host_key.algorithm.as_str(),
                fingerprint: host_key.fingerprint.as_str(),
            },
            QuestionKind::Other => Self::Other,
        }
    }
}

/// A login overlay's own words, beside the shared question sentences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LoginLabel {
    Title,
    Cancel,
    LogIn,
    Trust,
    /// The host-key step was submitted without `yes`.
    TypeYes,
    /// The answer is on its way to `ssh`.
    Sending,
    /// Where a secret goes and that it is not kept.
    Privacy,
    /// Closes the overlay of a login that ended without the person.
    Close,
}

impl Locale {
    /// The mark before each line of `ssh`'s own prompt.
    pub const fn machine_question_said(self) -> &'static str {
        match self {
            Self::EnUs => "ssh: ",
            Self::ZhCn => "ssh：",
        }
    }

    /// Orchbee's one line for a question to the Machine whose `ssh` target is `target`;
    /// `repeated` says `ssh` refused the previous answer of this kind.
    pub fn machine_question_asked(
        self,
        target: &str,
        question: &MachineQuestion<'_>,
        repeated: bool,
    ) -> String {
        let target = escape(target);
        let refused = match (self, repeated) {
            (_, false) => "",
            (Self::EnUs, true) => "Not accepted. ",
            (Self::ZhCn, true) => "未被接受。",
        };
        let asked = match (self, question) {
            (Self::EnUs, MachineQuestion::Password) => format!("Password for {target}: "),
            (Self::ZhCn, MachineQuestion::Password) => format!("{target} 的密码："),
            (Self::EnUs, MachineQuestion::KeyPassphrase) => {
                format!("Key passphrase for {target}: ")
            }
            (Self::ZhCn, MachineQuestion::KeyPassphrase) => format!("{target} 的密钥口令："),
            (
                _,
                MachineQuestion::HostKey {
                    algorithm,
                    fingerprint,
                },
            ) => {
                let host = self.machine_question_new_host_with(&target, algorithm, fingerprint);
                let yes = self.machine_question_type_yes();
                match self {
                    Self::EnUs => format!("{host} {yes}"),
                    Self::ZhCn => format!("{host}{yes}"),
                }
            }
            (Self::EnUs, MachineQuestion::Other) => {
                format!("Answer for ssh above, connecting to {target}: ")
            }
            (Self::ZhCn, MachineQuestion::Other) => {
                format!("回答上面 ssh 的提问（正在连接 {target}）：")
            }
        };
        format!("{refused}{asked}")
    }

    /// The first-time host sentence up to the offered key, for an interface that shows the
    /// fingerprint whole on its own lines after it and asks for the typed `yes` in its own field
    /// with [`Self::machine_question_type_yes`].
    pub fn machine_question_new_host_lead(self, target: &str, algorithm: &str) -> String {
        let target = escape(target);
        let algorithm = escape(algorithm);
        match self {
            Self::EnUs => format!("New host {target}, {algorithm} key:"),
            Self::ZhCn => format!("新主机 {target}，{algorithm} 密钥："),
        }
    }

    fn machine_question_new_host_with(
        self,
        target: &str,
        algorithm: &str,
        fingerprint: &str,
    ) -> String {
        let algorithm = escape(algorithm);
        let fingerprint = escape(fingerprint);
        match self {
            Self::EnUs => format!("New host {target}, {algorithm} key {fingerprint}."),
            Self::ZhCn => format!("新主机 {target}，{algorithm} 密钥 {fingerprint}。"),
        }
    }

    /// What the first-time host question asks to be typed.
    pub const fn machine_question_type_yes(self) -> &'static str {
        match self {
            Self::EnUs => "Type yes to trust it: ",
            Self::ZhCn => "输入 yes 以信任：",
        }
    }

    /// Why an answer was not delivered: `again` when it held a character `ssh` cannot take and
    /// the question is asked again; otherwise its endpoint was gone or refused it and the login
    /// stops.
    pub const fn machine_answer_undelivered(self, again: bool) -> &'static str {
        match (self, again) {
            (Self::EnUs, true) => "That answer holds a character ssh cannot take. Type it again.",
            (Self::ZhCn, true) => "该回答含有 ssh 无法接收的字符，请重新输入。",
            (Self::EnUs, false) => "The answer did not reach ssh, so signing in stops.",
            (Self::ZhCn, false) => "回答未能送达 ssh，登录已停止。",
        }
    }

    pub(crate) const fn machine_login_label(self, label: LoginLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, LoginLabel::Title) => "Log in to machine",
            (Self::ZhCn, LoginLabel::Title) => "登录机器",
            (Self::EnUs, LoginLabel::Cancel) => "Cancel",
            (Self::ZhCn, LoginLabel::Cancel) => "取消",
            (Self::EnUs, LoginLabel::LogIn) => "Log in",
            (Self::ZhCn, LoginLabel::LogIn) => "登录",
            (Self::EnUs, LoginLabel::Trust) => "Trust and continue",
            (Self::ZhCn, LoginLabel::Trust) => "信任并继续",
            (Self::EnUs, LoginLabel::TypeYes) => "Type yes to trust this host.",
            (Self::ZhCn, LoginLabel::TypeYes) => "输入 yes 以信任这台主机。",
            (Self::EnUs, LoginLabel::Sending) => "Logging in…",
            (Self::ZhCn, LoginLabel::Sending) => "正在登录…",
            (Self::EnUs, LoginLabel::Privacy) => "Goes only to ssh. Orchbee does not keep it.",
            (Self::ZhCn, LoginLabel::Privacy) => "只交给 ssh，Orchbee 不保存。",
            (Self::EnUs, LoginLabel::Close) => "Close",
            (Self::ZhCn, LoginLabel::Close) => "关闭",
        }
    }

    /// How a login ended without the person's act, in the words the picker uses for the same end
    /// once the overlay closes.
    pub(crate) fn machine_login_outcome(self, ending: Ending) -> &'static str {
        match ending {
            Ending::Hub(LoginEnd::Succeeded) => match self {
                Self::EnUs => "Logged in.",
                Self::ZhCn => "已登录。",
            },
            Ending::Hub(LoginEnd::Failed) => self.machine_picker_text("login-failed-message"),
            Ending::Undelivered => self.machine_picker_text("login-undelivered"),
            Ending::Hub(LoginEnd::Dismissed | LoginEnd::Abandoned)
            | Ending::Dismissed
            | Ending::Lost => self.machine_picker_text("login-unanswered-message"),
            // Unavailable, or an end this build does not know.
            Ending::Hub(_) => self.machine_picker_text("failed"),
        }
    }

    /// The overlay's line when `ssh` refused the previous answer of this kind.
    pub(crate) const fn machine_login_repeated(
        self,
        question: &MachineQuestion<'_>,
    ) -> &'static str {
        match (self, question) {
            (Self::EnUs, MachineQuestion::Password) => "Wrong password. Try again.",
            (Self::ZhCn, MachineQuestion::Password) => "密码不对，请重试。",
            (Self::EnUs, MachineQuestion::KeyPassphrase) => "Wrong passphrase. Try again.",
            (Self::ZhCn, MachineQuestion::KeyPassphrase) => "口令不对，请重试。",
            (Self::EnUs, _) => "Not accepted. Try again.",
            (Self::ZhCn, _) => "未被接受，请重试。",
        }
    }

    /// A remote pane whose Machine's link was lost.
    pub(crate) const fn machine_link_lost(self) -> &'static str {
        match self {
            Self::EnUs => "connection lost · Enter to log in again",
            Self::ZhCn => "连接已断开 · Enter 重新登录",
        }
    }

    /// The selected row's explanation of a login state: what happened and what the person can
    /// do. Each line fits the 70-column picker.
    pub(crate) fn machine_login_detail(self, state: &LoginState) -> Vec<String> {
        let key = |key: &HostKey| {
            let algorithm = escape(&key.algorithm.as_str().to_ascii_uppercase());
            let fingerprint = escape(key.fingerprint.as_str());
            match self {
                Self::EnUs => format!("{algorithm} key {fingerprint}"),
                Self::ZhCn => format!("{algorithm} 密钥 {fingerprint}"),
            }
        };
        let again = match self {
            Self::EnUs => "Choose this machine to log in again.",
            Self::ZhCn => "选择这台机器即可重新登录。",
        };
        let log_in = match self {
            Self::EnUs => "Choose this machine to log in.",
            Self::ZhCn => "选择这台机器即可登录。",
        };
        let lines: Vec<String> = match (self, state) {
            (_, LoginState::NeedsLogin(cause)) => {
                let what = match (self, cause) {
                    (Self::EnUs, Some(NeedsLoginCause::RememberedRejected)) => {
                        "The saved password no longer works."
                    }
                    (Self::ZhCn, Some(NeedsLoginCause::RememberedRejected)) => "保存的密码已失效。",
                    (Self::EnUs, Some(NeedsLoginCause::LoginDismissed)) => {
                        "The login was dismissed."
                    }
                    (Self::ZhCn, Some(NeedsLoginCause::LoginDismissed)) => "登录已取消。",
                    (Self::EnUs, Some(NeedsLoginCause::Disconnected)) => {
                        "The machine was disconnected."
                    }
                    (Self::ZhCn, Some(NeedsLoginCause::Disconnected)) => "机器已断开连接。",
                    (Self::EnUs, Some(NeedsLoginCause::AuthRequired) | None) => {
                        "Logging in needs a password or another answer."
                    }
                    (Self::ZhCn, Some(NeedsLoginCause::AuthRequired) | None) => {
                        "登录需要密码或其他回答。"
                    }
                };
                vec![what.to_owned(), log_in.to_owned()]
            }
            (_, LoginState::LoginFailed(cause)) => {
                let what = match (self, cause) {
                    (Self::EnUs, Some(LoginFailedCause::PasswordRejected)) => {
                        "The password was not accepted."
                    }
                    (Self::ZhCn, Some(LoginFailedCause::PasswordRejected)) => "密码不对。",
                    (Self::EnUs, Some(LoginFailedCause::KeyRejected)) => {
                        "The server refused your key."
                    }
                    (Self::ZhCn, Some(LoginFailedCause::KeyRejected)) => "对方拒绝了你的密钥。",
                    (Self::EnUs, None) => "SSH did not accept the login.",
                    (Self::ZhCn, None) => "SSH 未接受登录。",
                };
                vec![what.to_owned(), again.to_owned()]
            }
            (Self::EnUs, LoginState::Untrusted(host_key)) => vec![
                "SSH has not seen this host before.".to_owned(),
                key(host_key),
                "Choose this machine to check and trust it.".to_owned(),
            ],
            (Self::ZhCn, LoginState::Untrusted(host_key)) => vec![
                "SSH 没有见过这台主机。".to_owned(),
                key(host_key),
                "选择这台机器即可查看并信任。".to_owned(),
            ],
            (_, LoginState::KeyChanged { host_key, host }) => {
                let mut lines = vec![
                    match self {
                        Self::EnUs => {
                            "The host key differs from last time; it may be impersonated."
                        }
                        Self::ZhCn => "主机密钥和上次不同，可能被冒充。",
                    }
                    .to_owned(),
                ];
                lines.extend(host_key.as_ref().map(key));
                // `[host]:port` is quoted so no shell reads the brackets as a pattern.
                let host = if host.starts_with('[') {
                    format!("\"{}\"", escape(host))
                } else {
                    escape(host)
                };
                lines.push(match self {
                    Self::EnUs => format!("If the change is expected, run: ssh-keygen -R {host}"),
                    Self::ZhCn => format!("如确认变更属实，请运行：ssh-keygen -R {host}"),
                });
                lines
            }
        };
        lines
    }
}
