//! One search: the matcher and file filter a query asks for, the walk over the Workspace on
//! several threads, and each file's matching lines.
use super::preview::previews;
use crate::files::Root;
use grep_matcher::{LineTerminator, Matcher as _};
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, Sink, SinkError as _, SinkMatch};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{WalkBuilder, WalkState};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_SEARCH_MATCHES, WorkspaceFilesOperation, WorkspaceSearchFile,
    WorkspaceSearchLine, WorkspaceSearchQuery, WorkspaceSearchState,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

/// Lines one result entry carries; a file with more comes as several entries in a row.
pub(super) const PIECE_LINES: usize = 64;
/// The most a line, or a multi-line search's file, may hold in memory; past it the file is
/// skipped rather than read whole.
const HEAP_LIMIT: usize = 64 * 1024 * 1024;

/// What the threads of one search share.
pub(super) struct Shared<'a> {
    cancelled: &'a AtomicBool,
    matches: AtomicU64,
    /// A match was left out because the search had already found its most.
    limited: AtomicBool,
}

impl<'a> Shared<'a> {
    pub(super) fn new(cancelled: &'a AtomicBool) -> Self {
        Self {
            cancelled,
            matches: AtomicU64::new(0),
            limited: AtomicBool::new(false),
        }
    }

    fn stopped(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed) || self.limited.load(Ordering::Relaxed)
    }

    /// Counts one match, or refuses it once the search has found its most.
    fn admit(&self) -> bool {
        if self.matches.fetch_add(1, Ordering::Relaxed) < MAXIMUM_WORKSPACE_SEARCH_MATCHES {
            true
        } else {
            self.matches.fetch_sub(1, Ordering::Relaxed);
            self.limited.store(true, Ordering::Relaxed);
            false
        }
    }

    /// How the search ended once its walk is over.
    pub(super) fn state(&self) -> WorkspaceSearchState {
        if self.limited.load(Ordering::Relaxed) {
            WorkspaceSearchState::LimitReached
        } else {
            WorkspaceSearchState::Complete
        }
    }
}

/// A query made ready to run beneath its root.
pub(super) struct Plan {
    root: Root,
    matcher: RegexMatcher,
    multi_line: bool,
    overrides: Override,
    use_ignore_files: bool,
    paths: Option<Vec<String>>,
}

/// Readies `query` beneath `root`, or names why it cannot run.
pub(super) fn prepare(
    root: &str,
    query: WorkspaceSearchQuery,
) -> Result<Plan, WorkspaceSearchState> {
    let operation = WorkspaceFilesOperation::Search { query };
    if operation.validate().is_err() {
        return Err(WorkspaceSearchState::Unavailable);
    }
    let WorkspaceFilesOperation::Search { query } = operation else {
        return Err(WorkspaceSearchState::Unavailable);
    };
    let root = Root::open(root).map_err(|_| WorkspaceSearchState::Unavailable)?;
    let (matcher, multi_line) = matcher(&query).ok_or(WorkspaceSearchState::InvalidPattern)?;
    let overrides = overrides(root.path(), &query).ok_or(WorkspaceSearchState::InvalidGlob)?;
    Ok(Plan {
        root,
        matcher,
        multi_line,
        overrides,
        use_ignore_files: query.use_ignore_files,
        paths: query.paths,
    })
}

/// The matcher for `query`, and whether a match may run across lines, which a line break in the
/// pattern asks for.
pub(super) fn matcher(query: &WorkspaceSearchQuery) -> Option<(RegexMatcher, bool)> {
    let multi_line = query.pattern.contains('\n') || (query.regex && query.pattern.contains("\\n"));
    let mut builder = RegexMatcherBuilder::new();
    builder
        .case_insensitive(!query.match_case)
        .word(query.whole_word)
        // Binary detection stops at the first NUL, so a pattern that needs one could never match.
        .ban_byte(Some(0))
        .crlf(true);
    let pattern = if query.regex {
        query.pattern.clone()
    } else if multi_line {
        // Literal lines that also match where the file ends its lines with CRLF.
        query
            .pattern
            .split('\n')
            .map(escape)
            .collect::<Vec<_>>()
            .join("\\r?\\n")
    } else {
        builder.fixed_strings(true);
        query.pattern.clone()
    };
    if multi_line {
        builder.line_terminator(None).multi_line(true);
    }
    builder
        .build(&pattern)
        .ok()
        .map(|matcher| (matcher, multi_line))
}

/// `text` with each regular expression meta character escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// The file filter for `query`'s include and exclude globs.
pub(super) fn overrides(root: &Path, query: &WorkspaceSearchQuery) -> Option<Override> {
    let mut builder = OverrideBuilder::new(root);
    // Names compare the way the platform's own file system usually does.
    builder
        .case_insensitive(cfg!(any(windows, target_os = "macos")))
        .ok()?;
    for glob in &query.includes {
        for pattern in expand(glob) {
            builder.add(&pattern).ok()?;
        }
    }
    for glob in &query.excludes {
        for pattern in expand(glob) {
            builder.add(&format!("!{pattern}")).ok()?;
        }
    }
    builder.build().ok()
}

/// The patterns one glob stands for: a glob starting with `./` names a path from the Workspace
/// root and any other matches at any depth, and either matches a directory's contents too.
pub(super) fn expand(glob: &str) -> [String; 2] {
    let glob = if cfg!(windows) {
        glob.replace('\\', "/")
    } else {
        glob.to_owned()
    };
    let glob = glob.trim_end_matches('/');
    let base = if let Some(anchored) = glob.strip_prefix("./") {
        format!("/{anchored}")
    } else if glob.is_empty() {
        "**".to_owned()
    } else if glob.starts_with('/') || glob.starts_with("**/") {
        glob.to_owned()
    } else {
        format!("**/{glob}")
    };
    let contents = format!("{base}/**");
    [base, contents]
}

/// The threads one search walks with: all but one of the processor's, at most eight.
fn threads() -> usize {
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .saturating_sub(1)
        .clamp(1, 8)
}

/// Runs `plan`, sending each file's matching lines to `found` as one message of entries in a row,
/// until the walk ends, the search is cancelled, or it has found its most.
pub(super) fn run(plan: &Plan, shared: &Shared<'_>, found: &Sender<Vec<WorkspaceSearchFile>>) {
    let mut builder = SearcherBuilder::new();
    builder
        .line_number(true)
        .line_terminator(LineTerminator::crlf())
        .binary_detection(BinaryDetection::quit(0))
        .multi_line(plan.multi_line)
        .heap_limit(Some(HEAP_LIMIT));
    match &plan.paths {
        Some(paths) => run_paths(plan, paths, &builder, shared, found),
        None => run_walk(plan, &builder, shared, found),
    }
}

/// Walks the whole Workspace, Git's own directory aside.
fn run_walk(
    plan: &Plan,
    searchers: &SearcherBuilder,
    shared: &Shared<'_>,
    found: &Sender<Vec<WorkspaceSearchFile>>,
) {
    let ignores = plan.use_ignore_files;
    let mut walk = WalkBuilder::new(plan.root.path());
    walk.hidden(false)
        .parents(ignores)
        .ignore(ignores)
        .git_ignore(ignores)
        .git_global(ignores)
        .git_exclude(ignores)
        .follow_links(false)
        .overrides(plan.overrides.clone())
        .threads(threads())
        .filter_entry(|entry| entry.file_name() != ".git");
    walk.build_parallel().run(|| {
        let mut searcher = searchers.build();
        let found = found.clone();
        Box::new(move |entry| {
            if shared.stopped() {
                return WalkState::Quit;
            }
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                return WalkState::Continue;
            }
            if let Some(relative) = relative(plan.root.path(), entry.path()) {
                search_file(&mut searcher, plan, entry.path(), relative, shared, &found);
            }
            if shared.stopped() {
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });
}

/// Searches only the files named, ignore files aside, still applying the include and exclude
/// globs.
fn run_paths(
    plan: &Plan,
    paths: &[String],
    searchers: &SearcherBuilder,
    shared: &Shared<'_>,
    found: &Sender<Vec<WorkspaceSearchFile>>,
) {
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads().min(paths.len()) {
            scope.spawn(|| {
                let mut searcher = searchers.build();
                while let Some(relative) = paths.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if shared.stopped() {
                        return;
                    }
                    if plan
                        .overrides
                        .matched(plan.root.join(relative), false)
                        .is_ignore()
                    {
                        continue;
                    }
                    let Ok(path) = plan.root.resolve(relative) else {
                        continue;
                    };
                    if path.is_file() {
                        search_file(&mut searcher, plan, &path, relative.clone(), shared, found);
                    }
                }
            });
        }
    });
}

/// `path` beneath `root` as a Workspace-relative path, or none when a name in it cannot be one.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let names: Option<Vec<&str>> = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect();
    let relative = names?.join("/");
    orchbee_workbench_protocol::is_workspace_relative_path(&relative).then_some(relative)
}

/// Searches one file and sends its lines, split into entries of at most [`PIECE_LINES`]. A file
/// that turns out binary or cannot be read gives nothing, and its matches no longer count.
fn search_file(
    searcher: &mut Searcher,
    plan: &Plan,
    path: &Path,
    relative: String,
    shared: &Shared<'_>,
    found: &Sender<Vec<WorkspaceSearchFile>>,
) {
    let mut sink = FileSink {
        matcher: &plan.matcher,
        shared,
        lines: Vec::new(),
        counted: 0,
        binary: false,
    };
    let searched = searcher.search_path(&plan.matcher, path, &mut sink).is_ok();
    if !searched || sink.binary || sink.lines.is_empty() {
        shared.matches.fetch_sub(sink.counted, Ordering::Relaxed);
        return;
    }
    let _closed = found.send(pieces(relative, sink.lines));
}

/// A file's lines as entries of at most [`PIECE_LINES`], in order.
pub(super) fn pieces(path: String, lines: Vec<WorkspaceSearchLine>) -> Vec<WorkspaceSearchFile> {
    let mut entries = Vec::with_capacity(lines.len().div_ceil(PIECE_LINES));
    let mut lines = lines.into_iter().peekable();
    while lines.peek().is_some() {
        entries.push(WorkspaceSearchFile {
            path: path.clone(),
            lines: lines.by_ref().take(PIECE_LINES).collect(),
        });
    }
    entries
}

/// Collects one file's matching lines.
struct FileSink<'s, 'a> {
    matcher: &'s RegexMatcher,
    shared: &'s Shared<'a>,
    lines: Vec<WorkspaceSearchLine>,
    counted: u64,
    binary: bool,
}

impl Sink for FileSink<'_, '_> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        found: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        if self.shared.stopped() {
            return Ok(false);
        }
        let bytes = found.bytes();
        let mut spans = Vec::new();
        let mut full = false;
        self.matcher
            .find_iter(bytes, |span| {
                if span.is_empty() {
                    return true;
                }
                if !self.shared.admit() {
                    full = true;
                    return false;
                }
                self.counted += 1;
                spans.push((span.start(), span.end()));
                true
            })
            .map_err(std::io::Error::error_message)?;
        self.lines
            .extend(group(found.line_number().unwrap_or(1), bytes, &spans));
        Ok(!full)
    }

    fn binary_data(&mut self, _searcher: &Searcher, _offset: u64) -> Result<bool, Self::Error> {
        self.binary = true;
        Ok(false)
    }
}

/// The preview lines for matches in `bytes`, whose first line is `first`: matches that start on
/// the same line share its previews, which run to the end of the line the last of them ends on.
pub(super) fn group(
    first: u64,
    bytes: &[u8],
    spans: &[(usize, usize)],
) -> Vec<WorkspaceSearchLine> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(
            bytes
                .iter()
                .enumerate()
                .filter(|(index, byte)| **byte == b'\n' && index + 1 < bytes.len())
                .map(|(index, _)| index + 1),
        )
        .collect();
    let line_of = |offset: usize| {
        starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    };
    let mut lines = Vec::new();
    let mut index = 0;
    while let Some(&(start, _)) = spans.get(index) {
        let line = line_of(start);
        let count = spans[index..]
            .iter()
            .take_while(|(start, _)| line_of(*start) == line)
            .count();
        let group = &spans[index..index + count];
        let from = starts[line];
        let last = group
            .last()
            .map_or(from, |(start, end)| end.saturating_sub(1).max(*start));
        let mut to = bytes[last..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| last + offset);
        if to > from && bytes[to - 1] == b'\r' && bytes.get(to) == Some(&b'\n') {
            to -= 1;
        }
        let relative: Vec<(usize, usize)> = group
            .iter()
            .map(|(start, end)| (start - from, (end - from).min(to - from)))
            .collect();
        let number = first.saturating_add(u64::try_from(line).unwrap_or(u64::MAX));
        lines.extend(previews(number, &bytes[from..to], &relative));
        index += count;
    }
    lines
}
