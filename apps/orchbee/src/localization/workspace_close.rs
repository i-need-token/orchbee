use super::Locale;
use crate::{
    terminal_session::{WorkspaceCloseStatus, WorkspaceCloseStatusView},
    terminal_text::sanitize_terminal_text,
};
impl Locale {
    pub(crate) const fn workspace_close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close Workspace?",
            Self::ZhCn => "关闭工作区？",
        }
    }
    pub(crate) const fn workspace_close_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Ends all terminals in this Workspace and its associated worktree Workspaces for every Client; keeps Git directories."
            }
            Self::ZhCn => {
                "结束此工作区及其关联 worktree 工作区的全部终端，影响所有 Client；保留 Git 目录。"
            }
        }
    }
    pub(crate) const fn workspace_close_actions(self) -> &'static str {
        match self {
            Self::EnUs => "Esc · cancel    y · close",
            Self::ZhCn => "Esc · 取消    y · 关闭",
        }
    }
    pub(crate) fn workspace_close_status(self, view: WorkspaceCloseStatusView<'_>) -> String {
        let phase = match (self, view.status()) {
            (Self::EnUs, WorkspaceCloseStatus::Closing) => "Closing Workspace",
            (Self::ZhCn, WorkspaceCloseStatus::Closing) => "正在关闭工作区",
            (Self::EnUs, WorkspaceCloseStatus::Closed) => "Workspace closed",
            (Self::ZhCn, WorkspaceCloseStatus::Closed) => "工作区已关闭",
            (Self::EnUs, WorkspaceCloseStatus::Failed) => {
                "Workspace Close stopped; known results preserved"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Failed) => "工作区关闭已停止；已知结果保留",
            (Self::EnUs, WorkspaceCloseStatus::Unknown) => {
                "Workspace Close outcome unknown; no replay"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Unknown) => "工作区关闭结果未知；不会重放",
            (Self::EnUs, WorkspaceCloseStatus::Refused) => {
                "Workspace Close was not submitted or was refused; refresh and try again"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Refused) => "工作区关闭未提交或被拒绝；刷新后重试",
        };
        let mut text = format!("{phase} · {}", sanitize_terminal_text(view.label()));
        if view.status() != WorkspaceCloseStatus::Closed
            && let Some(id) = view.operation_id()
        {
            text.push_str(&format!(
                " · orchbee workspace close-status --group --request {id}"
            ));
        }
        text
    }
}
