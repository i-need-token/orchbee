//! The Windows PATH step a completing Client runs under `install.lock`, before it moves the pointer:
//! the version's `<stem>-launcher.exe` becomes `<root>\bin\<stem>.exe` when the bytes differ, and the
//! user `Path` names `<root>\bin` by the [`path_value`] edit. A launcher already in place, running or
//! not, is renamed aside before its replacement is renamed in and renamed back when that fails, so
//! the step never ends with `bin` holding no launcher. No environment change is broadcast.
use super::{Error, path_value, platform};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// The key under `HKEY_CURRENT_USER` that holds the user `Path`.
pub(super) const USER_ENVIRONMENT: &str = "Environment";

/// Places the launcher of the version directory `product` names under `root`, then edits the `Path`
/// value of `HKEY_CURRENT_USER\<key>`, which is [`USER_ENVIRONMENT`] outside a rehearsal. A failure
/// at either part stops the step before the pointer moves.
pub(super) fn run(root: &Path, product: &str, key: &str) -> Result<(), Error> {
    let stem = orchbee_local_runtime::environment_stem();
    let source = root
        .join("versions")
        .join(product)
        .join(format!("{stem}-launcher.exe"));
    place_launcher(&root.join("bin"), &source, &format!("{stem}.exe"))
        .map_err(|_| Error::PathLauncher)?;
    let root = root.to_str().ok_or(Error::PathVariable)?;
    platform::edit_user_path(key, &|current| path_value::edit(current, root))
        .map_err(|_| Error::PathVariable)
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
