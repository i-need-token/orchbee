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
            (_, N::PresentationUnavailable) => self.presentation_notice(false),
            (_, N::Unknown) => self.outcome_unknown(),
            (Self::EnUs, N::EntryDeferred) => "The Workspace is ready. Select it in the sidebar.",
            (Self::ZhCn, N::EntryDeferred) => "工作区已就绪。请在边栏中选择它。",
            (Self::EnUs, N::Preparing) => "Reading repository…",
            (Self::ZhCn, N::Preparing) => "正在读取仓库…",
            (Self::EnUs, N::InvalidInput) => "Use a valid branch and base.",
            (Self::ZhCn, N::InvalidInput) => "请使用有效的分支和起点。",
            (Self::EnUs, N::InputLimit) => "Each field allows at most 1024 UTF-8 bytes.",
            (Self::ZhCn, N::InputLimit) => "每个字段最多允许 1024 个 UTF-8 字节。",
            (Self::EnUs, N::SourceUnavailable) => {
                "Couldn't find a folder to start from. Use a Workspace that has a folder or a Main terminal."
            }
            (Self::ZhCn, N::SourceUnavailable) => {
                "没能找到起始目录。请使用有目录或主终端的工作区。"
            }
            (Self::EnUs, N::ListingUnavailable) => "Couldn't list the Git checkouts.",
            (Self::ZhCn, N::ListingUnavailable) => "没能列出 Git 检出目录。",
            (Self::EnUs, N::NotRepository) => {
                "This directory is not in a Git repository, or Git does not trust its owner. Run git status there to see which."
            }
            (Self::ZhCn, N::NotRepository) => {
                "这个目录不在 Git 仓库中，或 Git 不信任其所有者。请在该目录运行 git status 查看原因。"
            }
            // The configuration file is the one place that fixes this, and its check names the line.
            (Self::EnUs, N::ConfigurationUnavailable) => {
                "Couldn't read the worktree settings. Run orchbee config check."
            }
            (Self::ZhCn, N::ConfigurationUnavailable) => {
                "没能读取工作树设置。请运行 orchbee config check。"
            }
            (Self::EnUs, N::Busy) => "Wait for the current switch to finish.",
            (Self::ZhCn, N::Busy) => "请等待当前切换完成。",
            (Self::EnUs, N::NotSubmitted) => "Couldn't start the worktree action.",
            (Self::ZhCn, N::NotSubmitted) => "没能开始工作树操作。",
            (Self::EnUs, N::Pending) => "Worktree operation is running…",
            (Self::ZhCn, N::Pending) => "工作树操作正在执行…",
            (Self::EnUs, N::Failed | N::SourceChanged) => "Couldn't finish the worktree action.",
            (Self::ZhCn, N::Failed | N::SourceChanged) => "没能完成工作树操作。",
            (Self::EnUs, N::Opened) => "Workspace opened.",
            (Self::ZhCn, N::Opened) => "工作区已打开。",
            (Self::EnUs, N::Removed) => "Worktree removed and Workspace closed. Branch kept.",
            (Self::ZhCn, N::Removed) => "工作树已移除，工作区已关闭，分支已保留。",
            (Self::EnUs, N::EntryPending) => "Workspace opened; connecting its terminal…",
            (Self::ZhCn, N::EntryPending) => "工作区已打开，正在连接终端…",
            (Self::EnUs, N::EntryFailed) => {
                "Couldn't open the terminal. Select the Workspace in the sidebar."
            }
            (Self::ZhCn, N::EntryFailed) => "没能打开终端。请在边栏中选择工作区。",
            // A dialog gives each sentence at most two lines, so the remedy takes two sentences.
            (Self::EnUs, N::ClientInCheckout) => {
                "Orchbee was started inside this worktree, so Windows keeps it in use. Quit Orchbee and leave the folder in that shell. Then remove the worktree from elsewhere."
            }
            (Self::ZhCn, N::ClientInCheckout) => {
                "Orchbee 是在这个工作树里启动的，Windows 会一直占用它。请退出 Orchbee，并在那个 shell 里离开该目录。然后从其他位置移除工作树。"
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
