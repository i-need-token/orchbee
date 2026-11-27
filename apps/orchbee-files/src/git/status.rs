//! One repository's status: `git status --porcelain=v2 -z --branch -uall` read as it streams and
//! stopped at the entry past the limit, the counts the Source Control groups show, and the
//! operation in progress, which Git records only as files in the repository's Git directory.

use super::{
    command::Access,
    discovery::Found,
    errors::{Classifier, GitError},
    output::GitOutput,
    porcelain::{Entry, EntryKind, Progress, StatusReader},
    runner::{self, Flow, Limits},
};
use orchbee_files_protocol::{ErrorCode, RepositoryOperation};
use std::{path::Path, sync::Mutex};

/// How long one status may run; a newer refresh cancels it sooner.
pub(super) const DEADLINE: std::time::Duration = std::time::Duration::from_secs(120);

/// VS Code's limit: past it the status is truncated and only explicit refreshes run.
pub(super) const ENTRY_LIMIT: usize = 10_000;

/// The branch HEAD names.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Head {
    pub(super) branch: Option<String>,
    pub(super) commit: Option<String>,
    pub(super) detached: bool,
    pub(super) unborn: bool,
}

/// The branch HEAD tracks. Ahead and behind are absent when the tracked branch is gone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Upstream {
    pub(super) name: String,
    pub(super) divergence: Option<(u64, u64)>,
}

/// The entries in each Source Control group, as VS Code groups porcelain letters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
    pub(super) merge: usize,
    pub(super) index: usize,
    pub(super) working_tree: usize,
    pub(super) untracked: usize,
}

/// One completed status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    pub(super) head: Head,
    pub(super) upstream: Option<Upstream>,
    pub(super) operation: RepositoryOperation,
    pub(super) counts: Counts,
    pub(super) entries: Vec<Entry>,
    pub(super) truncated: bool,
}

/// Runs status in `repository`, keeping at most `limit` entries.
pub(super) fn status(
    repository: &Found,
    limit: usize,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Snapshot, GitError> {
    let mut command = repository.command(Access::Read, None);
    command.args(["status", "--porcelain=v2", "-z", "--branch", "-uall"]);
    let mut reader = StatusReader::new(limit);
    runner::stream(command, Vec::new(), limits, output, |bytes| {
        match reader.read(bytes) {
            Ok(Progress::More) => Flow::More,
            Ok(Progress::Full) => Flow::Enough,
            Err(_) => Flow::Malformed,
        }
    })?
    .checked(Classifier::General)?;
    let status = reader
        .finish()
        .map_err(|_| GitError::new(ErrorCode::Failed))?;
    let branch = status.branch;
    let head = Head {
        detached: branch.head.is_none(),
        unborn: branch.commit.is_none(),
        branch: branch.head,
        commit: branch.commit,
    };
    let upstream = branch.upstream.map(|name| Upstream {
        name,
        divergence: branch.divergence,
    });
    Ok(Snapshot {
        head,
        upstream,
        operation: operation(&repository.git_dir),
        counts: counts(&status.entries),
        entries: status.entries,
        truncated: status.truncated,
    })
}

/// VS Code's grouping: an unmerged pair is a merge change and nothing else; otherwise the index
/// letter and the working tree letter each count in their own group.
pub(super) fn counts(entries: &[Entry]) -> Counts {
    let mut counts = Counts::default();
    for entry in entries {
        match entry.kind {
            EntryKind::Untracked => counts.untracked += 1,
            EntryKind::Ignored => {}
            EntryKind::Unmerged => counts.merge += 1,
            EntryKind::Changed | EntryKind::Renamed => {
                if b"MADRC".contains(&entry.x) {
                    counts.index += 1;
                }
                if b"MDART".contains(&entry.y) {
                    counts.working_tree += 1;
                }
            }
        }
    }
    counts
}

/// The operation the files in `git_dir` record; a rebase first, since one also leaves the files of
/// the pick it stopped at.
fn operation(git_dir: &Path) -> RepositoryOperation {
    let exists = |name: &str| git_dir.join(name).symlink_metadata().is_ok();
    if exists("rebase-merge") || exists("rebase-apply") {
        RepositoryOperation::Rebase
    } else if exists("MERGE_HEAD") {
        RepositoryOperation::Merge
    } else if exists("CHERRY_PICK_HEAD") {
        RepositoryOperation::CherryPick
    } else if exists("REVERT_HEAD") {
        RepositoryOperation::Revert
    } else {
        RepositoryOperation::None
    }
}
