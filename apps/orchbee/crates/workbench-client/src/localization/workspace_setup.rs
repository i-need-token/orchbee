use super::Locale;
use crate::localization::{LocationLabelStatus, LocationLabelTarget};

/// The setup field or confirmation currently selected by its feature owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupStage {
    ChooseWorkspaceProject,
    WorkspaceProjectName,
    DissolveProject,
    ProjectDeleteConfirm,
    TabGroupName,
    PreparingWorkspace,
    QuickProject,
    QuickName,
    QuickDirectory,
    DirectoryPath,
    ProjectRename,
    WorkspaceRename,
}

/// Observed setup outcome, including facts retained after loss of publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupStatus {
    PresentationUnavailable,
    WorkspaceOrganized,
    ProjectDissolved,
    TabsConfigured,
    NavigationConfigured,
    TabsNotApplied,
    TabsCommittedNeedRefresh,
    TabsOutcomeUnknown,
    LocationLabel {
        target: LocationLabelTarget,
        status: LocationLabelStatus,
    },
    NotApplied,
    /// Refused because the Workbench changed elsewhere; the session is resynchronizing.
    Stale,
    ProjectCreated,
    WorkspaceCreated,
    DirectoryAccepted,
    CommittedNeedRefresh,
    OutcomeUnknown,
}

/// The creation phase projected without waiting for preparation or terminal startup I/O.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCreationPhase {
    /// The source directory and launch command are being prepared.
    Preparing,
    /// The accepted launch or its canonical snapshot is still being reconciled.
    Starting,
    /// Canonical creation has settled; the sidebar observer may still be catching up.
    Committed,
}

impl Locale {
    pub(crate) const fn no_current_workspace(self) -> &'static str {
        match self {
            Self::EnUs => "No workspace open. Select a workspace or create one to begin.",
            Self::ZhCn => "当前未打开工作区。请选择已有工作区或新建工作区。",
        }
    }

    /// Confirms Dissolve Project, the menu action that opens this decision.
    pub(crate) const fn project_dissolve_label(self) -> &'static str {
        match self {
            Self::EnUs => "Dissolve",
            Self::ZhCn => "解散",
        }
    }
    pub(crate) const fn project_delete_title(self) -> &'static str {
        match self {
            Self::EnUs => "Dissolve Project?",
            Self::ZhCn => "解散项目？",
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
    pub(crate) const fn workspace_creation_progress(
        self,
        phase: crate::localization::WorkspaceCreationPhase,
    ) -> &'static str {
        use crate::localization::WorkspaceCreationPhase;
        match (self, phase) {
            (Self::EnUs, WorkspaceCreationPhase::Preparing) => "Creating workspace…",
            (Self::ZhCn, WorkspaceCreationPhase::Preparing) => "正在创建工作区…",
            (Self::EnUs, WorkspaceCreationPhase::Starting) => "Starting terminal…",
            (Self::ZhCn, WorkspaceCreationPhase::Starting) => "正在启动终端…",
            (Self::EnUs, WorkspaceCreationPhase::Committed) => "Workspace created",
            (Self::ZhCn, WorkspaceCreationPhase::Committed) => "工作区已创建",
        }
    }
    pub fn cli_default_workspace_label(self) -> &'static str {
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
            Self::ZhCn => "选择",
        }
    }
    pub(crate) const fn setup_choose_hint(self) -> &'static str {
        match self {
            Self::EnUs => "choose",
            Self::ZhCn => "确定",
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
    pub(crate) const fn setup_tabs_committed_fact(self) -> &'static str {
        match self {
            Self::EnUs => "Tabs updated · do not resubmit",
            Self::ZhCn => "标签页已更新 · 请勿重复提交",
        }
    }

    pub(crate) const fn setup_committed_fact(self) -> &'static str {
        match self {
            Self::EnUs => "Setup committed · do not resubmit",
            Self::ZhCn => "设置已提交 · 请勿重复提交",
        }
    }

    pub(crate) const fn setup_workspace_directory_action(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace created · its first directory is not configured",
            Self::ZhCn => "工作区已创建 · 尚未配置首个目录",
        }
    }

    pub(crate) const fn setup_title(self, stage: SetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, SetupStage::ChooseWorkspaceProject) => "Move Workspace to Project",
            (Self::ZhCn, SetupStage::ChooseWorkspaceProject) => "将工作区移入项目",
            (Self::EnUs, SetupStage::WorkspaceProjectName) => "Create Project and move Workspace",
            (Self::ZhCn, SetupStage::WorkspaceProjectName) => "新建项目并移入工作区",
            (Self::EnUs, SetupStage::DissolveProject) => "Dissolving Project…",
            (Self::ZhCn, SetupStage::DissolveProject) => "正在解散项目…",
            (_, SetupStage::ProjectDeleteConfirm) => self.project_delete_title(),
            (Self::EnUs, SetupStage::TabGroupName) => "Tab group name",
            (Self::ZhCn, SetupStage::TabGroupName) => "标签页分组名称",
            (Self::EnUs, SetupStage::PreparingWorkspace) => "Preparing Workspace directory…",
            (Self::ZhCn, SetupStage::PreparingWorkspace) => "正在准备工作区目录…",
            (Self::EnUs, SetupStage::QuickProject) => "New Workspace · choose Project",
            (Self::ZhCn, SetupStage::QuickProject) => "新建工作区 · 选择项目",
            (Self::EnUs, SetupStage::QuickName | SetupStage::QuickDirectory) => "New Workspace",
            (Self::ZhCn, SetupStage::QuickName | SetupStage::QuickDirectory) => "新建工作区",
            (Self::EnUs, SetupStage::DirectoryPath) => "Configure Workspace directory",
            (Self::ZhCn, SetupStage::DirectoryPath) => "配置工作区目录",
            (_, SetupStage::ProjectRename) => self.location_rename_title(true),
            (_, SetupStage::WorkspaceRename) => self.location_rename_title(false),
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
                "Enter creates here. Edit the path, or use Up/Down to choose current or recent workspace folders."
            }
            Self::ZhCn => "回车在此创建。可直接编辑路径，或用上下键选择当前及最近工作区目录。",
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

    pub(crate) const fn setup_status(self, status: SetupStatus) -> &'static str {
        match (self, status) {
            (Self::EnUs, SetupStatus::PresentationUnavailable) => {
                "Cannot open here: enlarge the terminal."
            }
            (Self::ZhCn, SetupStatus::PresentationUnavailable) => "终端尺寸不足，放大后重试。",
            (Self::EnUs, SetupStatus::WorkspaceOrganized) => "Workspace organization updated",
            (Self::ZhCn, SetupStatus::WorkspaceOrganized) => "工作区分组已更新",
            (Self::EnUs, SetupStatus::ProjectDissolved) => {
                "Project dissolved · Workspaces retained"
            }
            (Self::ZhCn, SetupStatus::ProjectDissolved) => "项目已解散 · 工作区已保留",
            (Self::EnUs, SetupStatus::NavigationConfigured) => "Sidebar organization updated",
            (Self::ZhCn, SetupStatus::NavigationConfigured) => "侧栏排序和分组已更新",
            (Self::EnUs, SetupStatus::TabsConfigured) => "Tabs updated",
            (Self::ZhCn, SetupStatus::TabsConfigured) => "标签页已更新",
            (Self::EnUs, SetupStatus::TabsNotApplied) => {
                "Tabs not updated · check the target before trying again"
            }
            (Self::ZhCn, SetupStatus::TabsNotApplied) => "标签页未更新 · 请检查目标后再试",
            (Self::EnUs, SetupStatus::TabsCommittedNeedRefresh) => {
                "Tabs updated · recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, SetupStatus::TabsCommittedNeedRefresh) => {
                "标签页已更新 · 恢复快照；请勿重复提交"
            }
            (Self::EnUs, SetupStatus::TabsOutcomeUnknown) => {
                "Tab update outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, SetupStatus::TabsOutcomeUnknown) => {
                "标签页更新结果未知 · 连接已退役；请勿重复变更"
            }
            (_, SetupStatus::LocationLabel { target, status }) => {
                self.location_label_status(target, status)
            }
            (Self::EnUs, SetupStatus::NotApplied) => {
                "Workspace setup not applied · check the target and input before trying again"
            }
            (Self::ZhCn, SetupStatus::NotApplied) => "工作区设置未应用 · 请检查目标及输入后再试",
            (Self::EnUs, SetupStatus::Stale) => "Setup not applied · Workbench changed",
            (Self::ZhCn, SetupStatus::Stale) => "设置未应用 · 工作台已变化",
            (Self::EnUs, SetupStatus::ProjectCreated) => {
                "Project created · the committed Project is retained"
            }
            (Self::ZhCn, SetupStatus::ProjectCreated) => "项目已创建 · 已提交的项目会保留",
            (Self::EnUs, SetupStatus::WorkspaceCreated) => "Workspace created",
            (Self::ZhCn, SetupStatus::WorkspaceCreated) => "工作区已创建",
            (Self::EnUs, SetupStatus::DirectoryAccepted) => {
                "Directory configured · ready to launch a terminal"
            }
            (Self::ZhCn, SetupStatus::DirectoryAccepted) => "目录已配置 · 可以启动终端",
            (Self::EnUs, SetupStatus::CommittedNeedRefresh) => {
                "Setup committed · recover the snapshot; do not resubmit"
            }
            (Self::ZhCn, SetupStatus::CommittedNeedRefresh) => {
                "设置已提交 · 恢复快照；请勿重复提交"
            }
            (Self::EnUs, SetupStatus::OutcomeUnknown) => {
                "Setup outcome unknown · connection retired; do not repeat the mutation"
            }
            (Self::ZhCn, SetupStatus::OutcomeUnknown) => "设置结果未知 · 连接已退役；请勿重复变更",
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
