//! Observes bounded absolute shell candidates from the inherited environment, current account,
//! and POSIX fallback, requiring a usable executable. It does not execute a candidate or
//! override the caller's launch selection.

use std::{
    env,
    ffi::OsStr,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use nix::{
    fcntl::{AT_FDCWD, AtFlags},
    unistd::{AccessFlags, Uid, User, faccessat},
};

use super::UsableShellCandidates;

const FALLBACK_SHELL: &str = "/bin/sh";
const MAXIMUM_SHELL_PATH_BYTES: usize = 4 * 1024;

pub(super) fn usable_shell_candidates() -> UsableShellCandidates {
    let inherited_shell = env::var_os("SHELL");
    let account_shell = current_account_shell();
    UsableShellCandidates {
        inherited: observe_usable_shell(inherited_shell.as_deref(), executable_is_usable),
        account: observe_usable_shell(
            account_shell.as_deref().map(Path::as_os_str),
            executable_is_usable,
        ),
        fallback: observe_usable_shell(Some(OsStr::new(FALLBACK_SHELL)), executable_is_usable),
    }
}

fn admissible_shell_path(candidate: &OsStr) -> Option<&str> {
    let bytes = candidate.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAXIMUM_SHELL_PATH_BYTES
        || bytes.contains(&0)
        || !Path::new(candidate).is_absolute()
    {
        return None;
    }
    candidate.to_str()
}

fn observe_usable_shell(
    candidate: Option<&OsStr>,
    executable_is_usable: impl FnOnce(&Path) -> bool,
) -> Option<String> {
    let executable = admissible_shell_path(candidate?)?;
    executable_is_usable(Path::new(executable)).then(|| executable.to_owned())
}

fn current_account_shell() -> Option<PathBuf> {
    User::from_uid(Uid::effective())
        .ok()
        .flatten()
        .map(|account| account.shell)
}

fn executable_is_usable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
        && faccessat(AT_FDCWD, path, AccessFlags::X_OK, AtFlags::AT_EACCESS).is_ok()
}

#[cfg(test)]
mod tests;
