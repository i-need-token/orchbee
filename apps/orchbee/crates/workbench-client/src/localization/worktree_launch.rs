//! Localized Worktree forms, retained receipts and progress hints.
use super::Locale;
mod removal;

/// The requested everyday Worktree operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeMode {
    Create,
    Open,
    Remove,
}

/// A presentation reason, independent of transport diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeNotice {
    PresentationUnavailable,
    Preparing,
    InvalidInput,
    InputLimit,
    SourceUnavailable,
    ListingUnavailable,
    /// Git's probe refused the source: not a repository, or an untrusted owner.
    NotRepository,
    ConfigurationUnavailable,
    Busy,
    NotSubmitted,
    Pending,
    Unknown,
    Failed,
    Opened,
    Removed,
    EntryDeferred,
    EntryPending,
    EntryFailed,
    SourceChanged,
    /// This Client process stands in the removal target, which Windows keeps in use.
    ClientInCheckout,
}

/// Why a Branch or Base value cannot be submitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeFieldError {
    Invalid,
    TooLong,
}

/// The current keyboard owner inside the Worktree surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeStage {
    Preparing,
    Branch,
    Base,
    Checkout,
    Remove,
    ResumeRemove,
    AbandonRemove,
    Waiting,
    Result,
}

impl Locale {
    /// Closing a running or settled operation never stops it, so its cancel button reads Close.
    pub(crate) fn worktree_cancel_label(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Waiting | WorktreeStage::Result) => "Close",
            (Self::ZhCn, WorktreeStage::Waiting | WorktreeStage::Result) => "关闭",
            (Self::EnUs, _) => "Cancel",
            (Self::ZhCn, _) => "取消",
        }
    }
    pub(crate) fn worktree_submit_label(self, mode: WorktreeMode) -> &'static str {
        match (self, mode) {
            (Self::EnUs, WorktreeMode::Create) => "Create",
            (Self::ZhCn, WorktreeMode::Create) => "创建",
            (Self::EnUs, WorktreeMode::Open) => "Open",
            (Self::ZhCn, WorktreeMode::Open) => "打开",
            (Self::EnUs, WorktreeMode::Remove) => "Remove",
            (Self::ZhCn, WorktreeMode::Remove) => "移除",
        }
    }
    /// The checkout picker's choice key.
    pub(crate) fn worktree_choose_hint(self) -> &'static str {
        match self {
            Self::EnUs => "open",
            Self::ZhCn => "打开",
        }
    }
    /// An empty base starts the worktree from the current commit.
    pub(crate) fn worktree_base_placeholder(self) -> &'static str {
        "HEAD"
    }
    pub(crate) fn worktree_branch_help(self) -> &'static str {
        match self {
            Self::EnUs => "Leave blank for a generated name.",
            Self::ZhCn => "留空则自动生成名称。",
        }
    }
    pub(crate) fn worktree_field_error(
        self,
        error: WorktreeFieldError,
        base: bool,
    ) -> &'static str {
        match (self, error, base) {
            (Self::EnUs, WorktreeFieldError::Invalid, false) => "Use a valid branch name.",
            (Self::ZhCn, WorktreeFieldError::Invalid, false) => "请使用有效的分支名称。",
            (Self::EnUs, WorktreeFieldError::Invalid, true) => "Use a valid branch, tag or commit.",
            (Self::ZhCn, WorktreeFieldError::Invalid, true) => "请使用有效的分支、标签或提交。",
            (Self::EnUs, WorktreeFieldError::TooLong, _) => "Use at most 1024 bytes.",
            (Self::ZhCn, WorktreeFieldError::TooLong, _) => "最多使用 1024 个字节。",
        }
    }
    pub(crate) fn worktree_default_workspace(self, ordinal: usize) -> String {
        match self {
            Self::EnUs => format!("Worktree {ordinal}"),
            Self::ZhCn => format!("工作树 {ordinal}"),
        }
    }
    pub(crate) fn worktree_title(self, mode: WorktreeMode) -> &'static str {
        match (self, mode) {
            (Self::EnUs, WorktreeMode::Create) => "New Worktree",
            (Self::EnUs, WorktreeMode::Open) => "Open Worktree",
            (Self::ZhCn, WorktreeMode::Create) => "新建工作树",
            (Self::ZhCn, WorktreeMode::Open) => "打开工作树",
            (Self::EnUs, WorktreeMode::Remove) => "Remove Worktree",
            (Self::ZhCn, WorktreeMode::Remove) => "移除工作树",
        }
    }
    pub(crate) fn worktree_field(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Base) => "Base",
            (Self::ZhCn, WorktreeStage::Base) => "起点",
            (Self::EnUs, _) => "Branch",
            (Self::ZhCn, _) => "分支",
        }
    }
    pub(crate) fn worktree_notice(self, notice: WorktreeNotice) -> &'static str {
        use WorktreeNotice as N;
        match (self, notice) {
            (Self::EnUs, N::PresentationUnavailable) => "Cannot open here: enlarge the terminal.",
            (Self::ZhCn, N::PresentationUnavailable) => "终端尺寸不足，放大后重试。",
            (Self::EnUs, N::EntryDeferred) => "Workspace ready; select it in the navigator.",
            (Self::ZhCn, N::EntryDeferred) => "工作区已就绪，可在导航中选择。",
            (Self::EnUs, N::Preparing) => "Reading repository…",
            (Self::ZhCn, N::Preparing) => "正在读取仓库…",
            (Self::EnUs, N::InvalidInput) => "Use a valid branch and base.",
            (Self::ZhCn, N::InvalidInput) => "请使用有效的分支和起点。",
            (Self::EnUs, N::InputLimit) => "Each field allows at most 1024 UTF-8 bytes.",
            (Self::ZhCn, N::InputLimit) => "每个字段最多允许 1024 个 UTF-8 字节。",
            (Self::EnUs, N::SourceUnavailable) => {
                "Source unavailable. Use a Workspace directory or an attached primary terminal."
            }
            (Self::ZhCn, N::SourceUnavailable) => "来源不可用。请使用工作区目录或已连接的主终端。",
            (Self::EnUs, N::ListingUnavailable) => {
                "Could not list Git checkouts. Inspect with orchbee worktree list."
            }
            (Self::ZhCn, N::ListingUnavailable) => {
                "无法列举 Git 检出目录。请用 orchbee worktree list 检查。"
            }
            (Self::EnUs, N::NotRepository) => {
                "This directory is not in a Git repository, or Git does not trust its owner. Run git status there to see which."
            }
            (Self::ZhCn, N::NotRepository) => {
                "这个目录不在 Git 仓库中，或 Git 不信任其所有者。请在该目录运行 git status 查看原因。"
            }
            (Self::EnUs, N::ConfigurationUnavailable) => {
                "Could not read worktree configuration. Run orchbee config check."
            }
            (Self::ZhCn, N::ConfigurationUnavailable) => {
                "无法读取工作树配置。请运行 orchbee config check。"
            }
            (Self::EnUs, N::Busy) => "Source is busy. Wait for the current transition.",
            (Self::ZhCn, N::Busy) => "来源忙碌。请等待当前切换完成。",
            (Self::EnUs, N::NotSubmitted) => {
                "Worktree was not submitted. Existing data is retained."
            }
            (Self::ZhCn, N::NotSubmitted) => "工作树操作未提交，已有数据已保留。",
            (Self::EnUs, N::Pending) => "Worktree operation is running…",
            (Self::ZhCn, N::Pending) => "工作树操作正在执行…",
            (Self::EnUs, N::Unknown) => "Result unknown. Check it with orchbee worktree status.",
            (Self::ZhCn, N::Unknown) => "结果未知。请用 orchbee worktree status 查询。",
            (Self::EnUs, N::Failed) => {
                "Operation stopped. Checkout and Workspace facts are retained."
            }
            (Self::ZhCn, N::Failed) => "操作已停止。检出目录和工作区信息已保留。",
            (Self::EnUs, N::Opened) => "Workspace opened.",
            (Self::ZhCn, N::Opened) => "工作区已打开。",
            (Self::EnUs, N::Removed) => "Worktree removed and Workspace closed. Branch kept.",
            (Self::ZhCn, N::Removed) => "工作树已移除，工作区已关闭，分支已保留。",
            (Self::EnUs, N::EntryPending) => "Workspace opened; connecting its terminal…",
            (Self::ZhCn, N::EntryPending) => "工作区已打开，正在连接终端…",
            (Self::EnUs, N::EntryFailed) => {
                "Workspace opened; terminal entry did not finish. Select it in the navigator."
            }
            (Self::ZhCn, N::EntryFailed) => "工作区已打开，终端进入未完成。请在导航器中选择它。",
            (Self::EnUs, N::SourceChanged) => {
                "Source changed. Confirmed operation facts are retained."
            }
            (Self::ZhCn, N::SourceChanged) => "来源已变化，已确认操作的信息已保留。",
            (Self::EnUs, N::ClientInCheckout) => {
                "Orchbee was started inside this worktree, so Windows keeps it in use. Quit Orchbee, leave the folder in that shell, then remove the worktree from elsewhere."
            }
            (Self::ZhCn, N::ClientInCheckout) => {
                "Orchbee 是在这个工作树里启动的，Windows 会一直占用它。请退出 Orchbee，在那个 shell 里离开该目录，再从其他位置移除工作树。"
            }
        }
    }
    pub(crate) fn worktree_status_line(
        self,
        notice: WorktreeNotice,
        stage: Option<WorktreeStage>,
    ) -> String {
        stage.map_or_else(
            || self.worktree_notice(notice).to_owned(),
            |stage| self.worktree_stage_notice(notice, stage).to_owned(),
        )
    }
    pub(crate) fn worktree_stage_notice(
        self,
        notice: WorktreeNotice,
        _stage: WorktreeStage,
    ) -> &'static str {
        self.worktree_notice(notice)
    }
}
