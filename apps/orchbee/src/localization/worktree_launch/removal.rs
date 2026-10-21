//! Localized Remove consequences, canonical failures and retained recovery facts.
use super::{Locale, WorktreeStage};
use orchbee_workbench_protocol::{
    WorktreeGitError, WorktreeOperationFailure, WorktreeShellRestoration,
    WorktreeWorkspaceDisposition,
};

impl Locale {
    pub(crate) fn worktree_remove_title(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("Remove worktree: {label}"),
            Self::ZhCn => format!("移除工作树：{label}"),
        }
    }
    pub(crate) fn worktree_remove_scope(self, terminals: usize) -> String {
        match self {
            Self::EnUs => format!(
                "Remove this checkout and close its Workspace and all {terminals} terminals."
            ),
            Self::ZhCn => format!("移除此检出目录，并关闭对应工作区及其全部 {terminals} 个终端。"),
        }
    }
    pub(crate) fn worktree_remove_branch(self, branch: Option<&str>) -> String {
        let branch = branch.map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch));
        match (self, branch) {
            (Self::EnUs, Some(branch)) => format!("Branch kept: {branch}"),
            (Self::ZhCn, Some(branch)) => format!("保留分支：{branch}"),
            (Self::EnUs, None) => "Detached checkout; existing branches are kept.".to_owned(),
            (Self::ZhCn, None) => "此检出未附着分支，已有分支均会保留。".to_owned(),
        }
    }
    pub(crate) fn worktree_remove_submit(
        self,
        stage: WorktreeStage,
        resumable: bool,
    ) -> &'static str {
        match (self, stage, resumable) {
            (Self::EnUs, WorktreeStage::Remove, _) => "Remove",
            (Self::ZhCn, WorktreeStage::Remove, _) => "移除",
            (Self::EnUs, WorktreeStage::ResumeRemove, _) => "Resume removal",
            (Self::ZhCn, WorktreeStage::ResumeRemove, _) => "恢复移除",
            (Self::EnUs, _, true) => "Resume removal…",
            (Self::ZhCn, _, true) => "恢复移除…",
            (Self::EnUs, _, false) => "Read status",
            (Self::ZhCn, _, false) => "查询状态",
        }
    }
    pub(crate) fn worktree_remove_failure(self, reason: &WorktreeOperationFailure) -> &'static str {
        use WorktreeOperationFailure as F;
        match (self, reason) {
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::Dirty,
                    ..
                },
            ) => {
                "Checkout has changes or untracked files. Commit, stash or clean them, then resume."
            }
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::Dirty,
                    ..
                },
            ) => "检出目录含修改或未跟踪文件。请先提交、贮藏或清理，再恢复移除。",
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::MainCheckout,
                    ..
                },
            ) => "The main checkout is protected and cannot be removed.",
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::MainCheckout,
                    ..
                },
            ) => "主检出受保护，无法移除。",
            (Self::EnUs, F::CheckoutInUse { .. }) => {
                "Another Workspace uses this checkout. Close that Workspace first."
            }
            (Self::ZhCn, F::CheckoutInUse { .. }) => {
                "其他工作区仍在使用此检出目录。请先关闭对应工作区。"
            }
            (Self::EnUs, F::CloseIncomplete { .. }) => {
                "Terminals could not all close. Check child terminals, pinned Tabs and Tab groups."
            }
            (Self::ZhCn, F::CloseIncomplete { .. }) => {
                "终端未能全部关闭。请检查子终端、固定标签页和标签组。"
            }
            (Self::EnUs, F::SourceChanged { .. }) => {
                "The checkout or Workspace changed. Read status before continuing."
            }
            (Self::ZhCn, F::SourceChanged { .. }) => "检出目录或工作区已变化。请先查询状态。",
            (Self::EnUs, F::PhysicalUnknown { .. } | F::TransportUnknown { .. }) => {
                "Removal outcome is unknown. Read status; do not start another removal."
            }
            (Self::ZhCn, F::PhysicalUnknown { .. } | F::TransportUnknown { .. }) => {
                "移除结果未知。请查询状态，不要重新发起移除。"
            }
            (
                Self::EnUs,
                F::WorkerUnavailable { .. } | F::WorkerBusy { .. } | F::WorkerCapacity { .. },
            ) => "The Worker is unavailable or busy. Read status, then resume when offered.",
            (
                Self::ZhCn,
                F::WorkerUnavailable { .. } | F::WorkerBusy { .. } | F::WorkerCapacity { .. },
            ) => "Worker 不可用或忙碌。请查询状态，在允许时恢复移除。",
            (Self::EnUs, F::Unsupported { .. }) => "This Worker does not support worktree removal.",
            (Self::ZhCn, F::Unsupported { .. }) => "此 Worker 不支持移除工作树。",
            (Self::EnUs, F::SiteStopping { .. }) => {
                "The Session is stopping. Read status before continuing."
            }
            (Self::ZhCn, F::SiteStopping { .. }) => "会话正在停止。请先查询状态。",
            (Self::EnUs, F::Git { .. }) => {
                "Git refused removal. Read status; known checkout and Workspace facts are retained."
            }
            (Self::ZhCn, F::Git { .. }) => {
                "Git 拒绝移除。请查询状态；已知检出目录和工作区信息已保留。"
            }
            (Self::EnUs, F::AssociationFailed { .. } | F::Internal { .. }) => {
                "Removal could not finish. Read status before continuing."
            }
            (Self::ZhCn, F::AssociationFailed { .. } | F::Internal { .. }) => {
                "移除未能完成。请先查询状态。"
            }
        }
    }
    pub(crate) fn worktree_remove_workspace_state(
        self,
        state: &WorktreeWorkspaceDisposition,
    ) -> &'static str {
        match (self, state) {
            (Self::EnUs, WorktreeWorkspaceDisposition::Open { .. }) => "Workspace is retained.",
            (Self::ZhCn, WorktreeWorkspaceDisposition::Open { .. }) => "工作区已保留。",
            (Self::EnUs, WorktreeWorkspaceDisposition::Closing { .. }) => {
                "Workspace closure is held until removal settles."
            }
            (Self::ZhCn, WorktreeWorkspaceDisposition::Closing { .. }) => {
                "工作区关闭正在等待移除结果。"
            }
            (Self::EnUs, WorktreeWorkspaceDisposition::Closed { .. }) => "Workspace is closed.",
            (Self::ZhCn, WorktreeWorkspaceDisposition::Closed { .. }) => "工作区已关闭。",
            (Self::EnUs, WorktreeWorkspaceDisposition::Unassociated { .. }) => {
                "Workspace state has not been observed."
            }
            (Self::ZhCn, WorktreeWorkspaceDisposition::Unassociated { .. }) => {
                "尚未观测到工作区状态。"
            }
        }
    }
    pub(crate) fn worktree_remove_restoration(self, receipt: &WorktreeShellRestoration) -> String {
        let WorktreeShellRestoration {
            total,
            started,
            failed,
            unknown,
            ..
        } = receipt;
        match self {
            Self::EnUs => format!(
                "Shell recovery: {started}/{total} started, {failed} failed, {unknown} unknown."
            ),
            Self::ZhCn => {
                format!("Shell 恢复：已启动 {started}/{total}，失败 {failed}，未知 {unknown}。")
            }
        }
    }
}
