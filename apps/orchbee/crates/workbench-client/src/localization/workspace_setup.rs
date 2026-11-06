use super::Locale;
use crate::localization::{LocationLabelStatus, LocationLabelTarget};

/// The setup field or confirmation currently selected by its feature owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupStage {
    ChooseWorkspaceProject,
    WorkspaceProjectName,
    TabGroupName,
    PreparingWorkspace,
    QuickProject,
    QuickName,
    QuickDirectory,
    DirectoryPath,
    ProjectRename,
    WorkspaceRename,
}

/// Why a form field cannot be submitted, shown in its help row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldError {
    NameRequired,
    NameTooLong,
    PathRequired,
    PathInvalid,
    PathTooLong,
}

/// Observed setup outcome, including facts retained after loss of publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupStatus {
    LaunchRequestTooLarge,
    PresentationUnavailable,
    WorkspaceOrganized,
    ProjectDissolved,
    /// A Project dissolved without member Workspaces, so nothing was retained.
    EmptyProjectDissolved,
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

    /// The title names the task; a follow-up step keeps the frame and states its own task.
    pub(crate) const fn setup_title(self, stage: SetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, SetupStage::ChooseWorkspaceProject) => "Move Workspace to Project",
            (Self::ZhCn, SetupStage::ChooseWorkspaceProject) => "将工作区移入项目",
            (Self::EnUs, SetupStage::WorkspaceProjectName) => "New Project",
            (Self::ZhCn, SetupStage::WorkspaceProjectName) => "新建项目",
            (Self::EnUs, SetupStage::TabGroupName) => "New Tab Group",
            (Self::ZhCn, SetupStage::TabGroupName) => "新建标签页分组",
            (
                Self::EnUs,
                SetupStage::PreparingWorkspace
                | SetupStage::QuickProject
                | SetupStage::QuickName
                | SetupStage::QuickDirectory,
            ) => "New Workspace",
            (
                Self::ZhCn,
                SetupStage::PreparingWorkspace
                | SetupStage::QuickProject
                | SetupStage::QuickName
                | SetupStage::QuickDirectory,
            ) => "新建工作区",
            (Self::EnUs, SetupStage::DirectoryPath) => "Set Workspace Directory",
            (Self::ZhCn, SetupStage::DirectoryPath) => "设置工作区目录",
            (_, SetupStage::ProjectRename) => self.location_rename_title(true),
            (_, SetupStage::WorkspaceRename) => self.location_rename_title(false),
        }
    }
    /// The primary names the outcome with the title's verb.
    pub(crate) const fn setup_submit_label(self, stage: SetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, SetupStage::ChooseWorkspaceProject) => "Move",
            (Self::ZhCn, SetupStage::ChooseWorkspaceProject) => "移入",
            (Self::EnUs, SetupStage::DirectoryPath) => "Set",
            (Self::ZhCn, SetupStage::DirectoryPath) => "设置",
            (Self::EnUs, SetupStage::ProjectRename | SetupStage::WorkspaceRename) => "Rename",
            (Self::ZhCn, SetupStage::ProjectRename | SetupStage::WorkspaceRename) => "重命名",
            (Self::EnUs, _) => "Create",
            (Self::ZhCn, _) => "创建",
        }
    }
    pub(crate) const fn tab_group_rename_title(self) -> &'static str {
        match self {
            Self::EnUs => "Rename Tab Group",
            Self::ZhCn => "重命名标签页分组",
        }
    }
    /// A picker's choice key names its outcome: moving the Workspace, or choosing a Project.
    pub(crate) const fn setup_choose_hint(self, stage: SetupStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, SetupStage::ChooseWorkspaceProject) => "move",
            (Self::ZhCn, SetupStage::ChooseWorkspaceProject) => "移入",
            (Self::EnUs, _) => "choose",
            (Self::ZhCn, _) => "选择",
        }
    }
    pub(crate) const fn setup_close_hint(self) -> &'static str {
        match self {
            Self::EnUs => "close",
            Self::ZhCn => "关闭",
        }
    }
    /// The heading of the recent folders listed under the directory field.
    pub(crate) const fn quick_workspace_recent(self) -> &'static str {
        match self {
            Self::EnUs => "Recent",
            Self::ZhCn => "最近",
        }
    }
    pub(crate) const fn setup_directory_help(self) -> &'static str {
        match self {
            Self::EnUs => "Use the full path of an existing folder.",
            Self::ZhCn => "使用现有文件夹的完整路径。",
        }
    }
    pub(crate) const fn setup_group_name_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Leave blank for an unnamed group.",
            Self::ZhCn => "留空则为未命名分组。",
        }
    }
    /// Field errors state the fix in one sentence.
    pub(crate) const fn field_error(self, error: FieldError) -> &'static str {
        match (self, error) {
            (Self::EnUs, FieldError::NameRequired) => "Enter a name.",
            (Self::ZhCn, FieldError::NameRequired) => "请输入名称。",
            (Self::EnUs, FieldError::NameTooLong) => "Use at most 512 characters.",
            (Self::ZhCn, FieldError::NameTooLong) => "最多使用 512 个字符。",
            (Self::EnUs, FieldError::PathRequired) => "Enter a path.",
            (Self::ZhCn, FieldError::PathRequired) => "请输入路径。",
            (Self::EnUs, FieldError::PathInvalid) => "Use the full path of a folder.",
            (Self::ZhCn, FieldError::PathInvalid) => "请使用文件夹的完整路径。",
            (Self::EnUs, FieldError::PathTooLong) => "Use a shorter path.",
            (Self::ZhCn, FieldError::PathTooLong) => "请使用更短的路径。",
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

    /// The New Workspace context line: the Project the Workspace joins.
    pub(crate) fn setup_project_context(self, project: &str) -> String {
        match self {
            Self::EnUs => format!("Project: {project}"),
            Self::ZhCn => format!("项目：{project}"),
        }
    }

    pub(crate) const fn setup_status(self, status: SetupStatus) -> &'static str {
        match (self, status) {
            (_, SetupStatus::LaunchRequestTooLarge) => self.launch_request_too_large(),
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
            (Self::EnUs, SetupStatus::EmptyProjectDissolved) => "Project dissolved",
            (Self::ZhCn, SetupStatus::EmptyProjectDissolved) => "项目已解散",
            (Self::EnUs, SetupStatus::NavigationConfigured) => "Sidebar organization updated",
            (Self::ZhCn, SetupStatus::NavigationConfigured) => "边栏排序和分组已更新",
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
