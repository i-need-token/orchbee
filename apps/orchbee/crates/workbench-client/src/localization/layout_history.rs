//! Copy for earlier layouts, shared by the TUI picker and notices and the CLI text output: when an
//! entry was saved (relative time and local clock), what it holds that is not open now, and what a
//! reopen did.
use super::Locale;
use orchbee_workbench_protocol::{
    LayoutHistoryEntry, LayoutHistoryOutcome, LayoutHistoryRejection, LayoutHistoryUnavailable,
};
use std::time::{SystemTime, UNIX_EPOCH};

impl Locale {
    pub const fn earlier_layouts_title(self) -> &'static str {
        match self {
            Self::EnUs => "Earlier layouts",
            Self::ZhCn => "早先的布局",
        }
    }

    pub const fn earlier_layouts_empty(self) -> &'static str {
        match self {
            Self::EnUs => {
                "No earlier layouts yet. One is kept every 15 minutes while the layout changes."
            }
            Self::ZhCn => "还没有早先的布局。布局有变化时，每 15 分钟保留一份。",
        }
    }

    /// When an entry was saved: relative time, then the local clock where the host provides one.
    #[must_use]
    pub fn earlier_layout_when(self, saved_at: u64, now: SystemTime) -> String {
        let now_ms = now
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|since| u64::try_from(since.as_millis()).ok())
            .unwrap_or(saved_at);
        let minutes = now_ms.saturating_sub(saved_at) / 60_000;
        let relative = match (self, minutes) {
            (Self::EnUs, 0) => "just now".to_owned(),
            (Self::ZhCn, 0) => "刚刚".to_owned(),
            (Self::EnUs, 1..60) => format!("{minutes} min ago"),
            (Self::ZhCn, 1..60) => format!("{minutes} 分钟前"),
            (Self::EnUs, 60..2880) => format!("{} h ago", minutes / 60),
            (Self::ZhCn, 60..2880) => format!("{} 小时前", minutes / 60),
            (Self::EnUs, _) => format!("{} days ago", minutes / 1440),
            (Self::ZhCn, _) => format!("{} 天前", minutes / 1440),
        };
        let Some(clock) = orchbee_local_runtime::local_clock(saved_at) else {
            return relative;
        };
        let today = orchbee_local_runtime::local_clock(now_ms).is_some_and(|now| {
            (now.year, now.month, now.day) == (clock.year, clock.month, clock.day)
        });
        if today {
            format!("{relative} · {:02}:{:02}", clock.hour, clock.minute)
        } else {
            format!(
                "{relative} · {}-{:02}-{:02} {:02}:{:02}",
                clock.year, clock.month, clock.day, clock.hour, clock.minute
            )
        }
    }

    /// What an entry holds that is not open now, or that everything in it is open.
    #[must_use]
    pub fn earlier_layout_summary(self, entry: &LayoutHistoryEntry) -> String {
        let (workspaces, tabs) = (entry.workspaces_not_open, entry.tabs_not_open);
        match (self, workspaces, tabs) {
            (Self::EnUs, 0, 0) => "Everything here is open now".to_owned(),
            (Self::ZhCn, 0, 0) => "其中内容都已打开".to_owned(),
            (Self::EnUs, 0, _) => format!("{} not open now", tab_count(self, tabs)),
            (Self::ZhCn, 0, _) => format!("{} 当前未打开", tab_count(self, tabs)),
            (Self::EnUs, _, _) => format!(
                "{}, {} not open now",
                workspace_count(self, workspaces),
                tab_count(self, tabs)
            ),
            (Self::ZhCn, _, _) => format!(
                "{}、{} 当前未打开",
                workspace_count(self, workspaces),
                tab_count(self, tabs)
            ),
        }
    }

    /// The notice for a reopen outcome. A listing has no notice and returns `None`.
    #[must_use]
    pub fn earlier_layout_outcome(self, outcome: &LayoutHistoryOutcome) -> Option<String> {
        EarlierLayoutNotice::from_outcome(outcome).map(|notice| self.earlier_layout_notice(notice))
    }

    /// The short notice an interactive Client shows for an earlier-layout request.
    #[must_use]
    pub fn earlier_layout_notice(self, notice: EarlierLayoutNotice) -> String {
        use EarlierLayoutNotice as N;
        match (self, notice) {
            (
                Self::EnUs,
                N::Reopened {
                    workspaces: 0,
                    tabs,
                },
            ) => {
                format!("Reopened {}", tab_count(self, tabs))
            }
            (
                Self::ZhCn,
                N::Reopened {
                    workspaces: 0,
                    tabs,
                },
            ) => {
                format!("已重新打开 {}", tab_count(self, tabs))
            }
            (
                Self::EnUs,
                N::Reopened {
                    workspaces,
                    tabs: 0,
                },
            ) => {
                format!("Reopened {}", workspace_count(self, workspaces))
            }
            (
                Self::ZhCn,
                N::Reopened {
                    workspaces,
                    tabs: 0,
                },
            ) => {
                format!("已重新打开 {}", workspace_count(self, workspaces))
            }
            (Self::EnUs, N::Reopened { workspaces, tabs }) => format!(
                "Reopened {}, {}",
                workspace_count(self, workspaces),
                tab_count(self, tabs)
            ),
            (Self::ZhCn, N::Reopened { workspaces, tabs }) => format!(
                "已重新打开 {}、{}",
                workspace_count(self, workspaces),
                tab_count(self, tabs)
            ),
            (Self::EnUs, N::NothingToReopen) => {
                "Nothing to reopen: everything in that layout is open now".to_owned()
            }
            (Self::ZhCn, N::NothingToReopen) => "无需重新打开：该布局中的内容都已打开".to_owned(),
            (Self::EnUs, N::NotFound) => "That earlier layout is no longer kept".to_owned(),
            (Self::ZhCn, N::NotFound) => "该早先布局已不再保留".to_owned(),
            (Self::EnUs, N::ResourceLimit) => {
                "Not reopened: it would exceed the Workspace or Tab limit".to_owned()
            }
            (Self::ZhCn, N::ResourceLimit) => "未重新打开：会超出工作区或标签页上限".to_owned(),
            (Self::EnUs, N::Busy) => "Not reopened: a Workspace is closing; try again".to_owned(),
            (Self::ZhCn, N::Busy) => "未重新打开：有工作区正在关闭，请重试".to_owned(),
            (Self::EnUs, N::Denied) => "Not reopened: this Client may not open Tabs".to_owned(),
            (Self::ZhCn, N::Denied) => "未重新打开：此客户端无权打开标签页".to_owned(),
            (Self::EnUs, N::Empty) => "No earlier layouts yet".to_owned(),
            (Self::ZhCn, N::Empty) => "还没有早先的布局".to_owned(),
            (Self::EnUs, N::Unknown) => "Reopen outcome unknown · not retried".to_owned(),
            (Self::ZhCn, N::Unknown) => "重新打开的结果未知 · 未重试".to_owned(),
            (Self::EnUs, N::Unavailable) => "Earlier layouts are unavailable right now".to_owned(),
            (Self::ZhCn, N::Unavailable) => "早先的布局暂不可用".to_owned(),
        }
    }

    /// A reopen whose outcome could not be confirmed. It may have opened items; do not repeat it.
    pub const fn earlier_layout_unknown(self) -> &'static str {
        match self {
            Self::EnUs => "Reopen outcome unknown. Check the open Tabs before trying again.",
            Self::ZhCn => "重新打开的结果未知。请先查看已打开的标签页再重试。",
        }
    }
}

/// What an interactive Client reports after an earlier-layout request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarlierLayoutNotice {
    Reopened {
        workspaces: u32,
        tabs: u32,
    },
    NothingToReopen,
    NotFound,
    ResourceLimit,
    Busy,
    Denied,
    /// The listing is empty, so no picker opens.
    Empty,
    /// The request was handed off but its outcome never arrived; it is not repeated.
    Unknown,
    Unavailable,
}

impl EarlierLayoutNotice {
    /// The notice for a Server outcome; a listing has none.
    #[must_use]
    pub const fn from_outcome(outcome: &LayoutHistoryOutcome) -> Option<Self> {
        Some(match outcome {
            LayoutHistoryOutcome::Listed { .. } => return None,
            LayoutHistoryOutcome::Reopened {
                workspaces, tabs, ..
            } => Self::Reopened {
                workspaces: *workspaces,
                tabs: *tabs,
            },
            LayoutHistoryOutcome::NothingToReopen {} => Self::NothingToReopen,
            LayoutHistoryOutcome::NotFound {} => Self::NotFound,
            LayoutHistoryOutcome::Rejected {
                reason: LayoutHistoryRejection::ResourceLimit,
            } => Self::ResourceLimit,
            LayoutHistoryOutcome::Unavailable {
                reason: LayoutHistoryUnavailable::Busy,
            } => Self::Busy,
            LayoutHistoryOutcome::Denied {} => Self::Denied,
            LayoutHistoryOutcome::Rejected {
                reason: LayoutHistoryRejection::SessionMismatch,
            }
            | LayoutHistoryOutcome::Unavailable {
                reason: LayoutHistoryUnavailable::ServiceUnavailable,
            } => Self::Unavailable,
        })
    }
}

fn tab_count(locale: Locale, tabs: u32) -> String {
    match (locale, tabs) {
        (Locale::EnUs, 1) => "1 Tab".to_owned(),
        (Locale::EnUs, _) => format!("{tabs} Tabs"),
        (Locale::ZhCn, _) => format!("{tabs} 个标签页"),
    }
}

fn workspace_count(locale: Locale, workspaces: u32) -> String {
    match (locale, workspaces) {
        (Locale::EnUs, 1) => "1 Workspace".to_owned(),
        (Locale::EnUs, _) => format!("{workspaces} Workspaces"),
        (Locale::ZhCn, _) => format!("{workspaces} 个工作区"),
    }
}
