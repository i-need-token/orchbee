//! Localized Agent board state and startup labels; does not reinterpret Agent state.

use super::Locale;
use crate::workbench_state::AgentStartRefusal;

/// Text-only startup outcome; operation ownership and running/cancellation policy stay with startup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentStartStatus {
    Starting,
    WaitingInput,
    Unconfirmed,
    Cancelling,
    Started,
    Failed,
    Refused(AgentStartRefusal),
    Unknown,
    Cancelled,
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
    /// The next step shown when the last completed Agent read has no rows.
    pub(crate) empty_start: &'static str,
    /// A partial empty read is not proof that there are no sessions.
    pub(crate) empty_partial: &'static str,
    /// PATH-backed startup form copy.
    pub(crate) start_checking: &'static str,
    pub(crate) start_platform_unsupported: &'static str,
    pub(crate) start_catalog_unavailable: &'static str,
    pub(crate) start_none_found: &'static str,
    pub(crate) start_providers: &'static str,
    pub(crate) start_name: &'static str,
    pub(crate) start_name_placeholder: &'static str,
    pub(crate) start_name_invalid: &'static str,
    pub(crate) start_button: &'static str,
    pub(crate) start_cancel_button: &'static str,
    pub(crate) start_starting: &'static str,
    pub(crate) start_waiting_input: &'static str,
    pub(crate) start_unconfirmed: &'static str,
    pub(crate) start_cancelling: &'static str,
    pub(crate) start_started: &'static str,
    pub(crate) start_failed: &'static str,
    /// Nothing was typed because the shell's prompt could not be confirmed; each ends in what
    /// to do before starting again.
    pub(crate) start_not_at_prompt: &'static str,
    pub(crate) start_unmarked: &'static str,
    pub(crate) start_session_too_old: &'static str,
    pub(crate) start_terminal_too_old: &'static str,
    pub(crate) start_unknown: &'static str,
    pub(crate) start_cancelled: &'static str,
    /// The shorter forms the status line shows when a start notice does not fit; each keeps
    /// the outcome and drops the advice the board states in full.
    pub(crate) start_short: AgentStartShortText,
    pub(crate) start_provider_hint: &'static str,
    pub(crate) start_edit_hint: &'static str,
    pub(crate) start_cancel_hint: &'static str,
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

/// Shorter start notices for a narrow status line. Cancelling is already short and has none.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentStartShortText {
    pub(crate) starting: &'static str,
    pub(crate) waiting_input: &'static str,
    pub(crate) unconfirmed: &'static str,
    pub(crate) started: &'static str,
    pub(crate) failed: &'static str,
    pub(crate) not_at_prompt: &'static str,
    pub(crate) unmarked: &'static str,
    pub(crate) session_too_old: &'static str,
    pub(crate) terminal_too_old: &'static str,
    pub(crate) unknown: &'static str,
    pub(crate) cancelled: &'static str,
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
    empty_start: "Choose an installed Agent. Its name is ready to use or edit.",
    empty_partial: "No sessions in this partial read.",
    start_checking: "Looking for installed commands; you can choose now.",
    start_platform_unsupported: "Starting an Agent is not supported on this platform.",
    start_catalog_unavailable: "Detection incomplete. Choose an Agent installed in this terminal.",
    start_none_found: "Nothing detected. You can still choose an installed Agent.",
    start_providers: "Found on PATH",
    start_name: "Agent name",
    start_name_placeholder: "choose a name",
    start_name_invalid: "Name: 1–32 lowercase letters, digits, _ or -; start with a letter.",
    start_button: "Start Agent",
    start_cancel_button: "Cancel start",
    start_starting: "Waiting for startup confirmation. Check the Agent terminal.",
    start_waiting_input: "Your input is needed in the terminal to continue startup.",
    start_unconfirmed: "Command sent. Check the terminal prompts; readiness is not confirmed. Do not start again.",
    start_cancelling: "Cancelling startup…",
    start_started: "Agent started. Refreshing sessions…",
    start_failed: "Agent did not start. Check the current panel and try again.",
    start_not_at_prompt: "Nothing was typed: the shell is not at an empty prompt. Finish the running command, clear the line, or press Enter for a new prompt, then try again.",
    start_unmarked: "Nothing was typed: this terminal has not shown a marked prompt. Wait for its prompt, or use a PowerShell or cmd terminal Orchbee started.",
    start_session_too_old: "Nothing was typed: this Session is too old to confirm the prompt. Update Orchbee and restart the Session.",
    start_terminal_too_old: "Nothing was typed: this terminal was started by an older Orchbee. Open a new terminal.",
    start_unknown: "Startup is unconfirmed. Check the terminal before starting another Agent.",
    start_cancelled: "Startup canceled. The Agent process may still be running.",
    start_short: AgentStartShortText {
        starting: "Waiting for Agent startup",
        waiting_input: "Agent startup needs your input",
        unconfirmed: "Command sent, not confirmed · do not start again",
        started: "Agent started",
        failed: "Agent did not start",
        not_at_prompt: "Nothing was typed: shell not at an empty prompt",
        unmarked: "Nothing was typed: no marked prompt yet",
        session_too_old: "Nothing was typed: Session too old",
        terminal_too_old: "Nothing was typed: terminal from an older Orchbee",
        unknown: "Startup unconfirmed · check the terminal",
        cancelled: "Startup canceled · Agent may still run",
    },
    start_provider_hint: "more Agents",
    start_edit_hint: "edit name",
    start_cancel_hint: "cancel start",
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
    empty_start: "选择已安装的智能体，名称已填好，也可修改。",
    empty_partial: "本次不完整读取暂未返回会话。",
    start_checking: "正在查找已安装的命令；也可直接选择。",
    start_platform_unsupported: "此平台尚不支持启动智能体。",
    start_catalog_unavailable: "检测未完成，请选择此终端中已安装的智能体。",
    start_none_found: "未检测到命令，仍可手动选择已安装的智能体。",
    start_providers: "已在 PATH 中找到",
    start_name: "智能体名称",
    start_name_placeholder: "请输入名称",
    start_name_invalid: "请以小写字母开头，后接字母、数字、_ 或 -，最多 32 个字符。",
    start_button: "启动智能体",
    start_cancel_button: "取消启动",
    start_starting: "正在等待启动确认，请查看智能体终端并按提示操作。",
    start_waiting_input: "请在终端中确认或输入，以继续启动。",
    start_unconfirmed: "命令已发送，请检查终端提示；尚未确认就绪，请勿重复启动。",
    start_cancelling: "正在取消启动…",
    start_started: "智能体已启动，正在刷新会话…",
    start_failed: "智能体未启动，请检查当前面板后重试。",
    start_not_at_prompt: "未输入任何内容：shell 不在空的提示符上。请先结束正在运行的命令、清空输入行或按回车获得新的提示符，再重试。",
    start_unmarked: "未输入任何内容：这个终端还没有出现带标记的提示符。请等提示符出现，或改用 Orchbee 启动的 PowerShell 或 cmd 终端。",
    start_session_too_old: "未输入任何内容：这个会话版本太旧，无法确认提示符。请更新 Orchbee 并重启会话。",
    start_terminal_too_old: "未输入任何内容：这个终端由旧版 Orchbee 启动。请新开一个终端。",
    start_unknown: "尚未确认启动结果，请先查看终端，再决定是否重新启动。",
    start_cancelled: "已取消启动；智能体进程仍可能在运行。",
    start_short: AgentStartShortText {
        starting: "正在等待启动确认",
        waiting_input: "启动需要你在终端中输入",
        unconfirmed: "命令已发送，未确认就绪 · 请勿重复启动",
        started: "智能体已启动",
        failed: "智能体未启动",
        not_at_prompt: "未输入任何内容：shell 不在空的提示符上",
        unmarked: "未输入任何内容：尚无带标记的提示符",
        session_too_old: "未输入任何内容：会话版本太旧",
        terminal_too_old: "未输入任何内容：终端由旧版 Orchbee 启动",
        unknown: "启动结果未确认 · 请查看终端",
        cancelled: "已取消启动 · 进程可能仍在运行",
    },
    start_provider_hint: "更多智能体",
    start_edit_hint: "编辑名称",
    start_cancel_hint: "取消启动",
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

    pub(crate) fn agent_start_phase(self, phase: impl Into<AgentStartStatus>) -> &'static str {
        use AgentStartStatus as Phase;
        let text = self.agent_board_text();
        match phase.into() {
            Phase::Starting => text.start_starting,
            Phase::WaitingInput => text.start_waiting_input,
            Phase::Unconfirmed => text.start_unconfirmed,
            Phase::Cancelling => text.start_cancelling,
            Phase::Started => text.start_started,
            Phase::Failed => text.start_failed,
            Phase::Refused(refusal) => text.start_refusal(refusal),
            Phase::Unknown => text.start_unknown,
            Phase::Cancelled => text.start_cancelled,
        }
    }

    /// The status line's shorter form of a start notice, for every phase that has one.
    pub(crate) fn agent_start_phase_shorter(
        self,
        phase: impl Into<AgentStartStatus>,
    ) -> Option<&'static str> {
        use AgentStartStatus as Phase;
        let short = self.agent_board_text().start_short;
        Some(match phase.into() {
            Phase::Starting => short.starting,
            Phase::WaitingInput => short.waiting_input,
            Phase::Unconfirmed => short.unconfirmed,
            Phase::Cancelling => return None,
            Phase::Started => short.started,
            Phase::Failed => short.failed,
            Phase::Refused(AgentStartRefusal::NotAtPrompt) => short.not_at_prompt,
            Phase::Refused(AgentStartRefusal::Unmarked) => short.unmarked,
            Phase::Refused(AgentStartRefusal::SessionTooOld) => short.session_too_old,
            Phase::Refused(AgentStartRefusal::TerminalTooOld) => short.terminal_too_old,
            Phase::Unknown => short.unknown,
            Phase::Cancelled => short.cancelled,
        })
    }
}
