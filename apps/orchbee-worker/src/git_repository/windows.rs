//! Windows repository paths: absolute drive paths only, compared without case or separator style.
use orchbee_capability_node_protocol::AbsoluteCwd;
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

pub(crate) const GIT_PROGRAM: &str = "git.exe";

/// The folders a Git for Windows installation keeps its Git in, one per build architecture.
const INSTALLATION_GIT_FOLDERS: [&str; 3] = ["mingw64", "clangarm64", "mingw32"];

/// The Git a read that starts no helper program runs. When the first `git.exe` process creation
/// would find is the Git for Windows launcher, `<installation>\cmd\git.exe` or
/// `<installation>\bin\git.exe`, which only sets the search path and variables helper programs use
/// and starts `<installation>\<architecture>\bin\git.exe`, the read starts that Git itself and
/// spares one process start. Any other first match, and any doubt, keeps the name, which process
/// creation resolves exactly as before.
pub(crate) fn read_program() -> OsString {
    searched_folders()
        .and_then(launcher_target)
        .map_or_else(|| GIT_PROGRAM.into(), PathBuf::into_os_string)
}

/// The folders process creation searches for a bare program name, in its order, when the command
/// leaves `PATH` as the Worker has it: the Worker's own folder, the system folder, the Windows
/// folder that holds it, then the Worker's `PATH`.
fn searched_folders() -> Option<Vec<PathBuf>> {
    let application = std::env::current_exe().ok()?.parent()?.to_owned();
    let system = PathBuf::from(winsafe::GetSystemDirectory().ok()?);
    let windows = system.parent()?.to_owned();
    let search_path = std::env::var_os("PATH")?;
    let listed =
        std::env::split_paths(&search_path).filter(|folder| !folder.as_os_str().is_empty());
    Some(
        [application, system, windows]
            .into_iter()
            .chain(listed)
            .collect(),
    )
}

fn launcher_target(folders: Vec<PathBuf>) -> Option<PathBuf> {
    let launcher = first_holding_git(folders)?;
    let folder = launcher.parent()?;
    let folder_name = folder.file_name()?.to_str()?;
    if !launcher.is_absolute()
        || !launcher.is_file()
        || !(folder_name.eq_ignore_ascii_case("cmd") || folder_name.eq_ignore_ascii_case("bin"))
    {
        return None;
    }
    let installation = folder.parent()?;
    let mut targets = INSTALLATION_GIT_FOLDERS
        .into_iter()
        .map(|architecture| {
            installation
                .join(architecture)
                .join("bin")
                .join(GIT_PROGRAM)
        })
        .filter(|target| target.is_file());
    let target = targets.next()?;
    targets.next().is_none().then_some(target)
}

/// The match process creation takes: the first folder holding anything named `git.exe`. A folder
/// that cannot be inspected leaves the answer in doubt.
fn first_holding_git(folders: Vec<PathBuf>) -> Option<PathBuf> {
    for folder in folders {
        let candidate = folder.join(GIT_PROGRAM);
        match candidate.symlink_metadata() {
            Ok(_) => return Some(candidate),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
    }
    None
}

pub(crate) fn is_admissible_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}

pub(crate) fn canonical_path(path: &Path) -> Result<AbsoluteCwd, ()> {
    // Filesystem access belongs to the Git job owner, never serialized ingress.
    if !is_admissible_path(path) {
        return Err(());
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| ())?;
    let value = canonical.to_str().ok_or(())?;
    if value.starts_with("\\\\?\\UNC\\") {
        return Err(());
    }
    let normalized = value
        .strip_prefix("\\\\?\\")
        .unwrap_or(value)
        .replace('\\', "/");
    AbsoluteCwd::new(&normalized).map_err(|_| ())
}

/// The spelling under which a checkout path Git recorded names the same checkout.
pub(crate) fn checkout_spelling(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests;
