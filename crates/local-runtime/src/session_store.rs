//! Private checkpoint transport in the Session folder: bounded checkpoint payloads, restore facts
//! and kept terminal output. The Server owns the retained document's meaning; the
//! private filesystem adapter owns admission and atomic publication.
use crate::{
    DataRoot, ModeCheckedLocalSessionRendezvous,
    checkpoint_rotation::{self as rotation, Candidate, HEADER_BYTES, SLOTS},
    private_filesystem as permissions,
};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

mod terminal_output;

/// Visible startup recovery facts; filesystem admission errors never become an empty fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckpointRestoreNotice {
    Recovered,
    Empty,
}

pub struct CheckpointLoad<T> {
    pub checkpoint: Option<T>,
    pub notice: Option<CheckpointRestoreNotice>,
}

pub struct SessionStore {
    pub(crate) directory: PathBuf,
    maximum_bytes: usize,
    _lock: File,
    _directories: Vec<File>,
}

impl SessionStore {
    /// Opens the checkpoint storage of the Session whose runtime `session` admitted: its folder in
    /// `root`'s `sessions/`, named by the Session and created private when missing.
    pub fn open(
        root: &DataRoot,
        session: &ModeCheckedLocalSessionRendezvous,
        maximum_bytes: usize,
    ) -> io::Result<Self> {
        if maximum_bytes == 0 || maximum_bytes.checked_add(HEADER_BYTES + 1).is_none() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let name = session.session_name()?;
        let directory = root.sessions().join(name.as_str());
        let directories = crate::data_root::create_state_directory(&directory)?;
        let lock_path = directory.join("state.lock");
        let lock = match permissions::open_file(&lock_path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&lock_path, false, true)?
            }
            Err(error) => return Err(error),
        };
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Error(error)) => return Err(error),
        }
        let store = Self {
            directory,
            maximum_bytes,
            _lock: lock,
            _directories: directories,
        };
        store.cleanup_pending()?;
        crate::checkpoint_restore_notice::remove_pending(
            &store
                .directory
                .join(crate::checkpoint_restore_notice::PENDING),
        )?;
        Ok(store)
    }

    pub fn read(&self) -> io::Result<Option<Vec<u8>>> {
        let loaded = self.read_validated(|bytes| Some(bytes.to_vec()))?;
        if loaded.checkpoint.is_none() && loaded.notice.is_some() {
            Err(io::ErrorKind::InvalidData.into())
        } else {
            Ok(loaded.checkpoint)
        }
    }

    /// Each complete payload is validated once. Invalid originals remain untouched by loading.
    pub fn read_validated<T>(
        &self,
        parse: impl FnMut(&[u8]) -> Option<T>,
    ) -> io::Result<CheckpointLoad<T>> {
        let (mut candidates, _pins) = self.read_slots()?;
        let mut values = rotation::validate(&mut candidates, parse);
        let checkpoint = rotation::latest(&candidates).and_then(|index| values[index].take());
        let invalid = candidates
            .iter()
            .any(|candidate| matches!(candidate, Candidate::Invalid { .. }));
        let notice = if checkpoint.is_some() {
            invalid.then_some(CheckpointRestoreNotice::Recovered)
        } else if invalid || self.has_corrupt_originals()? {
            Some(CheckpointRestoreNotice::Empty)
        } else {
            None
        };
        Ok(CheckpointLoad { checkpoint, notice })
    }

    /// Opaque transport compatibility. Domain owners use `publish_validated` for every old copy.
    pub fn publish(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.publish_validated(bytes, |_| true)
    }

    /// Flush one complete pending file, preserve any displaced invalid original, then replace
    /// only the missing/invalid/oldest slot. Disk effects never hold a Server Workbench gate.
    pub fn publish_validated(
        &mut self,
        bytes: &[u8],
        mut validate: impl FnMut(&[u8]) -> bool,
    ) -> io::Result<()> {
        if bytes.len() > self.maximum_bytes || !validate(bytes) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let (mut candidates, mut pins) = self.read_slots()?;
        rotation::validate(&mut candidates, |bytes| validate(bytes).then_some(()));
        self.cleanup_pending()?;
        let Some(publication) = rotation::prepare(&candidates, bytes, self.maximum_bytes)? else {
            return Ok(());
        };
        let pending = self.directory.join("checkpoint.pending");
        let target = self.directory.join(SLOTS[publication.slot]);
        let mut file = permissions::open_file(&pending, true, true)?;
        let written = file
            .write_all(&publication.encoded)
            .and_then(|()| file.sync_all());
        drop(file);
        let published = written.and_then(|()| {
            // All other candidates remain pinned against changes through publication.
            drop(pins[publication.slot].take());
            if publication.preserve_invalid {
                self.preserve_invalid(&target)?;
            }
            permissions::replace(&pending, &target)
        });
        if let Err(error) = published {
            return match self.cleanup_pending() {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::other(format!(
                    "{error}; checkpoint staging cleanup unconfirmed: {cleanup}"
                ))),
            };
        }
        Ok(())
    }

    fn read_slots(&self) -> io::Result<([Candidate; 3], [Option<File>; 3])> {
        let mut candidates = std::array::from_fn(|_| Candidate::Missing);
        let mut pins = std::array::from_fn(|_| None);
        for (index, name) in SLOTS.into_iter().enumerate() {
            match permissions::open_snapshot_file(&self.directory.join(name)) {
                Ok(mut file) => {
                    let mut bytes = Vec::new();
                    (&mut file)
                        .take((self.maximum_bytes + HEADER_BYTES + 1) as u64)
                        .read_to_end(&mut bytes)?;
                    candidates[index] = Candidate::decode(&bytes, self.maximum_bytes);
                    pins[index] = Some(file);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok((candidates, pins))
    }

    fn has_corrupt_originals(&self) -> io::Result<bool> {
        let mut present = false;
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(rotation::is_corrupt_original)
            {
                permissions::open_snapshot_file(&entry.path())?;
                present = true;
            }
        }
        Ok(present)
    }

    /// Moves an invalid original aside under the UTC second it was found, never over another one.
    fn preserve_invalid(&self, path: &Path) -> io::Result<()> {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_secs();
        let mut number = 1;
        let target = loop {
            let target = self
                .directory
                .join(rotation::corrupt_original(seconds, number));
            match fs::symlink_metadata(&target) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => break target,
                Err(error) => return Err(error),
                Ok(_) => number = number.checked_add(1).ok_or(io::ErrorKind::InvalidData)?,
            }
        };
        permissions::move_aside(path, &target)
    }

    fn cleanup_pending(&self) -> io::Result<()> {
        remove_if_present(&self.directory.join("checkpoint.pending"))
    }
}

/// Opens `directory` as a private folder, creating and protecting it when absent. The returned
/// handle pins it while the caller writes inside.
fn private_folder(directory: &Path) -> io::Result<File> {
    let created = match fs::create_dir(directory) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let mut handle = permissions::open_directory(directory, created)?;
    if created {
        permissions::protect_new(&mut handle)?;
    } else {
        permissions::require_private(&handle)?;
    }
    Ok(handle)
}

/// Writes `bytes` to `pending`, flushes it and atomically renames it over `target`. A failure
/// before the rename removes the staging file and leaves `target` unchanged.
fn publish_private(pending: &Path, target: &Path, bytes: &[u8], what: &str) -> io::Result<()> {
    remove_if_present(pending)?;
    let mut file = permissions::open_file(pending, true, true)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let published = written.and_then(|()| permissions::replace(pending, target));
    match published {
        Ok(()) => Ok(()),
        Err(error) => match remove_if_present(pending) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(io::Error::other(format!(
                "{error}; {what} staging cleanup unconfirmed: {cleanup}"
            ))),
        },
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match permissions::open_file(path, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
