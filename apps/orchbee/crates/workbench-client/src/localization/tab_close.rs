use super::Locale;
use crate::terminal_text::sanitize_terminal_text;

/// Whether the close surface asks for initial confirmation or explicit resumption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseConfirmationKind {
    Close,
    Resume,
}

/// The current close lifecycle description, separate from a close authority token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseStatusKind {
    AwaitingStatus,
    Closing,
    CleanupBlocked,
    OutcomeUnknown,
    DisposedAwaitingPublication,
    Closed,
}

/// What a close ends across the terminals in its scope. Only a launch failure, a Host settlement
/// on the terminal's exact attachment, or proven disposal counts a terminal as ended; every other
/// terminal, including one this Client does not observe, counts as running.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CloseImpact {
    pub(crate) running: usize,
    pub(crate) ended: usize,
}

/// Counts describe confirmed lifecycle closure, independently of snapshot/viewer delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CloseSetStatus {
    pub(crate) completed: usize,
    pub(crate) total: usize,
    pub(crate) halted: bool,
    /// The one member whose submitted Close stopped short and may still finish.
    pub(crate) unconfirmed: bool,
}

/// Current close feedback only; no target token or executable close authority.
#[derive(Clone, Copy)]
pub(crate) struct CloseStatusText<'a> {
    pub(crate) label: &'a str,
    pub(crate) kind: CloseStatusKind,
    pub(crate) resumable: bool,
    pub(crate) observation_paused: bool,
    pub(crate) unavailable: bool,
}

impl Locale {
    pub(crate) const fn tab_close_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }

    pub(crate) const fn tab_close_confirm_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }

    /// A count appears only when there are several Tabs; one Tab keeps the singular question.
    pub(crate) fn close_set_title(self, count: usize) -> String {
        match self {
            Self::EnUs if count == 1 => "Close Tab?".into(),
            Self::EnUs => format!("Close {count} Tabs?"),
            Self::ZhCn if count == 1 => "关闭标签页？".into(),
            Self::ZhCn => format!("关闭 {count} 个标签页？"),
        }
    }
    pub(crate) fn close_set_more(self, count: usize) -> String {
        match self {
            Self::EnUs if count == 1 => "… and 1 more Tab".to_owned(),
            Self::EnUs => format!("… and {count} more Tabs"),
            Self::ZhCn => format!("……以及另外 {count} 个标签页"),
        }
    }
    /// A sequence of one Tab shows only its halt. The completed form replaces the running count, so
    /// a finished close never reads as work that continues. A halted count names the Tabs left open
    /// apart from the one submitted close whose outcome is still unknown.
    pub(crate) fn close_set_status(self, status: crate::localization::CloseSetStatus) -> String {
        let crate::localization::CloseSetStatus {
            completed,
            total,
            halted,
            unconfirmed,
        } = status;
        let remaining = total.saturating_sub(completed);
        let open = remaining.saturating_sub(usize::from(unconfirmed));
        match (self, halted, remaining) {
            (Self::EnUs, true, _) if total == 1 => "Close not confirmed".into(),
            (Self::ZhCn, true, _) if total == 1 => "关闭未确认".into(),
            (Self::EnUs, true, _) if unconfirmed && open == 0 => {
                format!("Closing stopped: {completed} closed, 1 unconfirmed")
            }
            (Self::ZhCn, true, _) if unconfirmed && open == 0 => {
                format!("关闭已停止：{completed} 个已关闭，1 个未确认")
            }
            (Self::EnUs, true, _) if unconfirmed => {
                format!("Closing stopped: {completed} closed, 1 unconfirmed, {open} still open")
            }
            (Self::ZhCn, true, _) if unconfirmed => {
                format!("关闭已停止：{completed} 个已关闭，1 个未确认，{open} 个仍打开")
            }
            (Self::EnUs, true, _) => {
                format!("Closing stopped: {completed} closed, {open} still open")
            }
            (Self::ZhCn, true, _) => {
                format!("关闭已停止：{completed} 个已关闭，{open} 个仍打开")
            }
            (Self::EnUs, false, 0) => format!("{total} Tabs closed"),
            (Self::ZhCn, false, 0) => format!("已关闭 {total} 个标签页"),
            (Self::EnUs, false, _) => {
                format!("Closing Tabs: {completed} closed, {remaining} remaining")
            }
            (Self::ZhCn, false, _) => {
                format!("正在关闭标签页：{completed} 个已关闭，剩余 {remaining} 个")
            }
        }
    }
    pub(crate) const fn close_title(self, kind: CloseConfirmationKind) -> &'static str {
        match (self, kind) {
            (Self::EnUs, CloseConfirmationKind::Close) => "Close Tab?",
            (Self::ZhCn, CloseConfirmationKind::Close) => "关闭标签页？",
            (Self::EnUs, CloseConfirmationKind::Resume) => "Resume closing this Tab?",
            (Self::ZhCn, CloseConfirmationKind::Resume) => "继续关闭此标签页？",
        }
    }

    /// One sentence of facts with counts: the terminals a close ends and the retained output it
    /// discards. It serves one Tab, several Tabs and a Panel alike.
    pub(crate) fn close_warning(self, impact: CloseImpact) -> String {
        let plural = |count: usize| if count == 1 { "" } else { "s" };
        match (self, impact.running, impact.ended) {
            (Self::EnUs, 0, 0) => "Ends its terminals.".into(),
            (Self::ZhCn, 0, 0) => "将结束其中的终端。".into(),
            (Self::EnUs, running, 0) => format!("Ends {running} terminal{}.", plural(running)),
            (Self::ZhCn, running, 0) => format!("将结束 {running} 个终端。"),
            (Self::EnUs, 0, ended) => format!(
                "Discards the output of {ended} ended terminal{}.",
                plural(ended)
            ),
            (Self::ZhCn, 0, ended) => format!("将丢弃 {ended} 个已结束终端的输出。"),
            (Self::EnUs, running, ended) => format!(
                "Ends {running} terminal{} and discards the output of {ended} ended one{}.",
                plural(running),
                plural(ended)
            ),
            (Self::ZhCn, running, ended) => {
                format!("将结束 {running} 个终端，并丢弃 {ended} 个已结束终端的输出。")
            }
        }
    }

    pub(crate) fn native_close_status<'a>(self, status: impl Into<CloseStatusText<'a>>) -> String {
        let status = status.into();
        let phase = match (self, status.kind) {
            (Self::EnUs, CloseStatusKind::AwaitingStatus) => "Awaiting Close status",
            (Self::ZhCn, CloseStatusKind::AwaitingStatus) => "等待关闭状态",
            (Self::EnUs, CloseStatusKind::Closing) => "Closing",
            (Self::ZhCn, CloseStatusKind::Closing) => "正在关闭",
            (Self::EnUs, CloseStatusKind::CleanupBlocked) => "Close cleanup blocked",
            (Self::ZhCn, CloseStatusKind::CleanupBlocked) => "关闭清理受阻",
            (Self::EnUs, CloseStatusKind::OutcomeUnknown) => "Close outcome unknown",
            (Self::ZhCn, CloseStatusKind::OutcomeUnknown) => "关闭结果未知",
            (Self::EnUs, CloseStatusKind::DisposedAwaitingPublication) => {
                "Execution ended; awaiting Tab removal"
            }
            (Self::ZhCn, CloseStatusKind::DisposedAwaitingPublication) => {
                "执行已终止；等待移除标签页"
            }
            (Self::EnUs, CloseStatusKind::Closed) => "Closed",
            (Self::ZhCn, CloseStatusKind::Closed) => "已关闭",
        };
        let mut text = format!("{} · {phase}", sanitize_terminal_text(status.label));
        if status.unavailable {
            text.push_str(match self {
                Self::EnUs => " · Status check unavailable",
                Self::ZhCn => " · 状态查询不可用",
            });
        }
        if status.observation_paused {
            text.push_str(match self {
                Self::EnUs => " · Automatic status checks paused",
                Self::ZhCn => " · 自动状态查询已暂停",
            });
        }
        if status.resumable {
            text.push_str(match self {
                Self::EnUs => " · Close can be retried",
                Self::ZhCn => " · 可以重试关闭",
            });
        }
        text
    }
}
