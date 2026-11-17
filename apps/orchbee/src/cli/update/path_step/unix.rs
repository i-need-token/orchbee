//! The macOS and Linux PATH step: the link `~/.local/bin/<stem>` names the Client of the version
//! directory, `<root>/versions/<version>/<stem>`, so a shell that searches `~/.local/bin` finds the
//! installed version by name. When the installing process's `PATH` lacks that folder, macOS also
//! puts it on new shells' `PATH` through the person's shell startup file (`profile`), and `install`
//! prints the line that adds it wherever that did not happen. The link is the step's only entry in
//! `~/.local/bin`: something else at its name, a file or a link that names no Orchbee version's
//! Client, is never replaced, and the step fails and leaves it. A link to another store's version is
//! Orchbee's and is replaced. Each placement renames a new link over the old one, so the name never
//! stands empty.
#[cfg(target_os = "macos")]
mod profile;

use super::super::{Error, PathStep};
use super::Planned;
use crate::release_store::{ReleaseStore, version_directory_parts};
use orchbee_local_runtime::{CohortArtifact, environment_stem};
use std::{
    ffi::OsStr,
    fs, io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Links the Client of the version directory `product` names under `root` at `~/.local/bin/<stem>`,
/// creating the folder when it is missing, then on macOS reaches it from new shells. A failure to
/// link stops the step before the pointer moves.
pub(in crate::cli::update) fn run(root: &Path, product: &str) -> Result<(), Error> {
    let link = own_link().ok_or(Error::PathLauncher)?;
    place(&link, &client(root, product), root).map_err(|_| Error::PathLauncher)?;
    #[cfg(target_os = "macos")]
    reach(&link);
    Ok(())
}

/// On macOS, puts `~/.local/bin` on new shells' `PATH` through the startup file of the shell
/// `SHELL` names when this process's `PATH` lacks it; best effort, as `profile` says.
#[cfg(target_os = "macos")]
fn reach(link: &Path) {
    if let Some(folder) = link.parent()
        && !names_folder(std::env::var_os("PATH").as_deref(), folder)
        && let Some(home) = folder.parent().and_then(Path::parent)
    {
        let _added = profile::add(home, std::env::var_os("SHELL").as_deref());
    }
}

/// Without the PATH step, an existing link to this store follows the version that becomes active,
/// so the command a shell finds stays the installed one; nothing is created. Best effort: the link
/// keeps its version when it cannot be renewed, and the install goes on.
pub(in crate::cli::update) fn refresh_present(root: &Path, product: &str) {
    if let Some(link) = own_link()
        && matches!(occupant(&link, root), Ok(Occupant::Store))
    {
        let _kept = place(&link, &client(root, product), root);
    }
}

/// Renews an existing link to this store for `product` under `install.lock`, before the automatic
/// helper's completion, which runs without the PATH step; a store without the link stays without
/// it.
///
/// # Errors
/// `busy` while another writer holds `install.lock`, `storage` when the lock cannot be taken, and
/// `activation` when the link cannot be renewed, as a failed PATH step reports for `update`.
pub(in crate::cli::update) fn refresh_launcher(root: &Path, product: &str) -> Result<(), Error> {
    let _store = ReleaseStore::acquire(root).map_err(|error| {
        if error.kind() == io::ErrorKind::WouldBlock {
            Error::Busy
        } else {
            Error::Storage
        }
    })?;
    let Some(link) = own_link() else {
        return Ok(());
    };
    match occupant(&link, root) {
        Ok(Occupant::Store) => place(&link, &client(root, product), root)
            .map(drop)
            .map_err(|_| Error::Activation),
        Ok(_) => Ok(()),
        Err(_) => Err(Error::Activation),
    }
}

/// What the step would do for the store `root`, read without writing; none when something else
/// holds the link's name, where the step will fail.
pub(in crate::cli::update) fn plan(root: &Path) -> Option<Planned> {
    match occupant(&own_link()?, root).ok()? {
        Occupant::Absent => Some(Planned::Add),
        Occupant::Store => Some(Planned::Keep),
        Occupant::OtherStore => Some(Planned::Replace),
        Occupant::Foreign => None,
    }
}

/// The link the step places, as people read it.
pub(in crate::cli::update) fn location(_root: &Path) -> Option<String> {
    own_link().map(|link| crate::native_path::display(&link.to_string_lossy()))
}

/// The note `install` adds after the step's line when this process's `PATH` lacks `~/.local/bin`:
/// on macOS, that a startup file now puts it there; otherwise the line that does.
pub(in crate::cli::update) fn follow_up(step: PathStep) -> Option<&'static str> {
    let linked = matches!(
        step,
        PathStep::Added | PathStep::Present | PathStep::Replaced
    );
    let folder = own_link()?.parent()?.to_owned();
    if !linked || names_folder(std::env::var_os("PATH").as_deref(), &folder) {
        return None;
    }
    #[cfg(target_os = "macos")]
    let reached = folder
        .parent()
        .and_then(Path::parent)
        .is_some_and(|home| !profile::holders(home).is_empty());
    #[cfg(not(target_os = "macos"))]
    let reached = false;
    Some(if reached {
        "path-profile"
    } else {
        "path-not-on-path"
    })
}

/// The link the step places for this person, from `HOME`.
pub(in crate::cli::update) fn own_link() -> Option<PathBuf> {
    link_path(std::env::var_os("HOME").as_deref())
}

/// `<home>/.local/bin/<stem>`; none when `home` is unset or not an absolute path.
fn link_path(home: Option<&OsStr>) -> Option<PathBuf> {
    let home = Path::new(home?);
    home.is_absolute()
        .then(|| home.join(".local").join("bin").join(environment_stem()))
}

/// The Client of the version directory `product` names under `root`.
fn client(root: &Path, product: &str) -> PathBuf {
    root.join("versions")
        .join(product)
        .join(CohortArtifact::Client.file_name())
}

/// What holds the link's name.
#[derive(Debug, Eq, PartialEq)]
enum Occupant {
    Absent,
    /// A link to a version's Client in this store.
    Store,
    /// A link to a version's Client in another store.
    OtherStore,
    /// A file, a folder, or a link that names no version's Client.
    Foreign,
}

fn occupant(link: &Path, root: &Path) -> io::Result<Occupant> {
    let metadata = match fs::symlink_metadata(link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Occupant::Absent),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_symlink() {
        return Ok(Occupant::Foreign);
    }
    let target = fs::read_link(link)?;
    let client = CohortArtifact::Client.file_name();
    Ok(
        match target
            .parent()
            .filter(|_| target.is_absolute() && target.file_name() == Some(client.as_ref()))
            .and_then(version_directory_parts)
        {
            Some((store, _)) if store == root => Occupant::Store,
            Some(_) => Occupant::OtherStore,
            None => Occupant::Foreign,
        },
    )
}

/// Makes `link` name `target`, a version's Client in the store `root`, and says what that changed.
/// A link that already names `target` is left as it is.
fn place(link: &Path, target: &Path, root: &Path) -> io::Result<Planned> {
    let folder = link.parent().ok_or(io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(folder)?;
    let planned = match occupant(link, root)? {
        Occupant::Absent => Planned::Add,
        Occupant::Store if fs::read_link(link)? == target => return Ok(Planned::Keep),
        Occupant::Store => Planned::Keep,
        Occupant::OtherStore => Planned::Replace,
        Occupant::Foreign => return Err(io::ErrorKind::AlreadyExists.into()),
    };
    let name = link.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let staged = folder.join(format!(
        ".{}.new-{:x}-{:x}",
        name.to_string_lossy(),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    ));
    std::os::unix::fs::symlink(target, &staged)?;
    if let Err(error) = fs::rename(&staged, link) {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    Ok(planned)
}

/// Whether `path` names `folder` as one of its entries, ignoring trailing separators.
fn names_folder(path: Option<&OsStr>, folder: &Path) -> bool {
    let trim = |entry: &[u8]| -> Vec<u8> {
        let mut entry = entry.to_vec();
        while entry.len() > 1 && entry.last() == Some(&b'/') {
            entry.pop();
        }
        entry
    };
    let wanted = trim(folder.as_os_str().as_bytes());
    path.is_some_and(|path| {
        path.as_bytes()
            .split(|byte| *byte == b':')
            .any(|entry| trim(entry) == wanted)
    })
}

/// Whether the link at `link` names a version's Client in the store `root`: the entry install's
/// PATH step placed, which uninstall removes.
pub(in crate::cli::update) fn store_link(link: &Path, root: &Path) -> io::Result<bool> {
    Ok(occupant(link, root)? == Occupant::Store)
}

/// Whether a startup file holds the two lines the step added to put `~/.local/bin` on `PATH`.
#[cfg(target_os = "macos")]
pub(in crate::cli::update) fn profile_present() -> bool {
    home().is_some_and(|home| !profile::holders(&home).is_empty())
}

/// Removes those two lines from every startup file that holds them.
#[cfg(target_os = "macos")]
pub(in crate::cli::update) fn remove_profile() -> io::Result<()> {
    home().map_or(Ok(()), |home| profile::remove(&home))
}

/// The person's home, from `HOME`, when it is an absolute path.
#[cfg(target_os = "macos")]
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
}

#[cfg(test)]
mod tests;
