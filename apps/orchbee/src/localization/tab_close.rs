use super::Locale;
use crate::{
    terminal_text::sanitize_terminal_text,
    workbench_client::{NativeCloseConfirmationKind, NativeCloseStatusKind, NativeCloseStatusView},
};

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

    pub(crate) fn close_set_title(self, count: usize) -> String {
        match self {
            Self::EnUs if count == 1 => "Close 1 Tab?".into(),
            Self::EnUs => format!("Close {count} Tabs?"),
            Self::ZhCn => format!("关闭 {count} 个标签页？"),
        }
    }
    pub(crate) fn close_set_context(self, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match self {
            Self::EnUs => format!("Location: {label}"),
            Self::ZhCn => format!("位置：{label}"),
        }
    }
    pub(crate) fn close_set_more(self, count: usize) -> String {
        match self {
            Self::EnUs if count == 1 => "… and 1 more Tab".to_owned(),
            Self::EnUs => format!("… and {count} more Tabs"),
            Self::ZhCn => format!("……以及另外 {count} 个标签页"),
        }
    }
    pub(crate) const fn close_set_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Closing ends the running work in these Tabs for everyone connected to this workspace. Tabs close one at a time; a failure stops further closes."
            }
            Self::ZhCn => {
                "关闭将终止这些标签页中运行的任务，影响所有连接到此工作区的人。标签页将逐个关闭；失败时停止后续关闭。"
            }
        }
    }
    pub(crate) fn close_set_status(
        self,
        status: crate::workbench_client::state::CloseSetStatus,
    ) -> String {
        let completed = status.completed;
        let remaining = status.total.saturating_sub(completed);
        match (self, status.halted) {
            (Self::EnUs, true) => format!(
                "Closing stopped: {completed} closed, {remaining} not confirmed closed. Submitted closes may continue."
            ),
            (Self::ZhCn, true) => format!(
                "关闭已停止：{completed} 个已关闭，{remaining} 个未确认关闭。已提交的关闭操作可能继续。"
            ),
            (Self::EnUs, false) => {
                format!("Closing Tabs: {completed} closed, {remaining} remaining")
            }
            (Self::ZhCn, false) => {
                format!("正在关闭标签页：{completed} 个已关闭，剩余 {remaining} 个")
            }
        }
    }
    pub(crate) const fn close_title(self, kind: NativeCloseConfirmationKind) -> &'static str {
        match (self, kind) {
            (Self::EnUs, NativeCloseConfirmationKind::Close) => "Close Tab?",
            (Self::ZhCn, NativeCloseConfirmationKind::Close) => "关闭标签页？",
            (Self::EnUs, NativeCloseConfirmationKind::Resume) => "Resume closing this Tab?",
            (Self::ZhCn, NativeCloseConfirmationKind::Resume) => "继续关闭此标签页？",
        }
    }

    pub(crate) const fn close_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Closing this Tab ends its running work for everyone connected to this workspace."
            }
            Self::ZhCn => "关闭此标签页将终止其中运行的任务，影响所有连接到此工作区的人。",
        }
    }

    pub(crate) fn native_close_status(self, status: &NativeCloseStatusView<'_>) -> String {
        let phase = match (self, status.status()) {
            (Self::EnUs, NativeCloseStatusKind::AwaitingStatus) => "Awaiting Close status",
            (Self::ZhCn, NativeCloseStatusKind::AwaitingStatus) => "等待关闭状态",
            (Self::EnUs, NativeCloseStatusKind::Closing) => "Closing",
            (Self::ZhCn, NativeCloseStatusKind::Closing) => "正在关闭",
            (Self::EnUs, NativeCloseStatusKind::CleanupBlocked) => "Close cleanup blocked",
            (Self::ZhCn, NativeCloseStatusKind::CleanupBlocked) => "关闭清理受阻",
            (Self::EnUs, NativeCloseStatusKind::OutcomeUnknown) => "Close outcome unknown",
            (Self::ZhCn, NativeCloseStatusKind::OutcomeUnknown) => "关闭结果未知",
            (Self::EnUs, NativeCloseStatusKind::DisposedAwaitingPublication) => {
                "Execution ended; awaiting Tab removal"
            }
            (Self::ZhCn, NativeCloseStatusKind::DisposedAwaitingPublication) => {
                "执行已终止；等待移除标签页"
            }
            (Self::EnUs, NativeCloseStatusKind::Closed) => "Closed",
            (Self::ZhCn, NativeCloseStatusKind::Closed) => "已关闭",
        };
        let mut text = format!(
            "{} · {phase}",
            sanitize_terminal_text(status.target_label())
        );
        if status.unavailable() {
            text.push_str(match self {
                Self::EnUs => " · Status check unavailable",
                Self::ZhCn => " · 状态查询不可用",
            });
        }
        if status.observation_paused() {
            text.push_str(match self {
                Self::EnUs => " · Automatic status checks paused",
                Self::ZhCn => " · 自动状态查询已暂停",
            });
        }
        if status.resumable() {
            text.push_str(match self {
                Self::EnUs => " · Close can be retried",
                Self::ZhCn => " · 可以重试关闭",
            });
        }
        text
    }
}
