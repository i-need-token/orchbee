//! Earlier-layout history beside the checkpoint slots: one atomically renamed file per entry,
//! named by its save time, with the oldest beyond the kept count removed on the same write.
use super::{SessionStore, private_folder, publish_private, remove_if_present};
use crate::{LayoutHistoryEntry, layout_history as history, private_filesystem as permissions};
use std::{
    fs,
    io::{self, Read},
    num::NonZeroU64,
    path::PathBuf,
};

impl SessionStore {
    fn history_directory(&self) -> PathBuf {
        self.directory.join(history::DIRECTORY)
    }

    /// Every kept entry, newest first. An absent folder is an empty history; an unreadable entry
    /// is listed without a payload so the caller can skip it while it still counts as kept.
    pub fn read_layout_history(&self) -> io::Result<Vec<LayoutHistoryEntry>> {
        let mut entries = Vec::new();
        for saved_at in self.history_times()? {
            entries.push(LayoutHistoryEntry {
                saved_at: saved_at.get(),
                payload: self.read_history_entry(saved_at)?,
            });
        }
        Ok(entries)
    }

    /// The payload saved at `saved_at`, or `None` when no readable entry has that time.
    pub fn read_layout_history_entry(&self, saved_at: u64) -> io::Result<Option<Vec<u8>>> {
        match NonZeroU64::new(saved_at) {
            Some(saved_at) => self.read_history_entry(saved_at),
            None => Ok(None),
        }
    }

    /// Publishes one entry through a flushed staging file and an atomic rename, then removes the
    /// oldest entries beyond `keep`. A failure before the rename leaves the history unchanged.
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
        publish_private(
            &directory.join(history::PENDING),
            &directory.join(history::entry_name(saved_at)),
            &history::frame(saved_at, payload),
            "layout history",
        )?;
        for old in self.history_times()?.into_iter().skip(keep) {
            remove_if_present(&directory.join(history::entry_name(old)))?;
        }
        Ok(())
    }

    /// Entry save times, newest first; names outside the entry grammar are not entries.
    fn history_times(&self) -> io::Result<Vec<NonZeroU64>> {
        let entries = match fs::read_dir(self.history_directory()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut times = Vec::new();
        for entry in entries {
            if let Some(time) = entry?.file_name().to_str().and_then(history::entry_time) {
                times.push(time);
            }
        }
        times.sort_unstable_by(|a, b| b.cmp(a));
        Ok(times)
    }

    fn read_history_entry(&self, saved_at: NonZeroU64) -> io::Result<Option<Vec<u8>>> {
        let path = self.history_directory().join(history::entry_name(saved_at));
        let mut file = match permissions::open_snapshot_file(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(history::frame_limit(self.maximum_bytes) as u64)
            .read_to_end(&mut bytes)?;
        Ok(history::unframe(saved_at, &bytes, self.maximum_bytes))
    }
}
