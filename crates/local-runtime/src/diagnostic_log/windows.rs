//! Windows daily files: private, regular, single-link files in the data root's private `logs`
//! folder. An append holds the day's file with read sharing only, so concurrent writers, Desktop
//! Main's exclusive open included, admit one record at a time and wait up to half a second for each
//! other.
use crate::private_filesystem as permissions;
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    thread,
    time::{Duration, Instant},
};

const SHARING_VIOLATION: i32 = 32;
const LOCK_VIOLATION: i32 = 33;
const REPARSE_POINT: u32 = 0x400;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const SHARE_READ: u32 = 1;
const SHARE_READ_WRITE: u32 = 3;
/// How long an append or read waits for another writer before it gives up.
const SHARING_WAIT: Duration = Duration::from_millis(500);

/// Opens or creates the day's file and holds it against every other writer until it closes.
pub(super) fn open_daily(path: &Path) -> io::Result<File> {
    with_sharing_wait(|| {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .share_mode(SHARE_READ)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(path)?;
        require_regular_private(&file)?;
        Ok(file)
    })
}

/// Opens an existing daily file to read, beside any writer, or `None` when it is absent.
pub(super) fn open_existing(path: &Path) -> io::Result<Option<File>> {
    let file = with_sharing_wait(|| {
        OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(OPEN_REPARSE_POINT)
            .open(path)
    });
    let file = match file {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    require_regular_private(&file)?;
    Ok(Some(file))
}

fn require_regular_private(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_attributes() & REPARSE_POINT != 0
        || winapi_util::file::information(file)?.number_of_links() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe daily log file",
        ));
    }
    permissions::require_private(file)
}

fn with_sharing_wait(mut open: impl FnMut() -> io::Result<File>) -> io::Result<File> {
    let deadline = Instant::now() + SHARING_WAIT;
    loop {
        match open() {
            Err(error)
                if matches!(
                    error.raw_os_error(),
                    Some(SHARING_VIOLATION | LOCK_VIOLATION)
                ) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result,
        }
    }
}
