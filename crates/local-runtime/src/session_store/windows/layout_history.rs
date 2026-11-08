//! Earlier-layout history in the Session folder: one atomically renamed file per entry, named by
//! its UTC save second, with the oldest beyond the kept count removed on the same write.
use super::{SessionStore, private_folder, publish_private, remove_if_present};
use crate::{LayoutHistoryEntry, layout_history as history, private_filesystem as permissions};
use std::{
    fs,
    io::{self, Read},
    num::NonZeroU64,
    path::PathBuf,
};

/// One entry file: the UTC second and number its name carries.
struct Name {
    second: u64,
    number: u32,
    text: String,
}

impl SessionStore {
    fn history_directory(&self) -> PathBuf {
        self.directory.join(history::DIRECTORY)
    }

    /// Every kept entry, newest first. An absent folder is an empty history; an unreadable entry
    /// is listed without a payload, at the start of the second its name carries, so the caller
    /// can skip it while it still counts as kept.
    pub fn read_layout_history(&self) -> io::Result<Vec<LayoutHistoryEntry>> {
        let mut entries = Vec::new();
        for name in self.history_names()? {
            let (saved_at, payload) = match self.read_history_file(&name)? {
                Some((saved_at, payload)) => (saved_at.get(), Some(payload)),
                None => ((name.second * 1000).max(1), None),
            };
            entries.push(LayoutHistoryEntry { saved_at, payload });
        }
        Ok(entries)
    }

    /// The payload saved at `saved_at`, or `None` when no readable entry has that time.
    pub fn read_layout_history_entry(&self, saved_at: u64) -> io::Result<Option<Vec<u8>>> {
        for name in self.history_names()? {
            if name.second == saved_at / 1000
                && let Some((at, payload)) = self.read_history_file(&name)?
                && at.get() == saved_at
            {
                return Ok(Some(payload));
            }
        }
        Ok(None)
    }

    /// Publishes one entry through a flushed staging file and an atomic rename, then removes the
    /// oldest entries beyond `keep`. An entry with the same frame keeps its name; one with
    /// different content in the same second takes the next free number. A failure before the
    /// rename leaves the history unchanged.
    pub fn record_layout_history(
        &mut self,
        saved_at: u64,
        payload: &[u8],
        keep: usize,
    ) -> io::Result<()> {
        let saved_at = NonZeroU64::new(saved_at).ok_or(io::ErrorKind::InvalidInput)?;
        if payload.len() > self.maximum_bytes || keep == 0 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let directory = self.history_directory();
        let _folder = private_folder(&directory)?;
        let frame = history::frame(saved_at, payload);
        let mut number = 1;
        let target = loop {
            let target = directory.join(history::entry_name(saved_at, number));
            match self.read_frame(&target)? {
                Some(existing) if existing != frame => {
                    number = number.checked_add(1).ok_or(io::ErrorKind::InvalidData)?;
                }
                _ => break target,
            }
        };
        publish_private(
            &directory.join(history::PENDING),
            &target,
            &frame,
            "layout history",
        )?;
        for old in self.history_names()?.into_iter().skip(keep) {
            remove_if_present(&directory.join(old.text))?;
        }
        Ok(())
    }

    /// Entry names, newest first; names outside the entry grammar are not entries.
    fn history_names(&self) -> io::Result<Vec<Name>> {
        let entries = match fs::read_dir(self.history_directory()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut names = Vec::new();
        for entry in entries {
            let leaf = entry?.file_name();
            if let Some(text) = leaf.to_str()
                && let Some((second, number)) = history::entry_key(text)
            {
                names.push(Name {
                    second,
                    number,
                    text: text.to_owned(),
                });
            }
        }
        names.sort_unstable_by_key(|name| std::cmp::Reverse((name.second, name.number)));
        Ok(names)
    }

    fn read_history_file(&self, name: &Name) -> io::Result<Option<(NonZeroU64, Vec<u8>)>> {
        Ok(self
            .read_frame(&self.history_directory().join(&name.text))?
            .and_then(|bytes| history::unframe(name.second, &bytes, self.maximum_bytes)))
    }

    /// The bytes of one entry file, bounded by the largest admitted frame.
    fn read_frame(&self, path: &std::path::Path) -> io::Result<Option<Vec<u8>>> {
        let mut file = match permissions::open_snapshot_file(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(history::frame_limit(self.maximum_bytes) as u64)
            .read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    }
}
