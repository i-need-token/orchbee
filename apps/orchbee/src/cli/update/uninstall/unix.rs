//! macOS and Linux effects of uninstall outside the two roots: the command link install's PATH step
//! placed, `~/.local/bin/<stem>`, removed only while it names a version of this store, and on macOS the
//! two lines it added to the person's shell startup file; the
//! processes running from the store, through the shared process scan; the npm carrier; and the
//! machine hub, through its client's ending path. No installer leaves files, entries or a
//! registration here. A running program's files can be deleted here, so the removal never waits
//! for this process to exit.
use super::super::path_step;
pub(super) use super::super::running_programs::{StoreProcess, ancestors, inside, same};
use orchbee_local_runtime::DataRoot;
use orchbee_workbench_client::application::machine_hub;
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Instant,
};

/// Where this environment's entries outside the roots are.
pub(super) struct Locations {
    /// The command link, `~/.local/bin/<stem>`.
    link: PathBuf,
}

/// No platform installer leaves files in the store root here.
pub(super) const INSTALLER_FILES: &[&str] = &[];

/// The store root this build installs into.
pub(super) fn store_root() -> io::Result<PathBuf> {
    super::super::platform::install_root()
}

/// The folders no root may equal or hold: the person's home and the folders of `~/.local`, the
/// temporary folder, and the system's own folders.
pub(super) fn protected_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = [
        "/Applications",
        "/Library",
        "/System",
        "/Users",
        "/bin",
        "/etc",
        "/home",
        "/opt",
        "/private",
        "/sbin",
        "/tmp",
        "/usr",
        "/var",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
    {
        let local = home.join(".local");
        folders.extend([
            local.join("share"),
            local.join("bin"),
            local,
            home.join("Library"),
            home,
        ]);
    }
    folders.push(std::env::temp_dir());
    folders
}

/// This environment's link, from `HOME`.
pub(super) fn locations() -> io::Result<Locations> {
    Ok(Locations {
        link: path_step::own_link().ok_or(io::ErrorKind::NotFound)?,
    })
}

impl Locations {
    /// No registry key holds the entry: it is a link.
    pub(super) fn path_key(&self) -> Option<String> {
        None
    }

    /// The command link.
    pub(super) fn path_location(&self, _root: &Path) -> Option<PathBuf> {
        Some(self.link.clone())
    }

    /// No uninstall registration exists here.
    pub(super) fn registration_key(&self) -> Option<String> {
        None
    }

    pub(super) fn shortcut(&self) -> Option<&Path> {
        None
    }
}

/// Whether the command link names a version of the store `root`, or on macOS a startup file holds
/// the PATH lines install added; a link elsewhere, or anything else at its name, is not install's
/// and stays.
pub(super) fn path_entry_present(locations: &Locations, root: &Path) -> io::Result<bool> {
    let linked = path_step::store_link(&locations.link, root)?;
    #[cfg(target_os = "macos")]
    let linked = linked || path_step::profile_present();
    Ok(linked)
}

/// Removes the command link while it still names a version of the store `root`, and on macOS the
/// PATH lines install added to the person's shell startup file; a link that no longer names this
/// store is left. The folder stays.
pub(super) fn remove_path_entry(locations: &Locations, root: &Path) -> io::Result<()> {
    if path_step::store_link(&locations.link, root)? {
        match fs::remove_file(&locations.link) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    #[cfg(target_os = "macos")]
    path_step::remove_profile()?;
    Ok(())
}

pub(super) fn registration_present(_locations: &Locations) -> io::Result<bool> {
    Ok(false)
}

pub(super) fn remove_registration(_locations: &Locations) -> io::Result<()> {
    Ok(())
}

/// The process id of the machine hub serving `data`; none when none runs or none answers.
pub(super) fn machine_hub(data: &DataRoot, deadline: Instant) -> Option<u32> {
    machine_hub::running(data, deadline).ok().flatten()
}

/// Ends the machine hub serving `data` through the hub client's one ending path; whether a hub
/// ended.
pub(super) fn end_machine_hub(data: &DataRoot, deadline: Instant) -> bool {
    matches!(machine_hub::end(data, deadline), Ok(Some(_)))
}

/// The processes whose program file is inside `root`, other than this one, those in `except` and
/// their descendants. A process this user cannot query is not reported.
pub(super) fn store_processes(root: &Path, except: &[u32]) -> Vec<StoreProcess> {
    let mut except = except.to_vec();
    except.push(std::process::id());
    super::super::running_programs::processes_inside(root, &except).0
}

/// Whether the npm carrier started this Client: its launcher runs the version's Client directly
/// under Node.js.
pub(super) fn npm_carrier(parent: Option<&StoreProcess>) -> bool {
    parent.is_some_and(|parent| parent.image.file_name().is_some_and(|name| name == "node"))
}

/// Never needed: a running program's files are deleted with the rest, so nothing is left for later.
pub(super) fn schedule_removal(
    _files: &[PathBuf],
    _folders: &[PathBuf],
    _marks: &[PathBuf],
    _root: Option<&Path>,
) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}
