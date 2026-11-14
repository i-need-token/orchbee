//! Localized Remove consequences, canonical failures and retained recovery facts.
use super::{Locale, WorktreeStage};
use orchbee_workbench_protocol::{
    WorktreeGitError, WorktreeOperationFailure, WorktreeShellRestoration,
    WorktreeWorkspaceDisposition,
};

impl Locale {
    pub(crate) fn worktree_remove_title(self) -> &'static str {
        match self {
            Self::EnUs => "Remove Worktree?",
            Self::ZhCn => "移除工作树？",
        }
    }
    /// What ends and what is deleted, with counts.
    pub(crate) fn worktree_remove_scope(self, terminals: usize) -> String {
        match (self, terminals) {
            (Self::EnUs, 0) => "Closes its Workspace and deletes the checkout.".to_owned(),
            (Self::ZhCn, 0) => "将关闭其工作区，并删除检出目录。".to_owned(),
            (Self::EnUs, 1) => {
                "Ends 1 terminal, closes its Workspace and deletes the checkout.".to_owned()
            }
            (Self::EnUs, terminals) => format!(
                "Ends {terminals} terminals, closes its Workspace and deletes the checkout."
            ),
            (Self::ZhCn, terminals) => {
                format!("将结束 {terminals} 个终端，关闭其工作区，并删除检出目录。")
            }
        }
    }
    /// What is kept, said before the branch name, so a dialog that cuts a long name keeps it.
    pub(crate) fn worktree_remove_branch(self, branch: Option<&str>) -> String {
        let branch = branch.map(|branch| branch.strip_prefix("refs/heads/").unwrap_or(branch));
        match (self, branch) {
            (Self::EnUs, Some(branch)) => format!("Keeps branch {branch}."),
            (Self::ZhCn, Some(branch)) => format!("保留分支 {branch}。"),
            (Self::EnUs, None) => "Keeps existing branches.".to_owned(),
            (Self::ZhCn, None) => "保留已有分支。".to_owned(),
        }
    }
    /// The primary of a removal decision; a retained failure first offers its resume step.
    pub(crate) fn worktree_remove_submit(self, stage: WorktreeStage) -> &'static str {
        match (self, stage) {
            (Self::EnUs, WorktreeStage::Remove | WorktreeStage::ResumeRemove) => "Remove",
            (Self::ZhCn, WorktreeStage::Remove | WorktreeStage::ResumeRemove) => "移除",
            (Self::EnUs, WorktreeStage::AbandonRemove) => "Stop Tracking",
            (Self::ZhCn, WorktreeStage::AbandonRemove) => "停止跟踪",
            (Self::EnUs, _) => "Remove Worktree…",
            (Self::ZhCn, _) => "移除工作树…",
        }
    }
    pub(crate) fn worktree_remove_failure(self, reason: &WorktreeOperationFailure) -> &'static str {
        use WorktreeOperationFailure as F;
        match (self, reason) {
            (_, F::LaunchRequestTooLarge {}) => self.launch_request_too_large(),
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::Dirty,
                    ..
                },
            ) => "Checkout has changes or untracked files. Commit, stash or clean them.",
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::Dirty,
                    ..
                },
            ) => "检出目录含修改或未跟踪文件。请先提交、贮藏或清理。",
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
                "Couldn't close every terminal. Check its terminals, pinned Tabs and Tab groups."
            }
            (Self::ZhCn, F::CloseIncomplete { .. }) => {
                "没能关闭所有终端。请检查其中的终端、固定标签页和标签页组。"
            }
            (Self::EnUs, F::SourceChanged { .. }) => {
                "The checkout or Workspace changed. Check it before you continue."
            }
            (Self::ZhCn, F::SourceChanged { .. }) => "检出目录或工作区已变化。请检查后再继续。",
            (_, F::PhysicalUnknown { .. } | F::TransportUnknown { .. }) => self.outcome_unknown(),
            (
                Self::EnUs,
                F::WorkerUnavailable { .. } | F::WorkerBusy { .. } | F::WorkerCapacity { .. },
            ) => "Orchbee is unavailable or busy. Try again later.",
            (
                Self::ZhCn,
                F::WorkerUnavailable { .. } | F::WorkerBusy { .. } | F::WorkerCapacity { .. },
            ) => "Orchbee 不可用或正忙。请稍后再试。",
            (Self::EnUs, F::Unsupported { .. }) => {
                "Orchbee on this machine doesn't support removing worktrees."
            }
            (Self::ZhCn, F::Unsupported { .. }) => "这台机器上的 Orchbee 不支持移除工作树。",
            (Self::EnUs, F::SessionStopping { .. }) => {
                "The Session is stopping. Continue after it restarts."
            }
            (Self::ZhCn, F::SessionStopping { .. }) => "会话正在停止。请在它重新启动后继续。",
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::CheckoutNotFound,
                    ..
                },
            ) => {
                "Git no longer lists this checkout as a worktree. Inspect the directory and git worktree list."
            }
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::CheckoutNotFound,
                    ..
                },
            ) => "Git 已不再将此检出登记为工作树。请检查该目录和 git worktree list。",
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::Io,
                    ..
                },
            ) => "The checkout directory is in use or cannot be deleted. Close programs using it.",
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::Io,
                    ..
                },
            ) => "检出目录正被占用或无法删除。请关闭占用它的程序。",
            (
                Self::EnUs,
                F::Git {
                    error: WorktreeGitError::GitFailed,
                    ..
                },
            ) => {
                "Git could not remove the checkout. If a program is using its directory, close it."
            }
            (
                Self::ZhCn,
                F::Git {
                    error: WorktreeGitError::GitFailed,
                    ..
                },
            ) => "Git 未能移除检出目录。如有程序正在使用该目录，请先关闭。",
            (Self::EnUs, F::Git { .. } | F::AssociationFailed { .. } | F::Internal { .. }) => {
                "Couldn't remove the worktree."
            }
            (Self::ZhCn, F::Git { .. } | F::AssociationFailed { .. } | F::Internal { .. }) => {
                "没能移除工作树。"
            }
        }
    }
    /// The button and confirmation for forgetting a retained removal in this Client only.
    pub(crate) fn worktree_remove_abandon_label(self) -> &'static str {
        match self {
            Self::EnUs => "Stop Tracking…",
            Self::ZhCn => "停止跟踪…",
        }
    }
    pub(crate) fn worktree_remove_abandon_title(self) -> &'static str {
        match self {
            Self::EnUs => "Stop Tracking Removal?",
            Self::ZhCn => "停止跟踪移除？",
        }
    }
    pub(crate) fn worktree_remove_abandon_notice(self) -> &'static str {
        match self {
            Self::EnUs => "The removal continues; only this window stops tracking it.",
            Self::ZhCn => "移除会继续进行，只是此窗口不再跟踪。",
        }
    }
    /// A failed removal that cannot resume can still hold its Workspace closing.
    pub(crate) fn worktree_remove_held_close(self) -> &'static str {
        match self {
            Self::EnUs => "The Workspace is still closing. Close it from its menu in the sidebar.",
            Self::ZhCn => "工作区仍在关闭中。请在边栏中通过它的菜单关闭。",
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
    /// How restarting the checkout's shells went: said only when some did not start; counts stay
    /// out of the copy.
    pub(crate) fn worktree_remove_restoration(
        self,
        receipt: &WorktreeShellRestoration,
    ) -> Option<String> {
        let mut text = if receipt.unknown > 0 {
            self.outcome_unknown().to_owned()
        } else if receipt.failed > 0 || receipt.started < receipt.total {
            self.say("Couldn't restart every shell.", "没能重新启动所有 Shell。")
        } else {
            return None;
        };
        if matches!(
            receipt.failure,
            Some(WorktreeOperationFailure::LaunchRequestTooLarge {})
        ) {
            text.push(' ');
            text.push_str(self.launch_request_too_large());
        }
        Some(text)
    }
}
