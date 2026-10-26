//! Descriptor-relative private files and an OS-held lock; a crashed writer cannot leave a lease.
use super::super::{CommitError, RECORD_NAME, UserPreferencesScope};
use nix::{
    errno::Errno,
    fcntl::{OFlag, open, openat, renameat},
    sys::stat::{Mode, SFlag, fstat, mkdirat},
    unistd::{UnlinkatFlags, geteuid, unlinkat},
};
use orchbee_workbench_protocol::{MAXIMUM_USER_PREFERENCES_BYTES, PreferencesFailure};
use std::{
    fs::{File, TryLockError},
    io::{self, Read, Write},
    os::fd::{AsFd, OwnedFd},
    path::{Component, Path},
};
const LOCK: &str = "preferences.lock";
const PENDING: &str = "preferences.pending";
const DIRECTORY: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW);
const READ: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);
const WRITE: OFlag = OFlag::O_RDWR
    .union(OFlag::O_CLOEXEC)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_NONBLOCK);

fn directory(path: &Path, create: bool) -> io::Result<Option<OwnedFd>> {
    let mut current = open("/", DIRECTORY, Mode::empty())?;
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        let name = match component {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            _ => return Err(io::ErrorKind::InvalidInput.into()),
        };
        let child = match openat(&current, *name, DIRECTORY, Mode::empty()) {
            Ok(child) => child,
            Err(Errno::ENOENT) if create => {
                match mkdirat(&current, *name, Mode::from_bits_truncate(0o700)) {
                    Ok(()) | Err(Errno::EEXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                openat(&current, *name, DIRECTORY, Mode::empty())?
            }
            Err(Errno::ENOENT) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let stat = fstat(&child)?;
        let owned = stat.st_uid == geteuid().as_raw();
        let final_directory = index + 1 == components.len();
        let mode = stat.st_mode & 0o7777;
        let protected_ancestor = (owned || stat.st_uid == 0) && mode & 0o022 == 0;
        let system_temporary = stat.st_uid == 0 && mode & 0o1000 != 0;
        if final_directory && (!owned || mode != 0o700)
            || !final_directory && !protected_ancestor && !system_temporary
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        current = child;
    }
    Ok(Some(current))
}
fn validate(file: &impl AsFd) -> io::Result<()> {
    let stat = fstat(file)?;
    if stat.st_uid != geteuid().as_raw()
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
        || SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT != SFlag::S_IFREG
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
fn read_at(directory: &OwnedFd) -> io::Result<Option<Vec<u8>>> {
    let fd = match openat(directory, RECORD_NAME, READ, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::ENOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    validate(&fd)?;
    let file = File::from(fd);
    if file.metadata()?.len() > MAXIMUM_USER_PREFERENCES_BYTES as u64 {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    let mut bytes = Vec::new();
    file.take(MAXIMUM_USER_PREFERENCES_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAXIMUM_USER_PREFERENCES_BYTES {
        return Err(io::ErrorKind::FileTooLarge.into());
    }
    Ok(Some(bytes))
}
pub(in crate::user_preferences) fn read(
    scope: &UserPreferencesScope,
) -> Result<Option<Vec<u8>>, PreferencesFailure> {
    directory(&scope.directory, false)
        .and_then(|directory| directory.as_ref().map_or(Ok(None), read_at))
        .map_err(|_| PreferencesFailure::InvalidSource)
}
pub(in crate::user_preferences) struct Writer {
    directory: OwnedFd,
    _lock: File,
}
impl Writer {
    pub(in crate::user_preferences) fn acquire(
        scope: &UserPreferencesScope,
    ) -> Result<Self, PreferencesFailure> {
        let directory = directory(&scope.directory, true)
            .map_err(|_| PreferencesFailure::WriteFailed)?
            .ok_or(PreferencesFailure::WriteFailed)?;
        let fd = match openat(
            &directory,
            LOCK,
            WRITE.union(OFlag::O_CREAT).union(OFlag::O_EXCL),
            Mode::from_bits_truncate(0o600),
        ) {
            Ok(fd) => fd,
            Err(Errno::EEXIST) => openat(&directory, LOCK, WRITE, Mode::empty())
                .map_err(|_| PreferencesFailure::WriteFailed)?,
            Err(_) => return Err(PreferencesFailure::WriteFailed),
        };
        validate(&fd).map_err(|_| PreferencesFailure::InvalidSource)?;
        let lock = File::from(fd);
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(PreferencesFailure::Busy),
            Err(TryLockError::Error(_)) => return Err(PreferencesFailure::WriteFailed),
        }
        let writer = Self {
            directory,
            _lock: lock,
        };
        writer
            .remove_pending()
            .map_err(|_| PreferencesFailure::WriteFailed)?;
        Ok(writer)
    }
    fn remove_pending(&self) -> io::Result<()> {
        match openat(&self.directory, PENDING, READ, Mode::empty()) {
            Ok(fd) => {
                validate(&fd)?;
                unlinkat(&self.directory, PENDING, UnlinkatFlags::NoRemoveDir)?;
                Ok(())
            }
            Err(Errno::ENOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
    pub(in crate::user_preferences) fn read(&self) -> Result<Option<Vec<u8>>, PreferencesFailure> {
        read_at(&self.directory).map_err(|_| PreferencesFailure::InvalidSource)
    }
    pub(in crate::user_preferences) fn commit(
        &mut self,
        before: Option<&[u8]>,
        after: &[u8],
    ) -> Result<(), CommitError> {
        let result = (|| {
            let fd = openat(
                &self.directory,
                PENDING,
                WRITE.union(OFlag::O_CREAT).union(OFlag::O_EXCL),
                Mode::from_bits_truncate(0o600),
            )
            .map_err(|_| PreferencesFailure::WriteFailed)?;
            validate(&fd).map_err(|_| PreferencesFailure::WriteFailed)?;
            let mut file = File::from(fd);
            file.write_all(after)
                .and_then(|()| file.sync_all())
                .map_err(|_| PreferencesFailure::WriteFailed)?;
            if self.read()?.as_deref() != before {
                return Err(CommitError::Refused(PreferencesFailure::SourceChanged));
            }
            renameat(&self.directory, PENDING, &self.directory, RECORD_NAME)
                .map_err(|_| PreferencesFailure::WriteFailed)?;
            // A failure after rename cannot claim refusal or invite replay of the committed update.
            let directory = self
                .directory
                .try_clone()
                .map(File::from)
                .map_err(|_| CommitError::OutcomeUnknown)?;
            directory
                .sync_all()
                .map_err(|_| CommitError::OutcomeUnknown)?;
            Ok(())
        })();
        if matches!(result, Err(CommitError::Refused(_))) {
            self.remove_pending()
                .map_err(|_| PreferencesFailure::WriteFailed)?;
        }
        result
    }
}
