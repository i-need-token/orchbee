use super::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, UsableShellCandidates, default_launch,
    select_default_interactive_launch,
};
use orchbee_local_site_runtime::{WindowsShellMode, WindowsShellSettings};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

fn usable_shell_candidates(env: &impl Fn(&str) -> Option<OsString>) -> UsableShellCandidates {
    // Windows defines COMSPEC as the command interpreter. Resolve no executable
    // through PATH and accept only an existing absolute executable path.
    let inherited = env("COMSPEC").and_then(|path| usable(Path::new(&path)));
    let fallback = env("SystemRoot")
        .and_then(|root| usable(&Path::new(&root).join("System32").join("cmd.exe")));
    UsableShellCandidates {
        inherited,
        account: None,
        fallback,
    }
}

pub(super) fn resolve(
    shell: &WindowsShellSettings,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    let mut launch = if shell.executable().is_empty() {
        select_default_interactive_launch(usable_shell_candidates(&env))?
    } else {
        let executable = resolve_configured(shell.executable(), env("PATH"))
            .ok_or(DefaultInteractiveLaunchError::NoUsableShell)?;
        default_launch(&executable)
    };
    if shell.mode() == WindowsShellMode::Login && login_supported(&launch.executable) {
        launch.arguments.push("-l".to_owned());
    }
    Ok(launch)
}

fn resolve_configured(executable: &str, path: Option<OsString>) -> Option<String> {
    let selected = Path::new(executable);
    if selected.is_absolute() {
        return usable(selected);
    }
    let names: Vec<PathBuf> = if selected.extension().is_none() {
        vec![selected.to_owned(), selected.with_extension("exe")]
    } else {
        vec![selected.to_owned()]
    };
    path.as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| names.iter().find_map(|name| usable(&directory.join(name))))
}

fn login_supported(executable: &str) -> bool {
    let name = executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(executable)
        .to_ascii_lowercase();
    matches!(
        name.strip_suffix(".exe").unwrap_or(&name),
        "sh" | "bash" | "zsh" | "ksh" | "dash" | "ash" | "mksh" | "fish" | "csh" | "tcsh"
    )
}

fn usable(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (path.is_absolute() && path.is_file() && text.len() <= 4096 && !text.contains('\0'))
        .then(|| text.to_owned())
}

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
