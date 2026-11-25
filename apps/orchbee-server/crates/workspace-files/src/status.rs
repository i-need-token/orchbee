//! Git facts: a file's content in the index or `HEAD`, and the Workspace's change list.
use crate::files::{self, Root};
use crate::git::{self, Run};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_ENTRIES, MAXIMUM_WORKSPACE_FILE_TEXT_BYTES, WorkspaceFileContent,
    WorkspaceFileSource, WorkspaceFilesFailure as Failure, WorkspaceFilesOutcome as Outcome,
    WorkspaceGitStatusCode as Code, WorkspaceGitStatusEntry, is_workspace_relative_path,
};
use std::time::Instant;

/// Raw `git status` output read before the change list reports truncation.
const STATUS_OUTPUT_CAP: usize = 16 * 1024 * 1024;
/// Encoded path bytes one change list carries, well inside the result cap.
const STATUS_PATH_BUDGET: usize = 6 * 1024 * 1024;

/// The root's path inside its repository, `""` at the top or `dir/` beneath it.
fn prefix(root: &Root, deadline: Instant) -> Result<String, Failure> {
    match git::run(
        root.path(),
        &["rev-parse", "--show-prefix"],
        None,
        8 * 1024,
        deadline,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => String::from_utf8(stdout)
            .map(|prefix| prefix.trim_end_matches('\n').to_owned())
            .map_err(|_| Failure::Malformed),
        Run::Exited { .. } => Err(Failure::NotRepository),
        Run::TimedOut => Err(Failure::Timeout),
        Run::Unavailable => Err(Failure::Unavailable),
    }
}

pub(crate) fn read_blob(
    root: &Root,
    relative: &str,
    source: WorkspaceFileSource,
    deadline: Instant,
) -> Outcome {
    if let Err(reason) = prefix(root, deadline) {
        return Outcome::Failed { reason };
    }
    let object = match source {
        WorkspaceFileSource::Index => format!(":./{relative}"),
        WorkspaceFileSource::Head => format!("HEAD:./{relative}"),
        WorkspaceFileSource::Worktree => return files::read_worktree(root, relative),
    };
    let size = match git::run(
        root.path(),
        &["cat-file", "-s", &object],
        None,
        64,
        deadline,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => std::str::from_utf8(&stdout)
            .ok()
            .and_then(|size| size.trim().parse::<u64>().ok()),
        // No such object: an untracked file has no index or `HEAD` content, nor anything before
        // the first commit.
        Run::Exited { .. } => return files::absent(),
        Run::TimedOut => {
            return Outcome::Failed {
                reason: Failure::Timeout,
            };
        }
        Run::Unavailable => {
            return Outcome::Failed {
                reason: Failure::Unavailable,
            };
        }
    };
    let Some(byte_length) = size else {
        return Outcome::Failed {
            reason: Failure::Malformed,
        };
    };
    if byte_length > MAXIMUM_WORKSPACE_FILE_TEXT_BYTES as u64 {
        return Outcome::File {
            byte_length,
            content: WorkspaceFileContent::TooLarge {},
        };
    }
    match git::run(
        root.path(),
        &["cat-file", "blob", &object],
        None,
        MAXIMUM_WORKSPACE_FILE_TEXT_BYTES + 1,
        deadline,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated: false,
        } => files::file_outcome(stdout),
        // A path naming a tree or submodule commit has no blob to show.
        Run::Exited { .. } => Outcome::Failed {
            reason: Failure::NotFound,
        },
        Run::TimedOut => Outcome::Failed {
            reason: Failure::Timeout,
        },
        Run::Unavailable => Outcome::Failed {
            reason: Failure::Unavailable,
        },
    }
}

pub(crate) fn git_status(root: &Root, deadline: Instant) -> Outcome {
    let prefix = match prefix(root, deadline) {
        Ok(prefix) => prefix,
        Err(reason) => return Outcome::Failed { reason },
    };
    match git::run(
        root.path(),
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
            "--",
            ".",
        ],
        None,
        STATUS_OUTPUT_CAP,
        deadline,
    ) {
        Run::Exited {
            status: Some(0),
            stdout,
            truncated,
        } => match parse_status(&stdout, &prefix, truncated) {
            Ok(outcome) => outcome,
            Err(reason) => Outcome::Failed { reason },
        },
        Run::Exited { .. } => Outcome::Failed {
            reason: Failure::Unavailable,
        },
        Run::TimedOut => Outcome::Failed {
            reason: Failure::Timeout,
        },
        Run::Unavailable => Outcome::Failed {
            reason: Failure::Unavailable,
        },
    }
}

fn code(byte: u8) -> Result<Code, Failure> {
    Ok(match byte {
        b'.' => Code::Unmodified,
        b'M' => Code::Modified,
        b'T' => Code::TypeChanged,
        b'A' => Code::Added,
        b'D' => Code::Deleted,
        b'R' => Code::Renamed,
        b'C' => Code::Copied,
        b'U' => Code::Unmerged,
        _ => return Err(Failure::Malformed),
    })
}

/// Parses `git status --porcelain=v2 -z --branch` into a change list relative to `prefix`. A
/// `cut` output's last, possibly partial record is dropped and the list reports truncation.
pub(crate) fn parse_status(output: &[u8], prefix: &str, cut: bool) -> Result<Outcome, Failure> {
    let mut records: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    // A complete output ends with a NUL, leaving one empty piece; a cut one ends mid-record.
    records.pop();
    let mut truncated = cut;
    let mut branch = None;
    let mut entries = Vec::new();
    let mut budget = STATUS_PATH_BUDGET;
    let mut records = records.into_iter();
    let relative = |path: &str| -> Option<String> {
        let path = path.strip_prefix(prefix)?;
        is_workspace_relative_path(path).then(|| path.to_owned())
    };
    while let Some(record) = records.next() {
        let record = std::str::from_utf8(record).map_err(|_| Failure::Malformed)?;
        if let Some(header) = record.strip_prefix("# ") {
            if let Some(head) = header.strip_prefix("branch.head ") {
                branch = (head != "(detached)"
                    && !head.is_empty()
                    && head.len() <= 1024
                    && !head.chars().any(char::is_control))
                .then(|| head.to_owned());
            }
            continue;
        }
        let (kind, rest) = record.split_at_checked(1).ok_or(Failure::Malformed)?;
        let rest = rest.strip_prefix(' ').ok_or(Failure::Malformed)?;
        let (index, worktree, conflicted, path, original) = match kind {
            "1" => {
                // XY sub mH mI mW hH hI path
                let fields: Vec<&str> = rest.splitn(8, ' ').collect();
                let [xy, _, _, _, _, _, _, path] = fields[..] else {
                    return Err(Failure::Malformed);
                };
                let xy = xy.as_bytes();
                if xy.len() != 2 {
                    return Err(Failure::Malformed);
                }
                (code(xy[0])?, code(xy[1])?, false, path, None)
            }
            "2" => {
                // XY sub mH mI mW hH hI Xscore path NUL origPath
                let fields: Vec<&str> = rest.splitn(9, ' ').collect();
                let [xy, _, _, _, _, _, _, _, path] = fields[..] else {
                    return Err(Failure::Malformed);
                };
                let xy = xy.as_bytes();
                if xy.len() != 2 {
                    return Err(Failure::Malformed);
                }
                let Some(original) = records.next() else {
                    truncated = true;
                    break;
                };
                let original = std::str::from_utf8(original).map_err(|_| Failure::Malformed)?;
                (code(xy[0])?, code(xy[1])?, false, path, Some(original))
            }
            "u" => {
                // XY sub m1 m2 m3 mW h1 h2 h3 path
                let fields: Vec<&str> = rest.splitn(10, ' ').collect();
                let [xy, _, _, _, _, _, _, _, _, path] = fields[..] else {
                    return Err(Failure::Malformed);
                };
                let xy = xy.as_bytes();
                if xy.len() != 2 {
                    return Err(Failure::Malformed);
                }
                (code(xy[0])?, code(xy[1])?, true, path, None)
            }
            "?" => (Code::Untracked, Code::Untracked, false, rest, None),
            // Ignored entries are not asked for; anything else is a newer Git's record.
            _ => continue,
        };
        let Some(path) = relative(path) else {
            continue;
        };
        let original_path = original.and_then(relative);
        let cost = path.len() + original_path.as_ref().map_or(0, String::len) + 128;
        if entries.len() == MAXIMUM_WORKSPACE_FILE_ENTRIES || cost > budget {
            truncated = true;
            break;
        }
        budget -= cost;
        entries.push(WorkspaceGitStatusEntry {
            path,
            original_path,
            index,
            worktree,
            conflicted,
        });
    }
    Ok(Outcome::GitStatus {
        branch,
        entries,
        truncated,
    })
}
