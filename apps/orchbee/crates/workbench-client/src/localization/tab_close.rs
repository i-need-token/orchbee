use super::Locale;
use crate::terminal_text::sanitize_terminal_text;

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
    /// The close ended without removing its Tab or Panel, which shows again.
    pub(crate) stopped: bool,
    /// The Tab list is unavailable, so the finished close is the one fact on screen and the
    /// navigator's refresh restores the list from it.
    pub(crate) list_unavailable: bool,
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
    /// A set of several Tabs being closed: on its way, or stopped short. A finished close shows
    /// itself and says nothing; counts stay out of the copy.
    pub(crate) fn close_set_status(
        self,
        status: crate::localization::CloseSetStatus,
    ) -> Option<String> {
        let crate::localization::CloseSetStatus {
            completed,
            total,
            halted,
            unconfirmed,
        } = status;
        Some(match (halted, unconfirmed) {
            (true, true) => self.outcome_unknown().to_owned(),
            (true, false) => self.say("Couldn't close all the Tabs.", "没能关闭所有标签页。"),
            (false, _) if completed >= total => return None,
            (false, _) => self.say("Closing Tabs…", "正在关闭标签页…"),
        })
    }
    pub(crate) const fn close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close Tab?",
            Self::ZhCn => "关闭标签页？",
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

    /// One close of one Tab or one Panel: on its way, or, once it ended without removing what it
    /// closes, one sentence with no reason. A finished close shows itself and says nothing, unless
    /// the Tab list is unavailable and the close is the fact the refresh starts from. The
    /// Server retries and renews a close by itself, so an ended close needs nothing more from the
    /// person than closing it again.
    pub(crate) fn native_close_status<'a>(
        self,
        status: impl Into<CloseStatusText<'a>>,
    ) -> Option<String> {
        let status = status.into();
        let label = sanitize_terminal_text(status.label);
        Some(match (self, status.stopped, status.kind) {
            (Self::EnUs, true, _) => format!("Couldn't close {label}."),
            (Self::ZhCn, true, _) => format!("没能关闭 {label}。"),
            (Self::EnUs, false, CloseStatusKind::Closed) if status.list_unavailable => {
                format!("{label} · Closed")
            }
            (Self::ZhCn, false, CloseStatusKind::Closed) if status.list_unavailable => {
                format!("{label} · 已关闭")
            }
            (_, false, CloseStatusKind::Closed) => return None,
            (Self::EnUs, false, _) => format!("{label} · Closing"),
            (Self::ZhCn, false, _) => format!("{label} · 正在关闭"),
        })
    }
}
