//! A text search's results between its rg runs and `search.next`. The run queues what the search
//! accepts and waits while 2 MiB of results wait, so rg waits on its output; `search.next` takes
//! what fits one Result record, in order. A match too large for one record is split by its
//! ranges, each part with the same preview; a line whose preview or text alone is too large is
//! left out and counted. Sizes are those of the JSON the reply encodes.

use std::{
    collections::VecDeque,
    sync::{
        Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    ErrorCode, FilePath, FilesError, List, MAXIMUM_REPLY_BYTES, MAXIMUM_SAFE_INTEGER, SafeInteger,
    SearchContextLine, SearchFile, SearchMatch, SearchNextResult, SearchRange,
};

/// What a finished search reports: whether it stopped at its limit, or why it failed.
pub(super) type Outcome = Result<bool, FilesError>;

/// The results that may wait before the run waits for `search.next`.
const QUEUE_BYTES: usize = 2 * 1024 * 1024;
/// The longest a waiter sleeps before it looks at its cancellation again.
const WAIT_SLICE: Duration = Duration::from_millis(10);
/// A batch without files or folders, at its longest.
const ENVELOPE: usize =
    r#"{"files":[],"done":false,"limitHit":false,"omittedLongLines":9007199254740991}"#.len();
/// A file's entry around its path and first result: `{"path":`, `,"matches":[` or
/// `,"context":[`, then `]}`.
const FILE_ENTRY: usize = r#"{"path":,"matches":[]}"#.len();
/// The member that adds one result list to a file's entry: `,"matches":[` and `]`.
const RESULT_LIST: usize = r#","matches":[]"#.len();
/// The member that adds the folders whose links loop: `,"symlinkLoops":[` and `]`.
const LOOPS_MEMBER: usize = r#","symlinkLoops":[]"#.len();

/// One queued result, a match or a context line, with its file and the sizes of both as JSON.
pub(super) struct Item {
    path: FilePath,
    path_bytes: usize,
    value: Value,
    bytes: usize,
}

enum Value {
    Match(SearchMatch),
    Context(SearchContextLine),
}

impl Item {
    pub(super) fn matched(path: FilePath, found: SearchMatch) -> Self {
        Self {
            path_bytes: json_length(&path),
            bytes: json_length(&found),
            path,
            value: Value::Match(found),
        }
    }

    pub(super) fn context(path: FilePath, line: SearchContextLine) -> Self {
        Self {
            path_bytes: json_length(&path),
            bytes: json_length(&line),
            path,
            value: Value::Context(line),
        }
    }

    /// The most one result may take: a Result record less a batch's envelope and the result's
    /// own file entry.
    const fn room(path_bytes: usize) -> usize {
        MAXIMUM_REPLY_BYTES.saturating_sub(ENVELOPE + FILE_ENTRY + path_bytes)
    }

    /// This result as items that each fit a batch alone; none when even one range does not.
    fn fitted(self) -> Vec<Self> {
        let room = Self::room(self.path_bytes);
        if self.bytes <= room {
            return vec![self];
        }
        let Value::Match(found) = self.value else {
            return Vec::new();
        };
        // `{"ranges":[` and `],"preview":` around the ranges, then the preview and `}`.
        let fixed = r#"{"ranges":[],"preview":}"#.len() + json_length(&found.preview);
        let mut parts: Vec<Vec<SearchRange>> = Vec::new();
        let mut part: Vec<SearchRange> = Vec::new();
        let mut size = fixed;
        for range in found.ranges.as_slice() {
            let bytes = json_length(range);
            if size + usize::from(!part.is_empty()) + bytes > room {
                if part.is_empty() {
                    return Vec::new();
                }
                parts.push(std::mem::take(&mut part));
                size = fixed;
            }
            size += usize::from(!part.is_empty()) + bytes;
            part.push(*range);
        }
        parts.push(part);
        parts
            .into_iter()
            .map(|ranges| {
                let ranges = List::new(ranges).expect("a part holds some of a match's ranges");
                let preview = found.preview.clone();
                Self::matched(self.path.clone(), SearchMatch { ranges, preview })
            })
            .collect()
    }
}

/// One search's queue.
pub(super) struct Queue {
    state: Mutex<State>,
    changed: Condvar,
    cancelled: AtomicBool,
}

/// The lock guards the queued results; it is never held while a call is answered. A panic ends
/// the process, so a poisoned lock is never observed.
#[derive(Default)]
struct State {
    items: VecDeque<Item>,
    bytes: usize,
    outcome: Option<Outcome>,
    omitted: u64,
    /// Folders searched again without following links, not yet reported.
    loops: VecDeque<FilePath>,
    /// A `search.next` is waiting.
    taking: bool,
}

/// What `search.next` answers.
pub(super) enum Taken {
    /// A batch; the search ended with it when `done` holds.
    Batch(SearchNextResult),
    /// The search failed or was cancelled; nothing of it is left.
    Failed(FilesError),
    /// Another `search.next` of the search is waiting.
    Concurrent,
    /// The asking call was cancelled, and nothing was taken.
    Abandoned,
}

impl Queue {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Whether the search was cancelled.
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Cancels the search: its run stops, and what waits is dropped.
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let mut state = self.lock();
        state.items.clear();
        state.bytes = 0;
        drop(state);
        self.changed.notify_all();
    }

    /// Queues `item` once fewer than 2 MiB of results wait; nothing once the search is cancelled.
    pub(super) fn push(&self, item: Item) {
        let items = item.fitted();
        let mut state = self.lock();
        if items.is_empty() {
            state.omitted += 1;
            return;
        }
        while state.bytes >= QUEUE_BYTES && !self.cancelled() {
            state = self
                .changed
                .wait_timeout(state, WAIT_SLICE)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        if self.cancelled() {
            return;
        }
        for item in items {
            state.bytes += item.bytes;
            state.items.push_back(item);
        }
        drop(state);
        self.changed.notify_all();
    }

    /// Counts lines rg wrote that were too long to read.
    pub(super) fn omitted(&self, lines: u64) {
        if lines > 0 {
            self.lock().omitted += lines;
        }
    }

    /// Records that `folder` was searched again without following links.
    pub(super) fn symlink_loops(&self, folder: FilePath) {
        self.lock().loops.push_back(folder);
    }

    /// Ends the search with `outcome`.
    pub(super) fn finish(&self, outcome: Outcome) {
        self.lock().outcome = Some(outcome);
        self.changed.notify_all();
    }

    /// The next batch, once results wait or the search ended, or at `deadline` with what there
    /// is; `abandoned` tells whether the asking call was cancelled meanwhile.
    pub(super) fn take(&self, deadline: Instant, abandoned: impl Fn() -> bool) -> Taken {
        let mut state = self.lock();
        if state.taking {
            return Taken::Concurrent;
        }
        state.taking = true;
        let taken = loop {
            if self.cancelled() {
                break Taken::Failed(FilesError::new(ErrorCode::Cancelled));
            }
            if abandoned() {
                break Taken::Abandoned;
            }
            let now = Instant::now();
            if !state.items.is_empty()
                || !state.loops.is_empty()
                || state.outcome.is_some()
                || now >= deadline
            {
                break state.batch();
            }
            state = self
                .changed
                .wait_timeout(state, WAIT_SLICE.min(deadline - now))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        state.taking = false;
        drop(state);
        self.changed.notify_all();
        taken
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    /// The folders and results at the front that fit one Result record; the search's end once
    /// nothing is left.
    fn batch(&mut self) -> Taken {
        let limit_hit = matches!(self.outcome, Some(Ok(true)));
        let omitted = self.omitted.min(MAXIMUM_SAFE_INTEGER);
        let mut size = ENVELOPE;
        let mut symlink_loops = Vec::new();
        while let Some(folder) = self.loops.front() {
            let extra = json_length(folder)
                + if symlink_loops.is_empty() {
                    LOOPS_MEMBER
                } else {
                    1
                };
            if size + extra > MAXIMUM_REPLY_BYTES {
                break;
            }
            size += extra;
            symlink_loops.extend(self.loops.pop_front());
        }
        let mut files: Vec<SearchFile> = Vec::new();
        while let Some(item) = self.items.front() {
            let file = files.last().filter(|file| file.path == item.path);
            let extra = match (file, &item.value) {
                (Some(file), Value::Match(_)) if file.matches.is_empty() => RESULT_LIST,
                (Some(file), Value::Context(_)) if file.context.is_empty() => RESULT_LIST,
                (Some(_), _) => 1,
                (None, _) => usize::from(!files.is_empty()) + FILE_ENTRY + item.path_bytes,
            } + item.bytes;
            if size + extra > MAXIMUM_REPLY_BYTES {
                break;
            }
            size += extra;
            let same = file.is_some();
            let Some(item) = self.items.pop_front() else {
                break;
            };
            self.bytes -= item.bytes;
            if !same {
                files.push(SearchFile {
                    path: item.path,
                    matches: Vec::new(),
                    context: Vec::new(),
                });
            }
            if let Some(file) = files.last_mut() {
                match item.value {
                    Value::Match(found) => file.matches.push(found),
                    Value::Context(line) => file.context.push(line),
                }
            }
        }
        let ended = self.items.is_empty() && self.loops.is_empty();
        if ended
            && files.is_empty()
            && symlink_loops.is_empty()
            && let Some(Err(error)) = &self.outcome
        {
            return Taken::Failed(error.clone());
        }
        Taken::Batch(SearchNextResult {
            files,
            done: ended && matches!(self.outcome, Some(Ok(_))),
            limit_hit,
            omitted_long_lines: SafeInteger::new(omitted).unwrap_or_default(),
            symlink_loops,
        })
    }
}

/// The length of `value` as the reply encodes it.
fn json_length(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX / 4, |json| json.len())
}
