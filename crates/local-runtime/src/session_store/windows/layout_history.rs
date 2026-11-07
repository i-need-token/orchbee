//! Earlier-layout history beside the checkpoint slots: one atomically renamed file per entry,
//! named by its save time, with the oldest beyond the kept count removed on the same write.
use super::{SessionStore, text};
use crate::{LayoutHistoryEntry, layout_history as history, private_filesystem as permissions};
use std::{
    fs,
    io::{self, Read, Write},
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
        let created = match fs::create_dir(&directory) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error),
        };
        let mut handle = permissions::open_directory(&directory, created)?;
        if created {
            permissions::protect_new(&mut handle)?;
        } else {
            permissions::require_private(&handle)?;
        }
        let pending = directory.join(history::PENDING);
        remove_if_present(&pending)?;
        let target = directory.join(history::entry_name(saved_at));
        let mut file = permissions::open_file(&pending, true, true)?;
        let written = file
            .write_all(&history::frame(saved_at, payload))
            .and_then(|()| file.sync_all());
        drop(file);
        let published = written.and_then(|()| {
            winsafe::MoveFileEx(
                text(&pending)?,
                Some(text(&target)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
        });
        if let Err(error) = published {
            return match remove_if_present(&pending) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::other(format!(
                    "{error}; layout history staging cleanup unconfirmed: {cleanup}"
                ))),
            };
        }
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

fn remove_if_present(path: &std::path::Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
