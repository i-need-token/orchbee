//! Localized Worktree forms, retained receipts and progress hints.
use super::Locale;
use crate::terminal_session::{WorktreeMode, WorktreeNotice, WorktreeStage};
mod removal;

impl Locale {
    pub(crate) fn worktree_cancel_label(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Waiting) => "Hide",
            (Self::ZhCn, 工作树Stage::Waiting) => "隐藏",
            (Self::EnUs, WorktreeStage::Result) => "Close",
            (Self::ZhCn, 工作树Stage::Result) => "关闭",
            (Self::EnUs, _) => "Cancel",
            (Self::ZhCn, _) => "取消",
        }
    }
    pub(crate) fn worktree_submit_label(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Waiting | WorktreeStage::Result) => "Read status",
            (Self::ZhCn, 工作树Stage::Waiting | 工作树Stage::Result) => "查询状态",
            (Self::EnUs, _) => "Create",
            (Self::ZhCn, _) => "创建",
        }
    }
    pub(crate) fn worktree_default_workspace(self, ordinal: usize) -> String {
        match self {
            Self::EnUs => format!("Worktree {ordinal}"),
            Self::ZhCn => format!("工作树 {ordinal}"),
        }
    }
    pub(crate) fn worktree_workspace(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace",
            Self::ZhCn => "工作区",
        }
    }
    pub(crate) fn worktree_title(self, mode: WorktreeMode) -> &'static str {
        match (self, mode) {
            (Self::EnUs, WorktreeMode::Create) => "Create worktree",
            (Self::EnUs, WorktreeMode::Open) => "Open worktree",
            (Self::ZhCn, 工作树Mode::Create) => "创建 worktree",
            (Self::ZhCn, 工作树Mode::Open) => "打开 worktree",
            (Self::EnUs, WorktreeMode::Remove) => "Remove worktree",
            (Self::ZhCn, 工作树Mode::Remove) => "移除 worktree",
        }
    }
    pub(crate) fn worktree_field(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Checkout) => "Choose checkout (exact path)",
            (Self::ZhCn, 工作树Stage::Checkout) => "选择检出目录（精确路径）",
            (Self::EnUs, WorktreeStage::Base) => "Base (empty uses HEAD)",
            (Self::ZhCn, 工作树Stage::Base) => "起点（留空使用 HEAD）",
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
            (Self::EnUs, N::InvalidInput) => "Use a valid branch/base without control characters.",
            (Self::ZhCn, N::InvalidInput) => "请输入有效分支／起点，不得包含控制字符。",
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
            (Self::EnUs, N::ConfigurationUnavailable) => {
                "Could not read worktree configuration. Run orchbee config check."
            }
            (Self::ZhCn, N::ConfigurationUnavailable) => {
                "无法读取 worktree 配置。请运行 orchbee config check。"
            }
            (Self::EnUs, N::Busy) => "Source is busy. Wait for the current transition.",
            (Self::ZhCn, N::Busy) => "来源忙碌。请等待当前切换完成。",
            (Self::EnUs, N::NotSubmitted) => {
                "Worktree was not submitted. Existing data is retained."
            }
            (Self::ZhCn, N::NotSubmitted) => "worktree 操作未提交，已有数据已保留。",
            (Self::EnUs, N::Pending) => "Worktree operation is running…",
            (Self::ZhCn, N::Pending) => "worktree 操作正在执行…",
            (Self::EnUs, N::Unknown) => {
                "Result unknown. Read status before deciding what to do next."
            }
            (Self::ZhCn, N::Unknown) => "结果未知。请先查询状态，再决定下一步操作。",
            (Self::EnUs, N::Failed) => {
                "Operation stopped. Inspect status; checkout and Workspace facts are retained."
            }
            (Self::ZhCn, N::Failed) => "操作已停止。请查询状态；检出目录和工作区信息已保留。",
            (Self::EnUs, N::Opened) => "Workspace opened.",
            (Self::ZhCn, N::Opened) => "工作区已打开。",
            (Self::EnUs, N::Removed) => "Worktree removed and Workspace closed. Branch kept.",
            (Self::ZhCn, N::Removed) => "工作树 已移除，工作区已关闭，分支已保留。",
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
