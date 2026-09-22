use super::UsableShellCandidates;
use std::{env, path::Path};

pub(super) fn usable_shell_candidates() -> UsableShellCandidates {
    // Windows defines COMSPEC as the command interpreter. Resolve no executable
    // through PATH and accept only an existing absolute executable path.
    let inherited = env::var_os("COMSPEC").and_then(|path| usable(Path::new(&path)));
    let fallback = env::var_os("SystemRoot")
        .and_then(|root| usable(&Path::new(&root).join("System32").join("cmd.exe")));
    UsableShellCandidates {
        inherited,
        account: None,
        fallback,
    }
}

fn usable(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (path.is_absolute() && path.is_file() && text.len() <= 4096 && !text.contains('\0'))
        .then(|| text.to_owned())
}
