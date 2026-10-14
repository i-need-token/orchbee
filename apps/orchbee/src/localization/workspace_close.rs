use super::Locale;
use crate::{
    terminal_session::{WorkspaceCloseStatus, WorkspaceCloseStatusView},
    terminal_text::sanitize_terminal_text,
};
impl Locale {
    pub(crate) const fn workspace_close_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn workspace_close_cancel_hint(self) -> &'static str {
        match self {
            Self::EnUs => "cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn workspace_close_confirm_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }
    pub(crate) const fn workspace_close_confirm_hint(self) -> &'static str {
        match self {
            Self::EnUs => "close",
            Self::ZhCn => "关闭",
        }
    }
    pub(crate) const fn workspace_close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close Workspace?",
            Self::ZhCn => "关闭工作区？",
        }
    }
    pub(crate) const fn workspace_close_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Ends this Workspace's terminals for every Client; keeps Git branches and checkouts."
            }
            Self::ZhCn => "结束此工作区的全部终端，影响所有 Client；保留 Git 分支和检出目录。",
        }
    }
    pub(crate) const fn project_close_all_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close all Workspaces in Project?",
            Self::ZhCn => "关闭项目中的全部工作区？",
        }
    }
    pub(crate) fn project_close_all_warning(self, count: usize) -> String {
        match self {
            Self::EnUs => format!(
                "Closes all Workspaces (currently {count}) and ends their terminals for every Client; keeps the Project, Git branches and checkouts."
            ),
            Self::ZhCn => format!(
                "关闭全部工作区（当前 {count} 个）并结束其全部终端，影响所有 Client；保留项目、Git 分支和检出目录。"
            ),
        }
    }
    pub(crate) fn workspace_close_status(self, view: WorkspaceCloseStatusView<'_>) -> String {
        let project = view.workspace_id().is_none();
        let phase = match (self, view.status(), project) {
            (Self::EnUs, WorkspaceCloseStatus::Closing, true) => "Closing Project Workspaces",
            (Self::ZhCn, WorkspaceCloseStatus::Closing, true) => "正在关闭项目工作区",
            (Self::EnUs, WorkspaceCloseStatus::Closed, true) => "Project Workspaces closed",
            (Self::ZhCn, WorkspaceCloseStatus::Closed, true) => "项目工作区已关闭",
            (Self::EnUs, WorkspaceCloseStatus::Failed, true) => {
                "Project close-all stopped; known results preserved"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Failed, true) => "项目全部关闭已停止；已知结果保留",
            (Self::EnUs, WorkspaceCloseStatus::Unknown, true) => {
                "Project close-all outcome unknown; no replay"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Unknown, true) => "项目全部关闭结果未知；不会重放",
            (Self::EnUs, WorkspaceCloseStatus::Refused, true) => {
                "Project close-all was not submitted or was refused; refresh and try again"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Refused, true) => {
                "项目全部关闭未提交或被拒绝；刷新后重试"
            }
            (Self::EnUs, WorkspaceCloseStatus::Closing, false) => "Closing Workspace",
            (Self::ZhCn, WorkspaceCloseStatus::Closing, false) => "正在关闭工作区",
            (Self::EnUs, WorkspaceCloseStatus::Closed, false) => "Workspace closed",
            (Self::ZhCn, WorkspaceCloseStatus::Closed, false) => "工作区已关闭",
            (Self::EnUs, WorkspaceCloseStatus::Failed, false) => {
                "Workspace Close stopped; known results preserved"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Failed, false) => "工作区关闭已停止；已知结果保留",
            (Self::EnUs, WorkspaceCloseStatus::Unknown, false) => {
                "Workspace Close outcome unknown; no replay"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Unknown, false) => "工作区关闭结果未知；不会重放",
            (Self::EnUs, WorkspaceCloseStatus::Refused, false) => {
                "Workspace Close was not submitted or was refused; refresh and try again"
            }
            (Self::ZhCn, WorkspaceCloseStatus::Refused, false) => {
                "工作区关闭未提交或被拒绝；刷新后重试"
            }
        };
        let mut text = format!("{phase} · {}", sanitize_terminal_text(view.label()));
        if view.status() != WorkspaceCloseStatus::Closed
            && let Some(id) = view.operation_id()
        {
            if let Some(workspace) = view.workspace_id() {
                text.push_str(&format!(
                    " · orchbee workspace close-status --workspace {workspace}"
                ));
            } else {
                text.push_str(&format!(" · orchbee project close-status --request {id}"));
            }
        }
        text
    }
}
