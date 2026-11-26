//! Replacing what a search found. Each file is searched again with the search's own matcher and
//! written only while every match the client names still starts where the search found it, and
//! only while the file is still what was read, so a file that changed since the search, or while
//! it was replaced, is left as it is.
mod pattern;

use crate::files::{Root, hex};
use crate::search::matcher;
use crate::write::write_worktree;
use grep_matcher::{Captures as _, Matcher as _};
use grep_regex::RegexMatcher;
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES, WorkspaceFilesFailure as Failure,
    WorkspaceFilesOutcome as Outcome, WorkspaceReplaceFile, WorkspaceReplaceMatch,
    WorkspaceReplaceQuery, WorkspaceReplaceResult as Result, WorkspaceReplacedFile,
    WorkspaceSearchQuery,
};
use pattern::{Group, Replacement};
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::time::Instant;

/// Replaces the matches `files` name beneath `root`, one file at a time; a file not yet started
/// at `deadline` answers `timeout` and is left as it is.
pub(crate) fn replace_matches(
    root: &Root,
    query: &WorkspaceReplaceQuery,
    files: &[WorkspaceReplaceFile],
    deadline: Instant,
) -> Outcome {
    let Some((matcher, multi_line)) = matcher(&WorkspaceSearchQuery {
        pattern: query.pattern.clone(),
        regex: query.regex,
        match_case: query.match_case,
        whole_word: query.whole_word,
        includes: Vec::new(),
        excludes: Vec::new(),
        use_ignore_files: false,
        paths: None,
    }) else {
        return Outcome::Failed {
            reason: Failure::Malformed,
        };
    };
    let plan = Plan {
        matcher,
        multi_line,
        replacement: Replacement::parse(&query.replacement, query.regex),
        preserve_case: query.preserve_case,
    };
    Outcome::Replaced {
        files: files
            .iter()
            .map(|file| WorkspaceReplacedFile {
                path: file.path.clone(),
                result: if Instant::now() >= deadline {
                    Result::Failed {
                        reason: Failure::Timeout,
                    }
                } else {
                    replace_file(root, &plan, file)
                },
            })
            .collect(),
    }
}

/// What every file of one replacement shares.
struct Plan {
    matcher: RegexMatcher,
    multi_line: bool,
    replacement: Replacement,
    preserve_case: bool,
}

fn replace_file(root: &Root, plan: &Plan, file: &WorkspaceReplaceFile) -> Result {
    let path = match root.resolve(&file.path) {
        Ok(path) => path,
        Err(reason) => return Result::Failed { reason },
    };
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            return Result::Failed {
                reason: Failure::NotFound,
            };
        }
        Err(error) => {
            return Result::Failed {
                reason: io_failure(&error),
            };
        }
    };
    if metadata.len() > MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES {
        return Result::NotEditable {};
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Result::Failed {
                reason: io_failure(&error),
            };
        }
    };
    if bytes.len() as u64 > MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES || bytes.contains(&0) {
        return Result::NotEditable {};
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Result::NotEditable {};
    };
    let Some(replaced) = replaced_text(plan, text, &file.matches) else {
        return Result::Changed {};
    };
    if replaced.len() as u64 > MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES {
        return Result::NotEditable {};
    }
    let expected = hex(&Sha256::digest(&bytes));
    match write_worktree(root, &file.path, &replaced, Some(&expected)) {
        Outcome::Written { version } => Result::Replaced { version },
        Outcome::Conflict { .. } => Result::Changed {},
        Outcome::Failed { reason } => Result::Failed { reason },
        _ => Result::Failed {
            reason: Failure::Unavailable,
        },
    }
}

/// `text` with the matches at `wanted` replaced, or none when one of them no longer starts a
/// match.
fn replaced_text(plan: &Plan, text: &str, wanted: &[WorkspaceReplaceMatch]) -> Option<String> {
    let found = matches(plan, text);
    let mut wanted = wanted.iter().peekable();
    let mut replaced = String::with_capacity(text.len());
    let mut copied = 0;
    for found in &found {
        if wanted.peek() != Some(&&found.start) {
            continue;
        }
        wanted.next();
        replaced.push_str(&text[copied..found.from]);
        replaced.push_str(&found.replacement);
        copied = found.to;
    }
    if wanted.next().is_some() {
        return None;
    }
    replaced.push_str(&text[copied..]);
    Some(replaced)
}

/// One match in a file: where it starts as a search reports it, its byte span and what it
/// becomes.
struct Found {
    start: WorkspaceReplaceMatch,
    from: usize,
    to: usize,
    replacement: String,
}

/// The matches a search reports in `text`, in order: matched line by line as the search does, or
/// across the whole file for a pattern that spans lines, leaving out empty matches.
fn matches(plan: &Plan, text: &str) -> Vec<Found> {
    let mut spans = Vec::new();
    if plan.multi_line {
        collect(plan, text, 0, text.len(), &mut spans);
    } else {
        let mut start = 0;
        for line in text.split_inclusive('\n') {
            collect(plan, text, start, start + line.len(), &mut spans);
            start += line.len();
        }
    }
    let mut lines = Lines::default();
    spans
        .into_iter()
        .filter_map(|(from, to, replacement)| {
            Some(Found {
                start: lines.position(text, from)?,
                from,
                to,
                replacement,
            })
        })
        .collect()
}

/// Adds the matches in `text[start..end]`, as byte spans of `text` with what each becomes.
fn collect(
    plan: &Plan,
    text: &str,
    start: usize,
    end: usize,
    spans: &mut Vec<(usize, usize, String)>,
) {
    let Ok(mut captures) = plan.matcher.new_captures() else {
        return;
    };
    let _searched =
        plan.matcher
            .captures_iter(&text.as_bytes()[start..end], &mut captures, |captures| {
                let Some(whole) = captures.get(0).filter(|whole| !whole.is_empty()) else {
                    return true;
                };
                let (from, to) = (start + whole.start(), start + whole.end());
                let Some(matched) = text.get(from..to) else {
                    return true;
                };
                let group = |group: &Group| {
                    let index = match group {
                        Group::Index(index) => Some(*index),
                        Group::Name(name) => plan.matcher.capture_index(name),
                    }?;
                    (index < captures.len()).then(|| {
                        captures
                            .get(index)
                            .and_then(|span| text.get(start + span.start()..start + span.end()))
                    })
                };
                let replacement = plan.replacement.expand(matched, plan.preserve_case, group);
                spans.push((from, to, replacement));
                true
            });
}

/// Walks a text forwards, counting its lines and the UTF-16 code units of the current one, to say
/// where each byte offset asked about stands.
#[derive(Default)]
struct Lines {
    offset: usize,
    line: u64,
    /// The code units of the current line before `offset`.
    units: usize,
}

impl Lines {
    /// Where the byte at `offset`, no earlier than the last one asked about, stands as a search
    /// reports it: its line counted from 1 and the UTF-16 code units of that line before it, a
    /// carriage return just before a line feed at `offset` not counted, as a preview leaves it
    /// out.
    fn position(&mut self, text: &str, offset: usize) -> Option<WorkspaceReplaceMatch> {
        let passed = text.get(self.offset..offset)?;
        match passed.rfind('\n') {
            Some(last) => {
                self.line += u64::try_from(passed.matches('\n').count()).ok()?;
                self.units = passed[last + 1..].encode_utf16().count();
            }
            None => self.units += passed.encode_utf16().count(),
        }
        self.offset = offset;
        let dropped = text[..offset].ends_with('\r') && text[offset..].starts_with('\n');
        Some(WorkspaceReplaceMatch {
            line: self.line.checked_add(1)?,
            column: u32::try_from(self.units - usize::from(dropped)).ok()?,
        })
    }
}

fn io_failure(error: &std::io::Error) -> Failure {
    match error.kind() {
        ErrorKind::NotFound => Failure::NotFound,
        ErrorKind::PermissionDenied => Failure::Denied,
        _ => Failure::Unavailable,
    }
}

#[cfg(test)]
mod tests;
