use super::Locale;
use crate::terminal_text::sanitize_terminal_text;

/// Lifecycle truth; an absent observation never implies successful removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCloseStatus {
    Closing,
    Closed,
    Failed,
    Unknown,
    Refused,
    ConfirmationRequired,
}

/// Frozen close-result display facts; the original operation and recovery authority stay owned.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseText<'a> {
    pub(crate) label: &'a str,
    pub(crate) status: WorkspaceCloseStatus,
    pub(crate) operation_id: Option<&'a orchbee_workbench_protocol::WorkbenchRequestId>,
    pub(crate) workspace_id: Option<&'a orchbee_workbench_protocol::WorkbenchWorkspaceId>,
}

impl Locale {
    pub(crate) const fn workspace_close_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn workspace_close_confirm_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
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
            Self::ZhCn => "结束此工作区的全部终端，影响所有客户端；保留 Git 分支和检出目录。",
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
                "关闭全部工作区（当前 {count} 个）并结束其全部终端，影响所有客户端；保留项目、Git 分支和检出目录。"
            ),
        }
    }
    pub(crate) fn workspace_close_status<'a>(
        self,
        view: impl Into<WorkspaceCloseText<'a>>,
    ) -> String {
        let view = view.into();
        let mut text = self.workspace_close_scope(view);
        if view.status != WorkspaceCloseStatus::Closed
            && let Some(id) = view.operation_id
        {
            if let Some(workspace) = view.workspace_id {
                text.push_str(&format!(
                    " · orchbee workspace close-status --workspace {workspace}"
                ));
            } else {
                text.push_str(&format!(" · orchbee project close-status --request {id}"));
            }
        }
        text
    }
    /// The documented shorter forms, longest first, for a status line too narrow for the complete
    /// status: the phase and scope label without the status command, then the phase alone.
    pub(crate) fn workspace_close_status_shorter<'a>(
        self,
        view: impl Into<WorkspaceCloseText<'a>>,
    ) -> [String; 2] {
        let view = view.into();
        [
            self.workspace_close_scope(view),
            self.workspace_close_phase(view).to_owned(),
        ]
    }
    fn workspace_close_scope(self, view: WorkspaceCloseText<'_>) -> String {
        format!(
            "{} · {}",
            self.workspace_close_phase(view),
            sanitize_terminal_text(view.label)
        )
    }
    fn workspace_close_phase(self, view: WorkspaceCloseText<'_>) -> &'static str {
        let project = view.workspace_id.is_none();
        match (self, view.status, project) {
            (Self::EnUs, WorkspaceCloseStatus::ConfirmationRequired, _) => {
                "Confirmation required before closing"
            }
            (Self::ZhCn, WorkspaceCloseStatus::ConfirmationRequired, _) => "关闭前需要确认",
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
        }
    }
}
