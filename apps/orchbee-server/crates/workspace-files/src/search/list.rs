//! One file listing: the paths of the files a search with the same exclude globs and ignore
//! choice would read, walked on several threads and reported in pages of
//! [`MAXIMUM_WORKSPACE_FILE_LIST_PAGE`] paths, for a client to match file names against.
use super::engine::{overrides, relative, walker};
use crate::files::Root;
use ignore::WalkState;
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_LIST_PAGE, MAXIMUM_WORKSPACE_LISTED_FILES, WorkspaceFileListQuery,
    WorkspaceFilesOperation, WorkspaceSearchState,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// How long found paths wait for company before they are reported.
const BATCH_WAIT: Duration = Duration::from_millis(50);

/// Lists the files beneath `root` as `query` asks, sending each batch of at most
/// [`MAXIMUM_WORKSPACE_FILE_LIST_PAGE`] paths to `report`, and answers how the listing ended.
pub(super) fn list(
    root: &str,
    query: WorkspaceFileListQuery,
    cancelled: &AtomicBool,
    report: &mut dyn FnMut(Vec<String>),
) -> WorkspaceSearchState {
    let operation = WorkspaceFilesOperation::ListFiles { query };
    if operation.validate().is_err() {
        return WorkspaceSearchState::Unavailable;
    }
    let WorkspaceFilesOperation::ListFiles { query } = operation else {
        return WorkspaceSearchState::Unavailable;
    };
    let Ok(root) = Root::open(root) else {
        return WorkspaceSearchState::Unavailable;
    };
    let Some(overrides) = overrides(root.path(), &[], &query.excludes) else {
        return WorkspaceSearchState::InvalidGlob;
    };
    let listed = AtomicU64::new(0);
    let limited = AtomicBool::new(false);
    let stopped = || cancelled.load(Ordering::Relaxed) || limited.load(Ordering::Relaxed);
    let (found, paths) = mpsc::channel::<String>();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            walker(&root, query.use_ignore_files, overrides).run(|| {
                let found = found.clone();
                let (root, listed, limited) = (&root, &listed, &limited);
                Box::new(move |entry| {
                    if stopped() {
                        return WalkState::Quit;
                    }
                    let Ok(entry) = entry else {
                        return WalkState::Continue;
                    };
                    if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                        return WalkState::Continue;
                    }
                    let Some(path) = relative(root.path(), entry.path()) else {
                        return WalkState::Continue;
                    };
                    if listed.fetch_add(1, Ordering::Relaxed) >= MAXIMUM_WORKSPACE_LISTED_FILES {
                        limited.store(true, Ordering::Relaxed);
                        return WalkState::Quit;
                    }
                    if found.send(path).is_err() {
                        return WalkState::Quit;
                    }
                    WalkState::Continue
                })
            });
            drop(found);
        });
        batch(&paths, cancelled, report);
    });
    if cancelled.load(Ordering::Relaxed) {
        WorkspaceSearchState::Unavailable
    } else if limited.load(Ordering::Relaxed) {
        WorkspaceSearchState::LimitReached
    } else {
        WorkspaceSearchState::Complete
    }
}

/// Gathers paths into batches of a page, each reported within [`BATCH_WAIT`] of its first path,
/// until the walk stops sending; nothing is reported once the listing is cancelled.
fn batch(paths: &Receiver<String>, cancelled: &AtomicBool, report: &mut dyn FnMut(Vec<String>)) {
    let mut held = Vec::new();
    let mut since: Option<Instant> = None;
    let mut flush = |held: &mut Vec<String>| {
        let batch = std::mem::take(held);
        if !batch.is_empty() && !cancelled.load(Ordering::Relaxed) {
            report(batch);
        }
    };
    loop {
        let wait = since.map_or(Duration::from_secs(60), |since| {
            BATCH_WAIT.saturating_sub(since.elapsed())
        });
        match paths.recv_timeout(wait) {
            Ok(path) => {
                held.push(path);
                since.get_or_insert_with(Instant::now);
                if held.len() >= MAXIMUM_WORKSPACE_FILE_LIST_PAGE {
                    flush(&mut held);
                    since = None;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                flush(&mut held);
                since = None;
            }
            Err(RecvTimeoutError::Disconnected) => {
                flush(&mut held);
                return;
            }
        }
    }
}

/// Sends the found paths of one listing to the lane's loop as they come.
pub(super) fn sender(id: u64, reports: &Sender<super::Input>) -> impl FnMut(Vec<String>) + '_ {
    move |paths| {
        let _closed = reports.send(super::Input::Report(super::SearchReport::Listed {
            id,
            paths,
        }));
    }
}
