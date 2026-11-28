//! `files-search`: text search and Quick Open, served with the rg that ships beside this process,
//! as VS Code ef5ca45 serves them with its own.
//!
//! `search.text` starts a search, which runs on a thread of its own and queues its results;
//! `search.next` takes them in batches, one call at a time, so the Client's pace bounds what waits
//! on the link. A new search from a connection in a Workspace cancels the one before it, and a
//! search ends with its last batch, its lane, its connection, its Workspace's scope change or the
//! drain. `search.find-files` scores a Quick Open file list, made by the first query that names
//! it. Every rg this process runs takes one of 4 slots, and a cancelled run stops rg's process
//! tree before it gives its slot back.

mod args;
mod array_sort;
mod file_list;
mod fuzzy;
mod glob;
mod js;
mod json;
mod listing;
mod lists;
mod paths;
mod pattern;
mod preview;
mod program;
mod queue;
mod regexp;
mod rg;
mod scoring;
mod slots;
mod text;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    CallClose, ConnectionId, ErrorCode, FilesError, FilesReply, LaneId, List, SearchCloseCache,
    SearchFindFiles, SearchFindFilesResult, SearchId, SearchRequest, SearchText, WorkspaceId,
};

use self::{
    lists::{Key, Lists},
    preview::PreviewOptions,
    queue::{Queue, Taken},
    rg::Launch,
    slots::Slots,
};
use crate::{
    calls::Call,
    scope::{View, path::NativePath},
};

/// The longest `search.next` waits for results before it answers with what there is.
const BATCH_WAIT: Duration = Duration::from_millis(250);

/// The searches and file lists of every connection.
pub(crate) struct Service {
    launch: Arc<dyn Launch>,
    slots: Arc<Slots>,
    /// The running searches; the lock is never held while a call is answered.
    texts: Mutex<HashMap<(ConnectionId, SearchId), Running>>,
    lists: Lists,
}

/// One search that has not reported its end.
struct Running {
    workspace: WorkspaceId,
    lane: LaneId,
    queue: Arc<Queue>,
}

impl Service {
    /// Searches with the rg beside this process's executable.
    pub(crate) fn new() -> Self {
        Self::with_launch(Arc::new(rg::Process::new()))
    }

    fn with_launch(launch: Arc<dyn Launch>) -> Self {
        let slots = Arc::new(Slots::new());
        Self {
            lists: Lists::new(Arc::clone(&launch), Arc::clone(&slots)),
            launch,
            slots,
            texts: Mutex::default(),
        }
    }

    /// `search.text`: starts the search and answers at once; its results wait for `search.next`.
    pub(crate) fn text(&self, call: Call, view: &View, request: &SearchText) {
        let folders = request.folders.as_slice().iter().map(|folder| {
            view.searchable(&folder.path).map(|path| text::Folder {
                path,
                includes: globs(&folder.includes),
            })
        });
        let folders = match folders.collect::<Result<Vec<_>, _>>() {
            Ok(folders) => folders,
            Err(refused) => return call.reply(&FilesReply::Error(refused)),
        };
        let search = text::Request {
            pattern: request.pattern.as_str().to_owned(),
            regex: request.regex,
            case_sensitive: request.case_sensitive,
            whole_word: request.whole_word,
            multiline: request.multiline,
            encoding: request
                .encoding
                .as_ref()
                .map(|encoding| encoding.as_str().to_owned()),
            context: request.context.map_or(0, |context| {
                u32::try_from(context.get()).unwrap_or(u32::MAX)
            }),
            folders,
            includes: globs(&request.includes),
            excludes: globs(&request.excludes),
            use_ignore_files: request.use_ignore_files,
            use_parent_ignore_files: request.use_parent_ignore_files,
            follow_symlinks: request.follow_symlinks,
            max_results: request.max_results.get(),
            preview: PreviewOptions {
                match_lines: u32::try_from(request.preview.match_lines.get()).unwrap_or(u32::MAX),
                chars_per_line: u32::try_from(request.preview.chars_per_line.get())
                    .unwrap_or(u32::MAX),
            },
        };
        let key = (call.connection(), request.search);
        let queue = Arc::new(Queue::new());
        let mut texts = self.lock();
        if texts.contains_key(&key) {
            drop(texts);
            return call.reply(&FilesReply::Error(FilesError::new(
                ErrorCode::InvalidRequest,
            )));
        }
        let previous: Vec<Running> = texts
            .extract_if(|(connection, _), running| {
                *connection == key.0 && running.workspace == request.workspace_id
            })
            .map(|(_, running)| running)
            .collect();
        texts.insert(
            key,
            Running {
                workspace: request.workspace_id,
                lane: call.lane(),
                queue: Arc::clone(&queue),
            },
        );
        drop(texts);
        for running in previous {
            running.queue.cancel();
        }
        let (launch, slots, worker) = (
            Arc::clone(&self.launch),
            Arc::clone(&self.slots),
            Arc::clone(&queue),
        );
        let started = thread::Builder::new()
            .name("orchbee-files-search".into())
            .spawn(move || text::run(&search, launch.as_ref(), &slots, &worker));
        if started.is_err() {
            self.forget(key, &queue);
            return call.close(CallClose::Capacity);
        }
        call.reply(&FilesReply::Empty);
    }

    /// `search.next`, on a thread of its own: the search's next batch, once results wait or the
    /// search ended, or within 250 ms with what there is.
    pub(crate) fn next(&self, call: Call, request: &SearchRequest) {
        let key = (call.connection(), request.search);
        let queue = self
            .lock()
            .get(&key)
            .filter(|running| running.workspace == request.workspace_id)
            .map(|running| Arc::clone(&running.queue));
        let Some(queue) = queue else {
            return answer(call, &invalid());
        };
        match queue.take(Instant::now() + BATCH_WAIT, || call.cancelled()) {
            Taken::Batch(batch) => {
                if batch.done {
                    self.forget(key, &queue);
                }
                answer(call, &FilesReply::SearchNext(batch));
            }
            Taken::Failed(error) => {
                self.forget(key, &queue);
                answer(call, &FilesReply::Error(error));
            }
            Taken::Concurrent => answer(call, &invalid()),
            // Dropping the call closes it as cancelled.
            Taken::Abandoned => drop(call),
        }
    }

    /// `search.find-files`, on a thread of its own, in `folders`, which the call's scope admitted:
    /// the best files of the list `request.cache` names, once the list is made.
    pub(crate) fn find_files(
        &self,
        call: Call,
        folders: Vec<NativePath>,
        request: &SearchFindFiles,
    ) {
        let key = Key {
            connection: call.connection(),
            workspace: request.workspace_id,
            cache: request.cache,
        };
        let listing = listing::Request {
            folders,
            includes: globs(&request.includes),
            excludes: globs(&request.excludes),
            use_ignore_files: request.use_ignore_files,
            use_parent_ignore_files: request.use_parent_ignore_files,
            follow_symlinks: request.follow_symlinks,
        };
        let list = match self.lists.get(key, &listing, || call.cancelled()) {
            Ok(list) => list,
            Err(error) => return answer(call, &FilesReply::Error(error)),
        };
        let maximum = usize::try_from(request.max_results.get()).unwrap_or(usize::MAX);
        let Some(found) =
            scoring::find(&list, request.query.as_str(), maximum, || call.cancelled())
        else {
            return drop(call);
        };
        let results = List::new(found.results).expect("a query returns at most its maximum");
        let result = SearchFindFilesResult {
            results,
            truncated: list.truncated(),
            limit_hit: found.limit_hit,
        };
        answer(call, &FilesReply::SearchFindFiles(result));
    }

    /// `search.close-cache`: drops the list, made or not.
    pub(crate) fn close_cache(&self, call: Call, request: &SearchCloseCache) {
        self.lists.close(Key {
            connection: call.connection(),
            workspace: request.workspace_id,
            cache: request.cache,
        });
        call.reply(&FilesReply::Empty);
    }

    /// The Client closed `lane`, which carried searches.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.cancel(|(held, _), running| *held == connection && running.lane == lane);
    }

    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.cancel(|(held, _), _| *held == connection);
        self.lists.connection_lost(connection);
    }

    /// The scope of `workspace` changed, so its searches and lists read the old root.
    pub(crate) fn scope_changed(&self, workspace: WorkspaceId) {
        self.cancel(|_, running| running.workspace == workspace);
        self.lists.scope_changed(workspace);
    }

    pub(crate) fn drain(&self) {
        self.cancel(|_, _| true);
        self.lists.drain();
    }

    /// Cancels and forgets every search that `matches`.
    fn cancel(&self, matches: impl Fn(&(ConnectionId, SearchId), &Running) -> bool) {
        let cancelled: Vec<Running> = self
            .lock()
            .extract_if(|key, running| matches(key, running))
            .map(|(_, running)| running)
            .collect();
        for running in cancelled {
            running.queue.cancel();
        }
    }

    /// Forgets search `key` once it reported its end, unless a later search took its name.
    fn forget(&self, key: (ConnectionId, SearchId), queue: &Arc<Queue>) {
        let mut texts = self.lock();
        if texts
            .get(&key)
            .is_some_and(|running| Arc::ptr_eq(&running.queue, queue))
        {
            texts.remove(&key);
        }
        drop(texts);
        queue.cancel();
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<(ConnectionId, SearchId), Running>> {
        self.texts.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Ends `call` with `reply`, unless its Client cancelled it, which dropping the call reports.
fn answer(call: Call, reply: &FilesReply) {
    if !call.cancelled() {
        call.reply(reply);
    }
}

fn invalid() -> FilesReply {
    FilesReply::Error(FilesError::new(ErrorCode::InvalidRequest))
}

/// Globs as the user wrote them.
fn globs<const MIN: usize, const MAX: usize>(
    globs: &List<orchbee_files_protocol::SearchGlob, MIN, MAX>,
) -> Vec<String> {
    globs
        .as_slice()
        .iter()
        .map(|glob| glob.as_str().to_owned())
        .collect()
}
