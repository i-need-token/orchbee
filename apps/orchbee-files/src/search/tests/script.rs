//! A scripted rg for the search suites: each run plays the next queued script, then the standing
//! one, and is recorded with its arguments, its folder and how it ended.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use super::super::rg::{Ended, Feed, Launch};
use crate::scope::path::NativePath;

/// How long a suite waits for a run to reach a state before it fails.
pub(super) const WAIT: Duration = Duration::from_secs(20);

/// What one run writes, in order, and how it ends unless its feed stops it first.
#[derive(Clone, Default)]
pub(super) struct Run {
    writes: Vec<Write>,
    /// After its writes the run waits for its feed to stop it, as rg waits on a full pipe.
    hold: bool,
    /// How the run ends after its writes; rg's own exit unless set.
    ended: Option<Ended>,
}

#[derive(Clone)]
enum Write {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

impl Run {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn stdout(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.writes.push(Write::Stdout(bytes.into()));
        self
    }

    pub(super) fn stderr(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.writes.push(Write::Stderr(bytes.into()));
        self
    }

    pub(super) fn hold(mut self) -> Self {
        self.hold = true;
        self
    }

    pub(super) fn ending(mut self, ended: Ended) -> Self {
        self.ended = Some(ended);
        self
    }
}

/// One finished run.
#[derive(Clone, Debug)]
pub(super) struct Seen {
    pub(super) arguments: Vec<String>,
    pub(super) folder: PathBuf,
    pub(super) ended: Ended,
}

impl Seen {
    /// Whether an argument holds `text`.
    pub(super) fn names(&self, text: &str) -> bool {
        self.arguments
            .iter()
            .any(|argument| argument.contains(text))
    }
}

#[derive(Default)]
pub(super) struct Script {
    queued: Mutex<VecDeque<Run>>,
    standing: Run,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The arguments of each run in progress.
    running: Vec<Vec<String>>,
    /// The most runs in progress at once.
    peak: usize,
    seen: Vec<Seen>,
}

impl Script {
    /// Plays `queued` in order, then runs that exit without output.
    pub(super) fn new(queued: impl IntoIterator<Item = Run>) -> Self {
        Self::with(queued, Run::new())
    }

    /// Plays `standing` for every run.
    pub(super) fn standing(standing: Run) -> Self {
        Self::with([], standing)
    }

    pub(super) fn with(queued: impl IntoIterator<Item = Run>, standing: Run) -> Self {
        Self {
            queued: Mutex::new(queued.into_iter().collect()),
            standing,
            state: Mutex::default(),
        }
    }

    pub(super) fn seen(&self) -> Vec<Seen> {
        self.lock().seen.clone()
    }

    /// The arguments of each run in progress.
    pub(super) fn running(&self) -> Vec<Vec<String>> {
        self.lock().running.clone()
    }

    pub(super) fn peak(&self) -> usize {
        self.lock().peak
    }

    /// Waits until `holds` is true of the script; fails the test after [`WAIT`].
    pub(super) fn wait_for(&self, holds: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + WAIT;
        while !holds(self) {
            assert!(
                Instant::now() < deadline,
                "the script never reached the state"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Launch for Script {
    fn run(&self, arguments: &[String], folder: &Path, feed: &mut dyn Feed) -> Ended {
        let run = self
            .queued
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop_front()
            .unwrap_or_else(|| self.standing.clone());
        {
            let mut state = self.lock();
            state.running.push(arguments.to_vec());
            state.peak = state.peak.max(state.running.len());
        }
        let ended = play(&run, feed);
        let mut state = self.lock();
        if let Some(at) = state.running.iter().position(|held| held == arguments) {
            state.running.remove(at);
        }
        state.seen.push(Seen {
            arguments: arguments.to_vec(),
            folder: folder.to_path_buf(),
            ended,
        });
        ended
    }
}

fn play(run: &Run, feed: &mut dyn Feed) -> Ended {
    for write in &run.writes {
        if !feed.more() {
            return Ended::Stopped;
        }
        match write {
            Write::Stdout(bytes) => {
                if !feed.stdout(bytes) {
                    return Ended::Stopped;
                }
            }
            Write::Stderr(bytes) => feed.stderr(bytes),
        }
    }
    if run.hold {
        while feed.more() {
            thread::sleep(Duration::from_millis(1));
        }
        return Ended::Stopped;
    }
    run.ended.unwrap_or(Ended::Exited)
}

/// rg's `match` message for `line` of `path`, 1-based, with a submatch at each of `spans`.
pub(super) fn matched(path: &str, line: u64, text: &str, spans: &[(usize, usize)]) -> String {
    let submatches: Vec<serde_json::Value> = spans
        .iter()
        .map(|&(start, end)| {
            serde_json::json!({"match": {"text": &text[start..end]}, "start": start, "end": end})
        })
        .collect();
    let message = serde_json::json!({
        "type": "match",
        "data": {
            "path": {"text": native(path)},
            "lines": {"text": format!("{text}\n")},
            "line_number": line,
            "absolute_offset": 0,
            "submatches": submatches,
        },
    });
    format!("{message}\n")
}

/// `rg --files` output listing `paths`.
pub(super) fn listed(paths: &[&str]) -> String {
    paths
        .iter()
        .map(|path| format!("{}\n", native(path)))
        .collect()
}

/// `path`, its segments separated by `/`, as rg prints it on this platform.
pub(super) fn native(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

/// `path` relative to `folder`, its segments separated by `/`.
pub(super) fn relative(path: &str, folder: &NativePath) -> String {
    path.strip_prefix(folder.as_str())
        .unwrap_or(path)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/")
}

pub(super) fn folder(path: &Path) -> NativePath {
    NativePath::parse(path.to_str().unwrap()).unwrap()
}
