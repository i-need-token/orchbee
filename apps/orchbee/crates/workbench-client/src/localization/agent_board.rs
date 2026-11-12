//! Localized Agent board, Start Agent picker and start notice labels; does not reinterpret Agent
//! state.

use super::Locale;
use crate::workbench_state::{AgentStartNotice, AgentStartRefusal};

/// Text-only start outcome a status notice reports; a successful start reports nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentStartStatus {
    Failed,
    NotFound(&'static str),
    Exited(&'static str),
    UnsupportedShell(&'static str),
    Refused(AgentStartRefusal),
    Unknown,
    PanelBusy,
}

impl From<AgentStartNotice> for AgentStartStatus {
    fn from(notice: AgentStartNotice) -> Self {
        match notice {
            AgentStartNotice::Failed => Self::Failed,
            AgentStartNotice::NotFound(product) => Self::NotFound(product),
            AgentStartNotice::Exited(product) => Self::Exited(product),
            AgentStartNotice::UnsupportedShell(product) => Self::UnsupportedShell(product),
            AgentStartNotice::Refused(refusal) => Self::Refused(refusal),
            AgentStartNotice::Unknown => Self::Unknown,
            AgentStartNotice::PanelBusy => Self::PanelBusy,
        }
    }
}

/// The Agent board's localized copy. Read-state lines, jump-refusal notices,
/// action labels and shortcut verbs live here; the Board key map supplies the
/// separately formatted effective key labels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentBoardText {
    pub(crate) title: &'static str,
    pub(crate) open_button: &'static str,
    pub(crate) refresh_button: &'static str,
    /// No completed read yet — never rendered as "no Agents".
    pub(crate) unread: &'static str,
    pub(crate) in_flight: &'static str,
    /// The page walk exhausted its bound — shown, never hidden.
    pub(crate) incomplete: &'static str,
    pub(crate) read_denied: &'static str,
    pub(crate) read_unavailable: &'static str,
    pub(crate) read_transport: &'static str,
    pub(crate) lane_unavailable: &'static str,
    /// A completed read found zero sessions.
    pub(crate) empty: &'static str,
    /// A partial empty read is not proof that there are no sessions.
    pub(crate) empty_partial: &'static str,
    /// The board's first row and its key hint, which open the Start Agent picker.
    pub(crate) start_row: &'static str,
    pub(crate) start_hint: &'static str,
    pub(crate) picker: AgentPickerText,
    pub(crate) start_failed: &'static str,
    /// `{product}` is the provider's product name.
    pub(crate) start_not_found: &'static str,
    pub(crate) start_exited: &'static str,
    /// The new shell takes no start command, so nothing was typed into it.
    pub(crate) start_unsupported_shell: &'static str,
    /// Nothing was typed because the shell's prompt could not be confirmed; each ends in what
    /// to do before starting again.
    pub(crate) start_not_at_prompt: &'static str,
    pub(crate) start_unmarked: &'static str,
    pub(crate) start_session_too_old: &'static str,
    pub(crate) start_terminal_too_old: &'static str,
    pub(crate) start_unknown: &'static str,
    pub(crate) start_panel_busy: &'static str,
    /// The shorter forms the status line shows when a start notice does not fit; each keeps
    /// the outcome and drops the advice.
    pub(crate) start_short: AgentStartShortText,
    pub(crate) jumping: &'static str,
    pub(crate) close_hint: &'static str,
    pub(crate) select_hint: &'static str,
    pub(crate) jump_hint: &'static str,
    pub(crate) refresh_hint: &'static str,
    pub(crate) stale: &'static str,
    pub(crate) disconnected: &'static str,
    pub(crate) no_workspace: &'static str,
    /// Location placeholder for a session without a navigable panel.
    pub(crate) no_panel: &'static str,
    pub(crate) jump_gone: &'static str,
    pub(crate) jump_unavailable: &'static str,
    pub(crate) jump_denied: &'static str,
    pub(crate) jump_ended: &'static str,
    pub(crate) jump_no_placement: &'static str,
    pub(crate) jump_expired: &'static str,
}

impl AgentBoardText {
    pub(crate) const fn start_refusal(self, refusal: AgentStartRefusal) -> &'static str {
        match refusal {
            AgentStartRefusal::NotAtPrompt => self.start_not_at_prompt,
            AgentStartRefusal::Unmarked => self.start_unmarked,
            AgentStartRefusal::SessionTooOld => self.start_session_too_old,
            AgentStartRefusal::TerminalTooOld => self.start_terminal_too_old,
        }
    }
}

/// The Start Agent picker's copy. Product names and commands are never translated.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentPickerText {
    pub(crate) title: &'static str,
    pub(crate) search: &'static str,
    pub(crate) opens_in: &'static str,
    pub(crate) this_panel: &'static str,
    pub(crate) new_tab: &'static str,
    pub(crate) split_right: &'static str,
    pub(crate) split_down: &'static str,
    /// The place whose start first opens the New Worktree form.
    pub(crate) new_worktree: &'static str,
    pub(crate) recent: &'static str,
    pub(crate) installed: &'static str,
    pub(crate) not_found: &'static str,
    pub(crate) not_checked: &'static str,
    /// The fold row over providers that are all not found; `{count}` is how many it holds.
    pub(crate) fold_not_found: &'static str,
    /// The fold row over providers that all can't be checked; `{count}` is how many it holds.
    pub(crate) fold_cant_check: &'static str,
    /// The fold row over some of each; `{count}` is how many it holds.
    pub(crate) fold_both: &'static str,
    pub(crate) meta_not_found: &'static str,
    pub(crate) meta_cant_check: &'static str,
    pub(crate) no_match: &'static str,
    pub(crate) checking: &'static str,
    /// No lookup completed on the Session's machine and the last one failed or cannot run there.
    pub(crate) couldnt_check: &'static str,
    pub(crate) unsupported: &'static str,
    pub(crate) start_hint: &'static str,
    /// The footer verb for `Tab`, which moves to the next place.
    pub(crate) place_hint: &'static str,
}

/// Shorter start notices for a narrow status line. A panel-busy notice is already short.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentStartShortText {
    pub(crate) failed: &'static str,
    pub(crate) not_found: &'static str,
    pub(crate) exited: &'static str,
    pub(crate) unsupported_shell: &'static str,
    pub(crate) not_at_prompt: &'static str,
    pub(crate) unmarked: &'static str,
    pub(crate) session_too_old: &'static str,
    pub(crate) terminal_too_old: &'static str,
    pub(crate) unknown: &'static str,
}

const EN_US_BOARD: AgentBoardText = AgentBoardText {
    title: "Agent board",
    open_button: "Open",
    refresh_button: "Refresh",
    unread: "No completed Agent read yet",
    in_flight: "Reading Agent sessions…",
    incomplete: "listing incomplete",
    read_denied: "Agent read denied",
    read_unavailable: "Agent runtime unavailable",
    read_transport: "Agent read failed: transport",
    lane_unavailable: "Agent board unavailable: no read lane",
    empty: "No Agent sessions",
    empty_partial: "No sessions in this partial read.",
    start_row: "Start Agent",
    start_hint: "start Agent",
    picker: AgentPickerText {
        title: "Start Agent",
        search: "Search",
        opens_in: "Opens in: {place}",
        this_panel: "this Panel",
        new_tab: "new Tab",
        split_right: "split right",
        split_down: "split down",
        new_worktree: "new Worktree…",
        recent: "Recent",
        installed: "Installed",
        not_found: "Not found",
        not_checked: "Not checked on this machine",
        fold_not_found: "{count} more not found on this computer",
        fold_cant_check: "{count} more couldn’t be checked",
        fold_both: "{count} more not found or not checked",
        meta_not_found: "not found",
        meta_cant_check: "can’t check",
        no_match: "No Agent matches.",
        checking: "Looking for installed Agents…",
        couldnt_check: "Couldn’t check installed Agents",
        unsupported: "Starting an Agent is not supported on this platform.",
        start_hint: "start",
        place_hint: "place",
    },
    start_failed: "Agent did not start. Check its Panel and try again.",
    start_not_found: "{product} not found · Ctrl+B G choose another",
    start_exited: "{product} exited · see its Tab",
    start_unsupported_shell: "{product} not started · this shell takes no start command",
    start_not_at_prompt: "Nothing was typed: the shell is not at an empty prompt. Finish the running command, or clear the line and press Enter for a new prompt, then try again.",
    start_unmarked: "Nothing was typed: this terminal has not shown a marked prompt. Wait for its prompt, or use a PowerShell or cmd terminal Orchbee started.",
    start_session_too_old: "Nothing was typed: this Session is too old to confirm the prompt. Update Orchbee and restart the Session.",
    start_terminal_too_old: "Nothing was typed: this terminal was started by an older Orchbee. Open a new terminal.",
    start_unknown: "Startup is unconfirmed. Check the terminal before starting another Agent.",
    start_panel_busy: "Panel busy · opened a new Tab",
    start_short: AgentStartShortText {
        failed: "Agent did not start",
        not_found: "{product} not found",
        exited: "{product} exited",
        unsupported_shell: "{product} not started",
        not_at_prompt: "Nothing was typed: shell not at an empty prompt",
        unmarked: "Nothing was typed: no marked prompt yet",
        session_too_old: "Nothing was typed: Session too old",
        terminal_too_old: "Nothing was typed: terminal from an older Orchbee",
        unknown: "Startup unconfirmed · check the terminal",
    },
    jumping: "Jumping…",
    close_hint: "close",
    select_hint: "select",
    jump_hint: "jump",
    refresh_hint: "refresh",
    stale: "stale",
    disconnected: "disconnected",
    no_workspace: "No workspace",
    no_panel: "No navigable panel",
    jump_gone: "Session or its panel is gone",
    jump_unavailable: "Cannot verify session · read unavailable",
    jump_denied: "Session read denied",
    jump_ended: "Session has ended",
    jump_no_placement: "Session has no navigable panel",
    jump_expired: "Jump timed out",
};

const ZH_CN_BOARD: AgentBoardText = AgentBoardText {
    open_button: "打开",
    refresh_button: "刷新",
    title: "智能体看板",
    unread: "尚未完成智能体读取",
    in_flight: "正在读取智能体会话…",
    incomplete: "列表不完整",
    read_denied: "智能体读取被拒绝",
    read_unavailable: "智能体运行时不可用",
    read_transport: "智能体读取失败：传输错误",
    lane_unavailable: "看板不可用：无读取通道",
    empty: "没有智能体会话",
    empty_partial: "本次不完整读取暂未返回会话。",
    start_row: "启动智能体",
    start_hint: "启动智能体",
    picker: AgentPickerText {
        title: "启动智能体",
        search: "搜索",
        opens_in: "打开位置：{place}",
        this_panel: "当前面板",
        new_tab: "新标签页",
        split_right: "向右拆分",
        split_down: "向下拆分",
        new_worktree: "新工作树…",
        recent: "最近使用",
        installed: "已安装",
        not_found: "未找到",
        not_checked: "未在此机器上检查",
        fold_not_found: "另有 {count} 个未在此电脑上找到",
        fold_cant_check: "另有 {count} 个无法检查",
        fold_both: "另有 {count} 个未找到或无法检查",
        meta_not_found: "未找到",
        meta_cant_check: "无法检查",
        no_match: "没有匹配的智能体。",
        checking: "正在查找已安装的智能体…",
        couldnt_check: "无法检查已安装的智能体",
        unsupported: "此平台尚不支持启动智能体。",
        start_hint: "启动",
        place_hint: "位置",
    },
    start_failed: "智能体未启动，请检查其面板后重试。",
    start_not_found: "未找到 {product} · Ctrl+B G 换一个",
    start_exited: "{product} 已退出 · 请查看其标签页",
    start_unsupported_shell: "{product} 未启动 · 这个 shell 不接受启动命令",
    start_not_at_prompt: "未输入任何内容：shell 不在空的提示符上。请先结束正在运行的命令，或清空输入行后按回车获得新的提示符，再重试。",
    start_unmarked: "未输入任何内容：这个终端还没有出现带标记的提示符。请等提示符出现，或改用 Orchbee 启动的 PowerShell 或 cmd 终端。",
    start_session_too_old: "未输入任何内容：这个会话版本太旧，无法确认提示符。请更新 Orchbee 并重启会话。",
    start_terminal_too_old: "未输入任何内容：这个终端由旧版 Orchbee 启动。请新开一个终端。",
    start_unknown: "尚未确认启动结果，请先查看终端，再决定是否重新启动。",
    start_panel_busy: "面板忙碌 · 已在新标签页打开",
    start_short: AgentStartShortText {
        failed: "智能体未启动",
        not_found: "未找到 {product}",
        exited: "{product} 已退出",
        unsupported_shell: "{product} 未启动",
        not_at_prompt: "未输入任何内容：shell 不在空的提示符上",
        unmarked: "未输入任何内容：尚无带标记的提示符",
        session_too_old: "未输入任何内容：会话版本太旧",
        terminal_too_old: "未输入任何内容：终端由旧版 Orchbee 启动",
        unknown: "启动结果未确认 · 请查看终端",
    },
    jumping: "正在跳转…",
    close_hint: "关闭",
    select_hint: "选择",
    jump_hint: "跳转",
    refresh_hint: "刷新",
    stale: "已过期",
    disconnected: "已断连",
    no_workspace: "未打开工作区",
    no_panel: "无可跳转面板",
    jump_gone: "会话或其面板已不存在",
    jump_unavailable: "无法验证会话 · 读取不可用",
    jump_denied: "会话读取被拒绝",
    jump_ended: "会话已结束",
    jump_no_placement: "会话没有可跳转的面板",
    jump_expired: "跳转超时",
};

/// The renderer-facing read state of the board's last list attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardRead {
    /// No completed read yet — never rendered as "no Agents".
    Unread,
    InFlight,
    /// `complete` marks the page walk reached the canonical end marker;
    /// `false` means the walk exhausted its budget — shown, never hidden.
    Ready {
        complete: bool,
        at: std::time::Instant,
    },
    Failed(AgentBoardReadFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentBoardReadFailure {
    /// The dedicated read lane could not be established at all.
    LaneUnavailable,
    Denied,
    /// The Agent runtime answered `unavailable`.
    Unavailable,
    /// Transport/connection failure, including post-submission ambiguity.
    Transport,
}

/// Why a jump refused; every variant maps to a localized board notice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JumpFailure {
    /// Get found the session but no resolvable navigable attachment, or the
    /// session disappeared / its presentation identity vanished.
    Gone,
    /// Get could not produce an observation (transport or runtime
    /// unavailability) — the jump never proceeds from unverified state.
    Unavailable,
    Denied,
    /// The canonical lifecycle says the session already ended.
    Ended,
    /// The session has no navigable presentation attachment at all.
    NoPlacement,
    /// The jump's total budget elapsed before landing.
    Expired,
}

/// The sidebar's honest summary of the last completed read — `None` means
/// no completed read ever ran and the sidebar shows no count (never zero).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentBoardSummary {
    pub(crate) sessions: usize,
    pub(crate) complete: bool,
    /// When that read completed — the sidebar renders its age.
    pub(crate) at: std::time::Instant,
}

impl Locale {
    pub(super) const fn agent_board_text(self) -> AgentBoardText {
        match self {
            Self::EnUs => EN_US_BOARD,
            Self::ZhCn => ZH_CN_BOARD,
        }
    }

    /// One board row's canonical semantic status, rendered verbatim —
    /// the board localizes the server's verdict, never reinterprets it.
    pub(crate) fn agent_status(
        self,
        status: &orchbee_agent_observation_protocol::AgentSemanticStatus,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentSemanticStatus;
        match status {
            AgentSemanticStatus::NeedsAttention { count } => match self {
                Self::EnUs => format!("needs attention · {}", count.value()),
                Self::ZhCn => format!("需要关注 · {}", count.value()),
            },
            AgentSemanticStatus::Failed {} => match self {
                Self::EnUs => "failed".to_owned(),
                Self::ZhCn => "失败".to_owned(),
            },
            AgentSemanticStatus::Succeeded {} => match self {
                Self::EnUs => "Done".to_owned(),
                Self::ZhCn => "完成".to_owned(),
            },
            AgentSemanticStatus::Canceled {} => match self {
                Self::EnUs => "canceled".to_owned(),
                Self::ZhCn => "已取消".to_owned(),
            },
            AgentSemanticStatus::Interrupted {} => match self {
                Self::EnUs => "interrupted".to_owned(),
                Self::ZhCn => "已中断".to_owned(),
            },
            AgentSemanticStatus::Working { .. } => match self {
                Self::EnUs => "working".to_owned(),
                Self::ZhCn => "工作中".to_owned(),
            },
            AgentSemanticStatus::Idle {} => match self {
                Self::EnUs => "idle".to_owned(),
                Self::ZhCn => "空闲".to_owned(),
            },
            AgentSemanticStatus::Unknown { .. } => match self {
                Self::EnUs => "unknown".to_owned(),
                Self::ZhCn => "未知".to_owned(),
            },
            AgentSemanticStatus::Ended {} => match self {
                Self::EnUs => "ended".to_owned(),
                Self::ZhCn => "已结束".to_owned(),
            },
        }
    }

    /// The board body's line for a non-ready read state.
    pub(crate) fn agent_board_read_line(
        self,
        read: crate::localization::AgentBoardRead,
    ) -> &'static str {
        use crate::localization::{AgentBoardRead, AgentBoardReadFailure};
        let board = self.agent_board_text();
        match read {
            AgentBoardRead::Unread => board.unread,
            AgentBoardRead::InFlight => board.in_flight,
            AgentBoardRead::Failed(AgentBoardReadFailure::Denied) => board.read_denied,
            AgentBoardRead::Failed(AgentBoardReadFailure::Unavailable) => board.read_unavailable,
            AgentBoardRead::Failed(AgentBoardReadFailure::Transport) => board.read_transport,
            AgentBoardRead::Failed(AgentBoardReadFailure::LaneUnavailable) => {
                board.lane_unavailable
            }
            AgentBoardRead::Ready { .. } => board.empty,
        }
    }

    /// A jump refusal's board notice.
    pub(crate) fn agent_board_notice(
        self,
        failure: crate::localization::JumpFailure,
    ) -> &'static str {
        use crate::localization::JumpFailure;
        let board = self.agent_board_text();
        match failure {
            JumpFailure::Gone => board.jump_gone,
            JumpFailure::Unavailable => board.jump_unavailable,
            JumpFailure::Denied => board.jump_denied,
            JumpFailure::Ended => board.jump_ended,
            JumpFailure::NoPlacement => board.jump_no_placement,
            JumpFailure::Expired => board.jump_expired,
        }
    }

    /// The sidebar's one-line Agent summary: the last *completed* read's
    /// count plus its completeness and age marks — `None` input means no
    /// completed read ever ran and the line must not exist at all.
    pub(crate) fn agent_sidebar_line(
        self,
        summary: crate::localization::AgentBoardSummary,
        elapsed: std::time::Duration,
    ) -> String {
        let seconds = elapsed.as_secs();
        let mark = if summary.complete { "" } else { "+" };
        match self {
            Self::EnUs => {
                format!(
                    "  {} {} · {}s ago",
                    summary.sessions,
                    if summary.sessions == 1 {
                        "agent"
                    } else {
                        "agents"
                    },
                    seconds,
                ) + mark
            }
            Self::ZhCn => format!("  智能体 {} · {}秒前", summary.sessions, seconds) + mark,
        }
    }

    pub(crate) fn agent_start_phase(self, status: impl Into<AgentStartStatus>) -> String {
        use AgentStartStatus as Status;
        let text = self.agent_board_text();
        match status.into() {
            Status::Failed => text.start_failed,
            Status::NotFound(product) => return text.start_not_found.replace("{product}", product),
            Status::Exited(product) => return text.start_exited.replace("{product}", product),
            Status::UnsupportedShell(product) => {
                return text.start_unsupported_shell.replace("{product}", product);
            }
            Status::Refused(refusal) => text.start_refusal(refusal),
            Status::Unknown => text.start_unknown,
            Status::PanelBusy => text.start_panel_busy,
        }
        .to_owned()
    }

    /// The status line's shorter form of a start notice, for every notice that has one.
    pub(crate) fn agent_start_phase_shorter(
        self,
        status: impl Into<AgentStartStatus>,
    ) -> Option<String> {
        use AgentStartStatus as Status;
        let short = self.agent_board_text().start_short;
        Some(
            match status.into() {
                Status::Failed => short.failed,
                Status::NotFound(product) => {
                    return Some(short.not_found.replace("{product}", product));
                }
                Status::Exited(product) => {
                    return Some(short.exited.replace("{product}", product));
                }
                Status::UnsupportedShell(product) => {
                    return Some(short.unsupported_shell.replace("{product}", product));
                }
                Status::Refused(AgentStartRefusal::NotAtPrompt) => short.not_at_prompt,
                Status::Refused(AgentStartRefusal::Unmarked) => short.unmarked,
                Status::Refused(AgentStartRefusal::SessionTooOld) => short.session_too_old,
                Status::Refused(AgentStartRefusal::TerminalTooOld) => short.terminal_too_old,
                Status::Unknown => short.unknown,
                Status::PanelBusy => return None,
            }
            .to_owned(),
        )
    }
}
