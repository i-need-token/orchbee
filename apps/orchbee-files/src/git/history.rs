//! The history Source Control's Graph and Timeline read, each as VS Code's Git extension reads it
//! at ef5ca45: pages of commits with their parents, references and statistics, walked from the
//! revisions a Client names or from HEAD, and the commits of one path, following a file through
//! its renames with the path it had in each; the changes between two commits; their merge base;
//! and the stash list. Each page lists afresh, so history that moves between pages may repeat or
//! skip commits.

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode},
    native_paths,
    output::GitOutput,
    runner::{self, Flow, Limits},
};
use orchbee_files_protocol::ErrorCode;
use std::{sync::Mutex, time::Duration};

/// History reads fail after a minute.
pub(super) const DEADLINE: Duration = Duration::from_secs(60);
/// The most standard output one log page reads; Git is stopped there and the commits read whole
/// make the page.
const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
/// The most of a commit message a page carries; a longer one is cut and says so.
pub(super) const MESSAGE_LIMIT: usize = 64 * 1024;
/// The most bytes of an author's name or address a page carries; longer ones are cut.
pub(super) const NAME_LIMIT: usize = 1024;
/// The most references and parents one commit carries.
const LIST_LIMIT: usize = 256;
/// Git 2.31 added `--diff-merges=first-parent`, which gives a merge the statistics of its first
/// parent's diff.
const FIRST_PARENT_VERSION: (u32, u32, u32) = (2, 31, 0);
/// One commit: a record separator, then the commit, its parents, the author's name and address,
/// the author and committer times, the references and the raw message, which `-z` ends with NUL.
/// Statistics follow the NUL.
const FORMAT: &str = "--format=%x1e%H%n%P%n%aN%n%aE%n%at%n%ct%n%D%n%B";
const SEPARATOR: u8 = 0x1e;
/// The empty tree, which Git knows without storing it, in each object format.
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const EMPTY_TREE_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";
/// VS Code's default `git.similarityThreshold`.
const RENAMES: &str = "--find-renames=50%";
/// Git reads `--skip` as an `int`; a page that starts past it starts past every commit.
const SKIP_LIMIT: u64 = 2_147_483_647;

/// What one log page lists.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LogQuery {
    /// Revisions to walk from, which reach Git through its standard input as VS Code passes them.
    /// With `all`, the branches, remote branches and tags as well; with neither, HEAD.
    pub(super) revisions: Vec<String>,
    pub(super) all: bool,
    /// A native path in the repository whose commits alone are listed; the toplevel lists all.
    pub(super) path: Option<String>,
    /// Follows the file at `path` through renames and reports its path in each commit.
    pub(super) follow: bool,
    pub(super) author: Option<String>,
    pub(super) grep: Option<String>,
    pub(super) statistics: bool,
    /// 1 to 1,000 commits.
    pub(super) limit: usize,
    pub(super) skip: u64,
}

/// What a commit changed: files, and lines added and removed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Statistics {
    pub(super) files: u64,
    pub(super) insertions: u64,
    pub(super) deletions: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Commit {
    pub(super) commit: String,
    pub(super) parents: Vec<String>,
    pub(super) author_name: String,
    pub(super) author_email: String,
    /// Seconds since the Unix epoch.
    pub(super) author_time: i64,
    pub(super) commit_time: i64,
    /// The references at the commit as Git decorates them in full, such as
    /// `HEAD -> refs/heads/main` and `tag: refs/tags/v1`.
    pub(super) references: Vec<String>,
    /// The raw message without its last line end, cut to [`MESSAGE_LIMIT`].
    pub(super) message: String,
    pub(super) message_cut: bool,
    pub(super) statistics: Option<Statistics>,
    /// The followed file's native path in this commit, when Git reports it.
    pub(super) path: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LogPage {
    /// At most the query's limit, in Git's order.
    pub(super) commits: Vec<Commit>,
    /// Whether commits may follow these.
    pub(super) more: bool,
}

/// What a paged read found from the index its page starts at in the whole listing, which the next
/// page's cursor counts from.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Paged<T> {
    pub(super) start: u64,
    /// What the read found from `start` on.
    pub(super) value: T,
}

/// Leaves out the first `start` of `values`, which earlier pages held.
pub(super) fn skip<T>(values: &mut Vec<T>, start: u64) {
    let skipped = usize::try_from(start).map_or(values.len(), |start| start.min(values.len()));
    values.drain(..skipped);
}

/// `git.log`: one page of commits, in topological order when it walks named revisions, as VS
/// Code's Graph reads them, else in Git's default order, as its Timeline does. A branch without
/// commits has an empty history.
pub(super) fn log(
    found: &Found,
    version: (u32, u32, u32),
    query: &LogQuery,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<LogPage, GitError> {
    let pathspec = match &query.path {
        Some(path) if native_paths::same(&found.toplevel, path) => None,
        Some(path) => Some(found.relative(path)?),
        None => None,
    };
    if query.follow && pathspec.is_none() {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    if !query.revisions.iter().all(|revision| walkable(revision)) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    if query.skip > SKIP_LIMIT {
        return Ok(LogPage::default());
    }
    let mut command = found.command(Access::Read, None);
    command.args([
        "--literal-pathspecs",
        "-c",
        "i18n.logOutputEncoding=UTF-8",
        "log",
        FORMAT,
        "-z",
        "--decorate=full",
    ]);
    if query.follow {
        command.args(["--numstat", "--follow"]);
    } else if query.statistics {
        command.arg("--shortstat");
        if version >= FIRST_PARENT_VERSION {
            command.arg("--diff-merges=first-parent");
        }
    }
    command.arg(format!("-n{}", query.limit.saturating_add(1)));
    if let Some(author) = &query.author {
        command.arg(format!("--author={author}"));
    }
    if let Some(grep) = &query.grep {
        command.args([
            format!("--grep={grep}"),
            "--extended-regexp".to_owned(),
            "--regexp-ignore-case".to_owned(),
        ]);
    }
    if query.skip > 0 {
        command.arg(format!("--skip={}", query.skip));
    }
    let mut input = Vec::new();
    if query.all || !query.revisions.is_empty() {
        command.arg("--topo-order");
        if query.all {
            command.args(["--branches", "--remotes", "--tags"]);
        }
        if !query.revisions.is_empty() {
            command.arg("--stdin");
            for revision in &query.revisions {
                input.extend_from_slice(revision.as_bytes());
                input.push(b'\n');
            }
        }
    }
    command.arg("--");
    command.args(pathspec);
    let mut bytes = Vec::new();
    let mut cut = false;
    let ran = runner::stream(command, input, limits, output, |piece| {
        if bytes.len() + piece.len() > OUTPUT_LIMIT {
            cut = true;
            return Flow::Enough;
        }
        bytes.extend_from_slice(piece);
        Flow::More
    })?;
    if !cut && !ran.status.is_some_and(|status| status.success()) {
        if String::from_utf8_lossy(&ran.stderr).contains("does not have any commits yet") {
            return Ok(LogPage::default());
        }
        ran.checked(Classifier::General)?;
    }
    let mut records = Vec::new();
    let mut rest = &bytes[..];
    while !rest.is_empty() {
        match record(rest, query.follow) {
            Some((fields, counted, next)) => {
                records.push((fields, counted));
                rest = next;
            }
            None if cut => break,
            None => return Err(GitError::new(ErrorCode::Failed)),
        }
    }
    if cut && rest.is_empty() {
        // The last record ran to where reading stopped, which may have cut its statistics.
        records.pop();
    }
    if records.is_empty() && cut {
        return Err(GitError::git(GitErrorCode::OutputTooLarge));
    }
    let mut commits = records
        .into_iter()
        .map(|(fields, counted)| {
            commit(
                fields,
                counted,
                &found.toplevel,
                query.follow,
                query.statistics,
            )
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(GitError::new(ErrorCode::Failed))?;
    let more = cut || commits.len() > query.limit;
    commits.truncate(query.limit);
    Ok(LogPage { commits, more })
}

/// A revision Git reads from its standard input as one name, never as an option.
fn walkable(revision: &str) -> bool {
    !revision.is_empty() && !revision.starts_with('-') && !revision.contains(['\n', '\r', '\0'])
}

/// The record at the start of `rest`: its fields through the message, what `--shortstat` or
/// `--numstat` wrote after them, and the bytes after the record; none when it is cut short or
/// malformed. A record is read field by field, never by searching for the next separator, since a
/// message or a path may hold one: the message runs to NUL, which Git never writes in one, a
/// `--shortstat` line holds only counts and words, and each `--numstat` entry ends with NUL.
fn record(rest: &[u8], follow: bool) -> Option<(&[u8], &[u8], &[u8])> {
    let rest = rest.strip_prefix(&[SEPARATOR])?;
    let end = rest.iter().position(|byte| *byte == 0)?;
    let (fields, rest) = (&rest[..end], &rest[end + 1..]);
    let length = if follow {
        numstat_length(rest)?
    } else {
        rest.iter()
            .position(|byte| *byte == SEPARATOR)
            .unwrap_or(rest.len())
    };
    Some((fields, &rest[..length], &rest[length..]))
}

/// The length of a record's `--numstat` entries: after a line end, each is two counts ended by
/// tabs, then a path ended by NUL, or NUL and a rename's old and new paths, each ended by NUL.
fn numstat_length(rest: &[u8]) -> Option<usize> {
    let mut at = usize::from(rest.first() == Some(&b'\n'));
    while rest.get(at).is_some_and(|byte| *byte != SEPARATOR) {
        for _ in 0..2 {
            let count = rest[at..]
                .iter()
                .position(|byte| !(byte.is_ascii_digit() || *byte == b'-'))?;
            if rest.get(at + count) != Some(&b'\t') {
                return None;
            }
            at += count + 1;
        }
        let paths = if rest.get(at) == Some(&0) {
            at += 1;
            2
        } else {
            1
        };
        for _ in 0..paths {
            at += rest.get(at..)?.iter().position(|byte| *byte == 0)? + 1;
        }
    }
    Some(at)
}

/// One record's commit from its fields and what the statistics options wrote after them.
fn commit(
    head: &[u8],
    tail: &[u8],
    toplevel: &str,
    follow: bool,
    statistics: bool,
) -> Option<Commit> {
    let text = String::from_utf8_lossy(head);
    let mut fields = text.splitn(8, '\n');
    let commit = fields.next().filter(|name| command::is_object_name(name))?;
    let parents = fields
        .next()?
        .split(' ')
        .filter(|parent| !parent.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parents.len() > LIST_LIMIT || !parents.iter().all(|parent| command::is_object_name(parent)) {
        return None;
    }
    let author_name = cut(fields.next()?, NAME_LIMIT).0;
    let author_email = cut(fields.next()?, NAME_LIMIT).0;
    let author_time = fields.next()?.parse::<i64>().ok()?;
    let commit_time = fields.next()?.parse::<i64>().ok()?;
    let references = fields
        .next()?
        .split(", ")
        .filter(|reference| !reference.is_empty())
        .take(LIST_LIMIT)
        .map(str::to_owned)
        .collect();
    let raw = fields.next().unwrap_or_default();
    let raw = raw.strip_suffix('\n').unwrap_or(raw);
    let (message, message_cut) = cut(raw, MESSAGE_LIMIT);
    let (counted, path) = if follow {
        let (counted, relative) = numstat(tail);
        let path = relative.map(|relative| native_paths::join(toplevel, relative));
        (counted, path)
    } else {
        (shortstat(tail), None)
    };
    Some(Commit {
        commit: commit.to_owned(),
        parents,
        author_name,
        author_email,
        author_time,
        commit_time,
        references,
        message,
        message_cut,
        statistics: counted.filter(|_| statistics),
        path,
    })
}

/// `text` cut to at most `limit` bytes at a character boundary, and whether it was cut.
pub(super) fn cut(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_owned(), false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

/// The statistics of a `--shortstat` line, as VS Code reads ` 2 files changed, 3 insertions(+),
/// 1 deletion(-)`; none when Git wrote none.
fn shortstat(tail: &[u8]) -> Option<Statistics> {
    let text = String::from_utf8_lossy(tail);
    let line = text.trim();
    if line.is_empty() {
        return None;
    }
    let mut statistics = Statistics::default();
    for part in line.split(", ") {
        let (count, label) = part.trim().split_once(' ')?;
        let count = count.parse::<u64>().ok()?;
        if label.starts_with("file") {
            statistics.files = count;
        } else if label.starts_with("insertion") {
            statistics.insertions = count;
        } else if label.starts_with("deletion") {
            statistics.deletions = count;
        }
    }
    Some(statistics)
}

/// The statistics of a followed file's `--numstat` entries and its path below the toplevel in the
/// commit: the new path of a rename. A binary file counts no lines; a path that is not UTF-8 is
/// not reported.
fn numstat(tail: &[u8]) -> (Option<Statistics>, Option<&str>) {
    let tail = tail.strip_prefix(b"\n").unwrap_or(tail);
    let mut fields = tail.split(|byte| *byte == 0);
    let mut statistics = None::<Statistics>;
    let mut path = None;
    while let Some(field) = fields.next() {
        let mut columns = field.splitn(3, |byte| *byte == b'\t');
        let (Some(added), Some(removed), Some(named)) =
            (columns.next(), columns.next(), columns.next())
        else {
            break;
        };
        let named = if named.is_empty() {
            // A rename names its old path, then its new one.
            let _ = fields.next();
            fields.next().unwrap_or_default()
        } else {
            named
        };
        let counted = statistics.get_or_insert_default();
        counted.files += 1;
        counted.insertions += lines(added);
        counted.deletions += lines(removed);
        path = std::str::from_utf8(named)
            .ok()
            .filter(|named| !named.is_empty());
    }
    (statistics, path)
}

/// A `--numstat` count; a binary file's `-` counts none.
fn lines(column: &[u8]) -> u64 {
    std::str::from_utf8(column)
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(0)
}

/// How a path changed between two commits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Change {
    pub(super) kind: ChangeKind,
    /// The native path in the later commit; a deleted path's in the earlier one.
    pub(super) path: String,
    /// A renamed path's native path in the earlier commit.
    pub(super) original: Option<String>,
    /// Lines added and removed; none for a binary file.
    pub(super) insertions: Option<u64>,
    pub(super) deletions: Option<u64>,
}

/// The two commits whose changes one `git.changes` lists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ChangesQuery {
    /// The earlier commit, whose merge base with `to` the changes start from.
    pub(super) from: Option<String>,
    pub(super) to: String,
}

/// `git.changes`: the paths added, modified, deleted and renamed from `from` to `to` with their
/// line counts, as VS Code lists a commit's or a comparison's changes. With `from`, the changes
/// since the two commits' merge base; without it, everything `to` holds, as a first commit's
/// changes read against the empty tree. A path that is not UTF-8 is left out.
pub(super) fn changes(
    found: &Found,
    query: &ChangesQuery,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Vec<Change>, GitError> {
    let (from, to) = (query.from.as_deref(), query.to.as_str());
    if !command::is_object_name(to) || !from.is_none_or(command::is_object_name) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    let mut command = found.command(Access::Read, None);
    let flags = ["--raw", "--numstat", "--diff-filter=ADMR", "-z", RENAMES];
    match from {
        Some(from) => {
            command
                .arg("diff")
                .args(flags)
                .arg(format!("{from}...{to}"));
        }
        None => {
            let empty = if to.len() == 64 {
                EMPTY_TREE_SHA256
            } else {
                EMPTY_TREE_SHA1
            };
            command
                .args(["diff-tree", "-r"])
                .args(flags)
                .args([empty, to]);
        }
    }
    command.arg("--");
    let ran = runner::run(command, Vec::new(), limits, output)?.checked(Classifier::General)?;
    Ok(parse_changes(&ran.stdout, &found.toplevel))
}

/// `--raw` entries, then `--numstat` entries, each NUL-separated.
fn parse_changes(bytes: &[u8], toplevel: &str) -> Vec<Change> {
    let mut fields = bytes.split(|byte| *byte == 0).peekable();
    let mut changes = Vec::<(Change, String)>::new();
    let mut counts = std::collections::HashMap::<String, (Option<u64>, Option<u64>)>::new();
    while let Some(field) = fields.next() {
        if field.is_empty() {
            continue;
        }
        if let Some(raw) = field.strip_prefix(b":") {
            let status = raw.rsplit(|byte| *byte == b' ').next().unwrap_or_default();
            let first = fields.next().unwrap_or_default();
            let (kind, original, relative) = match status.first() {
                Some(b'A') => (ChangeKind::Added, None, first),
                Some(b'M') => (ChangeKind::Modified, None, first),
                Some(b'D') => (ChangeKind::Deleted, None, first),
                Some(b'R') => (
                    ChangeKind::Renamed,
                    Some(first),
                    fields.next().unwrap_or_default(),
                ),
                _ => break,
            };
            let Ok(relative) = std::str::from_utf8(relative) else {
                continue;
            };
            let original = match original.map(std::str::from_utf8) {
                Some(Ok(original)) => Some(native_paths::join(toplevel, original)),
                Some(Err(_)) => continue,
                None => None,
            };
            changes.push((
                Change {
                    kind,
                    path: native_paths::join(toplevel, relative),
                    original,
                    insertions: None,
                    deletions: None,
                },
                relative.to_owned(),
            ));
        } else {
            let mut columns = field.splitn(3, |byte| *byte == b'\t');
            let (Some(added), Some(removed), Some(named)) =
                (columns.next(), columns.next(), columns.next())
            else {
                break;
            };
            let named = if named.is_empty() {
                let _ = fields.next();
                fields.next().unwrap_or_default()
            } else {
                named
            };
            let count = |column: &[u8]| {
                std::str::from_utf8(column)
                    .ok()
                    .and_then(|count| count.parse::<u64>().ok())
            };
            if let Ok(named) = std::str::from_utf8(named) {
                counts.insert(named.to_owned(), (count(added), count(removed)));
            }
        }
    }
    changes
        .into_iter()
        .map(|(mut change, relative)| {
            if let Some((insertions, deletions)) = counts.get(&relative) {
                change.insertions = *insertions;
                change.deletions = *deletions;
            }
            change
        })
        .collect()
}

/// `git.merge-base`: the best common ancestor of two revisions, or none when their histories never
/// meet.
pub(super) fn merge_base(
    found: &Found,
    first: &str,
    second: &str,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Option<String>, GitError> {
    if !walkable(first) || !walkable(second) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    let mut command = found.command(Access::Read, None);
    command.args(["merge-base", first, second]);
    let ran = runner::run(command, Vec::new(), limits, output)?;
    if ran.exited_with(1) && ran.stderr.is_empty() {
        return Ok(None);
    }
    let ran = ran.checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&ran.stdout);
    let commit = text.trim();
    if command::is_object_name(commit) {
        Ok(Some(commit.to_owned()))
    } else {
        Err(GitError::new(ErrorCode::Failed))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Stash {
    /// `n` of `stash@{n}`.
    pub(super) index: u64,
    pub(super) commit: String,
    /// The commit the stash was taken on, its index, and with untracked files their commit.
    pub(super) parents: Vec<String>,
    /// The branch it was taken on.
    pub(super) branch: String,
    /// What follows the branch in its subject: the message, or HEAD's commit and subject.
    pub(super) description: String,
    /// Whether Git named it `WIP on`, taken without a message.
    pub(super) wip: bool,
    pub(super) author_time: i64,
    pub(super) commit_time: i64,
}

/// `git.stashes`: the stash list, newest first. As in VS Code, an entry whose subject does not read
/// `WIP on <branch>: ` or `On <branch>: ` is not listed.
pub(super) fn stashes(
    found: &Found,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Vec<Stash>, GitError> {
    let mut command = found.command(Access::Read, None);
    command.args([
        "-c",
        "i18n.logOutputEncoding=UTF-8",
        "stash",
        "list",
        "--format=%H%n%P%n%gd%n%gs%n%at%n%ct",
        "-z",
    ]);
    let ran = runner::run(command, Vec::new(), limits, output)?.checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&ran.stdout);
    Ok(text
        .split('\0')
        .filter_map(|record| stash(record.trim_start_matches('\n')))
        .collect())
}

fn stash(record: &str) -> Option<Stash> {
    let mut fields = record.split('\n');
    let commit = fields.next().filter(|name| command::is_object_name(name))?;
    let parents = fields
        .next()?
        .split(' ')
        .filter(|parent| !parent.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !parents.iter().all(|parent| command::is_object_name(parent)) {
        return None;
    }
    let index = fields
        .next()?
        .strip_prefix("stash@{")?
        .strip_suffix('}')?
        .parse::<u64>()
        .ok()?;
    let subject = fields.next()?;
    let (wip, rest) = match subject.get(..4) {
        Some(prefix) if prefix.eq_ignore_ascii_case("wip ") => (true, &subject[4..]),
        _ => (false, subject),
    };
    let rest = rest
        .get(..3)
        .filter(|prefix| prefix.eq_ignore_ascii_case("on "))
        .map(|_| &rest[3..])?;
    let (branch, description) = rest.split_once(": ")?;
    if branch.is_empty() {
        return None;
    }
    let author_time = fields.next()?.parse::<i64>().ok()?;
    let commit_time = fields.next()?.parse::<i64>().ok()?;
    Some(Stash {
        index,
        commit: commit.to_owned(),
        parents,
        branch: cut(branch.trim(), NAME_LIMIT).0,
        description: cut(description.trim(), MESSAGE_LIMIT).0,
        wip,
        author_time,
        commit_time,
    })
}
