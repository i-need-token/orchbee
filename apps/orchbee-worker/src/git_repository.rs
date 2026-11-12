//! Literal repository selection and command-local Git trust shared by reads and mutations. Path
//! syntax, canonical spelling and the Git program name belong to the platform adapter.
use std::{path::Path, process::Command};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub(crate) use unix::{GIT_PROGRAM, canonical_path, checkout_spelling, is_admissible_path};
#[cfg(windows)]
pub(crate) use windows::{GIT_PROGRAM, canonical_path, checkout_spelling, is_admissible_path};

pub(crate) fn configure(command: &mut Command, at: &Path, trust: bool) -> Result<(), ()> {
    // A long-lived Worker must not inherit repository selection or config injection.
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C");
    if trust {
        let root = at
            .ancestors()
            .find(|path| {
                path.join(".git").exists()
                    || (path.join("HEAD").is_file() && path.join("objects").is_dir())
            })
            .ok_or(())?;
        let root = canonical_path(root)?;
        command.args(["-c", &format!("safe.directory={}", root.as_str())]);
    }
    Ok(())
}
