//! The words of a remote Machine's detail lines and its action, the same in every interface.
//! `machine_status` chooses the facts; each line fits the 70-column picker.

use super::Locale;
use crate::machine_status::{Action, Explanation, Line, Unreachable};
use crate::terminal_text::sanitize_terminal_text;

impl Locale {
    /// The detail lines of `explanation` for the Machine named `label`.
    pub(crate) fn machine_status_lines(
        self,
        explanation: &Explanation,
        label: &str,
    ) -> Vec<String> {
        let label = sanitize_terminal_text(label);
        explanation
            .lines
            .iter()
            .flat_map(|line| self.machine_status_line(line, &label))
            .collect()
    }

    fn machine_status_line(self, line: &Line, label: &str) -> Vec<String> {
        // A host without a local clock shows no time rather than a wrong one.
        let clock = |at: u64| {
            orchbee_local_runtime::local_clock(at)
                .map(|clock| format!("{:02}:{:02}", clock.hour, clock.minute))
        };
        let text = match (self, line) {
            (_, Line::Login(state)) => return self.machine_login_detail(state),
            (_, Line::Hub(status)) => return self.machine_hub_detail(status).into_iter().collect(),
            (
                _,
                Line::Contents {
                    projects,
                    workspaces,
                    tabs,
                },
            ) => self.machine_details(*projects, *workspaces, *tabs),
            (_, Line::Label(name)) => format!("  {}", sanitize_terminal_text(name)),
            (Self::EnUs, Line::Version(version)) => {
                format!("Orchbee {}", sanitize_terminal_text(version))
            }
            (Self::ZhCn, Line::Version(version)) => {
                format!("Orchbee {}", sanitize_terminal_text(version))
            }
            (Self::EnUs, Line::Unreachable { host, why }) => {
                let host = sanitize_terminal_text(host);
                match why {
                    Unreachable::TimedOut => format!("Can't reach {host}: timed out."),
                    Unreachable::Refused => format!("Can't reach {host}: connection refused."),
                    Unreachable::Unresolved => {
                        format!("Can't reach {host}: the name does not resolve.")
                    }
                    Unreachable::Other => format!("Can't reach {host}."),
                }
            }
            (Self::ZhCn, Line::Unreachable { host, why }) => {
                let host = sanitize_terminal_text(host);
                match why {
                    Unreachable::TimedOut => format!("连不上 {host}：超时。"),
                    Unreachable::Refused => format!("连不上 {host}：被拒绝。"),
                    Unreachable::Unresolved => format!("连不上 {host}：域名解析不了。"),
                    Unreachable::Other => format!("连不上 {host}。"),
                }
            }
            (Self::EnUs, Line::NoSsh) => "This computer has no OpenSSH.".into(),
            (Self::ZhCn, Line::NoSsh) => "这台电脑没有 OpenSSH。".into(),
            (Self::EnUs, Line::InstallSsh) => install_ssh_en().into(),
            (Self::ZhCn, Line::InstallSsh) => install_ssh_zh().into(),
            (Self::EnUs, Line::SshFailed { host }) => {
                format!("ssh could not connect to {}.", sanitize_terminal_text(host))
            }
            (Self::ZhCn, Line::SshFailed { host }) => {
                format!("ssh 没能连上 {}。", sanitize_terminal_text(host))
            }
            (Self::EnUs, Line::RemoteProtocol) => {
                format!("Orchbee on {label} answered outside the protocol.")
            }
            (Self::ZhCn, Line::RemoteProtocol) => format!("{label} 上的 Orchbee 应答不符合协议。"),
            (Self::EnUs, Line::NotStarted) => format!("Orchbee on {label} didn't start."),
            (Self::ZhCn, Line::NotStarted) => format!("{label} 上的 Orchbee 没能启动。"),
            (Self::EnUs, Line::LinkLost) => "The connection was lost.".into(),
            (Self::ZhCn, Line::LinkLost) => "连接断开了。".into(),
            (_, Line::NextAttempt(at) | Line::LastAttempt(at)) => {
                let Some(clock) = clock(*at) else {
                    return Vec::new();
                };
                match (self, line) {
                    (Self::EnUs, Line::NextAttempt(_)) => format!("Next try at {clock}."),
                    (Self::ZhCn, Line::NextAttempt(_)) => format!("下次重试：{clock}。"),
                    (Self::EnUs, _) => format!("Last tried at {clock}."),
                    (Self::ZhCn, _) => format!("上次尝试：{clock}。"),
                }
            }
            (Self::EnUs, Line::Unresponsive) => {
                "The connection is still there, but the other side doesn't answer.".into()
            }
            (Self::ZhCn, Line::Unresponsive) => "连接还在，但对方没有回应。".into(),
            (Self::EnUs, Line::Installing) => format!("Copying Orchbee to {label}."),
            (Self::ZhCn, Line::Installing) => format!("正在把 Orchbee 复制到 {label}。"),
            (Self::EnUs, Line::NotInstalled) if orchbee_local_runtime::is_release() => {
                format!("Orchbee isn't on {label}.")
            }
            (Self::ZhCn, Line::NotInstalled) if orchbee_local_runtime::is_release() => {
                format!("{label} 上没有 Orchbee。")
            }
            // A development or test build runs only its own environment's Orchbee there.
            (Self::EnUs, Line::NotInstalled) => {
                format!("This environment's Orchbee isn't on {label}.")
            }
            (Self::ZhCn, Line::NotInstalled) => format!("{label} 上没有这个环境的 Orchbee。"),
            (
                Self::EnUs,
                Line::Unsupported {
                    platform: Some(platform),
                    architecture: Some(architecture),
                },
            ) => format!(
                "{label} is {} on {}; this Orchbee can't run there.",
                system(platform),
                sanitize_terminal_text(architecture)
            ),
            (
                Self::ZhCn,
                Line::Unsupported {
                    platform: Some(platform),
                    architecture: Some(architecture),
                },
            ) => format!(
                "{label} 是 {} 的 {}，这个版本的 Orchbee 还不能在它上面运行。",
                sanitize_terminal_text(architecture),
                system(platform)
            ),
            (Self::EnUs, Line::Unsupported { .. }) => {
                format!("The system on {label} can't run Orchbee.")
            }
            (Self::ZhCn, Line::Unsupported { .. }) => format!("{label} 的系统运行不了 Orchbee。"),
            (Self::EnUs, Line::Supported) => "Supported: Windows (x64, ARM64).".into(),
            (Self::ZhCn, Line::Supported) => "支持：Windows（x64、ARM64）。".into(),
            (Self::EnUs, Line::RemoteTooOld { version: Some(v) }) => format!(
                "Orchbee {} on {label} is too old.",
                sanitize_terminal_text(v)
            ),
            (Self::ZhCn, Line::RemoteTooOld { version: Some(v) }) => {
                format!("{label} 上的 Orchbee {} 太旧。", sanitize_terminal_text(v))
            }
            (Self::EnUs, Line::RemoteTooOld { version: None }) => {
                format!("Orchbee on {label} is too old.")
            }
            (Self::ZhCn, Line::RemoteTooOld { version: None }) => {
                format!("{label} 上的 Orchbee 太旧。")
            }
            (Self::EnUs, Line::SessionTooOld) => {
                format!("The Session running on {label} is too old.")
            }
            (Self::ZhCn, Line::SessionTooOld) => format!("{label} 上运行的会话太旧。"),
            (Self::EnUs, Line::UpdateRemote) => {
                "Update Orchbee there, then run `orchbee server stop` there.".into()
            }
            (Self::ZhCn, Line::UpdateRemote) => {
                "请在远端更新 Orchbee，再在远端运行 `orchbee server stop`。".into()
            }
            (Self::EnUs, Line::LocalTooOld) => {
                format!("This computer's Orchbee is too old; {label} doesn't accept it.")
            }
            (Self::ZhCn, Line::LocalTooOld) => {
                format!("这台电脑的 Orchbee 太旧，{label} 不接受。")
            }
            (Self::EnUs, Line::UpdateLocal) => "Update Orchbee on this computer.".into(),
            (Self::ZhCn, Line::UpdateLocal) => "请更新这台电脑上的 Orchbee。".into(),
            (Self::EnUs, Line::OtherEnvironment) => {
                "The remote is an Orchbee of another environment.".into()
            }
            (Self::ZhCn, Line::OtherEnvironment) => "远端是另一个环境的 Orchbee。".into(),
        };
        vec![text]
    }

    /// The label of the action a Machine's detail offers.
    pub(crate) const fn machine_status_action(self, action: Action) -> &'static str {
        match (self, action) {
            (Self::EnUs, Action::Open) => "Open",
            (Self::ZhCn, Action::Open) => "打开",
            (Self::EnUs, Action::Retry) => "Retry",
            (Self::ZhCn, Action::Retry) => "重试",
            (Self::EnUs, Action::RetryNow) => "Retry now",
            (Self::ZhCn, Action::RetryNow) => "立即重试",
            (Self::EnUs, Action::Reconnect) => "Reconnect",
            (Self::ZhCn, Action::Reconnect) => "重新连接",
            (Self::EnUs, Action::Enable) => "Enable",
            (Self::ZhCn, Action::Enable) => "启用",
            (Self::EnUs, Action::LogIn) => "Log in",
            (Self::ZhCn, Action::LogIn) => "登录",
            (Self::EnUs, Action::LogInAgain) => "Log in again",
            (Self::ZhCn, Action::LogInAgain) => "重新登录",
            (Self::EnUs, Action::Trust) => "Review and trust",
            (Self::ZhCn, Action::Trust) => "查看并信任",
        }
    }
}

/// The name of a remote system token, as the launch script reports it.
fn system(platform: &str) -> String {
    match platform {
        "windows" => "Windows".to_owned(),
        "macos" | "darwin" => "macOS".to_owned(),
        "linux" => "Linux".to_owned(),
        other => sanitize_terminal_text(other).into_owned(),
    }
}

/// How to install the OpenSSH client on this computer's system.
const fn install_ssh_en() -> &'static str {
    if cfg!(windows) {
        "Install OpenSSH Client in Settings > System > Optional features."
    } else if cfg!(target_os = "macos") {
        "macOS includes OpenSSH; Orchbee runs /usr/bin/ssh, so check that it is there."
    } else {
        "Install the openssh-client package with your package manager."
    }
}

const fn install_ssh_zh() -> &'static str {
    if cfg!(windows) {
        "在「设置 > 系统 > 可选功能」中安装 OpenSSH 客户端。"
    } else if cfg!(target_os = "macos") {
        "macOS 自带 OpenSSH；Orchbee 运行 /usr/bin/ssh，请确认它存在。"
    } else {
        "用系统的包管理器安装 openssh-client。"
    }
}
