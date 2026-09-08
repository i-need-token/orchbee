use super::Locale;
use crate::{
    terminal_session::{NativeCloseConfirmationKind, NativeCloseStatusKind, NativeCloseStatusView},
    terminal_text::sanitize_terminal_text,
};

impl Locale {
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
            Self::EnUs => "y · confirm    n / Esc · cancel",
            Self::ZhCn => "y · 确认    n / Esc · 取消",
        }
    }

    pub(crate) const fn close_resize_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Enlarge the terminal to read the confirmation. n / Esc · cancel",
            Self::ZhCn => "请扩大终端以阅读完整确认内容。n / Esc · 取消",
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
