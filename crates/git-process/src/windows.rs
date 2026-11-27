//! Windows Git program and paths: the Git for Windows launcher's own Git for reads, and absolute
//! drive paths only, compared without case or separator style.
use std::{
    borrow::Cow,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

/// The name process creation resolves to the first `git.exe` it finds.
pub const GIT_PROGRAM: &str = "git.exe";

/// The folders a Git for Windows installation keeps its Git in, one per build architecture.
const INSTALLATION_GIT_FOLDERS: [&str; 3] = ["mingw64", "clangarm64", "mingw32"];

/// The Git a read that starts no helper program runs. When the first `git.exe` process creation
/// would find is the Git for Windows launcher, `<installation>\cmd\git.exe` or
/// `<installation>\bin\git.exe`, which only sets the search path and variables helper programs use
/// and starts `<installation>\<architecture>\bin\git.exe`, the read starts that Git itself and
/// spares one process start. Any other first match, and any doubt, keeps the name, which process
/// creation resolves exactly as before.
pub fn read_program() -> OsString {
    searched_folders()
        .and_then(launcher_target)
        .map_or_else(|| GIT_PROGRAM.into(), PathBuf::into_os_string)
}

/// The folders process creation searches for a bare program name, in its order, when the command
/// leaves `PATH` as the running process has it: the process's own folder, the system folder, the
/// Windows folder that holds it, then the process's `PATH`.
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

/// Whether `path` is one a Git owner may resolve: an absolute drive path, verbatim or not. Network,
/// device and relative paths are not.
pub fn is_admissible_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}

/// A resolved path without its verbatim prefix; none for a network location.
pub(crate) fn native_spelling(resolved: String) -> Option<String> {
    if resolved.starts_with("\\\\?\\UNC\\") {
        return None;
    }
    Some(match resolved.strip_prefix("\\\\?\\") {
        Some(native) => native.to_owned(),
        None => resolved,
    })
}

/// Git prints and accepts a Windows path with forward slashes.
pub(crate) fn git_spelling(native: &str) -> Cow<'_, str> {
    Cow::Owned(native.replace('\\', "/"))
}

/// The spelling under which a checkout path Git recorded names the same checkout.
pub fn checkout_spelling(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests;
