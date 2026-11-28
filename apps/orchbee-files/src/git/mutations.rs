//! The local Git mutations Source Control runs, each as VS Code's Git extension runs it at
//! ef5ca45: stage, unstage, discard, content staging, clean, commit, init and trusting an unsafe
//! repository. Paths reach Git literally, in batches of at most 30,000 characters, and a command
//! that meets another process's lock is retried with VS Code's quadratic backoff. Each working-tree
//! entry a discard, a clean or an init changes passes the call's scope first, so a link or junction
//! on the way cannot carry the change outside it.

use super::{
    command::{self, Access, RelayVariables},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode},
    output::GitOutput,
    ports::{Scope, Trash, TrashError},
    runner::{self, Limits, Ran},
};
use orchbee_files_protocol::{CleanMode, CommitAll, ErrorCode};
use orchbee_git_process::canonical_path;
use std::{
    iter,
    path::Path,
    sync::{Mutex, atomic::Ordering},
    thread,
    time::{Duration, Instant},
};

/// Path characters one command carries, as VS Code splits long path lists.
const BATCH_CHARACTERS: usize = 30_000;
/// Attempts a locked repository gets, as VS Code retries.
const LOCK_ATTEMPTS: u32 = 10;
/// The deadline of each batch of paths, of content staging and of `init`.
pub(super) const BATCH_DEADLINE: Duration = Duration::from_secs(60);
/// The deadline of a command that reads or writes configuration.
pub(super) const CONFIG_DEADLINE: Duration = Duration::from_secs(10);
const COMMIT_DEADLINE: Duration = Duration::from_secs(30 * 60);
/// The longest an editor prompt may stay open.
const EDITOR_DEADLINE: Duration = Duration::from_secs(8 * 60 * 60);
/// The mode of content staged for a path HEAD does not have.
const NEW_FILE_MODE: &str = "100644";
/// The paths a clean moves to the trash at once, as VS Code's clean does.
const TRASH_CONCURRENCY: usize = 5;
/// The first branch of a new repository when the user's Git names none: VS Code's default
/// `git.defaultBranchName`, passed with `init -b`, which Git 2.28 added.
const DEFAULT_BRANCH: &str = "main";
const INIT_BRANCH_VERSION: (u32, u32, u32) = (2, 28, 0);

/// Where a commit's message comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Message {
    Text(String),
    /// Git's editor call, which the relay turns into an editor prompt.
    Editor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Commit {
    pub(super) message: Message,
    pub(super) amend: bool,
    pub(super) signoff: bool,
    pub(super) all: CommitAll,
    pub(super) no_verify: bool,
}

/// One `git.clean` answer: the path, and why it stayed when it did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Cleaned {
    pub(super) path: String,
    pub(super) error: Option<GitError>,
}

/// What one Git mutation needs besides its arguments.
pub(super) struct Context<'a> {
    pub(super) limits: &'a Limits,
    pub(super) output: &'a Mutex<GitOutput>,
    /// The relay of the operation, for the commands that may prompt.
    pub(super) relay: Option<&'a RelayVariables>,
}

/// `git.stage`: `git add -A` of the paths.
pub(super) fn stage(
    found: &Found,
    paths: &[String],
    context: &Context<'_>,
) -> Result<(), GitError> {
    batched(found, &["add", "-A"], paths, Classifier::General, context)
}

/// `git.unstage`: the paths' index entries back to HEAD, or out of the index before the first
/// commit. Git's report of paths that still need a merge is no failure, as in VS Code.
pub(super) fn unstage(
    found: &Found,
    paths: &[String],
    context: &Context<'_>,
) -> Result<(), GitError> {
    let arguments: &[&str] = if born(found, context)? {
        &["reset", "-q", "HEAD"]
    } else {
        &["rm", "--cached", "-r", "-q"]
    };
    batched(found, arguments, paths, Classifier::General, context)
}

/// `git.discard`: the working-tree changes of tracked paths, from the index; a submodule is
/// updated to the commit its repository records instead. Nothing changes unless every path
/// passes `scope`.
pub(super) fn discard(
    found: &Found,
    paths: &[String],
    submodules: &[String],
    scope: &dyn Scope,
    context: &Context<'_>,
) -> Result<(), GitError> {
    for path in paths.iter().chain(submodules) {
        admitted(found, path, scope)?;
    }
    if !paths.is_empty() {
        batched(
            found,
            &["checkout", "-q"],
            paths,
            Classifier::Checkout,
            context,
        )?;
    }
    if !submodules.is_empty() {
        batched(
            found,
            &["submodule", "update"],
            submodules,
            Classifier::General,
            context,
        )?;
    }
    Ok(())
}

/// `git.stage-content`: `content` becomes `path`'s index entry with the mode HEAD, or before the
/// first commit the index, records for it, as VS Code stages a selection: the frontend sends the
/// whole content the index should hold.
pub(super) fn stage_content(
    found: &Found,
    path: &str,
    content: Vec<u8>,
    context: &Context<'_>,
) -> Result<(), GitError> {
    let relative = found.relative(path)?;
    let mut hash = found.command(Access::Write, context.relay);
    hash.args(["hash-object", "--stdin", "-w", "--path", &relative]);
    let hashed =
        runner::run(hash, content, context.limits, context.output)?.checked(Classifier::General)?;
    let object = String::from_utf8_lossy(&hashed.stdout).trim().to_owned();
    if !command::is_object_name(&object) {
        return Err(GitError::new(ErrorCode::Failed));
    }
    let listing: &[&str] = if born(found, context)? {
        &["ls-tree", "-l", "HEAD", "--", &relative]
    } else {
        &["ls-files", "--stage", "--", &relative]
    };
    let listed =
        literal(found, Access::Read, None, listing, context)?.checked(Classifier::General)?;
    let mode = String::from_utf8_lossy(&listed.stdout)
        .lines()
        .next()
        .and_then(|line| line.split(' ').next())
        .filter(|mode| !mode.is_empty())
        .map(str::to_owned);
    let mut arguments = vec!["update-index"];
    if mode.is_none() {
        arguments.push("--add");
    }
    arguments.extend([
        "--cacheinfo",
        mode.as_deref().unwrap_or(NEW_FILE_MODE),
        &object,
        &relative,
    ]);
    retried(context, || {
        literal(found, Access::Write, context.relay, &arguments, context)?
            .checked(Classifier::General)
    })
    .map(drop)
}

/// `git.clean`: untracked paths to the machine's trash, five at a time, or removed with
/// `git clean -f`, each once it passes `scope`. Paths not yet reached when the call is cancelled
/// answer `cancelled`.
pub(super) fn clean(
    found: &Found,
    paths: &[String],
    mode: CleanMode,
    trash: &dyn Trash,
    scope: &dyn Scope,
    context: &Context<'_>,
) -> Vec<Cleaned> {
    match mode {
        CleanMode::Trash => {
            let mut cleaned = Vec::with_capacity(paths.len());
            for group in paths.chunks(TRASH_CONCURRENCY) {
                if context.limits.cancel.load(Ordering::Acquire) {
                    cleaned.extend(group.iter().map(|path| Cleaned {
                        path: path.clone(),
                        error: Some(GitError::new(ErrorCode::Cancelled)),
                    }));
                    continue;
                }
                thread::scope(|threads| {
                    let moving = group
                        .iter()
                        .map(|path| {
                            thread::Builder::new()
                                .name("orchbee-files-git-trash".into())
                                .spawn_scoped(threads, move || trash_one(found, path, trash, scope))
                        })
                        .collect::<Vec<_>>();
                    for (path, moving) in group.iter().zip(moving) {
                        // A path whose thread did not start moves on this one.
                        let moved = match moving {
                            Ok(moving) => moving
                                .join()
                                .unwrap_or_else(|_| Err(GitError::new(ErrorCode::Failed))),
                            Err(_) => trash_one(found, path, trash, scope),
                        };
                        cleaned.push(Cleaned {
                            path: path.clone(),
                            error: moved.err(),
                        });
                    }
                });
            }
            cleaned
        }
        CleanMode::Permanent => {
            let checked = paths
                .iter()
                .map(|path| admitted(found, path, scope))
                .collect::<Vec<_>>();
            let removable = paths
                .iter()
                .zip(&checked)
                .filter(|(_, checked)| checked.is_ok())
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            let mut removed = Vec::with_capacity(removable.len());
            for batch in batches(&removable) {
                let error = batched(
                    found,
                    &["clean", "-f", "-q"],
                    batch,
                    Classifier::General,
                    context,
                )
                .err();
                removed.extend(iter::repeat_n(error, batch.len()));
            }
            let mut removed = removed.into_iter();
            paths
                .iter()
                .zip(checked)
                .map(|(path, checked)| Cleaned {
                    path: path.clone(),
                    error: match checked {
                        Ok(()) => removed.next().flatten(),
                        Err(error) => Some(error),
                    },
                })
                .collect()
        }
    }
}

/// Admits a working-tree path a discard, a clean or an init changes: below the repository's
/// toplevel, and the entry passes the call's scope.
fn admitted(found: &Found, path: &str, scope: &dyn Scope) -> Result<(), GitError> {
    found.relative(path)?;
    scope.entry(path).map_err(GitError::new)
}

/// Moves one untracked path to the trash once it passes `scope`.
fn trash_one(
    found: &Found,
    path: &str,
    trash: &dyn Trash,
    scope: &dyn Scope,
) -> Result<(), GitError> {
    admitted(found, path, scope)?;
    let path = Path::new(path);
    if path.symlink_metadata().is_err() {
        return Err(GitError::new(ErrorCode::NotFound));
    }
    trash.trash(path).map_err(|error| match error {
        TrashError::Unavailable => GitError::new(ErrorCode::TrashUnavailable),
        TrashError::Failed => GitError::new(ErrorCode::TrashFailed),
    })
}

/// `git.commit`, answering the commit HEAD names afterwards. `all` first stages every tracked
/// change, or every change, as VS Code's smart commit does. While a rebase waits on a commit, the
/// commit continues it instead, keeping the message Git prepared, as VS Code does. A failure that
/// names no cause of its own is checked against the user's identity, as VS Code checks it.
pub(super) fn commit(
    found: &Found,
    commit: &Commit,
    context: &Context<'_>,
) -> Result<String, GitError> {
    let staged = match commit.all {
        CommitAll::None => None,
        CommitAll::Tracked => Some(["add", "-u", "--", "."]),
        CommitAll::All => Some(["add", "-A", "--", "."]),
    };
    if let Some(arguments) = staged {
        retried(context, || {
            literal(found, Access::Write, context.relay, &arguments, context)?
                .checked(Classifier::General)
        })?;
    }
    let committed = if rebase_waits(found) {
        retried(context, || {
            let mut command = found.command(Access::Write, context.relay);
            command
                .env("GIT_EDITOR", "true")
                .args(["rebase", "--continue"]);
            runner::run(command, Vec::new(), context.limits, context.output)?
                .checked(Classifier::Commit)
        })
    } else {
        let (arguments, input) = commit_arguments(commit);
        retried(context, || {
            let mut command = found.command(Access::Write, context.relay);
            command.args(&arguments);
            runner::run(command, input.clone(), context.limits, context.output)?
                .checked(Classifier::Commit)
        })
    };
    if let Err(error) = committed {
        return Err(identity_failure(found, error, context));
    }
    let head =
        run(found, Access::Read, &["rev-parse", "HEAD"], context)?.checked(Classifier::General)?;
    let name = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    if command::is_object_name(&name) {
        Ok(name)
    } else {
        Err(GitError::new(ErrorCode::Failed))
    }
}

/// The arguments and standard input of `git commit`, as VS Code builds them with Git's user
/// configuration required.
fn commit_arguments(commit: &Commit) -> (Vec<&'static str>, Vec<u8>) {
    let mut arguments = vec!["-c", "user.useConfigOnly=true", "commit", "--quiet"];
    let mut input = Vec::new();
    match &commit.message {
        Message::Text(text) => {
            if text.is_empty() && commit.amend {
                arguments.extend(["--amend", "--no-edit"]);
            } else {
                text.as_bytes().clone_into(&mut input);
                arguments.extend(["--file", "-"]);
                if commit.amend {
                    arguments.push("--amend");
                }
            }
            arguments.push("--allow-empty-message");
        }
        Message::Editor => {
            if commit.amend {
                arguments.push("--amend");
            }
        }
    }
    if commit.signoff {
        arguments.push("--signoff");
    }
    if commit.no_verify {
        arguments.push("--no-verify");
    }
    (arguments, input)
}

/// Whether a rebase stopped at a commit that a commit continues, as VS Code's rebase commit
/// reads it: a rebase folder and a readable `REBASE_HEAD`.
fn rebase_waits(found: &Found) -> bool {
    let rebasing = ["rebase-merge", "rebase-apply"]
        .iter()
        .any(|folder| found.git_dir.join(folder).is_dir());
    rebasing && std::fs::read(found.git_dir.join("REBASE_HEAD")).is_ok()
}

/// The commit deadline: 30 minutes, and for a message from the editor, its prompt's time too.
pub(super) const fn commit_deadline(commit: &Commit) -> Duration {
    match commit.message {
        Message::Editor => COMMIT_DEADLINE.saturating_add(EDITOR_DEADLINE),
        Message::Text(_) => COMMIT_DEADLINE,
    }
}

/// A commit failure read again as VS Code reads it: a missing identity names itself.
fn identity_failure(found: &Found, error: GitError, context: &Context<'_>) -> GitError {
    let named = matches!(
        error.git_error_code,
        Some(GitErrorCode::UnmergedChanges | GitErrorCode::EmptyCommitMessage)
    );
    if named || error.code != ErrorCode::GitFailed {
        return error;
    }
    for (key, missing) in [
        ("user.name", GitErrorCode::NoUserNameConfigured),
        ("user.email", GitErrorCode::NoUserEmailConfigured),
    ] {
        let set = run(found, Access::Read, &["config", "--get", key], context)
            .is_ok_and(|ran| ran.status.is_some_and(|status| status.success()));
        if !set {
            return GitError {
                git_error_code: Some(missing),
                ..error
            };
        }
    }
    error
}

/// `git.init` in `path`, answering the new repository's canonical toplevel, once the `.git` entry
/// it creates passes `scope`. The first branch is the one the user's `init.defaultBranch` names,
/// else VS Code's default where Git can name it.
pub(super) fn init(
    path: &str,
    version: (u32, u32, u32),
    scope: &dyn Scope,
    context: &Context<'_>,
) -> Result<String, GitError> {
    let directory = Path::new(path);
    let entry = directory
        .join(".git")
        .into_os_string()
        .into_string()
        .map_err(|_| GitError::new(ErrorCode::InvalidRequest))?;
    scope.entry(&entry).map_err(GitError::new)?;
    let mut configured = command::git(directory, Access::Read, None);
    configured.args(["config", "--get", "init.defaultBranch"]);
    let named = runner::run(configured, Vec::new(), context.limits, context.output)?
        .status
        .is_some_and(|status| status.success());
    let mut command = command::git(directory, Access::Write, None);
    command.arg("init");
    if !named && version >= INIT_BRANCH_VERSION {
        command.args(["-b", DEFAULT_BRANCH]);
    }
    runner::run(command, Vec::new(), context.limits, context.output)?
        .checked(Classifier::General)?;
    canonical_path(directory)
        .map(|canonical| canonical.as_native().to_owned())
        .map_err(|_| GitError::new(ErrorCode::Failed))
}

/// `git.mark-safe`: adds a repository Git refused for its owner to the user's global
/// `safe.directory`, from the home folder as VS Code's Manage Unsafe Repositories does.
pub(super) fn mark_safe(path: &str, context: &Context<'_>) -> Result<(), GitError> {
    let canonical =
        canonical_path(Path::new(path)).map_err(|_| GitError::new(ErrorCode::NotFound))?;
    let home = std::env::home_dir().ok_or(GitError::new(ErrorCode::Failed))?;
    let mut command = command::git(&home, Access::Write, None);
    command.args([
        "config",
        "--global",
        "--add",
        "safe.directory",
        &canonical.git_spelling(),
    ]);
    runner::run(command, Vec::new(), context.limits, context.output)?
        .checked(Classifier::General)
        .map(drop)
}

/// Runs `arguments -- <paths>` over batches of literal paths below the repository. A batch that
/// fails only because paths still need a merge counts as done, as VS Code counts an unstage.
fn batched(
    found: &Found,
    arguments: &[&str],
    paths: &[String],
    classifier: Classifier,
    context: &Context<'_>,
) -> Result<(), GitError> {
    let relatives = paths
        .iter()
        .map(|path| found.relative(path))
        .collect::<Result<Vec<_>, _>>()?;
    for batch in batches(&relatives) {
        retried(context, || {
            let mut command = found.command(Access::Write, context.relay);
            command
                .arg("--literal-pathspecs")
                .args(arguments)
                .arg("--")
                .args(batch);
            let ran = runner::run(command, Vec::new(), context.limits, context.output)?;
            if needs_merge(&ran) {
                return Ok(ran);
            }
            ran.checked(classifier)
        })?;
    }
    Ok(())
}

/// Whether a failed command printed that paths need a merge.
fn needs_merge(ran: &Ran) -> bool {
    let failed = ran.status.is_some_and(|status| !status.success());
    let text = String::from_utf8_lossy(&ran.stdout);
    failed && text.lines().any(|line| line.ends_with(": needs merge"))
}

/// Splits paths into batches of at most [`BATCH_CHARACTERS`] characters, each holding at least
/// one path.
fn batches(paths: &[String]) -> Vec<&[String]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut characters = 0;
    for (index, path) in paths.iter().enumerate() {
        let length = path.chars().count();
        if index > start && characters + length > BATCH_CHARACTERS {
            batches.extend(paths.get(start..index));
            start = index;
            characters = 0;
        }
        characters += length;
    }
    batches.extend(paths.get(start..).filter(|rest| !rest.is_empty()));
    batches
}

/// Whether HEAD names a commit.
fn born(found: &Found, context: &Context<'_>) -> Result<bool, GitError> {
    Ok(run(
        found,
        Access::Read,
        &["rev-parse", "--verify", "-q", "HEAD"],
        context,
    )?
    .status
    .is_some_and(|status| status.success()))
}

/// Runs `operation` again while Git reports another process's lock, at most ten times, waiting
/// `attempt² × 50 ms` between attempts as VS Code does.
pub(super) fn retried<T>(
    context: &Context<'_>,
    mut operation: impl FnMut() -> Result<T, GitError>,
) -> Result<T, GitError> {
    retried_while(context, &[GitErrorCode::RepositoryIsLocked], &mut operation)
}

/// [`retried`] for the codes `retry` names, as VS Code also retries a pull, a sync or a fetch that
/// could not lock a reference.
pub(super) fn retried_while<T>(
    context: &Context<'_>,
    retry: &[GitErrorCode],
    operation: &mut impl FnMut() -> Result<T, GitError>,
) -> Result<T, GitError> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match operation() {
            Err(error)
                if attempt <= LOCK_ATTEMPTS
                    && error
                        .git_error_code
                        .is_some_and(|code| retry.contains(&code)) =>
            {
                let resume =
                    Instant::now() + Duration::from_millis(u64::from(attempt * attempt) * 50);
                while Instant::now() < resume {
                    if context.limits.cancel.load(Ordering::Acquire) {
                        return Err(GitError::new(ErrorCode::Cancelled));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            result => return result,
        }
    }
}

fn run(
    found: &Found,
    access: Access,
    arguments: &[&str],
    context: &Context<'_>,
) -> Result<Ran, GitError> {
    let mut command = found.command(access, None);
    command.args(arguments);
    runner::run(command, Vec::new(), context.limits, context.output)
}

/// Runs a command whose paths Git takes literally.
fn literal(
    found: &Found,
    access: Access,
    relay: Option<&RelayVariables>,
    arguments: &[&str],
    context: &Context<'_>,
) -> Result<Ran, GitError> {
    let mut command = found.command(access, relay);
    command.arg("--literal-pathspecs").args(arguments);
    runner::run(command, Vec::new(), context.limits, context.output)
}
