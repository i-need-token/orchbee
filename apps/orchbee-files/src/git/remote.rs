//! The operations that reach a remote, each as VS Code's Git extension runs it at ef5ca45 with
//! its default settings, since this process reads none: fetch, pull and sync take tags along, a
//! lease push also requires the remote branch to be included where Git can check it, a sync
//! pushes only what the pull left ahead, and a clone fills a new folder outside any repository.
//! Git reports progress, so the watchdog that fails a silent command sees a slow transfer move.

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode, PushMode},
    mutations::{Context, retried, retried_while},
    reads,
    runner::{self, Ran},
};
use orchbee_files_protocol::{ErrorCode, PushForce};
use orchbee_git_process::canonical_path;
use std::{fs, io, path::Path, time::Duration};

/// A remote command fails after an hour, or after five minutes without output while no prompt
/// waits on the user.
pub(super) const DEADLINE: Duration = Duration::from_secs(60 * 60);
pub(super) const IDLE: Duration = Duration::from_secs(5 * 60);
/// Git 2.30 added `--force-if-includes`.
const IF_INCLUDES_VERSION: (u32, u32, u32) = (2, 30, 0);
/// Git 2.16 added `%(upstream:remotename)` and `%(upstream:remoteref)`.
const UPSTREAM_PARTS_VERSION: (u32, u32, u32) = (2, 16, 0);
/// The failures VS Code retries for a fetch, a pull and a sync.
const REMOTE_RETRIES: [GitErrorCode; 3] = [
    GitErrorCode::RepositoryIsLocked,
    GitErrorCode::CantLockRef,
    GitErrorCode::CantRebaseMultipleBranches,
];

/// Which remotes a fetch reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FetchRemotes {
    /// The remote Git picks for the current branch.
    Default,
    Remote(String),
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Fetch {
    pub(super) remotes: FetchRemotes,
    pub(super) prune: bool,
}

/// What a clone takes: the folder the person chose, the new folder of it the clone fills, and the
/// repository it clones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Cloning {
    pub(super) parent: String,
    pub(super) target: String,
    pub(super) url: String,
    /// Also clone the submodules.
    pub(super) recursive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Push {
    pub(super) remote: Option<String>,
    pub(super) branch: Option<String>,
    /// Publishing: the pushed branch tracks the remote one.
    pub(super) set_upstream: bool,
    pub(super) force: PushForce,
    pub(super) tags: bool,
}

/// `git.fetch`.
pub(super) fn fetch(found: &Found, fetch: &Fetch, context: &Context<'_>) -> Result<(), GitError> {
    let mut arguments = vec!["fetch".to_owned(), "--progress".to_owned()];
    match &fetch.remotes {
        FetchRemotes::Default => {}
        FetchRemotes::Remote(remote) => arguments.push(name(remote)?.to_owned()),
        FetchRemotes::All => arguments.push("--all".to_owned()),
    }
    if fetch.prune {
        arguments.push("--prune".to_owned());
    }
    retried_while(context, &REMOTE_RETRIES, &mut || {
        run(found, &arguments, context)?.checked(Classifier::Fetch)
    })
    .map(drop)
}

/// `git.clone` in the chosen folder, with the URL after `--`, so no URL reads as an option; its
/// answer is the new repository's canonical toplevel. Git removes the folder of a clone that
/// fails, but cannot once stopped, so a clone this process cancelled or stopped at its deadline
/// removes the folder it created; one that was there before stays.
pub(super) fn clone(cloning: &Cloning, context: &Context<'_>) -> Result<String, GitError> {
    let target = Path::new(&cloning.target);
    let created = fs::symlink_metadata(target)
        .is_err_and(|failure| failure.kind() == io::ErrorKind::NotFound);
    let mut command = command::git(Path::new(&cloning.parent), Access::Write, context.relay);
    command.args(["clone", "--progress"]);
    if cloning.recursive {
        command.arg("--recursive");
    }
    command.arg("--").arg(&cloning.url).arg(target);
    let ran = runner::run(command, Vec::new(), context.limits, context.output);
    if let Err(error) = &ran
        && created
        && matches!(error.code, ErrorCode::Cancelled | ErrorCode::Timeout)
    {
        let _ = fs::remove_dir_all(target);
    }
    ran?.checked(Classifier::General)?;
    canonical_path(target)
        .map(|canonical| canonical.as_native().to_owned())
        .map_err(|_| GitError::new(ErrorCode::Failed))
}

/// `git.pull`: from the current branch's upstream when it has one, else as Git decides.
pub(super) fn pull(
    found: &Found,
    rebase: bool,
    version: (u32, u32, u32),
    context: &Context<'_>,
) -> Result<(), GitError> {
    let upstream = head(found, version, context)?.and_then(|head| head.upstream);
    pull_from(found, rebase, upstream.as_ref(), context)
}

/// `git.sync`: pulls from the current branch's upstream, then pushes to it when the branch is
/// still ahead, unless the remote has nowhere to push.
pub(super) fn sync(
    found: &Found,
    rebase: bool,
    version: (u32, u32, u32),
    context: &Context<'_>,
) -> Result<(), GitError> {
    let Some(head) = head(found, version, context)? else {
        return Err(GitError::git(GitErrorCode::NoUpstreamBranch));
    };
    let Some(upstream) = head.upstream else {
        return Err(GitError::git(GitErrorCode::NoUpstreamBranch));
    };
    pull_from(found, rebase, Some(&upstream), context)?;
    let read_only = reads::remotes(found, context.limits, context.output)?
        .iter()
        .find(|remote| remote.name == upstream.remote)
        .is_some_and(|remote| {
            remote
                .push_url
                .as_deref()
                .is_none_or(|url| url == "no_push")
        });
    if read_only || !ahead(found, context)? {
        return Ok(());
    }
    let refspec = format!("{}:{}", head.branch, upstream.name);
    push_with(
        found,
        Some(&upstream.remote),
        Some(&refspec),
        &[],
        PushMode::None,
        context,
    )
}

/// `git.push`: with neither remote nor branch, the current branch to its upstream as VS Code's
/// Push does, else what the request names; tags push alone.
pub(super) fn push(
    found: &Found,
    push: &Push,
    version: (u32, u32, u32),
    context: &Context<'_>,
) -> Result<(), GitError> {
    let force = match push.force {
        PushForce::None => PushMode::None,
        PushForce::Lease if version >= IF_INCLUDES_VERSION => PushMode::LeaseIfIncludes,
        PushForce::Lease => PushMode::Lease,
        PushForce::Force => PushMode::Force,
    };
    let mut options = Vec::new();
    if push.set_upstream {
        options.push("-u");
    }
    if push.tags {
        options.push("--tags");
    }
    let remote = push.remote.as_deref().map(name).transpose()?;
    let branch = push.branch.as_deref().map(name).transpose()?;
    if remote.is_some() || branch.is_some() || push.tags {
        return push_with(found, remote, branch, &options, force, context);
    }
    let refspec = head(found, version, context)?.and_then(|head| {
        head.upstream.map(|upstream| {
            (
                upstream.remote,
                format!("{}:{}", head.branch, upstream.name),
            )
        })
    });
    match &refspec {
        Some((remote, refspec)) => {
            push_with(found, Some(remote), Some(refspec), &options, force, context)
        }
        None => push_with(found, None, None, &options, force, context),
    }
}

fn pull_from(
    found: &Found,
    rebase: bool,
    upstream: Option<&Upstream>,
    context: &Context<'_>,
) -> Result<(), GitError> {
    let mut arguments = vec!["pull", "--progress", "--tags"];
    if rebase {
        arguments.push("-r");
    }
    if let Some(upstream) = upstream {
        arguments.extend([upstream.remote.as_str(), upstream.name.as_str()]);
    }
    retried_while(context, &REMOTE_RETRIES, &mut || {
        run(found, &arguments, context)?.checked(Classifier::Pull)
    })
    .map(drop)
}

fn push_with(
    found: &Found,
    remote: Option<&str>,
    refspec: Option<&str>,
    options: &[&str],
    force: PushMode,
    context: &Context<'_>,
) -> Result<(), GitError> {
    let mut arguments = vec!["push", "--progress"];
    match force {
        PushMode::None => {}
        PushMode::Lease => arguments.push("--force-with-lease"),
        PushMode::LeaseIfIncludes => {
            arguments.extend(["--force-with-lease", "--force-if-includes"])
        }
        PushMode::Force => arguments.push("--force"),
    }
    arguments.extend(options);
    arguments.extend(remote);
    arguments.extend(refspec);
    retried(context, || {
        run(found, &arguments, context)?.checked(Classifier::Push(force))
    })
    .map(drop)
}

/// The branch a local branch tracks, as VS Code's `getBranch` reads it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Upstream {
    remote: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Head {
    branch: String,
    upstream: Option<Upstream>,
}

/// The current branch and its upstream, unless HEAD is detached; an upstream that is gone counts
/// as none.
fn head(
    found: &Found,
    version: (u32, u32, u32),
    context: &Context<'_>,
) -> Result<Option<Head>, GitError> {
    let symbolic = read(found, &["symbolic-ref", "-q", "--short", "HEAD"], context)?;
    if !symbolic.status.is_some_and(|status| status.success()) {
        return Ok(None);
    }
    let branch = String::from_utf8_lossy(&symbolic.stdout).trim().to_owned();
    let mut format = String::from("--format=%(upstream:short)%00%(upstream:track)");
    if version >= UPSTREAM_PARTS_VERSION {
        format.push_str("%00%(upstream:remotename)%00%(upstream:remoteref)");
    }
    let reference = format!("refs/heads/{branch}");
    let listed = read(found, &["for-each-ref", &format, &reference], context)?
        .checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&listed.stdout);
    let mut fields = text.trim_end_matches('\n').split('\0');
    let short = fields.next().unwrap_or_default();
    let track = fields.next().unwrap_or_default();
    let remote_name = fields.next().unwrap_or_default();
    let remote_ref = fields.next().unwrap_or_default();
    let upstream = (!short.is_empty() && track != "[gone]").then(|| {
        let (remote, name) = short.split_once('/').unwrap_or(("", short));
        Upstream {
            remote: if remote_name.is_empty() {
                remote
            } else {
                remote_name
            }
            .to_owned(),
            name: remote_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(name)
                .to_owned(),
        }
    });
    Ok(Some(Head { branch, upstream }))
}

/// Whether the current branch has commits its upstream lacks.
fn ahead(found: &Found, context: &Context<'_>) -> Result<bool, GitError> {
    let counted = read(
        found,
        &["rev-list", "--count", "@{upstream}..HEAD"],
        context,
    )?
    .checked(Classifier::General)?;
    let count = String::from_utf8_lossy(&counted.stdout)
        .trim()
        .parse::<u64>();
    count
        .map(|count| count > 0)
        .map_err(|_| GitError::new(ErrorCode::Failed))
}

/// A remote or branch name, which must not read as an option.
fn name(name: &str) -> Result<&str, GitError> {
    if name.is_empty() || name.starts_with('-') {
        Err(GitError::new(ErrorCode::InvalidRequest))
    } else {
        Ok(name)
    }
}

fn run<S: AsRef<std::ffi::OsStr>>(
    found: &Found,
    arguments: &[S],
    context: &Context<'_>,
) -> Result<Ran, GitError> {
    let mut command = found.command(Access::Write, context.relay);
    command.args(arguments);
    runner::run(command, Vec::new(), context.limits, context.output)
}

fn read(found: &Found, arguments: &[&str], context: &Context<'_>) -> Result<Ran, GitError> {
    let mut command = found.command(Access::Read, None);
    command.args(arguments);
    runner::run(command, Vec::new(), context.limits, context.output)
}
