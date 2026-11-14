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
    /// `count` is the Project's Workspace count when closing every Workspace of a Project; a
    /// count appears only when there are several.
    pub(crate) fn workspace_close_title(self, count: Option<usize>) -> String {
        match (self, count) {
            (Self::EnUs, None | Some(1)) => "Close Workspace?".into(),
            (Self::ZhCn, None | Some(1)) => "关闭工作区？".into(),
            (Self::EnUs, Some(0)) => "Close Workspaces?".into(),
            (Self::ZhCn, Some(0)) => "关闭工作区？".into(),
            (Self::EnUs, Some(count)) => format!("Close {count} Workspaces?"),
            (Self::ZhCn, Some(count)) => format!("关闭 {count} 个工作区？"),
        }
    }
    /// What ends, then what is kept, one sentence per line.
    pub(crate) const fn workspace_close_consequence(self, project: bool) -> [&'static str; 2] {
        match (self, project) {
            (Self::EnUs, false) => [
                "Ends every terminal in this Workspace.",
                "Branches and checkouts are kept.",
            ],
            (Self::ZhCn, false) => ["将结束此工作区中的全部终端。", "分支和检出目录会保留。"],
            (Self::EnUs, true) => [
                "Ends every terminal in these Workspaces.",
                "The Project, branches and checkouts are kept.",
            ],
            (Self::ZhCn, true) => [
                "将结束这些工作区中的全部终端。",
                "项目、分支和检出目录会保留。",
            ],
        }
    }
    /// One Workspace Close, or a Project's close-all: on its way, waiting for confirmation, or
    /// stopped short. A finished close shows itself and says nothing.
    pub(crate) fn workspace_close_status<'a>(
        self,
        view: impl Into<WorkspaceCloseText<'a>>,
    ) -> Option<String> {
        let view = view.into();
        let label = sanitize_terminal_text(view.label);
        let project = view.workspace_id.is_none();
        Some(match (self, view.status, project) {
            (_, WorkspaceCloseStatus::Closed, _) => return None,
            (_, WorkspaceCloseStatus::Unknown, _) => self.outcome_unknown().to_owned(),
            (Self::EnUs, WorkspaceCloseStatus::ConfirmationRequired, _) => {
                format!("Confirm to close {label}.")
            }
            (Self::ZhCn, WorkspaceCloseStatus::ConfirmationRequired, _) => {
                format!("请确认关闭 {label}。")
            }
            (Self::EnUs, WorkspaceCloseStatus::Closing, true) => {
                format!("Closing Project Workspaces · {label}")
            }
            (Self::ZhCn, WorkspaceCloseStatus::Closing, true) => {
                format!("正在关闭项目工作区 · {label}")
            }
            (Self::EnUs, WorkspaceCloseStatus::Closing, false) => {
                format!("Closing Workspace · {label}")
            }
            (Self::ZhCn, WorkspaceCloseStatus::Closing, false) => {
                format!("正在关闭工作区 · {label}")
            }
            (Self::EnUs, WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, true) => {
                format!("Couldn't close every Workspace in {label}.")
            }
            (Self::ZhCn, WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, true) => {
                format!("没能关闭 {label} 中的所有工作区。")
            }
            (Self::EnUs, WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, false) => {
                format!("Couldn't close {label}.")
            }
            (Self::ZhCn, WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, false) => {
                format!("没能关闭 {label}。")
            }
        })
    }
    /// The documented shorter form, for a status line too narrow for the complete status: the
    /// outcome without the label.
    pub(crate) fn workspace_close_status_shorter<'a>(
        self,
        view: impl Into<WorkspaceCloseText<'a>>,
    ) -> Vec<String> {
        let view = view.into();
        let project = view.workspace_id.is_none();
        let (english, chinese) = match (view.status, project) {
            (WorkspaceCloseStatus::Closed | WorkspaceCloseStatus::Unknown, _) => return Vec::new(),
            (WorkspaceCloseStatus::ConfirmationRequired, _) => {
                ("Confirm to close.", "请确认关闭。")
            }
            (WorkspaceCloseStatus::Closing, true) => {
                ("Closing Project Workspaces", "正在关闭项目工作区")
            }
            (WorkspaceCloseStatus::Closing, false) => ("Closing Workspace", "正在关闭工作区"),
            (WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, true) => {
                ("Couldn't close the Workspaces.", "没能关闭这些工作区。")
            }
            (WorkspaceCloseStatus::Failed | WorkspaceCloseStatus::Refused, false) => {
                ("Couldn't close the Workspace.", "没能关闭工作区。")
            }
        };
        vec![self.say(english, chinese)]
    }
}
