use super::Locale;
use crate::terminal_session::{
    NativeLocationLabelStatus, NativeSessionNotice, NativeSetupStage, NativeSetupStatus,
};

impl Locale {
    pub(super) fn native_status_with_setup(
        self,
        phase: String,
        retired: bool,
        notice: Option<NativeSessionNotice>,
        setup: Option<NativeSetupStatus>,
        can_configure_directory: bool,
    ) -> String {
        let primary = if retired {
            Some(phase.clone())
        } else {
            notice.map(|notice| self.native_notice(notice))
        };
        match (primary, setup) {
            (
                Some(primary),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status: NativeLocationLabelStatus::CommittedNeedRefresh,
                }),
            ) if retired => {
                format!(
                    "{primary} · {}",
                    self.location_rename_committed_fact(target)
                )
            }
            (
                Some(primary),
                Some(NativeSetupStatus::LocationLabel {
                    target,
                    status:
                        status @ (NativeLocationLabelStatus::CommittedNeedRefresh
                        | NativeLocationLabelStatus::OutcomeUnknown),
                }),
            ) => {
                format!("{primary} · {}", self.location_label_status(target, status))
            }
            (Some(primary), Some(NativeSetupStatus::CommittedNeedRefresh)) if retired => {
                format!("{primary} · {}", self.setup_committed_fact())
            }
            (
                Some(primary),
                Some(
                    status @ (NativeSetupStatus::CommittedNeedRefresh
                    | NativeSetupStatus::OutcomeUnknown),
                ),
            ) => {
                format!("{primary} · {}", self.setup_status(status))
            }
            (Some(primary), _) => primary,
            (None, Some(NativeSetupStatus::WorkspaceCreated)) if can_configure_directory => {
                self.setup_workspace_directory_action().to_owned()
            }
            (None, Some(status)) => self.setup_status(status).to_owned(),
            (None, None) => phase,
        }
    }

    pub(super) const fn setup_committed_fact(self) -> &'static str {
        match self {
            Self::EnUs => "Setup committed · do not resubmit",
            Self::ZhCn => "设置已提交 · 请勿重复提交",
        }
    }

    const fn setup_workspace_directory_action(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace created · Ctrl+B e to configure its first directory",
            Self::ZhCn => "Workspace 已创建 · 按 Ctrl+B e 配置首个目录",
        }
    }

    pub(crate) const fn setup_title(self, stage: NativeSetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, NativeSetupStage::PreparingWorkspace) => "Preparing Workspace directory…",
            (Self::ZhCn, NativeSetupStage::PreparingWorkspace) => "正在准备工作区目录…",
            (Self::EnUs, NativeSetupStage::QuickProject) => "New Workspace · choose Project",
            (Self::ZhCn, NativeSetupStage::QuickProject) => "新建工作区 · 选择 Project",
            (Self::EnUs, NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory) => {
                "New Workspace"
            }
            (Self::ZhCn, NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory) => {
                "新建工作区"
            }
            (Self::EnUs, NativeSetupStage::ChooseProject) => "Create Workspace · choose Project",
            (Self::ZhCn, NativeSetupStage::ChooseProject) => "创建 Workspace · 选择 Project",
            (Self::EnUs, NativeSetupStage::ProjectLabel) => "Create Workspace · new Project name",
            (Self::ZhCn, NativeSetupStage::ProjectLabel) => "创建 Workspace · 新 Project 名称",
            (Self::EnUs, NativeSetupStage::WorkspaceLabel) => "Create Workspace · Workspace name",
            (Self::ZhCn, NativeSetupStage::WorkspaceLabel) => "创建 Workspace · Workspace 名称",
            (Self::EnUs, NativeSetupStage::DirectoryPath) => "Configure Workspace directory",
            (Self::ZhCn, NativeSetupStage::DirectoryPath) => "配置 Workspace 目录",
            (_, NativeSetupStage::ProjectRename) => self.location_rename_title(true),
            (_, NativeSetupStage::WorkspaceRename) => self.location_rename_title(false),
        }
    }
    pub(crate) const fn quick_workspace_name(self) -> &'static str {
        match self {
            Self::EnUs => "Name",
            Self::ZhCn => "名称",
        }
    }
    pub(crate) const fn quick_workspace_directory(self) -> &'static str {
        match self {
            Self::EnUs => "Directory",
            Self::ZhCn => "目录",
        }
    }
    pub(crate) const fn quick_workspace_actions(self) -> &'static str {
        match self {
            Self::EnUs => "Tab: name / directory · Enter: create and enter · Esc: cancel",
            Self::ZhCn => "Tab：切换名称／目录 · Enter：创建并进入 · Esc：取消",
        }
    }
    pub(crate) const fn quick_workspace_preparing_actions(self) -> &'static str {
        match self {
            Self::EnUs => "Esc: cancel · no Workspace has been created",
            Self::ZhCn => "Esc：取消 · 尚未创建工作区",
        }
    }

    pub(crate) const fn setup_new_project(self) -> &'static str {
        match self {
            Self::EnUs => "New Project",
            Self::ZhCn => "新建 Project",
        }
    }

    pub(crate) const fn setup_project(self) -> &'static str {
        match self {
            Self::EnUs => "Project",
            Self::ZhCn => "Project",
        }
    }

    pub(crate) const fn setup_actions(self, picker: bool) -> &'static str {
        match (self, picker) {
            (Self::EnUs, true) => "[ / ] · choose Project    Enter · continue    Esc · cancel",
            (Self::ZhCn, true) => "[ / ] · 选择 Project    Enter · 继续    Esc · 取消",
            (Self::EnUs, false) => {
                "Enter · submit    Esc · cancel    Backspace · delete    Ctrl+U · clear"
            }
            (Self::ZhCn, false) => {
                "Enter · 提交    Esc · 取消    Backspace · 删除    Ctrl+U · 清空"
            }
        }
    }

    pub(crate) const fn setup_input_hint(self, directory: bool) -> &'static str {
        match (self, directory) {
            (Self::EnUs, false) => "Enter a name (1–512 characters).",
            (Self::ZhCn, false) => "请输入名称（1–512 个字符）。",
            (Self::EnUs, true) => "Enter the full path to an existing folder.",
            (Self::ZhCn, true) => "请输入现有文件夹的完整路径。",
        }
    }

    pub(crate) const fn setup_fault(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Input could not be accepted. Press Backspace or Ctrl+U to clear the input error."
            }
            Self::ZhCn => "无法接受输入。请按 Backspace 或 Ctrl+U 清除输入错误。",
        }
    }

    pub(crate) const fn setup_resize_hint(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Enlarge the window to see the complete Workspace setup, or press Esc to cancel."
            }
            Self::ZhCn => "请扩大窗口以查看完整 Workspace 设置，或按 Esc 取消。",
        }
    }

    pub(crate) const fn setup_project_retained(self) -> &'static str {
        match self {
            Self::EnUs => "Project created. Cancelling this step keeps the Project.",
            Self::ZhCn => "Project 已创建。取消此步骤会保留 Project。",
        }
    }

    pub(crate) const fn setup_status(self, status: NativeSetupStatus) -> &'static str {
        match (self, status) {
            (_, NativeSetupStatus::LocationLabel { target, status }) => {
                self.location_label_status(target, status)
            }
            (Self::EnUs, NativeSetupStatus::NotApplied) => {
                "Workspace setup not applied · check the target and input before trying again"
            }
            (Self::ZhCn, NativeSetupStatus::NotApplied) => {
                "Workspace 设置未应用 · 请检查目标及输入后再试"
            }
            (Self::EnUs, NativeSetupStatus::ProjectCreated) => {
                "Project created · the committed Project is retained"
            }
            (Self::ZhCn, NativeSetupStatus::ProjectCreated) => {
                "Project 已创建 · 已提交的 Project 会保留"
            }
            (Self::EnUs, NativeSetupStatus::WorkspaceCreated) => "Workspace created",
            (Self::ZhCn, NativeSetupStatus::WorkspaceCreated) => "Workspace 已创建",
            (Self::EnUs, NativeSetupStatus::DirectoryAccepted) => {
                "Directory configured · Ctrl+B c to launch a terminal"
            }
            (Self::ZhCn, NativeSetupStatus::DirectoryAccepted) => {
                "目录已配置 · 按 Ctrl+B c 启动终端"
            }
            (Self::EnUs, NativeSetupStatus::CommittedNeedRefresh) => {
                "Setup committed · Ctrl+B w to recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, NativeSetupStatus::CommittedNeedRefresh) => {
                "设置已提交 · 按 Ctrl+B w 恢复快照；请勿重复提交"
            }
            (Self::EnUs, NativeSetupStatus::OutcomeUnknown) => {
                "Setup outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, NativeSetupStatus::OutcomeUnknown) => {
                "设置结果未知 · 连接已退役；请勿重复变更"
            }
        }
    }

    pub(crate) fn setup_directory_ready(self, path: &str) -> String {
        match self {
            Self::EnUs => {
                format!("Directory: {path}\nCtrl+B c · launch a terminal in this directory")
            }
            Self::ZhCn => format!("目录：{path}\nCtrl+B c · 在此目录启动终端"),
        }
    }

    pub(crate) const fn setup_directory_needed(self) -> &'static str {
        match self {
            Self::EnUs => "No directory configured. Ctrl+B e · configure the first directory",
            Self::ZhCn => "尚未配置目录。Ctrl+B e · 配置首个目录",
        }
    }
}
