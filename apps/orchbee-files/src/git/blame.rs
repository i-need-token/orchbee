//! `git.blame`: which commit last changed each line of a file, as VS Code's Git extension blames
//! at ef5ca45 with `--incremental`, at a commit or in the working tree. Lines no commit holds
//! belong to the all-zero object name, which Git attributes to "Not Committed Yet".

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode},
    history::{MESSAGE_LIMIT, NAME_LIMIT, cut},
    output::GitOutput,
    runner::{self, Limits},
};
use orchbee_files_protocol::ErrorCode;
use std::{collections::HashMap, sync::Mutex};

/// A run of lines one commit last changed, numbered from 1 in the blamed version, both ends
/// included.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BlameRange {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) commit: String,
}

/// What Git says of a commit the first time a range names it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BlameCommit {
    pub(super) author_name: String,
    pub(super) author_email: String,
    /// Seconds since the Unix epoch.
    pub(super) author_time: i64,
    pub(super) summary: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Blame {
    /// In the order Git attributed them.
    pub(super) ranges: Vec<BlameRange>,
    pub(super) commits: HashMap<String, BlameCommit>,
}

/// What one blame reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BlameQuery {
    /// The file, a native path in the repository.
    pub(super) path: String,
    /// The commit whose version is blamed; `None` blames the working tree.
    pub(super) commit: Option<String>,
    /// Passes `-w`.
    pub(super) ignore_whitespace: bool,
}

/// Blames the file `query` names.
pub(super) fn blame(
    found: &Found,
    query: &BlameQuery,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Blame, GitError> {
    let relative = found.relative(&query.path)?;
    let commit = query.commit.as_deref();
    if !commit.is_none_or(command::is_object_name) {
        return Err(GitError::new(ErrorCode::InvalidRequest));
    }
    let mut command = found.command(Access::Read, None);
    command.args([
        "-c",
        "i18n.logOutputEncoding=UTF-8",
        "blame",
        "--root",
        "--incremental",
    ]);
    if query.ignore_whitespace {
        command.arg("-w");
    }
    command.args(commit);
    command.arg("--").arg(relative);
    let ran = runner::run(command, Vec::new(), limits, output)?;
    if !ran.exited_with(0) && String::from_utf8_lossy(&ran.stderr).contains("fatal: no such path") {
        return Err(GitError::git(GitErrorCode::NoPathFound));
    }
    let ran = ran.checked(Classifier::General)?;
    Ok(parse(&String::from_utf8_lossy(&ran.stdout)))
}

/// Reads `--incremental` output: each block opens with `<commit> <source line> <line> <count>`,
/// carries the commit's properties the first time it names the commit, and closes with its
/// `filename`.
fn parse(text: &str) -> Blame {
    let mut blame = Blame::default();
    let mut open: Option<(BlameRange, BlameCommit)> = None;
    for line in text.lines() {
        let Some((_, commit)) = open.as_mut() else {
            open = header(line).map(|range| {
                let commit = BlameCommit {
                    author_name: String::new(),
                    author_email: String::new(),
                    author_time: 0,
                    summary: String::new(),
                };
                (range, commit)
            });
            continue;
        };
        if let Some(name) = line.strip_prefix("author ") {
            commit.author_name = cut(name, NAME_LIMIT).0;
        } else if let Some(mail) = line.strip_prefix("author-mail ") {
            let mail = mail.strip_prefix('<').unwrap_or(mail);
            commit.author_email = cut(mail.strip_suffix('>').unwrap_or(mail), NAME_LIMIT).0;
        } else if let Some(time) = line.strip_prefix("author-time ") {
            commit.author_time = time.parse().unwrap_or(0);
        } else if let Some(summary) = line.strip_prefix("summary ") {
            commit.summary = cut(summary, MESSAGE_LIMIT).0;
        } else if line.starts_with("filename ")
            && let Some((range, commit)) = open.take()
        {
            blame.commits.entry(range.commit.clone()).or_insert(commit);
            blame.ranges.push(range);
        }
    }
    blame
}

/// A block's first line: the commit, its line in the source, its first line here, and how many.
fn header(line: &str) -> Option<BlameRange> {
    let mut parts = line.split(' ');
    let commit = parts.next().filter(|name| command::is_object_name(name))?;
    let _source = parts.next()?;
    let start = parts
        .next()?
        .parse::<u64>()
        .ok()
        .filter(|start| *start > 0)?;
    let count = parts
        .next()?
        .parse::<u64>()
        .ok()
        .filter(|count| *count > 0)?;
    Some(BlameRange {
        start,
        end: start.checked_add(count - 1)?,
        commit: commit.to_ascii_lowercase(),
    })
}
