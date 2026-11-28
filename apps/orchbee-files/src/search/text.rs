//! One text search, run as VS Code ef5ca45 runs one with ripgrep (`ripgrepTextSearchEngine.ts`
//! and `textSearchManager.ts`): rg in each folder with `getRgArgs`, and with exclude exceptions a
//! second run that lists only what they re-include; rg's JSON read by `RipgrepParser`'s rules; a
//! folder searched again without following links when they form too many loops; each result
//! checked against the request's globs in the deepest folder that holds it, limited to the
//! request's ranges and queued for `search.next`. Folders run one after another, where VS Code
//! runs them at once.

use std::collections::{HashMap, HashSet};

use orchbee_files_protocol::{
    ErrorCode, FilePath, FilesError, List, MAXIMUM_SAFE_INTEGER, SafeInteger, SearchContextLine,
    SearchMatch, SearchRange,
};

use super::{
    args::{self, FolderOptions, TextQuery},
    glob::{self, Excludes, Globs},
    js,
    json::{self, Event, Parser},
    paths,
    preview::{Located, PreviewOptions, Range},
    queue::{Item, Outcome, Queue},
    rg::{self, Ended, Feed, Launch, Stderr},
    slots::Slots,
};
use crate::scope::path::{CASE_SENSITIVE, NativePath};

/// A text search as the request states it, its globs as the user wrote them.
pub(super) struct Request {
    pub(super) pattern: String,
    pub(super) regex: bool,
    pub(super) case_sensitive: bool,
    pub(super) whole_word: bool,
    pub(super) multiline: bool,
    pub(super) encoding: Option<String>,
    pub(super) context: u32,
    pub(super) folders: Vec<Folder>,
    pub(super) includes: Vec<String>,
    /// Every exclude key, exceptions included.
    pub(super) excludes: Vec<String>,
    pub(super) use_ignore_files: bool,
    pub(super) use_parent_ignore_files: bool,
    pub(super) follow_symlinks: bool,
    pub(super) max_results: u64,
    pub(super) preview: PreviewOptions,
}

/// One folder a search reads.
pub(super) struct Folder {
    pub(super) path: NativePath,
    /// The globs the folder adds to the request's includes.
    pub(super) includes: Vec<String>,
}

/// Runs `request`, queuing what it accepts on `queue`, then ends the queue with the search's
/// outcome. `launch` starts rg; `slots` bounds the rg processes this process runs.
pub(super) fn run(request: &Request, launch: &dyn Launch, slots: &Slots, queue: &Queue) {
    let outcome = Search::new(request, launch, slots, queue).run();
    queue.finish(outcome);
}

/// The manager's state for one search: `TextSearchManager`'s count and limit, and each folder's
/// glob test.
struct Search<'a> {
    request: &'a Request,
    launch: &'a dyn Launch,
    slots: &'a Slots,
    queue: &'a Queue,
    folders: Vec<Tested>,
    /// The ranges accepted so far.
    count: u64,
    /// The search reached its limit and stopped.
    limit_hit: bool,
}

/// A folder and the globs its results pass, as `QueryGlobTester` holds them.
struct Tested {
    path: NativePath,
    includes: Vec<String>,
    tester: Tester,
}

struct Tester {
    excludes: Excludes,
    includes: Option<Globs>,
}

impl Tester {
    /// `includedInQuery`: not excluded, and included when includes exist.
    fn included(&self, path: &[u16], basename: &[u16]) -> bool {
        !self.excludes.matches(path, Some(basename))
            && self
                .includes
                .as_ref()
                .is_none_or(|includes| includes.matches(path, Some(basename)))
    }
}

/// What one rg run ended with.
struct Complete {
    limit_hit: bool,
}

/// How a run's results reach the manager.
enum Pass<'p> {
    /// The only run of a folder without exceptions.
    Only,
    /// The first run of a folder with exceptions, which records what it reports.
    First(&'p mut HashSet<String>),
    /// The run that lists what exceptions re-include, skipping what the first one reported.
    Exceptions {
        reported: &'p HashSet<String>,
        reincluded: &'p Reincluded,
    },
}

/// `isReincluded`: an exception covers the path and no folder of the search nested in this one
/// holds it, since that folder is searched on its own.
struct Reincluded {
    excepted: Excludes,
    nested: Vec<Vec<u16>>,
    ignore_case: bool,
}

impl Reincluded {
    fn holds(&self, path: &[u16]) -> bool {
        self.excepted.is_excepted(path, None)
            && !self
                .nested
                .iter()
                .any(|folder| paths::is_equal_or_parent(path, folder, self.ignore_case, js::SLASH))
    }
}

/// Why a run stopped rg early, besides a retry.
#[derive(Clone, Copy)]
enum Stop {
    /// The parser reached the run's limit.
    Limit,
    /// The search was cancelled, or reached its own limit.
    Cancelled,
    /// rg wrote a line that is not JSON.
    Malformed,
}

impl<'a> Search<'a> {
    fn new(
        request: &'a Request,
        launch: &'a dyn Launch,
        slots: &'a Slots,
        queue: &'a Queue,
    ) -> Self {
        let ignore_case = !CASE_SENSITIVE;
        let folders = request
            .folders
            .iter()
            .map(|folder| {
                let includes = union(&request.includes, &folder.includes);
                let tester = Tester {
                    excludes: Excludes::new(&request.excludes, ignore_case),
                    includes: (!includes.is_empty())
                        .then(|| Globs::new(includes.iter().map(String::as_str), ignore_case)),
                };
                Tested {
                    path: folder.path.clone(),
                    includes,
                    tester,
                }
            })
            .collect();
        Self {
            request,
            launch,
            slots,
            queue,
            folders,
            count: 0,
            limit_hit: false,
        }
    }

    /// Whether nothing more is accepted: the search reached its limit or was cancelled.
    fn stopped(&self) -> bool {
        self.limit_hit || self.queue.cancelled()
    }

    fn run(mut self) -> Outcome {
        let mut provider_limit = false;
        for index in 0..self.folders.len() {
            if self.stopped() {
                break;
            }
            rg::searchable(self.folders[index].path.as_path())?;
            provider_limit |= self.provide(index)?.limit_hit;
        }
        Ok(self.limit_hit || provider_limit)
    }

    /// `provideTextSearchResultsWithRgOptions` for folder `index`.
    fn provide(&mut self, index: usize) -> Result<Complete, FilesError> {
        let exceptions = glob::exceptions(&self.request.excludes);
        if exceptions.is_empty() {
            return self.search(index, &mut Pass::Only, None, self.request.max_results);
        }
        let mut reported = HashSet::new();
        let complete = self.search(
            index,
            &mut Pass::First(&mut reported),
            None,
            self.request.max_results,
        )?;
        if complete.limit_hit || self.stopped() {
            return Ok(complete);
        }
        let ignore_case = !CASE_SENSITIVE;
        let keys: Vec<String> = exceptions
            .iter()
            .map(|exception| format!("!{exception}"))
            .collect();
        let folder = &self.folders[index].path;
        let nested = self
            .folders
            .iter()
            .filter(|other| other.path.is_within(folder) && other.path.key() != folder.key())
            .filter_map(|other| other.path.below(folder))
            .map(|segments| js::units(&segments.collect::<Vec<_>>().join("/")))
            .collect();
        let reincluded = Reincluded {
            excepted: Excludes::new(&keys, ignore_case),
            nested,
            ignore_case,
        };
        let mut pass = Pass::Exceptions {
            reported: &reported,
            reincluded: &reincluded,
        };
        // The search's own limit applies to what it accepts, so the run lists everything.
        self.search(index, &mut pass, Some(&exceptions), MAXIMUM_SAFE_INTEGER)
    }

    /// `searchWithRipgrep` for folder `index`, following links as the request asks.
    fn search(
        &mut self,
        index: usize,
        pass: &mut Pass<'_>,
        exceptions: Option<&[String]>,
        maximum: u64,
    ) -> Result<Complete, FilesError> {
        let follow = self.request.follow_symlinks;
        let ran = self.ripgrep(index, pass, exceptions, maximum, follow, None)?;
        if !ran.retry || self.stopped() {
            return Ok(Complete {
                limit_hit: ran.limit_hit,
            });
        }
        // `searchAgainWithoutFollowingSymlinks`: rg reports a file's results in the same order
        // every time, so as many of each file's results as were reported are skipped.
        if let Ok(folder) = FilePath::new(self.folders[index].path.as_str()) {
            self.queue.symlink_loops(folder);
        }
        let mut skip: HashMap<String, usize> = HashMap::new();
        for path in ran.reported {
            *skip.entry(path).or_default() += 1;
        }
        let again = self.ripgrep(index, pass, exceptions, maximum, false, Some(&mut skip))?;
        Ok(Complete {
            limit_hit: again.limit_hit,
        })
    }

    /// One rg process.
    fn ripgrep(
        &mut self,
        index: usize,
        pass: &mut Pass<'_>,
        exceptions: Option<&[String]>,
        maximum: u64,
        follow: bool,
        skip: Option<&mut HashMap<String, usize>>,
    ) -> Result<Ran, FilesError> {
        let request = self.request;
        let folder = self.folders[index].path.clone();
        let includes = self.folders[index].includes.clone();
        let excludes: Vec<String> = request
            .excludes
            .iter()
            .filter(|key| !glob::is_exception(key))
            .cloned()
            .collect();
        let options = FolderOptions {
            includes: &includes,
            excludes: &excludes,
            use_ignore_files: request.use_ignore_files,
            use_parent_ignore_files: request.use_parent_ignore_files,
            follow_symlinks: follow,
            ignore_glob_case: !CASE_SENSITIVE,
            below_top: args::below_top(folder.as_path()),
        };
        let query = TextQuery {
            pattern: &request.pattern,
            regex: request.regex,
            case_sensitive: request.case_sensitive,
            whole_word: request.whole_word,
            multiline: request.multiline,
            encoding: request.encoding.as_deref(),
            context: request.context,
        };
        let arguments = args::text(&query, &options, exceptions)
            .map_err(|_| FilesError::new(ErrorCode::InvalidRegex))?;
        let (launch, slots, queue) = (self.launch, self.slots, self.queue);
        let Some(_slot) = slots.acquire(|| queue.cancelled()) else {
            return Ok(Ran::default());
        };
        let mut observer = Observed {
            search: self,
            folder: &folder,
            pass,
            skip,
            parser: Parser::new(request.preview, maximum),
            follow,
            reported: Vec::new(),
            stderr: Stderr::default(),
            got_data: false,
            stop: None,
            retry: false,
        };
        let ended = launch.run(&arguments, folder.as_path(), &mut observer);
        let Observed {
            parser,
            reported,
            stderr,
            got_data,
            stop,
            retry,
            ..
        } = observer;
        queue.omitted(parser.omitted_long_lines());
        // As VS Code's run ends: rg that did not start, then a retry, then a stop, then rg's own
        // end, whose standard error fails the search only when rg wrote nothing else and VS Code
        // shows that error.
        let limit_hit = match (ended, stop) {
            (Ended::Unavailable, _) => return Err(rg::unavailable()),
            (Ended::Failed, _) | (_, Some(Stop::Malformed)) => {
                return Err(FilesError::new(ErrorCode::Failed));
            }
            _ if retry => false,
            (_, Some(Stop::Limit)) => true,
            (_, Some(Stop::Cancelled)) => parser.hit_limit(),
            (Ended::Exited | Ended::Stopped, None) => {
                if let Some(error) = (!got_data).then(|| rg::text_error(stderr.kept())).flatten() {
                    return Err(error);
                }
                parser.hit_limit()
            }
        };
        Ok(Ran {
            limit_hit,
            retry,
            reported,
        })
    }

    /// `progress.report` of `TextSearchManager`, then its `onResult`: the result in the deepest
    /// folder that holds `path`, tested against that folder's globs, then counted.
    fn report(&mut self, path: &NativePath, single: Single) {
        if self.stopped() {
            return;
        }
        let Some(deepest) = self
            .folders
            .iter()
            .filter(|folder| path.is_within(&folder.path))
            .max_by_key(|folder| folder.path.as_str().len())
        else {
            return;
        };
        let Some(relative) = path
            .below(&deepest.path)
            .map(|segments| segments.collect::<Vec<_>>())
        else {
            return;
        };
        let Some(basename) = relative.last() else {
            return;
        };
        let relative_units = js::units(&relative.join("/"));
        if !deepest
            .tester
            .included(&relative_units, &js::units(basename))
        {
            return;
        }
        let Ok(wire_path) = FilePath::new(path.as_str()) else {
            return;
        };
        match single {
            Single::Match(found) => {
                let mut ranges = found.ranges;
                let size = ranges.len() as u64;
                if self.count + size > self.request.max_results {
                    self.limit_hit = true;
                    ranges.truncate(
                        usize::try_from(self.request.max_results - self.count).unwrap_or(0),
                    );
                }
                self.count += ranges.len() as u64;
                if !ranges.is_empty() {
                    self.queue.push(matched(wire_path, &ranges, found.preview));
                }
            }
            Single::Context(line) => {
                let context = SearchContextLine {
                    line: safe(line.line),
                    text: line.text,
                };
                self.queue.push(Item::context(wire_path, context));
            }
        }
    }
}

/// One result as VS Code's parser reports it: a match, or one line of context.
enum Single {
    Match(json::Found),
    Context(json::ContextLine),
}

/// What one rg process reported.
#[derive(Default)]
struct Ran {
    limit_hit: bool,
    /// Symbolic links formed too many loops.
    retry: bool,
    /// The path of every result the parser produced, when the run followed links.
    reported: Vec<String>,
}

/// The consumer of one rg process: its output parsed, each result passed through the run's
/// filters to the manager.
struct Observed<'s, 'a, 'p, 'k> {
    search: &'s mut Search<'a>,
    folder: &'s NativePath,
    pass: &'s mut Pass<'p>,
    skip: Option<&'k mut HashMap<String, usize>>,
    parser: Parser,
    follow: bool,
    reported: Vec<String>,
    stderr: Stderr,
    got_data: bool,
    /// Why the run stopped rg, which also ends the count of loops.
    stop: Option<Stop>,
    /// Symbolic links formed too many loops, so the run stops for a retry.
    retry: bool,
}

impl Feed for Observed<'_, '_, '_, '_> {
    fn more(&mut self) -> bool {
        if self.retry || self.stop.is_some() {
            return false;
        }
        if self.search.stopped() {
            return self.stop(Stop::Cancelled);
        }
        true
    }

    fn stdout(&mut self, bytes: &[u8]) -> bool {
        if !bytes.is_empty() {
            self.got_data = true;
        }
        let mut events = Vec::new();
        let fed = self.parser.feed(bytes, &mut |event| events.push(event));
        for event in events {
            match event {
                Event::Match { path, found } => self.result(&path, Single::Match(found)),
                Event::Context { path, lines } => {
                    for line in lines {
                        self.result(&path, Single::Context(line));
                    }
                }
            }
        }
        if fed.is_err() {
            return self.stop(Stop::Malformed);
        }
        if self.parser.hit_limit() {
            return self.stop(Stop::Limit);
        }
        self.more()
    }

    fn stderr(&mut self, bytes: &[u8]) {
        // `RipgrepSymlinkLoopDetector`, read until the run is done.
        let counting = self.follow && self.stop.is_none() && !self.retry;
        if self.stderr.push(bytes, counting) {
            self.retry = true;
        }
    }
}

impl Observed<'_, '_, '_, '_> {
    /// Stops the run for `stop`, unless it is stopping already; always false, for the feed.
    fn stop(&mut self, stop: Stop) -> bool {
        self.stop.get_or_insert(stop);
        false
    }

    /// One result the parser produced, through the retry's skips and the pass's filter.
    fn result(&mut self, path: &json::Text, single: Single) {
        // A path that is not UTF-8 cannot be named on the wire.
        let json::Text::Utf8(relative) = path else {
            return;
        };
        let Some(path) = join(self.folder, relative) else {
            return;
        };
        if self.follow {
            self.reported.push(path.as_str().to_owned());
        }
        if let Some(skip) = self.skip.as_deref_mut()
            && let Some(count) = skip.get_mut(path.as_str())
            && *count > 0
        {
            *count -= 1;
            return;
        }
        match &mut *self.pass {
            Pass::Only => {}
            Pass::First(reported) => {
                reported.insert(path.as_str().to_owned());
            }
            Pass::Exceptions {
                reported,
                reincluded,
            } => {
                if reported.contains(path.as_str()) {
                    return;
                }
                let Some(below) = path.below(self.folder) else {
                    return;
                };
                let relative = js::units(&below.collect::<Vec<_>>().join("/"));
                if !reincluded.holds(&relative) {
                    return;
                }
            }
        }
        self.search.report(&path, single);
    }
}

/// The path rg printed, relative to `folder` after its leading `.`, as a path below the folder.
fn join(folder: &NativePath, printed: &str) -> Option<NativePath> {
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    let mut joined = folder.as_str().to_owned();
    let mut segments = printed.split(separators).peekable();
    if segments.peek() == Some(&".") {
        segments.next();
    }
    let separator = if cfg!(windows) { '\\' } else { '/' };
    for segment in segments {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        if !joined.ends_with(separator) {
            joined.push(separator);
        }
        joined.push_str(segment);
    }
    NativePath::parse(&joined).ok()
}

/// The request's includes, then the folder's own not among them: the keys of
/// `{...global, ...folder}`.
fn union(global: &[String], folder: &[String]) -> Vec<String> {
    let mut keys: Vec<String> = Vec::with_capacity(global.len() + folder.len());
    for key in global.iter().chain(folder) {
        if !keys.contains(key) {
            keys.push(key.clone());
        }
    }
    keys
}

fn safe(value: u64) -> SafeInteger {
    SafeInteger::new(value.min(MAXIMUM_SAFE_INTEGER)).unwrap_or_default()
}

/// A range on the wire; a negative column, which no preview produces, reads as 0.
fn span(range: &Range) -> [SafeInteger; 4] {
    let at = |value: i64| safe(u64::try_from(value).unwrap_or(0));
    [
        at(range.start_line),
        at(range.start_column),
        at(range.end_line),
        at(range.end_column),
    ]
}

/// The queued match of `path` with `ranges`, 1 to 20,000 of them.
fn matched(path: FilePath, ranges: &[Located], preview: String) -> Item {
    let ranges = ranges
        .iter()
        .map(|range| SearchRange {
            source: span(&range.source),
            preview: span(&range.preview),
        })
        .collect();
    let ranges = List::new(ranges).expect("a match holds 1 to 20,000 ranges");
    Item::matched(path, SearchMatch { ranges, preview })
}
