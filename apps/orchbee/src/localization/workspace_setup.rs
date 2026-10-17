use super::Locale;
use crate::terminal_session::{
    NativeLocationLabelStatus, NativeSessionNotice, NativeSetupStage, NativeSetupStatus,
};

impl Locale {
    pub(crate) const fn no_current_workspace(self) -> &'static str {
        match self {
            Self::EnUs => "No workspace open. Select a workspace or create one to begin.",
            Self::ZhCn => "当前未打开工作区。请选择已有工作区或新建工作区。",
        }
    }

    pub(crate) const fn legacy_workspace_action_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Legacy workspace tabs remain usable · create a new workspace for this action"
            }
            Self::ZhCn => "原有工作区的 Tab 可继续使用 · 请新建工作区后执行此操作",
        }
    }

    pub(crate) const fn sidebar_delete_label(self) -> &'static str {
        match self {
            Self::EnUs => "Delete",
            Self::ZhCn => "删除",
        }
    }
    pub(crate) const fn project_delete_title(self) -> &'static str {
        match self {
            Self::EnUs => "Delete Project?",
            Self::ZhCn => "删除项目？",
        }
    }
    pub(crate) const fn project_delete_warning(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Remove this Project organization. All member Workspaces, terminals and programs are kept and become ungrouped."
            }
            Self::ZhCn => {
                "移除此项目组织关系。全部成员工作区、终端和程序都会保留，并成为未分组工作区。"
            }
        }
    }
    /// Localized progress for the Client-local Workspace creation projection.
    #[cfg(windows)]
    pub(crate) const fn workspace_creation_progress(
        self,
        phase: crate::terminal_session::WorkspaceCreationPhase,
    ) -> &'static str {
        use crate::terminal_session::WorkspaceCreationPhase;
        match (self, phase) {
            (Self::EnUs, WorkspaceCreationPhase::Preparing) => "Creating workspace…",
            (Self::ZhCn, WorkspaceCreationPhase::Preparing) => "正在创建工作区…",
            (Self::EnUs, WorkspaceCreationPhase::Starting) => "Starting terminal…",
            (Self::ZhCn, WorkspaceCreationPhase::Starting) => "正在启动终端…",
            (Self::EnUs, WorkspaceCreationPhase::Committed) => "Workspace created",
            (Self::ZhCn, WorkspaceCreationPhase::Committed) => "工作区已创建",
        }
    }
    pub(crate) fn cli_default_workspace_label(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace",
            Self::ZhCn => "工作区",
        }
    }
    pub(crate) const fn setup_submit_label(self) -> &'static str {
        match self {
            Self::EnUs => "Submit",
            Self::ZhCn => "提交",
        }
    }
    pub(crate) const fn setup_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn setup_select_hint(self) -> &'static str {
        match self {
            Self::EnUs => "select",
            Self::ZhCn => "确定",
        }
    }
    pub(crate) const fn setup_choose_hint(self) -> &'static str {
        match self {
            Self::EnUs => "choose",
            Self::ZhCn => "选择",
        }
    }
    pub(crate) const fn setup_next_field_hint(self) -> &'static str {
        match self {
            Self::EnUs => "next field",
            Self::ZhCn => "切换字段",
        }
    }
    pub(crate) const fn setup_delete_hint(self) -> &'static str {
        match self {
            Self::EnUs => "delete",
            Self::ZhCn => "删除",
        }
    }
    pub(crate) const fn setup_clear_hint(self) -> &'static str {
        match self {
            Self::EnUs => "clear",
            Self::ZhCn => "清空",
        }
    }
    pub(crate) const fn setup_choice_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => "Unavailable",
            Self::ZhCn => "不可用",
        }
    }
}

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
            (Some(primary), Some(NativeSetupStatus::TabsCommittedNeedRefresh)) if retired => {
                format!(
                    "{primary} · {}",
                    match self {
                        Self::EnUs => "Tabs updated · do not resubmit",
                        Self::ZhCn => "标签页已更新 · 请勿重复提交",
                    }
                )
            }
            (
                Some(primary),
                Some(
                    status @ (NativeSetupStatus::CommittedNeedRefresh
                    | NativeSetupStatus::OutcomeUnknown
                    | NativeSetupStatus::TabsCommittedNeedRefresh
                    | NativeSetupStatus::TabsOutcomeUnknown),
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
            Self::EnUs => "Workspace created · its first directory is not configured",
            Self::ZhCn => "工作区已创建 · 尚未配置首个目录",
        }
    }

    pub(crate) const fn setup_title(self, stage: NativeSetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, NativeSetupStage::ChooseWorkspaceProject) => "Move Workspace to Project",
            (Self::ZhCn, NativeSetupStage::ChooseWorkspaceProject) => "将工作区移入项目",
            (Self::EnUs, NativeSetupStage::WorkspaceProjectName) => {
                "Create Project and move Workspace"
            }
            (Self::ZhCn, NativeSetupStage::WorkspaceProjectName) => "新建项目并移入工作区",
            (Self::EnUs, NativeSetupStage::DissolveProject) => "Dissolving Project…",
            (Self::ZhCn, NativeSetupStage::DissolveProject) => "正在解散项目…",
            (_, NativeSetupStage::ProjectDeleteConfirm) => self.project_delete_title(),
            (Self::EnUs, NativeSetupStage::TabGroupName) => "Tab group name",
            (Self::ZhCn, NativeSetupStage::TabGroupName) => "标签页分组名称",
            (Self::EnUs, NativeSetupStage::PreparingWorkspace) => "Preparing Workspace directory…",
            (Self::ZhCn, NativeSetupStage::PreparingWorkspace) => "正在准备工作区目录…",
            (Self::EnUs, NativeSetupStage::QuickProject) => "New Workspace · choose Project",
            (Self::ZhCn, NativeSetupStage::QuickProject) => "新建工作区 · 选择项目",
            (Self::EnUs, NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory) => {
                "New Workspace"
            }
            (Self::ZhCn, NativeSetupStage::QuickName | NativeSetupStage::QuickDirectory) => {
                "新建工作区"
            }
            (Self::EnUs, NativeSetupStage::DirectoryPath) => "Configure Workspace directory",
            (Self::ZhCn, NativeSetupStage::DirectoryPath) => "配置工作区目录",
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
    pub(crate) const fn quick_workspace_directory_hint(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Uses the terminal process directory; PowerShell cd may differ. Choose an existing folder for this Workspace."
            }
            Self::ZhCn => {
                "默认使用终端进程目录，可能与 PowerShell 的 cd 目录不同。可为本次工作区指定已有文件夹。"
            }
        }
    }
    pub(crate) const fn quick_workspace_preparing_fact(self) -> &'static str {
        match self {
            Self::EnUs => "No Workspace has been created yet.",
            Self::ZhCn => "尚未创建工作区。",
        }
    }

    pub(crate) const fn setup_new_project(self) -> &'static str {
        match self {
            Self::EnUs => "New Project",
            Self::ZhCn => "新建项目",
        }
    }
    pub(crate) const fn setup_ungrouped(self) -> &'static str {
        match self {
            Self::EnUs => "No Project",
            Self::ZhCn => "不属于项目",
        }
    }

    pub(crate) const fn setup_project(self) -> &'static str {
        match self {
            Self::EnUs => "Project",
            Self::ZhCn => "项目",
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

    pub(crate) const fn setup_group_name_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Group name (0–512 characters); leave blank for an unnamed group.",
            Self::ZhCn => "分组名称（0–512 个字符）；留空为未命名分组。",
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

    pub(crate) const fn setup_status(self, status: NativeSetupStatus) -> &'static str {
        match (self, status) {
            (Self::EnUs, NativeSetupStatus::PresentationUnavailable) => {
                "Cannot open here: enlarge the terminal."
            }
            (Self::ZhCn, NativeSetupStatus::PresentationUnavailable) => {
                "终端尺寸不足，放大后重试。"
            }
            (Self::EnUs, NativeSetupStatus::WorkspaceOrganized) => "Workspace organization updated",
            (Self::ZhCn, NativeSetupStatus::WorkspaceOrganized) => "工作区分组已更新",
            (Self::EnUs, NativeSetupStatus::ProjectDissolved) => {
                "Project dissolved · Workspaces retained"
            }
            (Self::ZhCn, NativeSetupStatus::ProjectDissolved) => "项目已解散 · 工作区已保留",
            (Self::EnUs, NativeSetupStatus::NavigationConfigured) => "Sidebar organization updated",
            (Self::ZhCn, NativeSetupStatus::NavigationConfigured) => "侧栏排序和分组已更新",
            (Self::EnUs, NativeSetupStatus::TabsConfigured) => "Tabs updated",
            (Self::ZhCn, NativeSetupStatus::TabsConfigured) => "标签页已更新",
            (Self::EnUs, NativeSetupStatus::TabsNotApplied) => {
                "Tabs not updated · check the target before trying again"
            }
            (Self::ZhCn, NativeSetupStatus::TabsNotApplied) => "标签页未更新 · 请检查目标后再试",
            (Self::EnUs, NativeSetupStatus::TabsCommittedNeedRefresh) => {
                "Tabs updated · recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, NativeSetupStatus::TabsCommittedNeedRefresh) => {
                "标签页已更新 · 恢复快照；请勿重复提交"
            }
            (Self::EnUs, NativeSetupStatus::TabsOutcomeUnknown) => {
                "Tab update outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, NativeSetupStatus::TabsOutcomeUnknown) => {
                "标签页更新结果未知 · 连接已退役；请勿重复变更"
            }
            (_, NativeSetupStatus::LocationLabel { target, status }) => {
                self.location_label_status(target, status)
            }
            (Self::EnUs, NativeSetupStatus::NotApplied) => {
                "Workspace setup not applied · check the target and input before trying again"
            }
            (Self::ZhCn, NativeSetupStatus::NotApplied) => {
                "工作区设置未应用 · 请检查目标及输入后再试"
            }
            (Self::EnUs, NativeSetupStatus::ProjectCreated) => {
                "Project created · the committed Project is retained"
            }
            (Self::ZhCn, NativeSetupStatus::ProjectCreated) => "项目已创建 · 已提交的项目会保留",
            (Self::EnUs, NativeSetupStatus::WorkspaceCreated) => "Workspace created",
            (Self::ZhCn, NativeSetupStatus::WorkspaceCreated) => "工作区已创建",
            (Self::EnUs, NativeSetupStatus::DirectoryAccepted) => {
                "Directory configured · ready to launch a terminal"
            }
            (Self::ZhCn, NativeSetupStatus::DirectoryAccepted) => "目录已配置 · 可以启动终端",
            (Self::EnUs, NativeSetupStatus::CommittedNeedRefresh) => {
                "Setup committed · recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, NativeSetupStatus::CommittedNeedRefresh) => {
                "设置已提交 · 恢复快照；请勿重复提交"
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
            Self::EnUs => format!("Directory: {path}"),
            Self::ZhCn => format!("目录：{path}"),
        }
    }

    pub(crate) const fn setup_directory_needed(self) -> &'static str {
        match self {
            Self::EnUs => "No directory configured.",
            Self::ZhCn => "尚未配置目录。",
        }
    }
}
