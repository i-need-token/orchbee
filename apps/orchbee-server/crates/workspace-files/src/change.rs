//! Changing the repository with Git: staging, unstaging, discarding and committing. Paths are
//! Workspace-relative and Git runs in the Workspace directory, so each one names the path it
//! names in the change list. Path lists go to Git on standard input, never on a command line a
//! long list would overflow, and are literal: no name is read as a pattern.
use crate::files::Root;
use crate::git::{self, Change, Run};
use crate::status::prefix;
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES, WorkspaceFilesFailure as Failure,
    WorkspaceFilesOutcome as Outcome,
};
use std::collections::BTreeSet;
use std::time::Instant;

/// Raw output read from `git ls-files --unmerged` before the conflict list is refused as too long.
const CONFLICT_LIST_CAP: usize = 4 * 1024 * 1024;

/// Stages `paths`, or, for none, every change in the Workspace directory that is not a conflict.
pub(crate) fn stage(root: &Root, paths: Option<&[String]>, deadline: Instant) -> Outcome {
    if let Err(reason) = prefix(root, deadline) {
        return Outcome::Failed { reason };
    }
    stage_in_repository(root, paths, deadline).unwrap_or_else(|outcome| outcome)
}

/// Staging in a root known to be in a repository; `Err` carries an outcome that ends the request.
fn stage_in_repository(
    root: &Root,
    paths: Option<&[String]>,
    deadline: Instant,
) -> Result<Outcome, Outcome> {
    let (args, list): (&[&str], Vec<u8>) = match paths {
        Some(paths) => (
            &[
                "--literal-pathspecs",
                "add",
                "-A",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            nul_list(paths.iter().map(String::as_str)),
        ),
        // Everything beneath the Workspace directory except each conflict, which staging would
        // mark resolved without anyone looking at it.
        None => {
            let conflicts = conflicts(root, deadline)?;
            let mut list = b":(literal).\0".to_vec();
            for conflict in &conflicts {
                list.extend_from_slice(b":(exclude,literal)");
                list.extend_from_slice(conflict.as_bytes());
                list.push(0);
            }
            (
                &["add", "-A", "--pathspec-from-file=-", "--pathspec-file-nul"],
                list,
            )
        }
    };
    let outcome = finished(git::change(
        root.path(),
        args,
        Some(list),
        MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES,
        deadline,
    ));
    if matches!(outcome, Outcome::GitDone {}) {
        Ok(outcome)
    } else {
        Err(outcome)
    }
}

/// Takes `paths`, or everything staged in the Workspace directory, out of the index. On a branch
/// with no commit yet that removes them from the index.
pub(crate) fn unstage(root: &Root, paths: Option<&[String]>, deadline: Instant) -> Outcome {
    if let Err(reason) = prefix(root, deadline) {
        return Outcome::Failed { reason };
    }
    let list = paths.map_or_else(
        || b".\0".to_vec(),
        |paths| nul_list(paths.iter().map(String::as_str)),
    );
    finished(git::change(
        root.path(),
        &[
            "--literal-pathspecs",
            "reset",
            "-q",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        Some(list),
        MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES,
        deadline,
    ))
}

/// Puts tracked `paths` back to what the index holds. Git checks every path before it changes
/// any, so an untracked path or a conflict refuses the whole list.
pub(crate) fn discard(root: &Root, paths: &[String], deadline: Instant) -> Outcome {
    if let Err(reason) = prefix(root, deadline) {
        return Outcome::Failed { reason };
    }
    finished(git::change(
        root.path(),
        &[
            "--literal-pathspecs",
            "checkout",
            "-q",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        Some(nul_list(paths.iter().map(String::as_str))),
        MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES,
        deadline,
    ))
}

/// Commits what is staged, after staging every change that is not a conflict for `stage_all`.
/// The message goes on standard input; an amend with an empty message keeps the last one's.
pub(crate) fn commit(
    root: &Root,
    message: &str,
    amend: bool,
    stage_all: bool,
    deadline: Instant,
) -> Outcome {
    if let Err(reason) = prefix(root, deadline) {
        return Outcome::Failed { reason };
    }
    if stage_all && let Err(outcome) = stage_in_repository(root, None, deadline) {
        return outcome;
    }
    let keeps_message = amend && message.trim().is_empty();
    let mut args = vec!["commit", "-q"];
    if amend {
        args.push("--amend");
    }
    if keeps_message {
        args.push("--no-edit");
    } else {
        args.extend(["-F", "-"]);
    }
    finished(git::change(
        root.path(),
        &args,
        (!keeps_message).then(|| message.as_bytes().to_vec()),
        MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES,
        deadline,
    ))
}

/// The conflicts beneath the Workspace directory, relative to it.
fn conflicts(root: &Root, deadline: Instant) -> Result<BTreeSet<String>, Outcome> {
    let failed = |reason| Outcome::Failed { reason };
    let output = match git::run(
        root.path(),
        &["ls-files", "-z", "--unmerged", "--", "."],
        None,
        CONFLICT_LIST_CAP,
        deadline,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => stdout,
        Run::Exited { .. } | Run::Unavailable => return Err(failed(Failure::Unavailable)),
        Run::TimedOut => return Err(failed(Failure::Timeout)),
    };
    // Each record is `mode object stage<TAB>path`, one for each stage a conflict has.
    output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            let tab = record.iter().position(|byte| *byte == b'\t');
            tab.and_then(|tab| std::str::from_utf8(&record[tab + 1..]).ok())
                .map(str::to_owned)
                .ok_or_else(|| failed(Failure::Malformed))
        })
        .collect()
}

fn nul_list<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<u8> {
    let mut list = Vec::new();
    for path in paths {
        list.extend_from_slice(path.as_bytes());
        list.push(0);
    }
    list
}

/// The outcome of a finished Git command.
fn finished(change: Change) -> Outcome {
    match change {
        Change::Exited { success: true, .. } => Outcome::GitDone {},
        Change::Exited {
            success: false,
            output,
            truncated,
        } => {
            let (output, cut) = readable(&output);
            Outcome::GitRejected {
                output,
                truncated: truncated || cut,
            }
        }
        Change::TimedOut => Outcome::Failed {
            reason: Failure::Timeout,
        },
        Change::Unavailable => Outcome::Failed {
            reason: Failure::Unavailable,
        },
    }
}

/// What a person can read of Git's output: lossily decoded, colors and other escape sequences
/// and control characters removed, a carriage return read as a line break, surrounding blank
/// lines trimmed, and cut on a character boundary to the output cap; true when something was cut.
pub(crate) fn readable(output: &[u8]) -> (String, bool) {
    let decoded = String::from_utf8_lossy(output);
    let mut text = String::with_capacity(decoded.len());
    let mut chars = decoded.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                // A control sequence: ESC [ parameters and intermediates, then one final byte.
                if chars.next_if_eq(&'[').is_some() {
                    while chars
                        .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
                        .is_some()
                    {}
                    let _final = chars.next_if(|c| ('\u{40}'..='\u{7e}').contains(c));
                } else {
                    let _next = chars.next();
                }
            }
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    text.push('\n');
                }
            }
            '\n' | '\t' => text.push(c),
            c if c.is_control() => {}
            c => text.push(c),
        }
    }
    let trimmed = text.trim_matches(|c| c == '\n' || c == ' ' || c == '\t');
    if trimmed.len() <= MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES {
        return (trimmed.to_owned(), false);
    }
    let mut end = MAXIMUM_WORKSPACE_GIT_OUTPUT_BYTES;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    (trimmed[..end].to_owned(), true)
}
