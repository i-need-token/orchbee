//! Windows effects inside the data root: private folders created on first write, the layout
//! record, and the default runtime base, `internal/runtime`, which every environment creates before
//! admitting it.
use super::DataRoot;
use crate::private_filesystem as permissions;
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Creates this environment's default runtime base as a private directory below its data root, so
/// activation can admit it like any existing base.
pub fn prepare_default_runtime_base(env: impl Fn(&str) -> Option<OsString>) -> io::Result<()> {
    let base = DataRoot::resolve(env)
        .map_err(|_| io::Error::from(io::ErrorKind::NotFound))?
        .runtime();
    create_state_directory(&base).map(drop)
}

/// Creates a directory inside this environment's data root as private, like every storage owner,
/// for a writer in another crate that then places its own files there. The data root and
/// `internal/version.json` are created as [`create_state_directory`] creates them.
pub fn create_private_directory(path: &Path) -> io::Result<Vec<File>> {
    create_state_directory(path)
}

/// Creates `path` and every missing directory above it as private directories and pins them all.
/// The data root, whose parent may be a home directory that other ACL entries share, is created
/// below that parent without admitting it; any other existing ancestor must already be private. A
/// directory inside the data root also gets `internal/version.json`, so a root holding anything
/// Orchbee wrote records its layout.
pub(crate) fn create_state_directory(path: &Path) -> io::Result<Vec<File>> {
    let data_root = DataRoot::resolve(|name: &str| std::env::var_os(name)).ok();
    let roots: Vec<PathBuf> = data_root
        .iter()
        .map(|root| root.path().to_owned())
        .collect();
    let pins = create_below_roots(path, &roots)?;
    if let Some(root) = data_root.filter(|root| path.starts_with(root.path())) {
        write_version_record(&root, &roots)?;
    }
    Ok(pins)
}

/// A state root whose parent is missing is created like any other directory.
fn create_below_roots(path: &Path, roots: &[PathBuf]) -> io::Result<Vec<File>> {
    if roots.iter().any(|root| root == path)
        && path
            .parent()
            .is_some_and(|parent| fs::symlink_metadata(parent).is_ok())
    {
        return create_state_root(path);
    }
    match permissions::open_tree(path) {
        Ok(pins) => Ok(pins),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|parent| *parent != path)
                .ok_or(io::ErrorKind::InvalidInput)?;
            let mut pins = create_below_roots(parent, roots)?;
            pins.push(private_directory(path)?);
            Ok(pins)
        }
        Err(error) => Err(error),
    }
}

/// Cross-runtime contract: search for `data-root-version-record` before changing this shape;
/// Desktop Main writes the same record when it creates the root.
const LAYOUT: u32 = 1;

/// Writes `internal/version.json` when it is absent: the layout and the version that created it.
/// An existing record is left as it is.
fn write_version_record(root: &DataRoot, roots: &[PathBuf]) -> io::Result<()> {
    let path = root.version_file();
    let _pins = create_below_roots(&root.internal(), roots)?;
    let mut file = match permissions::open_file(&path, true, true) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error),
    };
    let record = serde_json::json!({
        "layout": LAYOUT,
        "newestVersion": env!("CARGO_PKG_VERSION"),
    });
    let mut bytes = serde_json::to_vec(&record)?;
    bytes.push(b'\n');
    file.write_all(&bytes).and_then(|()| file.sync_all())
}

fn private_directory(path: &Path) -> io::Result<File> {
    let created = match fs::create_dir(path) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let mut directory = permissions::open_directory(path, created)?;
    if created {
        permissions::protect_new(&mut directory)?;
    } else {
        permissions::require_private(&directory)?;
    }
    Ok(directory)
}

/// Pins a state root and its ancestors and admits the root as a private directory. Only the root
/// must be private; its parent may be a home directory that other ACL entries share. A missing
/// root, or one whose data root is missing, is absent, never created.
pub(crate) fn open_state_root(root: &Path) -> io::Result<Option<Vec<File>>> {
    let mut pins = match permissions::open_ancestors(parent(root)?) {
        Ok(pins) => pins,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let directory = match permissions::open_directory(root, false) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    permissions::require_private(&directory)?;
    pins.push(directory);
    Ok(Some(pins))
}

/// Creates a missing state root as private, or admits an existing one, below an existing parent.
fn create_state_root(root: &Path) -> io::Result<Vec<File>> {
    let mut pins = permissions::open_ancestors(parent(root)?)?;
    pins.push(private_directory(root)?);
    Ok(pins)
}

fn parent(root: &Path) -> io::Result<&Path> {
    permissions::require_exact_absolute(root)?;
    root.parent()
        .ok_or_else(|| io::ErrorKind::InvalidInput.into())
}
