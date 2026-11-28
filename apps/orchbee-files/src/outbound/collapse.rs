//! What the Updates of a collapsed subscription fold into. An SCM summary states everything, so the
//! latest replaces the others. A watch sends its latest root, its latest watch state and then one
//! rescan, after the root, so the Client lists again under the root it then holds. The rescan names
//! the folder of each lost change and each folder a lost rescan named; past 500 folders or the
//! Notice record's bound the deepest give way to the folders that hold them, since a Client lists
//! again what it loaded below each folder a rescan names.

use std::collections::{BTreeMap, btree_map::Entry};

use orchbee_files_protocol::{
    CallId, FilePath, FilesNotice, FilesRecord, FrameKind, FsUpdate, List, MAXIMUM_REPLY_BYTES,
    Root, WatchState, encode_notice,
};

use crate::scope::path::NativePath;

/// The folders one rescan names.
const MAXIMUM_FOLDERS: usize = 500;

/// Whether `update` folds when its subscription collapses: a watch's and an SCM subscription's
/// do; a relay prompt never does.
pub(super) const fn folds(update: &FilesNotice) -> bool {
    matches!(
        update,
        FilesNotice::FsUpdate(_) | FilesNotice::ScmSummary(_)
    )
}

/// The Updates one collapsed subscription folded.
#[derive(Default)]
pub(super) struct Folded {
    /// An SCM subscription's latest summary, as its frame.
    summary: Option<Vec<u8>>,
    /// A watch's latest root.
    root: Option<Root>,
    /// A watch's latest watch state.
    watch: Option<WatchState>,
    /// The folders a watch's rescan names, by [`NativePath::key`].
    folders: BTreeMap<String, NativePath>,
    /// The length of their JSON, each with a comma.
    folder_bytes: usize,
}

impl Folded {
    /// Folds `update`, encoded as `frame`, into what replaces it.
    pub(super) fn fold(&mut self, update: &FilesNotice, frame: Vec<u8>) {
        match update {
            FilesNotice::ScmSummary(_) => self.summary = Some(frame),
            FilesNotice::FsUpdate(update) => self.fold_watch(update),
            // [`folds`] keeps every other notice out of a collapsed place.
            FilesNotice::FsSnapshot(_)
            | FilesNotice::RelaySnapshot(_)
            | FilesNotice::RelayUpdate(_) => {}
        }
    }

    fn fold_watch(&mut self, update: &FsUpdate) {
        match update {
            FsUpdate::Changes { changes } => {
                for change in changes.as_slice() {
                    if let Ok(path) = NativePath::parse(change.path.as_str()) {
                        self.add(path.parent().unwrap_or(path));
                    }
                }
                self.bound();
            }
            FsUpdate::Rescan { paths } => {
                for path in paths.as_slice() {
                    if let Ok(path) = NativePath::parse(path.as_str()) {
                        self.add(path);
                    }
                }
                self.bound();
            }
            FsUpdate::Root { root } => self.root = Some(root.clone()),
            FsUpdate::Watch { watch } => self.watch = Some(*watch),
        }
    }

    /// The frames of subscription `call` that replace every Update folded in, one after another.
    pub(super) fn frames(self, call: CallId) -> Vec<u8> {
        let mut frames = self.summary.unwrap_or_default();
        let folders = self
            .folders
            .into_values()
            .filter_map(|folder| FilePath::new(folder.as_str()).ok())
            .collect();
        let rescan = List::new(folders)
            .ok()
            .map(|paths| FsUpdate::Rescan { paths });
        let updates = self
            .root
            .map(|root| FsUpdate::Root { root })
            .into_iter()
            .chain(self.watch.map(|watch| FsUpdate::Watch { watch }))
            .chain(rescan);
        for update in updates {
            let record = FilesRecord::Notice(encode_notice(&FilesNotice::FsUpdate(update)));
            frames.extend(super::frame(call, FrameKind::Update, &record));
        }
        frames
    }

    fn add(&mut self, folder: NativePath) {
        if let Entry::Vacant(vacant) = self.folders.entry(folder.key()) {
            self.folder_bytes += json_length(&folder);
            vacant.insert(folder);
        }
    }

    /// Keeps the folders within one rescan: while too many or too long, the deepest give way to the
    /// folders that hold them. A filesystem root holds no other, and the machine has few.
    fn bound(&mut self) {
        let envelope = envelope();
        while self.folders.len() > MAXIMUM_FOLDERS
            || envelope + self.folder_bytes > MAXIMUM_REPLY_BYTES
        {
            let Some(deepest) = self
                .folders
                .values()
                .map(NativePath::depth)
                .max()
                .filter(|depth| *depth > 0)
            else {
                return;
            };
            let lifted: Vec<String> = self
                .folders
                .iter()
                .filter(|(_, folder)| folder.depth() == deepest)
                .map(|(key, _)| key.clone())
                .collect();
            for key in lifted {
                if let Some(folder) = self.folders.remove(&key) {
                    self.folder_bytes -= json_length(&folder);
                    if let Some(parent) = folder.parent() {
                        self.add(parent);
                    }
                }
            }
        }
    }
}

/// The length of `folder`'s JSON and the comma after it.
fn json_length(folder: &NativePath) -> usize {
    serde_json::to_vec(folder.as_str()).map_or(0, |json| json.len()) + 1
}

/// The bytes of a rescan's JSON around its folders.
fn envelope() -> usize {
    const ONE: &str = "a";
    let rescan = FilePath::new(ONE)
        .ok()
        .and_then(|path| List::new(vec![path]).ok())
        .map(|paths| FilesNotice::FsUpdate(FsUpdate::Rescan { paths }));
    rescan.map_or(0, |rescan| encode_notice(&rescan).len() - (ONE.len() + 2))
}
