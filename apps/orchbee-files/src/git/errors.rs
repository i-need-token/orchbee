//! The errors Git requests answer: the wire code, and for a failed Git command the `gitErrorCode`
//! VS Code's Git extension would give it at ef5ca45, classified from what Git wrote. A specific
//! classification of the command that failed wins over the general one, as there.

use orchbee_files_protocol::{ErrorCode, FilesError, Text, Token};

/// An error answer. `detail` is Git's own standard error, at most 4 KiB of its end, shown verbatim
/// and never logged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GitError {
    pub(super) code: ErrorCode,
    pub(super) git_error_code: Option<GitErrorCode>,
    /// Why an `unavailable` answer came: `git-missing`, `git-too-old` or `busy`.
    pub(super) reason: Option<&'static str>,
    pub(super) detail: Option<String>,
}

/// The most of Git's standard error an answer carries.
pub(super) const DETAIL_LIMIT: usize = 4 * 1024;

impl GitError {
    pub(super) const fn new(code: ErrorCode) -> Self {
        Self {
            code,
            git_error_code: None,
            reason: None,
            detail: None,
        }
    }

    pub(super) const fn unavailable(reason: &'static str) -> Self {
        Self {
            code: ErrorCode::Unavailable,
            git_error_code: None,
            reason: Some(reason),
            detail: None,
        }
    }

    /// A Git command that ran and failed: `git-failed` with its classification, or
    /// `unsafe-repository` when Git refused the repository's owner.
    pub(super) fn failed(classifier: Classifier, stdout: &[u8], stderr: &[u8]) -> Self {
        let git_error_code = classify(classifier, stdout, stderr);
        let code = if git_error_code == Some(GitErrorCode::NotASafeGitRepository) {
            ErrorCode::UnsafeRepository
        } else {
            ErrorCode::GitFailed
        };
        Self {
            code,
            git_error_code,
            reason: None,
            detail: detail(stderr),
        }
    }

    /// The error a request answers with on the wire.
    pub(super) fn into_wire(self) -> FilesError {
        let reason = self.reason.and_then(|reason| Token::new(reason).ok());
        let mut files = match (self.code, reason) {
            (ErrorCode::Unavailable, Some(reason)) => FilesError::unavailable(reason),
            (ErrorCode::Unavailable, None) => FilesError::new(ErrorCode::Failed),
            (code, _) => FilesError::new(code),
        };
        files.git_error_code = self
            .git_error_code
            .and_then(|code| Token::new(code.as_str()).ok());
        files.detail = self.detail.and_then(|detail| Text::new(detail).ok());
        files
    }

    pub(super) const fn git(git_error_code: GitErrorCode) -> Self {
        Self {
            code: ErrorCode::GitFailed,
            git_error_code: Some(git_error_code),
            reason: None,
            detail: None,
        }
    }
}

/// The end of `stderr` as text, at most [`DETAIL_LIMIT`] bytes and never splitting a character.
pub(super) fn detail(stderr: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim_end();
    if text.is_empty() {
        return None;
    }
    let mut start = text.len().saturating_sub(DETAIL_LIMIT);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text.get(start..).map(str::to_owned)
}

/// VS Code's `GitErrorCodes` this engine can meet, spelled in kebab case on the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GitErrorCode {
    AuthenticationFailed,
    BadConfigFile,
    BranchAlreadyExists,
    BranchFastForwardRejected,
    BranchNotFullyMerged,
    BranchNotYetBorn,
    CantAccessRemote,
    CantCreatePipe,
    CantLockRef,
    CantRebaseMultipleBranches,
    Conflict,
    DirtyWorkTree,
    EmptyCommitMessage,
    ForcePushWithLeaseIfIncludesRejected,
    ForcePushWithLeaseRejected,
    InvalidBranchName,
    IsInSubmodule,
    NoPathFound,
    NoRemoteReference,
    NoRemoteRepositorySpecified,
    NoUpstreamBranch,
    NoUserEmailConfigured,
    NoUserNameConfigured,
    NotAGitRepository,
    NotASafeGitRepository,
    /// Not VS Code's: standard output passed this engine's bound.
    OutputTooLarge,
    PermissionDenied,
    PushRejected,
    RemoteConnectionError,
    RepositoryIsLocked,
    RepositoryNotFound,
    TagConflict,
    UnmergedChanges,
    WorktreeAlreadyExists,
    WorktreeBranchAlreadyUsed,
    WorktreeContainsChanges,
}

impl GitErrorCode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticationFailed => "authentication-failed",
            Self::BadConfigFile => "bad-config-file",
            Self::BranchAlreadyExists => "branch-already-exists",
            Self::BranchFastForwardRejected => "branch-fast-forward-rejected",
            Self::BranchNotFullyMerged => "branch-not-fully-merged",
            Self::BranchNotYetBorn => "branch-not-yet-born",
            Self::CantAccessRemote => "cant-access-remote",
            Self::CantCreatePipe => "cant-create-pipe",
            Self::CantLockRef => "cant-lock-ref",
            Self::CantRebaseMultipleBranches => "cant-rebase-multiple-branches",
            Self::Conflict => "conflict",
            Self::DirtyWorkTree => "dirty-work-tree",
            Self::EmptyCommitMessage => "empty-commit-message",
            Self::ForcePushWithLeaseIfIncludesRejected => {
                "force-push-with-lease-if-includes-rejected"
            }
            Self::ForcePushWithLeaseRejected => "force-push-with-lease-rejected",
            Self::InvalidBranchName => "invalid-branch-name",
            Self::IsInSubmodule => "is-in-submodule",
            Self::NoPathFound => "no-path-found",
            Self::NoRemoteReference => "no-remote-reference",
            Self::NoRemoteRepositorySpecified => "no-remote-repository-specified",
            Self::NoUpstreamBranch => "no-upstream-branch",
            Self::NoUserEmailConfigured => "no-user-email-configured",
            Self::NoUserNameConfigured => "no-user-name-configured",
            Self::NotAGitRepository => "not-a-git-repository",
            Self::NotASafeGitRepository => "not-a-safe-git-repository",
            Self::OutputTooLarge => "output-too-large",
            Self::PermissionDenied => "permission-denied",
            Self::PushRejected => "push-rejected",
            Self::RemoteConnectionError => "remote-connection-error",
            Self::RepositoryIsLocked => "repository-is-locked",
            Self::RepositoryNotFound => "repository-not-found",
            Self::TagConflict => "tag-conflict",
            Self::UnmergedChanges => "unmerged-changes",
            Self::WorktreeAlreadyExists => "worktree-already-exists",
            Self::WorktreeBranchAlreadyUsed => "worktree-branch-already-used",
            Self::WorktreeContainsChanges => "worktree-contains-changes",
        }
    }
}

/// Which command failed, for the classifications VS Code applies to that command alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Classifier {
    General,
    Checkout,
    Commit,
    Fetch,
    Pull,
    Push(PushMode),
}

/// How a push overrides the remote, which decides how its rejection reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PushMode {
    None,
    /// `--force-with-lease`.
    Lease,
    /// `--force-with-lease --force-if-includes`, which Git 2.30 and later accept.
    LeaseIfIncludes,
    Force,
}

/// The classification of a failed command's output.
pub(super) fn classify(
    classifier: Classifier,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<GitErrorCode> {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    specific(classifier, &stdout, &stderr).or_else(|| general(&stderr))
}

fn general(stderr: &str) -> Option<GitErrorCode> {
    use GitErrorCode as C;
    let rules: [(bool, GitErrorCode); 16] = [
        (
            any(
                stderr,
                &[
                    "Another git process seems to be running in this repository",
                    "If no other git process is currently running",
                ],
            ),
            C::RepositoryIsLocked,
        ),
        (
            folded(stderr, "authentication failed"),
            C::AuthenticationFailed,
        ),
        (folded(stderr, "not a git repository"), C::NotAGitRepository),
        (stderr.contains("bad config file"), C::BadConfigFile),
        (
            any(
                stderr,
                &[
                    "cannot make pipe for command substitution",
                    "cannot create standard input pipe",
                ],
            ),
            C::CantCreatePipe,
        ),
        (
            stderr.contains("Repository not found"),
            C::RepositoryNotFound,
        ),
        (stderr.contains("unable to access"), C::CantAccessRemote),
        (
            quoted_between(stderr, "branch '", "' is not fully merged"),
            C::BranchNotFullyMerged,
        ),
        (
            stderr.contains("Couldn't find remote ref"),
            C::NoRemoteReference,
        ),
        (
            quoted_between(stderr, "A branch named '", "' already exists"),
            C::BranchAlreadyExists,
        ),
        (
            quoted_between(stderr, "'", "' is not a valid branch name"),
            C::InvalidBranchName,
        ),
        (commit_or_stash(stderr), C::DirtyWorkTree),
        (
            stderr.contains("detected dubious ownership in repository at"),
            C::NotASafeGitRepository,
        ),
        (
            any(
                stderr,
                &[
                    "contains modified or untracked files",
                    "use --force to delete it",
                ],
            ),
            C::WorktreeContainsChanges,
        ),
        (
            stderr.lines().any(|line| {
                line.match_indices("fatal: '").any(|(at, needle)| {
                    line.get(at + needle.len()..)
                        .and_then(|rest| rest.split_once('\''))
                        .is_some_and(|(name, rest)| {
                            !name.is_empty() && rest.starts_with(" already exists")
                        })
                })
            }),
            C::WorktreeAlreadyExists,
        ),
        (
            stderr.contains("is already used by worktree at"),
            C::WorktreeBranchAlreadyUsed,
        ),
    ];
    rules
        .into_iter()
        .find_map(|(matched, code)| matched.then_some(code))
}

fn specific(classifier: Classifier, stdout: &str, stderr: &str) -> Option<GitErrorCode> {
    use GitErrorCode as C;
    let rules: Vec<(bool, GitErrorCode)> = match classifier {
        Classifier::General => Vec::new(),
        Classifier::Checkout => vec![
            (commit_or_stash(stderr), C::DirtyWorkTree),
            (
                stderr.contains("You are on a branch yet to be born"),
                C::BranchNotYetBorn,
            ),
        ],
        Classifier::Commit => vec![
            (
                stderr.contains("not possible because you have unmerged files"),
                C::UnmergedChanges,
            ),
            (
                stderr.contains("Aborting commit due to empty commit message"),
                C::EmptyCommitMessage,
            ),
        ],
        Classifier::Fetch => vec![
            (
                stderr.contains("No remote repository specified."),
                C::NoRemoteRepositorySpecified,
            ),
            (
                stderr.contains("Could not read from remote repository"),
                C::RemoteConnectionError,
            ),
            (
                rejected(stderr, "(non-fast-forward)"),
                C::BranchFastForwardRejected,
            ),
        ],
        Classifier::Pull => vec![
            (
                stdout.lines().any(|line| {
                    line.strip_prefix("CONFLICT (")
                        .and_then(|rest| rest.split_once("): "))
                        .is_some_and(|(kind, rest)| {
                            !kind.is_empty() && !kind.contains(')') && starts_with_word(rest)
                        })
                }),
                C::Conflict,
            ),
            (
                stderr.contains("Please tell me who you are."),
                C::NoUserNameConfigured,
            ),
            (
                stderr.contains("Could not read from remote repository"),
                C::RemoteConnectionError,
            ),
            (
                [
                    "pull is not possible because you have unmerged files",
                    "pulling is not possible because you have unmerged files",
                    "cannot pull with rebase: you have unstaged changes",
                    "your local changes to the following files would be overwritten",
                    "please, commit your changes before you can merge",
                ]
                .iter()
                .any(|text| folded(stderr, text)),
                C::DirtyWorkTree,
            ),
            (
                folded(stderr, "cannot lock ref") || folded(stderr, "unable to update local ref"),
                C::CantLockRef,
            ),
            (
                folded(stderr, "cannot rebase onto multiple branches"),
                C::CantRebaseMultipleBranches,
            ),
            (
                stderr.lines().any(|line| {
                    line.starts_with(" ! [rejected]")
                        && line.ends_with("(would clobber existing tag)")
                }),
                C::TagConflict,
            ),
        ],
        Classifier::Push(force) => {
            let refused = stderr.lines().any(|line| {
                line.strip_prefix("error: failed to push some refs to")
                    .is_some_and(|rest| !starts_with_word(rest))
            });
            vec![
                (
                    refused && force == PushMode::Lease && rejected(stderr, "(stale info)"),
                    C::ForcePushWithLeaseRejected,
                ),
                (
                    refused
                        && force == PushMode::LeaseIfIncludes
                        && rejected(stderr, "(remote ref updated since checkout)"),
                    C::ForcePushWithLeaseIfIncludesRejected,
                ),
                (refused, C::PushRejected),
                (
                    stderr.lines().any(|line| {
                        line.split_once("Permission")
                            .is_some_and(|(_, rest)| rest.contains("denied"))
                    }),
                    C::PermissionDenied,
                ),
                (
                    stderr.contains("Could not read from remote repository"),
                    C::RemoteConnectionError,
                ),
                (
                    stderr
                        .lines()
                        .next()
                        .and_then(|line| line.strip_prefix("fatal: The current branch "))
                        .is_some_and(|rest| rest.contains(" has no upstream branch")),
                    C::NoUpstreamBranch,
                ),
            ]
        }
    };
    rules
        .into_iter()
        .find_map(|(matched, code)| matched.then_some(code))
}

fn any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

/// Whether `text` holds `needle`, written in lowercase, ignoring ASCII case.
fn folded(text: &str, needle: &str) -> bool {
    text.to_ascii_lowercase().contains(needle)
}

/// `Please commit your changes or stash them`, with or without the comma.
fn commit_or_stash(stderr: &str) -> bool {
    any(
        stderr,
        &[
            "Please commit your changes or stash them",
            "Please, commit your changes or stash them",
        ],
    )
}

/// A line where `after` follows `before` with at least one character between them.
fn quoted_between(stderr: &str, before: &str, after: &str) -> bool {
    stderr.lines().any(|line| {
        line.match_indices(before).any(|(at, _)| {
            line.get(at + before.len()..)
                .and_then(|rest| rest.rfind(after))
                .is_some_and(|length| length > 0)
        })
    })
}

/// Whether `text` starts with a character a regular expression's `` counts as a word's.
fn starts_with_word(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
}

/// A `! [rejected]` line that later names `reason`.
fn rejected(stderr: &str, reason: &str) -> bool {
    stderr.lines().any(|line| {
        line.split_once("! [rejected]")
            .is_some_and(|(_, rest)| rest.contains(reason))
    })
}

#[cfg(test)]
mod tests;
