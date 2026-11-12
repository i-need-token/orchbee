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
        let lock_path = root.internal().join("version.lock");
        let lock = match permissions::open_file(&lock_path, true, true) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                permissions::open_file(&lock_path, false, true)?
            }
            Err(error) => return Err(error),
        };
        lock.lock()?;
        if let Some(bytes) = read_settled(&path)?.and_then(|record| record.raised(PRODUCT_VERSION))
        {
            replace_record(&root.internal().join("version.pending"), &path, &bytes)?;
        }
    }
    mark_settled(root);
    Ok(())
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

fn replace_record(pending: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
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
    let replaced = written.and_then(|()| permissions::replace(pending, target));
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
/// An existing record was settled before any folder was created.
fn write_version_record(root: &DataRoot, roots: &[PathBuf]) -> io::Result<()> {
    let path = root.version_file();
    let _pins = create_below_roots(&root.internal(), roots)?;
    let mut file = match permissions::open_file(&path, true, true) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return settle_version_record(root);
        }
        Err(error) => return Err(error),
    };
    file.write_all(&VersionRecord::initial())
        .and_then(|()| file.sync_all())?;
    mark_settled(root);
    Ok(())
}
