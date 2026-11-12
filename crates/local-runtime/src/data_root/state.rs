//! Private folders created on first write and the layout record, `internal/version.json`, with its
//! refusal and newest version. The platform adapter creates and admits the folders; this module
//! decides when the record is read, raised and written.
use super::{
    DataRoot, PRODUCT_VERSION,
    platform::create_below_roots,
    version_record::{VersionRecord, read_record},
};
use crate::private_filesystem as permissions;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Creates `path` and every missing directory above it as private directories and pins them all.
/// A directory inside the data root also gets `internal/version.json`, so a root holding anything
/// Orchbee wrote records its layout; a root whose layout this build cannot read is refused before
/// anything in it is created.
pub(crate) fn create_state_directory(path: &Path) -> io::Result<Vec<File>> {
    let data_root = DataRoot::resolve(|name: &str| std::env::var_os(name)).ok();
    let roots: Vec<PathBuf> = data_root
        .iter()
        .map(|root| root.path().to_owned())
        .collect();
    let inside = data_root.filter(|root| path.starts_with(root.path()));
    if let Some(root) = &inside {
        settle_version_record(root)?;
    }
    let pins = create_below_roots(path, &roots)?;
    if let Some(root) = &inside {
        write_version_record(root, &roots)?;
    }
    Ok(pins)
}

/// The roots whose record this process has already settled, so it reads the record once.
static SETTLED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Refuses to write into a root whose layout this build cannot read, and raises the record's
/// `newestVersion` to this build's version when it names an older one, before anything else in
/// the root is created. A missing record is written by [`write_version_record`] once the folders
/// exist. The record is replaced only under its writer lock, `internal/version.lock`.
pub(crate) fn settle_version_record(root: &DataRoot) -> io::Result<()> {
    if SETTLED
        .lock()
        .is_ok_and(|settled| settled.iter().any(|path| path == root.path()))
    {
        return Ok(());
    }
    let path = root.version_file();
    let Some(record) = read_settled(&path)? else {
        return Ok(());
    };
    if record.raised(PRODUCT_VERSION).is_some() {
        let _lock = version_lock(root)?;
        if let Some(bytes) = read_settled(&path)?.and_then(|record| record.raised(PRODUCT_VERSION))
        {
            publish_record(root, &path, &bytes, permissions::replace)?;
        }
    }
    mark_settled(root);
    Ok(())
}

/// Takes the record's writer lock, `internal/version.lock`, waiting for another writer.
fn version_lock(root: &DataRoot) -> io::Result<File> {
    let lock_path = root.internal().join("version.lock");
    let lock = match permissions::open_file(&lock_path, true, true) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            permissions::open_file(&lock_path, false, true)?
        }
        Err(error) => return Err(error),
    };
    lock.lock()?;
    Ok(lock)
}

/// The record at `path`, refused when this build cannot read its layout.
fn read_settled(path: &Path) -> io::Result<Option<VersionRecord>> {
    let refused = || io::Error::new(io::ErrorKind::Unsupported, "data root layout refused");
    let Some(bytes) = read_record(path)? else {
        return Ok(None);
    };
    let record = VersionRecord::parse(&bytes).ok_or_else(refused)?;
    if record.refusal().is_some() {
        return Err(refused());
    }
    Ok(Some(record))
}

/// Writes `bytes` beside the record as `internal/version.pending` and moves them to `target` with
/// `publish`, so a reader sees the record whole or not at all. The caller holds the writer lock.
fn publish_record(
    root: &DataRoot,
    target: &Path,
    bytes: &[u8],
    publish: fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let pending = &root.internal().join("version.pending");
    match permissions::open_file(pending, false, false) {
        Ok(file) => {
            drop(file);
            fs::remove_file(pending)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = permissions::open_file(pending, true, true)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let replaced = written.and_then(|()| publish(pending, target));
    if replaced.is_err() {
        let _ = fs::remove_file(pending);
    }
    replaced
}

fn mark_settled(root: &DataRoot) {
    if let Ok(mut settled) = SETTLED.lock() {
        settled.push(root.path().to_owned());
    }
}

/// Writes `internal/version.json` when it is absent: the layout and the version that created it.
/// Processes that start together on a fresh root race to write it, and every one of them, and every
/// reader, must find it whole: the first writer under the writer lock moves a complete record into
/// place, never over a record another writer (Desktop Main among them) already placed, and every
/// other writer settles the record it finds. An existing record was settled before any folder was
/// created.
fn write_version_record(root: &DataRoot, roots: &[PathBuf]) -> io::Result<()> {
    let path = root.version_file();
    let _pins = create_below_roots(&root.internal(), roots)?;
    if read_record(&path)?.is_none() {
        let _lock = version_lock(root)?;
        if read_record(&path)?.is_none() {
            match publish_record(
                root,
                &path,
                &VersionRecord::initial(),
                permissions::move_aside,
            ) {
                Ok(()) => {
                    mark_settled(root);
                    return Ok(());
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
    settle_version_record(root)
}
