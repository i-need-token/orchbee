//! The words of Settings › Machines besides its rows' names and descriptions, which
//! `native_settings::rows` declares: the page's description, a machine's state and Status
//! sentence, its button, what it has free, the stop's confirmation and the outcomes on the status
//! line.

use super::{Locale, RememberText};
use crate::machine_capacity::{UNKNOWN, gigabyte_number};
use crate::machine_settings::{Action, Machine, Outcome, State};
use crate::terminal_text::sanitize_terminal_text;
use orchbee_machine_protocol::capacity::Capacity;

/// One fixed phrase of Settings › Machines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MachineText {
    /// The line under the category's title.
    Description,
    /// This computer's line while machines are saved: what its numbers are.
    ColumnNote,
    /// This computer's line while no machine is saved.
    NoMachine,
    /// A machine's word while its terminals are being stopped.
    Stopping,
    Forget,
    Stop,
    Remove,
}

impl Locale {
    pub(crate) const fn machine_text(self, text: MachineText) -> &'static str {
        match (self, text) {
            (Self::EnUs, MachineText::Description) => "Other computers that run your Workspaces.",
            (Self::ZhCn, MachineText::Description) => "可以运行你的工作区的其他电脑。",
            (Self::EnUs, MachineText::ColumnNote) => "free CPU · memory · disk",
            (Self::ZhCn, MachineText::ColumnNote) => "空闲 CPU · 内存 · 磁盘",
            (Self::EnUs, MachineText::NoMachine) => "Add another computer to run Workspaces on it.",
            (Self::ZhCn, MachineText::NoMachine) => "添加另一台电脑，让工作区可以运行在它上面。",
            (Self::EnUs, MachineText::Stopping) => "Stopping…",
            (Self::ZhCn, MachineText::Stopping) => "正在停止…",
            (Self::EnUs, MachineText::Forget) => "Forget",
            (Self::ZhCn, MachineText::Forget) => "忘记",
            (Self::EnUs, MachineText::Stop) => "Stop",
            (Self::ZhCn, MachineText::Stop) => "停止",
            (Self::EnUs, MachineText::Remove) => "Remove",
            (Self::ZhCn, MachineText::Remove) => "移除",
        }
    }

    /// The label of a Status row's button.
    pub(crate) const fn machine_action(self, action: Action) -> &'static str {
        match (self, action) {
            (Self::EnUs, Action::Disconnect) => "Disconnect",
            (Self::ZhCn, Action::Disconnect) => "断开",
            (Self::EnUs, Action::Connect) => "Connect",
            (Self::ZhCn, Action::Connect) => "连接",
            (Self::EnUs, Action::Retry) => "Retry",
            (Self::ZhCn, Action::Retry) => "重试",
            (Self::EnUs, Action::RetryNow) => "Retry now",
            (Self::ZhCn, Action::RetryNow) => "立即重试",
            (Self::EnUs, Action::LogIn) => "Log in",
            (Self::ZhCn, Action::LogIn) => "登录",
        }
    }

    /// A machine's state in one word, as the machine list says it; a disconnected one reads
    /// Offline.
    pub(crate) fn machine_state_word(self, machine: &Machine) -> &'static str {
        if machine.stopping {
            return self.machine_text(MachineText::Stopping);
        }
        self.machine_picker_text(match machine.state {
            State::Online => "online",
            State::Connecting => "opening",
            State::Reconnecting => "reconnecting",
            State::Offline | State::Disabled => "offline",
            State::NeedsLogin => "needs-login",
            State::NeedsUpdate => "needs-update",
            State::LocalNeedsUpdate => "local-needs-update",
            State::Other(key) => key,
        })
    }

    /// The sentence under a machine's Status.
    pub(crate) fn machine_status(self, machine: &Machine) -> String {
        let label = sanitize_terminal_text(&machine.label);
        // A host without a local clock shows no time rather than a wrong one.
        let clock = machine
            .last_attempt
            .and_then(orchbee_local_runtime::local_clock)
            .map(|clock| format!("{:02}:{:02}", clock.hour, clock.minute));
        let version = machine.version().map(sanitize_terminal_text);
        match (self, machine.state) {
            (Self::EnUs, State::Online) => version.map_or_else(
                || "Connected.".to_owned(),
                |version| format!("Connected. Orchbee {version}."),
            ),
            (Self::ZhCn, State::Online) => version.map_or_else(
                || "已连接。".to_owned(),
                |version| format!("已连接。Orchbee {version}。"),
            ),
            (Self::EnUs, State::Offline) => clock.map_or_else(
                || "Not connected.".to_owned(),
                |clock| format!("It didn't answer at {clock}."),
            ),
            (Self::ZhCn, State::Offline) => clock.map_or_else(
                || "未连接。".to_owned(),
                |clock| format!("{clock} 没有应答。"),
            ),
            (Self::EnUs, State::Disabled) => "Not connected.".to_owned(),
            (Self::ZhCn, State::Disabled) => "未连接。".to_owned(),
            (Self::EnUs, State::NeedsLogin) => "ssh needs your password.".to_owned(),
            (Self::ZhCn, State::NeedsLogin) => "ssh 需要你的密码。".to_owned(),
            (Self::EnUs, State::Reconnecting) => "The connection dropped. Trying again.".to_owned(),
            (Self::ZhCn, State::Reconnecting) => "连接断了，正在重试。".to_owned(),
            (Self::EnUs, State::Connecting) => "Connecting…".to_owned(),
            (Self::ZhCn, State::Connecting) => "正在连接…".to_owned(),
            (Self::EnUs, State::NeedsUpdate) => {
                format!("Orchbee on {label} is too old. Update it there.")
            }
            (Self::ZhCn, State::NeedsUpdate) => {
                format!("{label} 上的 Orchbee 太旧，请在那里更新。")
            }
            (Self::EnUs, State::LocalNeedsUpdate) => {
                format!("Orchbee here is too old for {label}. Update this computer.")
            }
            (Self::ZhCn, State::LocalNeedsUpdate) => {
                format!("这台电脑上的 Orchbee 比 {label} 上的旧，请先更新这台电脑。")
            }
            // Every interface's first detail line of the state, which names its cause.
            (_, State::Other(_)) => self
                .machine_status_lines(&machine.explanation, &machine.label)
                .into_iter()
                .map(|line| line.trim().to_owned())
                .find(|line| !line.is_empty())
                .unwrap_or_else(|| self.machine_state_word(machine).to_owned()),
        }
    }

    /// What a machine has free, written out, as its page's Free now says it.
    pub(crate) fn machine_capacity_long(self, capacity: Option<&Capacity>) -> String {
        let cpu = capacity.and_then(|capacity| capacity.cpu);
        let memory = capacity.and_then(|capacity| capacity.memory);
        let disk = capacity.and_then(|capacity| capacity.disk.as_ref());
        let cpu = cpu.map(|cpu| cpu.free_percent().to_string());
        let memory = memory.map(|memory| {
            (
                gigabyte_number(memory.free()),
                gigabyte_number(memory.total()),
            )
        });
        let disk = disk.map(|disk| {
            (
                gigabyte_number(disk.bytes.free()),
                gigabyte_number(disk.bytes.total()),
            )
        });
        let parts = match self {
            Self::EnUs => [
                cpu.map_or_else(
                    || format!("CPU {UNKNOWN}"),
                    |cpu| format!("{cpu}% CPU free"),
                ),
                memory.map_or_else(
                    || format!("memory {UNKNOWN}"),
                    |(free, total)| format!("{free} of {total} GB memory free"),
                ),
                disk.map_or_else(
                    || format!("disk {UNKNOWN}"),
                    |(free, total)| format!("{free} of {total} GB disk free"),
                ),
            ],
            Self::ZhCn => [
                cpu.map_or_else(
                    || format!("CPU {UNKNOWN}"),
                    |cpu| format!("CPU 空闲 {cpu}%"),
                ),
                memory.map_or_else(
                    || format!("内存 {UNKNOWN}"),
                    |(free, total)| format!("内存空闲 {free} / {total} GB"),
                ),
                disk.map_or_else(
                    || format!("磁盘 {UNKNOWN}"),
                    |(free, total)| format!("磁盘空闲 {free} / {total} GB"),
                ),
            ],
        };
        parts.join(" · ")
    }

    /// What Stop its terminals ends, by the counts its Session reported, as Quit All states them.
    pub(crate) fn machine_stop_confirmation(self, machine: &Machine) -> String {
        let label = sanitize_terminal_text(&machine.label);
        match (self, machine.terminals, machine.agents) {
            (Self::EnUs, None, _) => format!("Stops every terminal on {label}."),
            (Self::ZhCn, None, _) => format!("将停止 {label} 上的所有终端。"),
            // Quit All's words: the working Agents among the terminals that stop.
            (Self::EnUs, Some(terminals), Some(agents @ 1..)) => format!(
                "Stops {} on {label}, including {}.",
                plural(terminals, "terminal"),
                plural(agents, "working Agent")
            ),
            (Self::EnUs, Some(terminals), _) => {
                format!("Stops {} on {label}.", plural(terminals, "terminal"))
            }
            (Self::ZhCn, Some(terminals), Some(agents @ 1..)) => {
                format!("将停止 {label} 上的 {terminals} 个终端，其中 {agents} 个智能体正在工作。")
            }
            (Self::ZhCn, Some(terminals), _) => {
                format!("将停止 {label} 上的 {terminals} 个终端。")
            }
        }
    }

    /// What removing a machine leaves, by the Workspaces its Session holds when they are known.
    pub(crate) fn machine_remove_description(self, workspaces: Option<usize>) -> String {
        match (self, workspaces) {
            (Self::EnUs, Some(0)) => "Nothing runs on it from here.".to_owned(),
            (Self::ZhCn, Some(0)) => "这里没有东西运行在它上面。".to_owned(),
            (Self::EnUs, Some(1)) => {
                "Its Workspace leaves the sidebar. Its terminals keep running there.".to_owned()
            }
            (Self::EnUs, Some(count)) => format!(
                "Its {count} Workspaces leave the sidebar. Their terminals keep running there."
            ),
            (Self::ZhCn, Some(count)) => {
                format!("它的 {count} 个工作区会从侧边栏消失，上面的终端继续运行。")
            }
            (Self::EnUs, None) => {
                "Its Workspaces leave the sidebar. Their terminals keep running there.".to_owned()
            }
            (Self::ZhCn, None) => "它的工作区会从侧边栏消失，上面的终端继续运行。".to_owned(),
        }
    }

    /// What a machine that is not connected cannot do now, as Quit All names it.
    pub(crate) fn machine_not_connected(self, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match self {
            Self::EnUs => format!("{label} is not connected."),
            Self::ZhCn => format!("{label} 没有连接。"),
        }
    }

    /// The rule a machine's name follows, which a refused name shows.
    pub(crate) const fn machine_name_rule(self) -> &'static str {
        match self {
            Self::EnUs => {
                "A name has 1 to 128 characters, no control characters, and is not local, which always means this computer."
            }
            Self::ZhCn => {
                "名称为 1 到 128 个字符，不能含控制字符，也不能是 local（它始终指这台电脑）。"
            }
        }
    }

    pub(crate) const fn machine_save_failed(self) -> &'static str {
        match self {
            Self::EnUs => "The machine list couldn't be saved.",
            Self::ZhCn => "没能保存机器列表。",
        }
    }

    /// How the person's last request ended, for the page's status line.
    pub(crate) fn machine_outcome(self, outcome: &Outcome) -> String {
        match (self, outcome) {
            (Self::EnUs, Outcome::Removed(label)) => format!(
                "Removed {}. Its terminals keep running there.",
                sanitize_terminal_text(label)
            ),
            (Self::ZhCn, Outcome::Removed(label)) => format!(
                "已移除 {}。它上面的终端会继续运行。",
                sanitize_terminal_text(label)
            ),
            (Self::EnUs, Outcome::Stopped(label)) => {
                format!(
                    "Stopped the terminals on {}.",
                    sanitize_terminal_text(label)
                )
            }
            (Self::ZhCn, Outcome::Stopped(label)) => {
                format!("已停止 {} 上的终端。", sanitize_terminal_text(label))
            }
            (Self::EnUs, Outcome::StopFailed(label)) => format!(
                "The terminals on {} couldn't be stopped.",
                sanitize_terminal_text(label)
            ),
            (Self::ZhCn, Outcome::StopFailed(label)) => {
                format!("没能停止 {} 上的终端。", sanitize_terminal_text(label))
            }
            (_, Outcome::NotConnected(label)) => self.machine_not_connected(label),
            (_, Outcome::NameRefused) => self.machine_name_rule().to_owned(),
            (_, Outcome::SaveFailed) => self.machine_save_failed().to_owned(),
            (_, Outcome::PasswordForgotten) => self
                .machine_remember_text(RememberText::Forgotten)
                .to_owned(),
            (_, Outcome::NoPasswordSaved) => self
                .machine_remember_text(RememberText::NothingSaved)
                .to_owned(),
            (_, Outcome::ForgetFailed) => self
                .machine_remember_text(RememberText::ForgetFailed)
                .to_owned(),
        }
    }
}

/// `count` of `noun`, English plural by `s`.
fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}
