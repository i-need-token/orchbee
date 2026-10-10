use super::Locale;
use crate::{
    terminal_session::{NativeCloseConfirmationKind, NativeCloseStatusKind, NativeCloseStatusView},
    terminal_text::sanitize_terminal_text,
};

impl Locale {
    pub(crate) fn close_set_title(self, count: usize) -> String {
        match self {
            Self::EnUs => format!("Close {count} terminal Tabs?"),
            Self::ZhCn => format!("关闭 {count} 个终端 Tab？"),
        }
    }
    pub(crate) fn close_set_context(self, label: &str) -> String {
        let label = sanitize_terminal_text(label);
        match self {
            Self::EnUs if label.is_empty() => "Location: Standalone terminals".into(),
            Self::ZhCn if label.is_empty() => "位置：独立终端".into(),
            Self::EnUs => format!("Location: {label}"),
            Self::ZhCn => format!("位置：{label}"),
        }
    }
    pub(crate) fn close_set_more(self, count: usize) -> String {
        match self {
            Self::EnUs if count == 1 => "… and 1 more Tab".to_owned(),
            Self::EnUs => format!("… and {count} more Tabs"),
            Self::ZhCn => format!("……以及另外 {count} 个 Tab"),
        }
    }
    pub(crate) const fn close_set_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "This ends execution in these Tabs for all attached Clients. Tabs close one at a time; a failure stops further closes."
            }
            Self::ZhCn => {
                "这将终止这些 Tab 中的执行，影响所有已连接的 Client。Tab 将逐个关闭；失败时停止后续关闭。"
            }
        }
    }
    pub(crate) fn close_set_status(
        self,
        status: crate::terminal_session::state::CloseSetStatus,
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
                format!("正在关闭 Tab：{completed} 个已关闭，剩余 {remaining} 个")
            }
        }
    }
    pub(crate) const fn close_title(self, kind: NativeCloseConfirmationKind) -> &'static str {
        match (self, kind) {
            (Self::EnUs, NativeCloseConfirmationKind::Close) => "Close terminal Tab?",
            (Self::ZhCn, NativeCloseConfirmationKind::Close) => "关闭终端 Tab？",
            (Self::EnUs, NativeCloseConfirmationKind::Resume) => "Retry terminal Close?",
            (Self::ZhCn, NativeCloseConfirmationKind::Resume) => "重试关闭终端？",
        }
    }

    pub(crate) const fn close_warning(self) -> &'static str {
        match self {
            Self::EnUs => "Closing this Tab ends its execution for all attached Clients.",
            Self::ZhCn => "关闭此 Tab 将终止其执行，影响所有已连接的 Client。",
        }
    }

    pub(crate) const fn close_actions(self) -> &'static str {
        match self {
            Self::EnUs => "y · confirm    Esc · cancel",
            Self::ZhCn => "y · 确认    Esc · 取消",
        }
    }

    pub(crate) const fn close_resize_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Enlarge the terminal to read the confirmation. Esc · cancel",
            Self::ZhCn => "请扩大终端以阅读完整确认内容。Esc · 取消",
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
                "执行已终止；等待移除 Tab"
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
                Self::EnUs => " · Status check unavailable; Ctrl+B w to check again",
                Self::ZhCn => " · 状态查询不可用；按 Ctrl+B 后按 w 再次查询",
            });
        }
        if status.observation_paused() {
            text.push_str(match self {
                Self::EnUs => " · Automatic status checks paused; Ctrl+B w to check again",
                Self::ZhCn => " · 自动状态查询已暂停；按 Ctrl+B 后按 w 再次查询",
            });
        }
        if status.resumable() {
            text.push_str(match self {
                Self::EnUs => " · Ctrl+B x to retry Close",
                Self::ZhCn => " · 按 Ctrl+B 后按 x 重试关闭",
            });
        }
        text
    }
}
