//! The private download folder `<store root>\.download\` and the one archive an update fetches
//! into it: the folder's lock, the bounded transfer, the size and SHA-256 check against the signed
//! feed, and unpacking. Nothing from the archive is unpacked before that check passes: unpacking
//! takes a [`Verified`] archive, which only [`Download::fetch`] returns, and only after the check.
//!
//! The folder holds `download.lock`, `archive.zip` and `unpacked\`. Whoever holds the lock removes
//! the archive and the unpacked tree when it takes the lock and again when it finishes, so a run
//! that crashed leaves at most one of each for the next run to remove. The folder and its lock file
//! stay, like `install.lock`. The lock is always taken before `install.lock`, and `install` never
//! takes it.
use super::{Context, Error, Report, acquire::Source, feed::Archive, platform};
use orchbee_local_runtime::{CommandFailure, PrivateCache, private_filesystem as permissions};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};

/// The download folder's name in the store root.
pub(super) const FOLDER: &str = ".download";
/// The record name of the folder's lock, `download.lock`.
const LOCK: &str = "download";
const ARCHIVE: &str = "archive.zip";
const UNPACKED: &str = "unpacked";
/// The only arguments `tar.exe` receives: fixed ASCII words naming the archive beside its working
/// directory, `unpacked\`, so no path of the person's computer is an argument. The archive's one
/// top-level folder is the signed release directory, so its files land directly in `unpacked\`.
const UNPACK_ARGUMENTS: [&str; 5] = ["-x", "-f", r"..\archive.zip", "--strip-components", "1"];

/// Unpacks the verified `..\archive.zip` into the empty private folder it receives, which is also
/// its working directory.
pub(super) type Unpack<'a> = &'a dyn Fn(&Path, &mut Context) -> Result<(), Error>;

/// The held download folder of one update.
pub(super) struct Download {
    folder: PathBuf,
    /// A child process that wrote into the folder may still be running, so nothing in it is
    /// removed.
    unsettled: bool,
    _lock: PrivateCache,
}

/// The fetched archive after its length and SHA-256 matched the signed feed, held open so that no
/// other writer can change it before it is unpacked.
pub(super) struct Verified {
    _held: File,
}

impl Download {
    /// Takes `download.lock`, creating the folder with the store's private rule, then removes
    /// whatever an earlier run left. A held lock is another update: `busy`.
    ///
    /// # Errors
    /// `busy`, or `storage` when the folder is refused or its leftovers cannot be removed.
    pub fn acquire(root: &Path, context: &mut Context, report: &mut Report) -> Result<Self, Error> {
        context.check()?;
        let folder = root.join(FOLDER);
        let lock = PrivateCache::acquire(&folder, LOCK).map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                Error::Busy
            } else {
                context.storage(&folder, &error)
            }
        })?;
        let download = Self {
            folder,
            unsettled: false,
            _lock: lock,
        };
        if let Err(error) = download.clean() {
            report.cleanup_pending = Some(download.folder.clone());
            return Err(context.storage(&download.folder, &error));
        }
        Ok(download)
    }

    /// Downloads the archive from `source` into the folder, bounded by its signed size, then
    /// requires its length and SHA-256 to equal the signed ones.
    ///
    /// # Errors
    /// `checksum` when the bytes differ from the signed archive; the transfer's own failure
    /// otherwise.
    pub fn fetch(
        &mut self,
        source: &Source,
        archive: &Archive,
        context: &mut Context,
    ) -> Result<Verified, Error> {
        let path = self.folder.join(ARCHIVE);
        let mut file = permissions::open_file(&path, true, true)
            .map_err(|error| context.storage(&path, &error))?;
        let copied = source.copy(&path, &mut file, archive.size, context);
        self.unsettled |= matches!(copied, Err(Error::PhysicalUnknown));
        copied?;
        drop(file);
        // Reopened without sharing write access, so the bytes hashed are the bytes unpacked.
        let mut file = permissions::open_snapshot_file(&path)
            .map_err(|error| context.storage(&path, &error))?;
        if file.metadata().map_err(|_| Error::Checksum)?.len() != archive.size {
            return Err(Error::Checksum);
        }
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            context.check()?;
            let count = file.read(&mut buffer).map_err(|_| Error::Checksum)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != archive.sha256 {
            return Err(Error::Checksum);
        }
        Ok(Verified { _held: file })
    }

    /// Creates the private folder `unpacked\` and runs `unpack` there on the verified archive.
    /// Returns the unpacked release directory.
    ///
    /// # Errors
    /// `storage` when the folder cannot be created or the unpack failed; `download-unavailable`
    /// without `tar.exe`; cancellation and the deadline as themselves.
    pub fn unpack(
        &mut self,
        archive: Verified,
        unpack: Unpack<'_>,
        context: &mut Context,
    ) -> Result<PathBuf, Error> {
        context.check()?;
        let path = self.folder.join(UNPACKED);
        fs::create_dir(&path).map_err(|error| context.storage(&path, &error))?;
        let pin = permissions::open_directory(&path, false)
            .and_then(|folder| {
                permissions::require_private(&folder)
                    .map_err(|error| permissions::naming(error, &path))?;
                Ok(folder)
            })
            .map_err(|error| context.storage(&path, &error))?;
        let unpacked = unpack(&path, context);
        self.unsettled |= matches!(unpacked, Err(Error::PhysicalUnknown));
        drop((pin, archive));
        unpacked.map(|()| path)
    }

    /// Ends the run: removes the archive and the unpacked tree and releases the lock. When a child
    /// that wrote here may still run, or removal fails, nothing more is removed and
    /// `cleanupPending` names the folder, unless it already names a stage.
    pub fn finish(self, report: &mut Report) {
        if self.unsettled || self.clean().is_err() {
            report.cleanup_pending.get_or_insert(self.folder);
        }
    }

    /// Removes `archive.zip` and `unpacked\` from the held folder, following no link: a link in
    /// their place is removed as a link.
    fn clean(&self) -> io::Result<()> {
        let absent = |removed: io::Result<()>| match removed {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
        absent(fs::remove_file(self.folder.join(ARCHIVE)))?;
        absent(fs::remove_dir_all(self.folder.join(UNPACKED)))
    }
}

/// The system `tar.exe` command that unpacks `..\archive.zip` into `unpacked`, its working
/// directory: an absolute program path, a cleared environment and fixed arguments.
pub(super) fn unpack_command(unpacked: &Path) -> io::Result<Command> {
    let mut command = platform::tar_command()?;
    command.current_dir(unpacked).args(UNPACK_ARGUMENTS);
    Ok(command)
}

/// Unpacks with the system `tar.exe`, owned by the invocation's Job under its deadline and Ctrl+C.
/// Nothing is executed from the unpacked folder afterwards; the install reads only the files the
/// signed manifest lists.
///
/// # Errors
/// `download-unavailable` without `tar.exe`, `storage` when it fails, and cancellation, the
/// deadline and an unknown end as themselves.
pub(super) fn system_unpack(unpacked: &Path, context: &mut Context) -> Result<(), Error> {
    context.check()?;
    let command = unpack_command(unpacked).map_err(|_| Error::DownloadUnavailable)?;
    let result = platform::execute_contained(command, context)
        .map_err(|error| context.storage(unpacked, &error))?;
    match result.failure {
        None => Ok(()),
        Some(CommandFailure::Cancelled) => Err(Error::Cancelled),
        Some(CommandFailure::Timeout) => Err(Error::Timeout),
        Some(CommandFailure::PhysicalUnknown) => Err(Error::PhysicalUnknown),
        Some(CommandFailure::SpawnUnavailable) => Err(Error::DownloadUnavailable),
        Some(_) => Err(context.storage(unpacked, &io::Error::other("tar.exe failed"))),
    }
}
