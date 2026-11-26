//! The search lane: a child process that searches file contents, or lists the files, beneath a
//! Workspace for the Server, apart from the service's one-at-a-time requests, so a long search
//! delays nothing else. It reads commands from standard input and writes reports to standard
//! output, one JSON object per line. Each search or listing runs on threads of its own and reports
//! what it finds in batches until it ends or is cancelled.
mod engine;
mod list;
mod preview;

pub(crate) use engine::matcher;

use crate::IDLE_EXIT;
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILES_SEARCH_REQUEST_BYTES, WorkspaceFileListQuery, WorkspaceSearchFile,
    WorkspaceSearchQuery, WorkspaceSearchState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// The command-line flag that turns the Server's executable into the search lane.
pub const SEARCH_FLAG: &str = "--workspace-files-search";
/// The longest report line the lane writes, its line feed included; a batch of
/// [`BATCH_LINES`] preview lines stays far below it.
pub const MAXIMUM_SEARCH_REPORT_LINE_BYTES: usize = 4 * 1024 * 1024;
/// Searches and listings the lane runs at once; one past it ends at once as unavailable.
pub const MAXIMUM_RUNNING_SEARCHES: usize = 8;
/// One command line's bytes: the largest query and the root beside it.
const MAXIMUM_COMMAND_LINE_BYTES: u64 =
    MAXIMUM_WORKSPACE_FILES_SEARCH_REQUEST_BYTES as u64 + 8 * 1024;
/// Preview lines one report carries at most, unless a single entry holds more.
const BATCH_LINES: usize = 64;
/// How long found lines wait for company before they are reported.
const BATCH_WAIT: Duration = Duration::from_millis(50);

/// One command line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SearchCommand {
    /// Searches beneath `root`, an absolute Workspace directory. An `id` already running is
    /// ignored.
    Start {
        id: u64,
        root: String,
        query: WorkspaceSearchQuery,
    },
    /// Lists the files beneath `root`, as [`SearchCommand::Start`] would walk them, under an `id`
    /// of the same kind.
    List {
        id: u64,
        root: String,
        query: WorkspaceFileListQuery,
    },
    /// Stops a search or listing; it still ends with a report.
    Cancel { id: u64 },
}

/// One report line. A search's `found` reports come in order, and an entry continuing the
/// previous entry's file comes straight after it; a listing's `listed` reports carry at most a
/// page of paths each; `ended` comes last.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SearchReport {
    Found {
        id: u64,
        files: Vec<WorkspaceSearchFile>,
    },
    Listed {
        id: u64,
        paths: Vec<String>,
    },
    Ended {
        id: u64,
        state: WorkspaceSearchState,
    },
}

/// What reaches the lane's loop.
enum Input {
    Command(SearchCommand),
    Report(SearchReport),
    Closed,
}

/// Serves commands from `input` and writes reports to `output` until input closes, output fails,
/// or nothing has run for the idle time.
pub fn serve_search(input: impl BufRead + Send + 'static, mut output: impl Write) {
    let (sender, inputs) = mpsc::channel();
    let commands = sender.clone();
    let reader = std::thread::Builder::new()
        .name("orchbee-workspace-files-search-input".into())
        .spawn(move || read_commands(input, &commands));
    if reader.is_err() {
        return;
    }
    run(&inputs, &sender, &mut output);
}

/// Reads command lines until input ends; a line that is not a command is skipped.
fn read_commands(mut input: impl BufRead, sender: &Sender<Input>) {
    loop {
        let mut line = Vec::new();
        match (&mut input)
            .take(MAXIMUM_COMMAND_LINE_BYTES)
            .read_until(b'\n', &mut line)
        {
            Ok(0) | Err(_) => {
                let _closed = sender.send(Input::Closed);
                return;
            }
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') {
            let mut rest = Vec::new();
            if input
                .read_until(b'\n', &mut rest)
                .map_or(true, |read| read == 0)
            {
                let _closed = sender.send(Input::Closed);
                return;
            }
            continue;
        }
        if let Ok(command) = serde_json::from_slice::<SearchCommand>(&line)
            && sender.send(Input::Command(command)).is_err()
        {
            return;
        }
    }
}

fn run(inputs: &Receiver<Input>, sender: &Sender<Input>, output: &mut impl Write) {
    let mut running: HashMap<u64, Arc<AtomicBool>> = HashMap::new();
    let mut idle_since = Some(Instant::now());
    let cancel_all = |running: &HashMap<u64, Arc<AtomicBool>>| {
        for cancelled in running.values() {
            cancelled.store(true, Ordering::Relaxed);
        }
    };
    loop {
        let wait = idle_since.map_or(Duration::from_secs(60), |since| {
            IDLE_EXIT.saturating_sub(since.elapsed())
        });
        let report = match inputs.recv_timeout(wait) {
            Ok(Input::Command(
                command @ (SearchCommand::Start { .. } | SearchCommand::List { .. }),
            )) => {
                let (SearchCommand::Start { id, .. } | SearchCommand::List { id, .. }) = command
                else {
                    continue;
                };
                if running.contains_key(&id) {
                    continue;
                }
                let cancelled = Arc::new(AtomicBool::new(false));
                let started = running.len() < MAXIMUM_RUNNING_SEARCHES && {
                    let flag = Arc::clone(&cancelled);
                    let reports = sender.clone();
                    std::thread::Builder::new()
                        .name("orchbee-workspace-files-search".into())
                        .spawn(move || match command {
                            SearchCommand::Start { root, query, .. } => {
                                conduct(id, &root, query, &flag, &reports);
                            }
                            SearchCommand::List { root, query, .. } => {
                                let state = list::list(
                                    &root,
                                    query,
                                    &flag,
                                    &mut list::sender(id, &reports),
                                );
                                let _closed =
                                    reports.send(Input::Report(SearchReport::Ended { id, state }));
                            }
                            SearchCommand::Cancel { .. } => {}
                        })
                        .is_ok()
                };
                if !started {
                    write(
                        output,
                        &SearchReport::Ended {
                            id,
                            state: WorkspaceSearchState::Unavailable,
                        },
                    );
                    continue;
                }
                running.insert(id, cancelled);
                idle_since = None;
                continue;
            }
            Ok(Input::Command(SearchCommand::Cancel { id })) => {
                if let Some(cancelled) = running.get(&id) {
                    cancelled.store(true, Ordering::Relaxed);
                }
                continue;
            }
            Ok(Input::Report(report)) => report,
            Ok(Input::Closed) | Err(RecvTimeoutError::Disconnected) => {
                cancel_all(&running);
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                if idle_since.is_some_and(|since| since.elapsed() >= IDLE_EXIT) {
                    return;
                }
                continue;
            }
        };
        if let SearchReport::Ended { id, .. } = &report {
            running.remove(id);
            if running.is_empty() {
                idle_since = Some(Instant::now());
            }
        }
        if !write(output, &report) {
            cancel_all(&running);
            return;
        }
    }
}

/// Writes one report line, or for found entries too many for one line, several lines in order;
/// false once output has failed.
fn write(output: &mut impl Write, report: &SearchReport) -> bool {
    let Ok(mut line) = serde_json::to_vec(report) else {
        return true;
    };
    if line.len() >= MAXIMUM_SEARCH_REPORT_LINE_BYTES {
        return match report {
            SearchReport::Found { id, files } if files.len() > 1 => {
                let (first, second) = files.split_at(files.len() / 2);
                [first, second].into_iter().all(|files| {
                    write(
                        output,
                        &SearchReport::Found {
                            id: *id,
                            files: files.to_vec(),
                        },
                    )
                })
            }
            // One entry holds at most a few megabytes less than the line: never reached.
            _ => true,
        };
    }
    line.push(b'\n');
    output
        .write_all(&line)
        .and_then(|()| output.flush())
        .is_ok()
}

/// Runs one search to its end and reports it: what it finds in batches, then how it ended.
fn conduct(
    id: u64,
    root: &str,
    query: WorkspaceSearchQuery,
    cancelled: &AtomicBool,
    reports: &Sender<Input>,
) {
    let state = match engine::prepare(root, query) {
        Err(state) => state,
        Ok(plan) => {
            let shared = engine::Shared::new(cancelled);
            let (found, entries) = mpsc::channel();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    engine::run(&plan, &shared, &found);
                    drop(found);
                });
                batch(id, &entries, cancelled, reports);
            });
            shared.state()
        }
    };
    let _closed = reports.send(Input::Report(SearchReport::Ended { id, state }));
}

/// Gathers what the walk finds into reports of about [`BATCH_LINES`] lines, each sent within
/// [`BATCH_WAIT`] of its first line, until the walk stops sending.
fn batch(
    id: u64,
    entries: &Receiver<Vec<WorkspaceSearchFile>>,
    cancelled: &AtomicBool,
    reports: &Sender<Input>,
) {
    let mut held: Vec<WorkspaceSearchFile> = Vec::new();
    let mut lines = 0;
    let mut since: Option<Instant> = None;
    let flush = |held: &mut Vec<WorkspaceSearchFile>, lines: &mut usize| {
        *lines = 0;
        let held = std::mem::take(held);
        if cancelled.load(Ordering::Relaxed) {
            return;
        }
        for files in reports_of(held) {
            let _closed = reports.send(Input::Report(SearchReport::Found { id, files }));
        }
    };
    loop {
        let wait = since.map_or(Duration::from_secs(60), |since| {
            BATCH_WAIT.saturating_sub(since.elapsed())
        });
        match entries.recv_timeout(wait) {
            Ok(files) => {
                lines += files.iter().map(|file| file.lines.len()).sum::<usize>();
                held.extend(files);
                since.get_or_insert_with(Instant::now);
                if lines >= BATCH_LINES {
                    flush(&mut held, &mut lines);
                    since = None;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                flush(&mut held, &mut lines);
                since = None;
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush(&mut held, &mut lines);
                return;
            }
        }
    }
}

/// Entries split, in order, into reports of at most [`BATCH_LINES`] lines, an entry never split.
fn reports_of(entries: Vec<WorkspaceSearchFile>) -> Vec<Vec<WorkspaceSearchFile>> {
    let mut reports = Vec::new();
    let mut current: Vec<WorkspaceSearchFile> = Vec::new();
    let mut lines = 0;
    for entry in entries {
        if !current.is_empty() && lines + entry.lines.len() > BATCH_LINES {
            reports.push(std::mem::take(&mut current));
            lines = 0;
        }
        lines += entry.lines.len();
        current.push(entry);
    }
    if !current.is_empty() {
        reports.push(current);
    }
    reports
}

/// The search lane process: serves standard input until it closes.
pub fn serve_search_stdio() {
    serve_search(
        std::io::BufReader::new(std::io::stdin()),
        std::io::stdout().lock(),
    );
}

#[cfg(test)]
mod tests;
