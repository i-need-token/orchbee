//! Private checkpoint transport; the Server owns the retained document's meaning.
use crate::{ModeCheckedLocalSiteRendezvous, windows_permissions as permissions};
use std::{
    fs::{self, File, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub struct WindowsSessionStore {
    directory: PathBuf,
    maximum_bytes: usize,
    _lock: File,
    _directories: Vec<File>,
}

impl WindowsSessionStore {
    /// `local_app_data` is an existing private absolute directory. The admitted runtime identity
    /// separates Sites, including isolated development bases, without putting data in temp files.
    pub fn open(
        local_app_data: &Path,
        site: &ModeCheckedLocalSiteRendezvous,
        maximum_bytes: usize,
    ) -> io::Result<Self> {
        if maximum_bytes == 0
            || u64::try_from(maximum_bytes)
                .ok()
                .and_then(|size| size.checked_add(1))
                .is_none()
        {
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
        Ok(store)
    }

    pub fn read(&self) -> io::Result<Option<Vec<u8>>> {
        let Some(file) = self.committed_file()? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        file.take(self.maximum_bytes as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > self.maximum_bytes {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Some(bytes))
    }

    /// The successful file replacement is the publication point. The caller validates the
    /// checkpoint before calling; opaque empty bytes remain distinct from a missing checkpoint.
    pub fn publish(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() > self.maximum_bytes {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let previous = self.committed_file()?;
        self.cleanup_pending()?;
        let pending = self.directory.join("checkpoint.pending");
        let target = self.directory.join("checkpoint.json");
        let mut file = permissions::open_file(&pending, true, true)?;
        let written = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        drop(previous);
        let published = written.and_then(|()| {
            winsafe::MoveFileEx(
                text(&pending)?,
                Some(text(&target)?),
                winsafe::co::MOVEFILE::REPLACE_EXISTING,
            )
            .map_err(io::Error::other)
        });
        if let Err(error) = published {
            self.cleanup_pending()?;
            return Err(error);
        }
        Ok(())
    }

    fn committed_file(&self) -> io::Result<Option<File>> {
        match permissions::open_file(&self.directory.join("checkpoint.json"), false, false) {
            Ok(file) => {
                if file.metadata()?.len() > self.maximum_bytes as u64 {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                Ok(Some(file))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
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
