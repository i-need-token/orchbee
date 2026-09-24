//! Literal Windows repository selection and command-local Git trust shared by reads and mutations.
use orchbee_capability_node_protocol::AbsoluteCwd;
use std::{path::Path, process::Command};

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

pub(crate) fn is_drive_path(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    path.is_absolute()
        && matches!(path.components().next(), Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
}

pub(crate) fn canonical_path(path: &Path) -> Result<AbsoluteCwd, ()> {
    // Filesystem access belongs to the Git job owner, never serialized ingress.
    if !is_drive_path(path) {
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
