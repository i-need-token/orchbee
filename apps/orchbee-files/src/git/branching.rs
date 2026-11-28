//! The writes that move branches, HEAD and the stash, each as VS Code's Git extension runs it at
//! ef5ca45: checkout; creating, deleting, renaming and tracking branches; merging, rebasing and
//! cherry-picking, and aborting each; undoing the last commit; stashing and applying, popping,
//! dropping and clearing stashes; creating and deleting tags; adding and removing remotes; and
//! deleting a branch or a tag on a remote. A command that meets another process's lock is retried
//! as the other writes are.
//!
//! Three choices go beyond VS Code. A checkout ends its revision with `--`, so a name that is no
//! revision fails instead of checking out a file of that name. An undo names the commit HEAD must
//! still be, and applying, popping or dropping a stash names the commit its entry must still be;
//! when another process moved either, the write answers `stale-revision` and changes nothing.
//! A remote branch or tag is deleted by its full reference name, which cannot match the other kind.

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode},
    mutations::{Context, retried},
    remote,
    runner::{self, Ran},
};
use orchbee_files_protocol::ErrorCode;
use std::time::Duration;

/// Checkouts, branch writes, aborts and stash writes may run a `post-checkout` hook.
const WORKING_TREE_DEADLINE: Duration = Duration::from_secs(10 * 60);
/// Merges, rebases and cherry-picks may run commit hooks.
const MERGE_DEADLINE: Duration = Duration::from_secs(30 * 60);
/// Undoing a commit, tags and remotes write a few references.
const REFERENCE_DEADLINE: Duration = Duration::from_secs(60);
/// Git's answer to a stash push with nothing to stash, which exits successfully.
const NOTHING_TO_STASH: &str = "No local changes to save";

/// The operation an abort ends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Abort {
    Merge,
    Rebase,
    CherryPick,
}

/// A write that moves branches, HEAD or the stash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Branching {
    /// `git.checkout`: a branch, a remote branch to track, or a detached commit.
    Checkout {
        revision: String,
        detached: bool,
        track: bool,
    },
    /// `git.branch-create` at `start` or HEAD, checked out when `checkout` says so.
    BranchCreate {
        name: String,
        start: Option<String>,
        checkout: bool,
    },
    BranchDelete {
        name: String,
        force: bool,
    },
    /// `git.branch-rename` of the current branch.
    BranchRename {
        name: String,
    },
    BranchUpstream {
        name: String,
        upstream: String,
    },
    BranchDeleteRemote {
        remote: String,
        name: String,
    },
    Merge {
        revision: String,
    },
    Rebase {
        revision: String,
    },
    CherryPick {
        revision: String,
    },
    Abort(Abort),
    /// `git.undo-commit` of `commit`, which HEAD must still name.
    UndoCommit {
        commit: String,
    },
    StashPush {
        untracked: bool,
        staged: bool,
        message: Option<String>,
    },
    /// `git.stash-apply` or `git.stash-pop` of `stash@{index}`, which must still be `commit`.
    StashApply {
        index: u64,
        commit: String,
        pop: bool,
        staged: bool,
    },
    StashDrop {
        index: u64,
        commit: String,
    },
    StashClear,
    /// `git.tag-create` at `target` or HEAD; a message makes it annotated.
    TagCreate {
        name: String,
        message: Option<String>,
        target: Option<String>,
    },
    TagDelete {
        name: String,
    },
    TagDeleteRemote {
        remote: String,
        name: String,
    },
    RemoteAdd {
        name: String,
        url: String,
    },
    RemoteRemove {
        name: String,
    },
}

impl Branching {
    /// Whether it talks to a remote, so its commands may ask for credentials.
    pub(super) const fn prompts(&self) -> bool {
        matches!(
            self,
            Self::BranchDeleteRemote { .. } | Self::TagDeleteRemote { .. }
        )
    }

    /// The kind of command for the diagnostic log, the deadline, and the idle limit of a command
    /// that talks to a remote.
    pub(super) const fn limits(&self) -> (&'static str, Duration, Option<Duration>) {
        match self {
            Self::Checkout { .. } => ("checkout", WORKING_TREE_DEADLINE, None),
            Self::BranchCreate { .. }
            | Self::BranchDelete { .. }
            | Self::BranchRename { .. }
            | Self::BranchUpstream { .. } => ("branch", WORKING_TREE_DEADLINE, None),
            Self::BranchDeleteRemote { .. } | Self::TagDeleteRemote { .. } => {
                ("push", remote::DEADLINE, Some(remote::IDLE))
            }
            Self::Merge { .. } => ("merge", MERGE_DEADLINE, None),
            Self::Rebase { .. } => ("rebase", MERGE_DEADLINE, None),
            Self::CherryPick { .. } => ("cherry-pick", MERGE_DEADLINE, None),
            Self::Abort(_) => ("abort", MERGE_DEADLINE, None),
            Self::UndoCommit { .. } => ("undo-commit", REFERENCE_DEADLINE, None),
            Self::StashPush { .. }
            | Self::StashApply { .. }
            | Self::StashDrop { .. }
            | Self::StashClear => ("stash", WORKING_TREE_DEADLINE, None),
            Self::TagCreate { .. } | Self::TagDelete { .. } => ("tag", REFERENCE_DEADLINE, None),
            Self::RemoteAdd { .. } | Self::RemoteRemove { .. } => {
                ("remote", REFERENCE_DEADLINE, None)
            }
        }
    }
}

/// Runs `branching` in the repository.
pub(super) fn execute(
    found: &Found,
    branching: &Branching,
    context: &Context<'_>,
) -> Result<(), GitError> {
    match branching {
        Branching::Checkout {
            revision,
            detached,
            track,
        } => {
            let mut arguments = vec!["checkout", "-q"];
            if *track {
                arguments.push("--track");
            }
            if *detached {
                arguments.push("--detach");
            }
            arguments.extend([name(revision)?, "--"]);
            write(found, &arguments, Classifier::Checkout, context)
        }
        Branching::BranchCreate {
            name: branch,
            start,
            checkout,
        } => {
            let mut arguments = if *checkout {
                vec!["checkout", "-q", "-b", name(branch)?, "--no-track"]
            } else {
                vec!["branch", "-q", name(branch)?]
            };
            if let Some(start) = start {
                arguments.push(name(start)?);
            }
            write(found, &arguments, Classifier::Branch, context)
        }
        Branching::BranchDelete {
            name: branch,
            force,
        } => {
            let flag = if *force { "-D" } else { "-d" };
            write(
                found,
                &["branch", flag, name(branch)?],
                Classifier::General,
                context,
            )
        }
        Branching::BranchRename { name: branch } => write(
            found,
            &["branch", "-m", name(branch)?],
            Classifier::Branch,
            context,
        ),
        Branching::BranchUpstream {
            name: branch,
            upstream,
        } => write(
            found,
            &[
                "branch",
                "--set-upstream-to",
                name(upstream)?,
                name(branch)?,
            ],
            Classifier::General,
            context,
        ),
        Branching::BranchDeleteRemote {
            remote,
            name: branch,
        } => delete_remote(found, remote, "refs/heads/", branch, context),
        Branching::Merge { revision } => write(
            found,
            &["merge", name(revision)?],
            Classifier::Merge,
            context,
        ),
        Branching::Rebase { revision } => write(
            found,
            &["rebase", name(revision)?],
            Classifier::Rebase,
            context,
        ),
        Branching::CherryPick { revision } => cherry_pick(found, name(revision)?, context),
        Branching::Abort(abort) => {
            let command = match abort {
                Abort::Merge => "merge",
                Abort::Rebase => "rebase",
                Abort::CherryPick => "cherry-pick",
            };
            write(found, &[command, "--abort"], Classifier::General, context)
        }
        Branching::UndoCommit { commit } => undo_commit(found, commit, context),
        Branching::StashPush {
            untracked,
            staged,
            message,
        } => {
            let mut arguments = vec!["stash", "push"];
            if *untracked {
                arguments.push("-u");
            }
            if *staged {
                arguments.push("-S");
            }
            if let Some(message) = message.as_deref().filter(|message| !message.is_empty()) {
                arguments.extend(["-m", message]);
            }
            let ran = retried(context, || {
                run(found, &arguments, context)?.checked(Classifier::Stash)
            })?;
            if mentions(&ran, NOTHING_TO_STASH) {
                return Err(GitError::git(GitErrorCode::NoLocalChanges));
            }
            Ok(())
        }
        Branching::StashApply {
            index,
            commit,
            pop,
            staged,
        } => {
            let entry = stash_entry(found, *index, commit, context)?;
            let mut arguments = vec!["stash", if *pop { "pop" } else { "apply" }];
            if *staged {
                arguments.push("--index");
            }
            arguments.push(&entry);
            write(found, &arguments, Classifier::Stash, context)
        }
        Branching::StashDrop { index, commit } => {
            let entry = stash_entry(found, *index, commit, context)?;
            write(
                found,
                &["stash", "drop", &entry],
                Classifier::Stash,
                context,
            )
        }
        Branching::StashClear => write(found, &["stash", "clear"], Classifier::Stash, context),
        Branching::TagCreate {
            name: tag,
            message,
            target,
        } => {
            let mut arguments = match message.as_deref().filter(|message| !message.is_empty()) {
                Some(message) => vec!["tag", "-a", name(tag)?, "-m", message],
                None => vec!["tag", name(tag)?],
            };
            if let Some(target) = target {
                arguments.push(name(target)?);
            }
            write(found, &arguments, Classifier::General, context)
        }
        Branching::TagDelete { name: tag } => write(
            found,
            &["tag", "-d", name(tag)?],
            Classifier::General,
            context,
        ),
        Branching::TagDeleteRemote { remote, name: tag } => {
            delete_remote(found, remote, "refs/tags/", tag, context)
        }
        Branching::RemoteAdd { name: remote, url } => write(
            found,
            &["remote", "add", name(remote)?, name(url)?],
            Classifier::General,
            context,
        ),
        Branching::RemoteRemove { name: remote } => write(
            found,
            &["remote", "remove", name(remote)?],
            Classifier::General,
            context,
        ),
    }
}

/// A name, revision or address Git reads as an argument, never as an option.
fn name(value: &str) -> Result<&str, GitError> {
    if value.is_empty() || value.starts_with('-') || value.contains('\0') {
        Err(GitError::new(ErrorCode::InvalidRequest))
    } else {
        Ok(value)
    }
}

/// Runs a write, retried while another process holds a lock.
fn write(
    found: &Found,
    arguments: &[&str],
    classifier: Classifier,
    context: &Context<'_>,
) -> Result<(), GitError> {
    retried(context, || {
        run(found, arguments, context)?.checked(classifier)
    })
    .map(drop)
}

fn run(found: &Found, arguments: &[&str], context: &Context<'_>) -> Result<Ran, GitError> {
    let mut command = found.command(Access::Write, context.relay);
    command.args(arguments);
    runner::run(command, Vec::new(), context.limits, context.output)
}

/// The commit `revision` names, or none.
fn resolve(
    found: &Found,
    revision: &str,
    context: &Context<'_>,
) -> Result<Option<String>, GitError> {
    let mut command = found.command(Access::Read, None);
    command.args(["rev-parse", "--verify", "-q", revision]);
    let ran = runner::run(command, Vec::new(), context.limits, context.output)?;
    let resolved = String::from_utf8_lossy(&ran.stdout).trim().to_owned();
    Ok((ran.exited_with(0) && command::is_object_name(&resolved)).then_some(resolved))
}

/// Whether the run's output says `text`.
fn mentions(ran: &Ran, text: &str) -> bool {
    [&ran.stdout, &ran.stderr]
        .iter()
        .any(|output| String::from_utf8_lossy(output).contains(text))
}

/// `git push <remote> --delete <prefix><name>`, as VS Code deletes a remote reference.
fn delete_remote(
    found: &Found,
    remote: &str,
    prefix: &str,
    reference: &str,
    context: &Context<'_>,
) -> Result<(), GitError> {
    let reference = format!("{prefix}{}", name(reference)?);
    write(
        found,
        &["push", name(remote)?, "--delete", &reference],
        Classifier::General,
        context,
    )
}

/// `git cherry-pick`, as VS Code reads its failure: a pick that became empty is aborted and says
/// so, and any other failure but another process's lock reads as a conflict.
fn cherry_pick(found: &Found, revision: &str, context: &Context<'_>) -> Result<(), GitError> {
    let picked = write(
        found,
        &["cherry-pick", revision],
        Classifier::CherryPick,
        context,
    );
    if let Err(error) = &picked
        && error.git_error_code == Some(GitErrorCode::CherryPickEmpty)
    {
        // VS Code ignores whether the abort succeeds; the pick's own failure is the answer.
        let _ = write(
            found,
            &["cherry-pick", "--abort"],
            Classifier::General,
            context,
        );
    }
    picked
}

/// `git.undo-commit`: HEAD back to its first parent with the index and working tree kept, or for
/// a first commit the branch removed and everything unstaged, as VS Code undoes a commit.
fn undo_commit(found: &Found, commit: &str, context: &Context<'_>) -> Result<(), GitError> {
    if !command::is_object_name(commit) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    if resolve(found, "HEAD", context)?.as_deref() != Some(commit) {
        return Err(GitError::new(ErrorCode::StaleRevision));
    }
    if resolve(found, "HEAD~", context)?.is_some() {
        return write(
            found,
            &["reset", "--soft", "HEAD~"],
            Classifier::General,
            context,
        );
    }
    // The old value makes Git remove the branch only while it still names the commit.
    write(
        found,
        &["update-ref", "-d", "HEAD", commit],
        Classifier::General,
        context,
    )?;
    write(
        found,
        &["rm", "--cached", "-r", "-q", "--ignore-unmatch", "--", "."],
        Classifier::General,
        context,
    )
}

/// `stash@{index}` once it still names `commit`: `no-stash-found` without it, `stale-revision`
/// when it names another commit.
fn stash_entry(
    found: &Found,
    index: u64,
    commit: &str,
    context: &Context<'_>,
) -> Result<String, GitError> {
    if !command::is_object_name(commit) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    let entry = format!("stash@{{{index}}}");
    match resolve(found, &entry, context)? {
        None => Err(GitError::git(GitErrorCode::NoStashFound)),
        Some(resolved) if resolved == commit => Ok(entry),
        Some(_) => Err(GitError::new(ErrorCode::StaleRevision)),
    }
}
