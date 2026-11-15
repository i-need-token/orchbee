//! The Windows PATH step: the version's `<stem>-launcher.exe` becomes `<root>\bin\<stem>.exe` when
//! the bytes differ, and the user `Path` names `<root>\bin` by the [`path_value`] edit. A launcher
//! already in place, running or not, is renamed aside before its replacement is renamed in and
//! renamed back when that fails, so the step never ends with `bin` holding no launcher. No
//! environment change is broadcast. The automatic helper runs only the launcher half, and only where
//! the launcher already exists.
use super::super::{Error, PathStep, path_value, platform};
use super::Planned;
use crate::release_store::ReleaseStore;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Places the launcher of the version directory `product` names under `root`, then edits the `Path`
/// value of `HKEY_CURRENT_USER\<key>`, which is the user environment unless a test build names a
/// scratch key. A failure at either part stops the step before the pointer moves.
pub(in crate::cli::update) fn run(root: &Path, product: &str) -> Result<(), Error> {
    let key = platform::user_environment_key().map_err(|_| Error::PathVariable)?;
    let (bin, source, name) = launcher(root, product);
    place_launcher(&bin, &source, &name).map_err(|_| Error::PathLauncher)?;
    let root = root.to_str().ok_or(Error::PathVariable)?;
    platform::edit_user_path(&key, &|current| path_value::edit(current, root))
        .map_err(|_| Error::PathVariable)
}

/// Nothing to do without the PATH step: the launcher in `bin` starts the version the pointer names.
pub(in crate::cli::update) fn refresh_present(_root: &Path, _product: &str) {}

/// What the PATH step would do for the store `root`, from the user `Path` as it is now; none when
/// that value cannot be read. A completed step whose plan is unknown counts as an addition.
pub(in crate::cli::update) fn plan(root: &Path) -> Option<Planned> {
    Some(match platform::planned_path_edit(root)? {
        path_value::Edit::Unchanged => Planned::Keep,
        path_value::Edit::Write { removed: true, .. } => Planned::Replace,
        _ => Planned::Add,
    })
}

/// The folder the step names in the user `Path`, `<root>\bin`, as people read it.
pub(in crate::cli::update) fn location(root: &Path) -> Option<String> {
    root.to_str()
        .map(|root| crate::native_path::display(&path_value::bin(root)))
}

/// The note `install` adds after the step's line: the change is not broadcast, so programs started
/// earlier keep the old environment.
pub(in crate::cli::update) fn follow_up(step: PathStep) -> Option<&'static str> {
    matches!(step, PathStep::Added | PathStep::Replaced).then_some("path-new-terminal")
}

/// Refreshes `<root>\bin\<stem>.exe` from the version directory `product` names, under
/// `install.lock`, when anything exists at that name; a store without it stays without it, and the
/// user `Path` is never read or written. The automatic helper runs this before the completion,
/// which it asks for no PATH step, so a failure leaves the pointer where it was.
///
/// # Errors
/// `busy` while another writer holds `install.lock`, `storage` when the lock cannot be taken, and
/// `activation` when the launcher cannot be placed, as a failed PATH step reports for `update`.
pub(in crate::cli::update) fn refresh_launcher(root: &Path, product: &str) -> Result<(), Error> {
    let _store = ReleaseStore::acquire(root).map_err(|error| {
        if error.kind() == io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            Error::Storage
        }
    })?;
    let (bin, source, name) = launcher(root, product);
    match fs::symlink_metadata(bin.join(&name)) {
        Ok(_) => place_launcher(&bin, &source, &name).map_err(|_| Error::Activation),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Error::Activation),
    }
}

/// The store's `bin` folder, the version's launcher, and the name it takes in `bin`.
fn launcher(root: &Path, product: &str) -> (PathBuf, PathBuf, String) {
    let stem = orchbee_local_runtime::environment_stem();
    let source = root
        .join("versions")
        .join(product)
        .join(format!("{stem}-launcher.exe"));
    (root.join("bin"), source, format!("{stem}.exe"))
}

/// Makes `bin\<name>` hold the bytes of `source`, leaving an equal file untouched.
fn place_launcher(bin: &Path, source: &Path, name: &str) -> io::Result<()> {
    let replacement = fs::read(source)?;
    match fs::symlink_metadata(bin) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(bin)?,
        Err(error) => return Err(error),
    }
    remove_leftovers(bin, name);
    let target = bin.join(name);
    match fs::read(&target) {
        Ok(current) if current == replacement => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let nonce = format!(
        "{:x}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    );
    let staged = bin.join(format!("{name}.new-{nonce}"));
    if let Err(error) = write_new(&staged, &replacement) {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    // Windows renames a running executable but neither deletes nor overwrites it.
    let aside = bin.join(format!("{name}.old-{nonce}"));
    let moved_aside = match fs::rename(&target, &aside) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            let _ = fs::remove_file(&staged);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&staged, &target) {
        if moved_aside {
            let _ = fs::rename(&aside, &target);
        }
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    if moved_aside {
        // A launcher still running keeps its file until a later step removes it.
        let _ = fs::remove_file(&aside);
    }
    Ok(())
}

/// Removes what an interrupted or still-running earlier step left beside `bin\<name>`.
fn remove_leftovers(bin: &Path, name: &str) {
    let Ok(entries) = fs::read_dir(bin) else {
        return;
    };
    let (new, old) = (format!("{name}.new-"), format!("{name}.old-"));
    for entry in entries.flatten() {
        let file = entry.file_name();
        if file
            .to_str()
            .is_some_and(|file| file.starts_with(&new) || file.starts_with(&old))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
