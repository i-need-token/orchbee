//! The Worker's Git environment: literal repository selection and command-local Git trust shared
//! by reads and mutations, and the canonical spelling it reports. The Git program and path syntax
//! belong to `orchbee-git-process`.
use orchbee_capability_node_protocol::AbsoluteCwd;
use std::{path::Path, process::Command};

/// A path as the filesystem resolves it, in the spelling Git prints: forward slashes on Windows.
pub(crate) fn canonical_path(path: &Path) -> Result<AbsoluteCwd, ()> {
    let canonical = orchbee_git_process::canonical_path(path).map_err(|_| ())?;
    AbsoluteCwd::new(&canonical.git_spelling()).map_err(|_| ())
}

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
