//! The File Bar records of the engine's answers. Each value is checked against the bounds the
//! protocol states as it is built: the engine bounds what it reads from Git, so a value that still
//! falls outside them is one the Client could not name back, and it is left out rather than sent
//! altered. A summary or a relay snapshot keeps whole entries in order while their JSON fits one
//! Notice record, and a summary that leaves repositories out says it is truncated. A branch whose
//! upstream is gone has no upstream here, so the Client offers to publish it again.
//!
//! A history page keeps whole values in order while their JSON fits one Result record, and its
//! cursor names the first value it left for the next page; a value too large for any page is left
//! out. Git's times in seconds become milliseconds.

use super::{
    blame::Blame,
    engine::{Done, Ended, Prompt, RelayEvent, ScmSummary},
    errors::GitError,
    history::{Change, ChangeKind, Commit, LogPage, Paged, Stash},
    output::Page,
    reads::{RefsPage, UserConfig},
    workspace::{PageEntry, RepositorySummary, StatusPage},
};
use crate::requests::Outcome;
use orchbee_files_protocol::{
    BlameCommit, BlameRange, CommitName, Counter, ErrorCode, FilePath, FilesError, FilesNotice,
    FilesReply, GitBlameResult, GitChange, GitChangeKind, GitChangesResult, GitCheckIgnoreResult,
    GitCommitResult, GitInitResult, GitLogCommit, GitLogResult, GitMergeBaseResult, GitName,
    GitOutputResult, GitRef, GitRefsResult, GitRemote, GitStash, GitStashesResult, GitStatistics,
    GitStatusPageResult, GitUserConfigResult, Head, InvalidValue, List, MAXIMUM_REPLY_BYTES,
    MAXIMUM_SAFE_INTEGER, MutationState, PageCursor, PathItem, PathItemsResult, RefUpstream,
    RelaySnapshot, RelayUpdate, RequestOutcomeResult, SafeInteger, ScmRepository, Signed,
    StatusCode, StatusCounts, StatusEntry, Text, Upstream,
};
use serde::Serialize;
use std::{collections::HashSet, io};

/// What a summary's, a relay snapshot's or a history page's entries may take of a Notice or
/// Result body, leaving room for the fields around them.
const ENTRY_BUDGET: usize = MAXIMUM_REPLY_BYTES - 1024;
/// The most repositories, candidates or unsafe repositories a summary lists.
const SUMMARY_LIST: usize = 256;
/// The most prompts a relay snapshot carries.
const SNAPSHOT_PROMPTS: usize = 16;
/// The most values one page of each history read lists, as the protocol bounds them.
const LOG_PAGE: usize = 1000;
const BLAME_PAGE: usize = 10_000;
const CHANGES_PAGE: usize = 2000;
const STASHES_PAGE: usize = 1000;

/// `scm.subscribe`'s Snapshot and Updates.
pub(super) fn summary(summary: &ScmSummary) -> Option<FilesNotice> {
    let mut budget = ENTRY_BUDGET;
    let mut truncated = summary.summary.repositories_truncated;
    let mut repositories = Vec::new();
    for record in summary.summary.repositories.iter().filter_map(repository) {
        if repositories.len() == SUMMARY_LIST || !admit(&mut budget, &record) {
            truncated = true;
            break;
        }
        repositories.push(record);
    }
    let candidates = paths(&mut budget, &summary.summary.candidates);
    let unsafe_repositories = paths(&mut budget, &summary.summary.unsafe_paths);
    Some(FilesNotice::ScmSummary(Box::new(
        orchbee_files_protocol::ScmSummary {
            git: summary.git.clone(),
            repositories: List::new(repositories).ok()?,
            candidates: List::new(candidates).ok()?,
            unsafe_repositories: List::new(unsafe_repositories).ok()?,
            repositories_truncated: truncated,
        },
    )))
}

fn repository(shown: &RepositorySummary) -> Option<ScmRepository> {
    Some(ScmRepository {
        repository: FilePath::new(shown.repository.as_str()).ok()?,
        kind: shown.kind,
        head: Head {
            branch: shown
                .head
                .branch
                .as_deref()
                .and_then(|branch| Text::new(branch).ok()),
            commit: shown
                .head
                .commit
                .as_deref()
                .and_then(|commit| CommitName::new(commit).ok()),
            detached: shown.head.detached,
            unborn: shown.head.unborn,
        },
        upstream: shown.upstream.as_ref().and_then(|upstream| {
            let (ahead, behind) = upstream.divergence?;
            Some(Upstream {
                name: Text::new(upstream.name.as_str()).ok()?,
                ahead: safe(ahead),
                behind: safe(behind),
            })
        }),
        operation: shown.operation,
        counts: StatusCounts {
            merge: safe(count(shown.counts.merge)),
            index: safe(count(shown.counts.index)),
            working_tree: safe(count(shown.counts.working_tree)),
            untracked: safe(count(shown.counts.untracked)),
        },
        truncated: shown.truncated,
        revision: Counter::new(shown.revision),
        busy: shown.busy,
        warm: shown.warm,
        stale: shown.stale,
    })
}

/// The paths of `listed` that fit `budget`, in order, up to a summary list's bound.
fn paths(budget: &mut usize, listed: &[String]) -> Vec<FilePath> {
    let mut admitted = Vec::new();
    for path in listed
        .iter()
        .filter_map(|path| FilePath::new(path.as_str()).ok())
    {
        if admitted.len() == SUMMARY_LIST || !admit(budget, &path) {
            break;
        }
        admitted.push(path);
    }
    admitted
}

/// `relay.subscribe`'s Snapshot or Update for `event`; none for a prompt it cannot carry.
pub(super) fn relayed(event: &RelayEvent) -> Option<FilesNotice> {
    Some(match event {
        RelayEvent::Snapshot(prompts) => {
            let mut budget = ENTRY_BUDGET;
            let mut carried = Vec::new();
            for record in prompts.iter().filter_map(prompt) {
                if carried.len() == SNAPSHOT_PROMPTS || !admit(&mut budget, &record) {
                    break;
                }
                carried.push(record);
            }
            FilesNotice::RelaySnapshot(RelaySnapshot {
                prompts: List::new(carried).ok()?,
            })
        }
        RelayEvent::Opened(opened) => FilesNotice::RelayUpdate(RelayUpdate::Prompt {
            prompt: prompt(opened)?,
        }),
        RelayEvent::Closed(id) => FilesNotice::RelayUpdate(RelayUpdate::Closed { id: *id }),
    })
}

fn prompt(prompt: &Prompt) -> Option<orchbee_files_protocol::Prompt> {
    Some(orchbee_files_protocol::Prompt {
        id: prompt.id,
        kind: prompt.kind,
        text: Text::new(prompt.text.as_str()).ok()?,
        repository: FilePath::new(prompt.repository.as_str()).ok()?,
        path: match &prompt.path {
            Some(path) => Some(FilePath::new(path.as_str()).ok()?),
            None => None,
        },
    })
}

/// `git.status-page`.
pub(super) fn status_page(page: Result<StatusPage, GitError>) -> FilesReply {
    answered(page, |page| {
        Ok(FilesReply::GitStatusPage(GitStatusPageResult {
            entries: List::new(page.entries.into_iter().filter_map(status_entry).collect())?,
            cursor: page.cursor.map(page_cursor),
        }))
    })
}

fn status_entry(entry: PageEntry) -> Option<StatusEntry> {
    Some(StatusEntry {
        x: status_code(entry.x)?,
        y: status_code(entry.y)?,
        path: FilePath::new(entry.path).ok()?,
        original: match entry.original {
            Some(original) => Some(FilePath::new(original).ok()?),
            None => None,
        },
        submodule: entry.submodule,
    })
}

const fn status_code(letter: char) -> Option<StatusCode> {
    Some(match letter {
        '.' => StatusCode::Unmodified,
        'M' => StatusCode::Modified,
        'T' => StatusCode::TypeChanged,
        'A' => StatusCode::Added,
        'D' => StatusCode::Deleted,
        'R' => StatusCode::Renamed,
        'C' => StatusCode::Copied,
        'U' => StatusCode::Unmerged,
        '?' => StatusCode::Untracked,
        '!' => StatusCode::Ignored,
        _ => return None,
    })
}

/// `git.check-ignore`: the requested paths the repository ignores.
pub(super) fn check_ignore(ignored: Result<Vec<String>, GitError>) -> FilesReply {
    answered(ignored, |ignored| {
        Ok(FilesReply::GitCheckIgnore(GitCheckIgnoreResult {
            ignored: List::new(
                ignored
                    .into_iter()
                    .filter_map(|path| FilePath::new(path).ok())
                    .collect(),
            )?,
        }))
    })
}

/// `git.refs`.
pub(super) fn refs(page: Result<RefsPage, GitError>) -> FilesReply {
    answered(page, |page| {
        let refs = page
            .refs
            .into_iter()
            .filter_map(|reference| {
                Some(GitRef {
                    name: Text::new(reference.name).ok()?,
                    kind: reference.kind,
                    commit: CommitName::new(reference.commit).ok()?,
                    upstream: reference.upstream.and_then(|upstream| {
                        Some(RefUpstream {
                            remote: GitName::new(upstream.remote).ok()?,
                            name: GitName::new(upstream.name).ok()?,
                        })
                    }),
                })
            })
            .collect();
        let remotes = page
            .remotes
            .into_iter()
            .filter_map(|remote| {
                Some(GitRemote {
                    name: GitName::new(remote.name).ok()?,
                    fetch_url: remote.fetch_url.and_then(|url| Text::new(url).ok()),
                    push_url: remote.push_url.and_then(|url| Text::new(url).ok()),
                })
            })
            .collect();
        Ok(FilesReply::GitRefs(GitRefsResult {
            refs: List::new(refs)?,
            remotes: List::new(remotes)?,
            cursor: page.cursor.map(page_cursor),
        }))
    })
}

/// `git.user-config`.
pub(super) fn user_config(config: Result<UserConfig, GitError>) -> FilesReply {
    answered(config, |config| {
        Ok(FilesReply::GitUserConfig(GitUserConfigResult {
            name: config.name.and_then(|name| Text::new(name).ok()),
            email: config.email.and_then(|email| Text::new(email).ok()),
        }))
    })
}

/// `git.output`.
pub(super) fn output(page: Result<Page, GitError>) -> FilesReply {
    answered(page, |page| {
        Ok(FilesReply::GitOutput(GitOutputResult {
            // Text holds no NUL, which what Git wrote may.
            text: Text::new(page.text.replace('\0', "\u{fffd}"))?,
            cursor: Counter::new(page.cursor),
            dropped: Counter::new(page.dropped),
        }))
    })
}

/// `git.log`.
pub(super) fn log(page: Result<Paged<LogPage>, GitError>) -> FilesReply {
    answered(page, |Paged { start, value }| {
        let (commits, cursor) = paged(start, value.commits, value.more, LOG_PAGE, log_commit);
        Ok(FilesReply::GitLog(GitLogResult {
            commits: List::new(commits)?,
            cursor,
        }))
    })
}

/// A commit of a log page; a reference the protocol cannot carry is left out of it.
fn log_commit(commit: Commit) -> Option<GitLogCommit> {
    Some(GitLogCommit {
        commit: CommitName::new(commit.commit).ok()?,
        parents: commit_names(commit.parents)?,
        author_name: Text::new(commit.author_name).ok()?,
        author_email: Text::new(commit.author_email).ok()?,
        author_time_ms: milliseconds(commit.author_time),
        commit_time_ms: milliseconds(commit.commit_time),
        refs: List::new(
            commit
                .references
                .into_iter()
                .filter_map(|reference| Text::new(reference).ok())
                .collect(),
        )
        .ok()?,
        message: Text::new(commit.message).ok()?,
        message_cut: commit.message_cut,
        statistics: commit.statistics.map(|statistics| GitStatistics {
            files: safe(statistics.files),
            insertions: safe(statistics.insertions),
            deletions: safe(statistics.deletions),
        }),
        path: commit.path.map(FilePath::new).transpose().ok()?,
    })
}

/// `git.blame`: the ranges from the page's start, and each commit they name, described the first
/// time the page names it. A range whose commit Git did not describe is left out.
pub(super) fn blame(blame: Result<Paged<Blame>, GitError>) -> FilesReply {
    answered(blame, |Paged { start, value }| {
        let Blame {
            ranges: attributed,
            commits,
        } = value;
        let (total, mut budget) = (attributed.len(), ENTRY_BUDGET);
        let (mut ranges, mut described, mut named) = (Vec::new(), Vec::new(), HashSet::new());
        let mut taken = 0;
        for range in attributed {
            if ranges.len() == BLAME_PAGE {
                break;
            }
            // The page describes each commit once, with the first range that names it.
            let repeated = named.contains(&range.commit);
            let description = if repeated {
                None
            } else {
                commits
                    .get(&range.commit)
                    .and_then(|commit| blame_commit(&range.commit, commit))
            };
            let record = blame_range(&range).filter(|_| repeated || description.is_some());
            if let Some(record) = record {
                let its = description.as_ref().map_or(Some(0), size);
                let needed = size(&record)
                    .zip(its)
                    .map(|(own, its)| own.saturating_add(its));
                match needed.and_then(|needed| budget.checked_sub(needed)) {
                    Some(left) => {
                        budget = left;
                        if let Some(description) = description {
                            named.insert(range.commit);
                            described.push(description);
                        }
                        ranges.push(record);
                    }
                    // Too large for any page.
                    None if ranges.is_empty() => {}
                    None => break,
                }
            }
            taken += 1;
        }
        Ok(FilesReply::GitBlame(GitBlameResult {
            ranges: List::new(ranges)?,
            commits: List::new(described)?,
            cursor: (taken < total).then(|| page_cursor(start.saturating_add(count(taken)))),
        }))
    })
}

fn blame_range(range: &super::blame::BlameRange) -> Option<BlameRange> {
    Some(BlameRange {
        start: SafeInteger::new(range.start).ok()?,
        end: SafeInteger::new(range.end).ok()?,
        commit: CommitName::new(range.commit.as_str()).ok()?,
    })
}

fn blame_commit(name: &str, commit: &super::blame::BlameCommit) -> Option<BlameCommit> {
    Some(BlameCommit {
        commit: CommitName::new(name).ok()?,
        author_name: Text::new(commit.author_name.as_str()).ok()?,
        author_email: Text::new(commit.author_email.as_str()).ok()?,
        author_time_ms: milliseconds(commit.author_time),
        // Text holds no NUL, which a summary may.
        summary: Text::new(commit.summary.replace('\0', "\u{fffd}")).ok()?,
    })
}

/// `git.changes`.
pub(super) fn changes(changes: Result<Paged<Vec<Change>>, GitError>) -> FilesReply {
    answered(changes, |Paged { start, value }| {
        let (changes, cursor) = paged(start, value, false, CHANGES_PAGE, change);
        Ok(FilesReply::GitChanges(GitChangesResult {
            changes: List::new(changes)?,
            cursor,
        }))
    })
}

fn change(change: Change) -> Option<GitChange> {
    Some(GitChange {
        kind: match change.kind {
            ChangeKind::Added => GitChangeKind::Added,
            ChangeKind::Modified => GitChangeKind::Modified,
            ChangeKind::Deleted => GitChangeKind::Deleted,
            ChangeKind::Renamed => GitChangeKind::Renamed,
        },
        path: FilePath::new(change.path).ok()?,
        original: change.original.map(FilePath::new).transpose().ok()?,
        insertions: change.insertions.map(safe),
        deletions: change.deletions.map(safe),
    })
}

/// `git.merge-base`.
pub(super) fn merge_base(base: Result<Option<String>, GitError>) -> FilesReply {
    answered(base, |commit| {
        Ok(FilesReply::GitMergeBase(GitMergeBaseResult {
            commit: commit.map(CommitName::new).transpose()?,
        }))
    })
}

/// `git.stashes`.
pub(super) fn stashes(stashes: Result<Paged<Vec<Stash>>, GitError>) -> FilesReply {
    answered(stashes, |Paged { start, value }| {
        let (stashes, cursor) = paged(start, value, false, STASHES_PAGE, stash);
        Ok(FilesReply::GitStashes(GitStashesResult {
            stashes: List::new(stashes)?,
            cursor,
        }))
    })
}

fn stash(stash: Stash) -> Option<GitStash> {
    Some(GitStash {
        index: SafeInteger::new(stash.index).ok()?,
        commit: CommitName::new(stash.commit).ok()?,
        parents: commit_names(stash.parents)?,
        branch: Text::new(stash.branch).ok()?,
        description: Text::new(stash.description).ok()?,
        wip: stash.wip,
        author_time_ms: milliseconds(stash.author_time),
        commit_time_ms: milliseconds(stash.commit_time),
    })
}

/// One page of `values`, which start at index `start` of the whole listing: the records `record`
/// makes of them, in order, while their JSON fits one Result record and they number at most
/// `most`. A value the protocol cannot carry, or one too large for any page, is left out. The
/// cursor names the first value the page left, while one is left or `more` says the listing goes
/// on.
fn paged<T, R: Serialize>(
    start: u64,
    values: Vec<T>,
    more: bool,
    most: usize,
    record: impl Fn(T) -> Option<R>,
) -> (Vec<R>, Option<PageCursor>) {
    let (total, mut budget) = (values.len(), ENTRY_BUDGET);
    let (mut records, mut taken) = (Vec::new(), 0);
    for value in values {
        if records.len() == most {
            break;
        }
        if let Some(made) = record(value) {
            if admit(&mut budget, &made) {
                records.push(made);
            } else if !records.is_empty() {
                break;
            }
        }
        taken += 1;
    }
    let cursor = (taken < total || more).then(|| page_cursor(start.saturating_add(count(taken))));
    (records, cursor)
}

/// Commit names as the protocol lists them; `None` when one is not a commit name or they are too
/// many.
fn commit_names<const MAX: usize>(names: Vec<String>) -> Option<List<CommitName, 0, MAX>> {
    let names = names
        .into_iter()
        .map(CommitName::new)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    List::new(names).ok()
}

/// Git's `seconds` since the Unix epoch in milliseconds, held to the protocol's range.
fn milliseconds(seconds: i64) -> Signed {
    let limit = i64::try_from(MAXIMUM_SAFE_INTEGER).unwrap_or(i64::MAX);
    Signed::new(seconds.saturating_mul(1000).clamp(-limit, limit)).unwrap_or_default()
}

/// The answer of a request that reports only its success.
pub(super) fn empty(result: Result<(), GitError>) -> FilesReply {
    answered(result, |()| Ok(FilesReply::Empty))
}

/// How a mutation ended, as its Result record carries it.
pub(super) fn done(ended: Ended) -> FilesReply {
    mutation(ended).into()
}

/// `request.outcome`.
pub(super) fn outcome(outcome: Result<Outcome<Ended>, GitError>) -> FilesReply {
    answered(outcome, |outcome| {
        Ok(FilesReply::RequestOutcome(RequestOutcomeResult {
            state: match outcome {
                Outcome::Completed(ended) => MutationState::Completed(mutation(ended)),
                Outcome::Running => MutationState::Running,
                Outcome::Unknown => MutationState::Unknown,
            },
        }))
    })
}

/// How a mutation ended, as `request.outcome` repeats it. Paths that one Git command failed for
/// share its error; only the first carries what Git wrote, which the Git Output also holds.
fn mutation(ended: Ended) -> orchbee_files_protocol::MutationReply {
    use orchbee_files_protocol::{ErrorReply, MutationReply};
    let failed = || {
        MutationReply::Error(ErrorReply {
            error: FilesError::new(ErrorCode::Failed),
        })
    };
    let done = match ended {
        Ok(done) => done,
        Err(error) => {
            return MutationReply::Error(ErrorReply {
                error: error.into_wire(),
            });
        }
    };
    match done {
        Done::Empty => MutationReply::Empty(orchbee_files_protocol::Empty {}),
        Done::Cleaned(cleaned) => {
            let mut told = HashSet::new();
            let items = cleaned
                .into_iter()
                .map(|item| {
                    let error = item.error.map(|mut error| {
                        if error
                            .detail
                            .as_ref()
                            .is_some_and(|detail| !told.insert(detail.clone()))
                        {
                            error.detail = None;
                        }
                        error.into_wire()
                    });
                    Some(PathItem {
                        path: FilePath::new(item.path).ok()?,
                        error,
                    })
                })
                .collect::<Option<Vec<_>>>();
            items
                .and_then(|items| List::new(items).ok())
                .map_or_else(failed, |items| {
                    MutationReply::PathItems(PathItemsResult { items })
                })
        }
        Done::Committed(commit) => CommitName::new(commit).map_or_else(
            |_| failed(),
            |commit| MutationReply::GitCommit(GitCommitResult { commit }),
        ),
        Done::Initialized(repository) => FilePath::new(repository).map_or_else(
            |_| failed(),
            |repository| MutationReply::GitInit(GitInitResult { repository }),
        ),
    }
}

/// The record of `result`: its error, or what `record` makes of its value, which fails only for
/// a value the protocol cannot carry.
fn answered<T>(
    result: Result<T, GitError>,
    record: impl FnOnce(T) -> Result<FilesReply, InvalidValue>,
) -> FilesReply {
    match result {
        Ok(value) => {
            record(value).unwrap_or_else(|_| FilesReply::Error(FilesError::new(ErrorCode::Failed)))
        }
        Err(error) => FilesReply::Error(error.into_wire()),
    }
}

/// The index a page cursor names; one this engine did not write names an index past every page,
/// which reads as the end.
pub(super) fn page_index(cursor: PageCursor) -> u64 {
    u64::try_from(u128::from_be_bytes(*cursor.as_bytes())).unwrap_or(u64::MAX)
}

fn page_cursor(index: u64) -> PageCursor {
    PageCursor::from_bytes(u128::from(index).to_be_bytes())
}

/// Takes what `value`'s JSON and its separator need from `budget`; false, taking nothing, when
/// they do not fit.
fn admit(budget: &mut usize, value: &impl Serialize) -> bool {
    match size(value).and_then(|needed| budget.checked_sub(needed)) {
        Some(left) => {
            *budget = left;
            true
        }
        None => false,
    }
}

/// What `value`'s JSON and its separator take of a record.
fn size(value: &impl Serialize) -> Option<usize> {
    let mut measure = Measure(1);
    serde_json::to_writer(&mut measure, value)
        .ok()
        .map(|()| measure.0)
}

/// Counts the bytes written to it and keeps none.
struct Measure(usize);

impl io::Write for Measure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn safe(value: u64) -> SafeInteger {
    SafeInteger::new(value.min(MAXIMUM_SAFE_INTEGER)).unwrap_or_default()
}
