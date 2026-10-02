//! Private checkpoint transport; the Server owns the retained document's meaning.
use crate::{
    ModeCheckedLocalSiteRendezvous,
    checkpoint_rotation::{self as rotation, Candidate, HEADER_BYTES, SLOTS},
    windows_permissions as permissions,
};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub struct WindowsSessionStore {
    pub(crate) directory: PathBuf,
    maximum_bytes: usize,
    _lock: File,
    _directories: Vec<File>,
}

/// Visible startup recovery facts; filesystem admission errors never become an empty fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsCheckpointRestoreNotice {
    Recovered,
    Empty,
}

pub struct WindowsCheckpointLoad<T> {
    pub checkpoint: Option<T>,
    pub notice: Option<WindowsCheckpointRestoreNotice>,
}

impl WindowsSessionStore {
    /// `local_app_data` is an existing private absolute directory. The admitted runtime identity
    /// separates Sites, including isolated development bases, without putting data in temp files.
    pub fn open(
        local_app_data: &Path,
        site: &ModeCheckedLocalSiteRendezvous,
        maximum_bytes: usize,
    ) -> io::Result<Self> {
        if maximum_bytes == 0 || maximum_bytes.checked_add(HEADER_BYTES + 1).is_none() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut directories = permissions::open_tree(local_app_data)?;
        let mut directory = local_app_data.to_owned();
        let key = ModeCheckedLocalSiteRendezvous::host_endpoint_id(site.runtime_directory());
        for component in ["Orchbee", "native-sessions", key.as_str()] {
            directory.push(component);
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
            directories.push(handle);
        }
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
        crate::checkpoint_restore_notice_windows::remove_pending(
            &store
                .directory
                .join(crate::checkpoint_restore_notice_windows::PENDING),
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
    ) -> io::Result<WindowsCheckpointLoad<T>> {
        let (mut candidates, _pins) = self.read_slots()?;
        let mut values = rotation::validate(&mut candidates, parse);
        let checkpoint = rotation::latest(&candidates).and_then(|index| values[index].take());
        let invalid = candidates
            .iter()
            .any(|candidate| matches!(candidate, Candidate::Invalid { .. }));
        let notice = if checkpoint.is_some() {
            invalid.then_some(WindowsCheckpointRestoreNotice::Recovered)
        } else if invalid || self.has_corrupt_originals()? {
            Some(WindowsCheckpointRestoreNotice::Empty)
        } else {
            None
        };
        Ok(WindowsCheckpointLoad { checkpoint, notice })
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
            winsafe::MoveFileEx(
                text(&pending)?,
                Some(text(&target)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
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
                    candidates[index] = Candidate::decode(&bytes, self.maximum_bytes, index == 0);
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

    fn preserve_invalid(&self, path: &Path) -> io::Result<()> {
        let nonce = windows::core::GUID::new()
            .map_err(io::Error::other)?
            .to_u128();
        if nonce == 0 {
            return Err(io::ErrorKind::Other.into());
        }
        let target = self
            .directory
            .join(format!("checkpoint.corrupt-{nonce:032x}.raw"));
        winsafe::MoveFileEx(
            text(path)?,
            Some(text(&target)?),
            winsafe::co::MOVEFILE::WRITE_THROUGH,
        )
        .map_err(io::Error::other)
    }

    fn cleanup_pending(&self) -> io::Result<()> {
        let path = self.directory.join("checkpoint.pending");
        match permissions::open_file(&path, false, false) {
            Ok(file) => {
                drop(file);
                fs::remove_file(path)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn text(path: &Path) -> io::Result<&str> {
    path.to_str()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
