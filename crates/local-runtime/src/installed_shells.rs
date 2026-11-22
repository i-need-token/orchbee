//! The shells installed on this machine that a new launch profile can start, as Add profile lists
//! them: on Windows, PowerShell, Windows PowerShell, Command Prompt and Git Bash; elsewhere, each
//! shell `/etc/shells` names and each Homebrew shell. It only reads the file system and launches
//! nothing. Desktop's Main lists the same shells from the same places.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// A shell this build names in each language; any other is named by its file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstalledShellKind {
    /// PowerShell 7 or later, `pwsh.exe`.
    PowerShell,
    /// Windows PowerShell 5.1, `powershell.exe`.
    WindowsPowerShell,
    CommandPrompt,
    GitBash,
    /// A shell named by its file, such as `zsh`.
    Named,
}

/// One shell a new profile can start: its literal executable and the arguments it starts with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledShell {
    pub kind: InstalledShellKind,
    /// The name a [`InstalledShellKind::Named`] shell shows; the file name, with its folder when
    /// two shells share it.
    pub name: String,
    pub executable: String,
    pub arguments: Vec<String>,
}

/// The shells installed here, in the order Add profile lists them; empty where none is found.
#[must_use]
pub fn installed_shells() -> Vec<InstalledShell> {
    installed_shells_in(&|name| std::env::var_os(name))
}

#[cfg(windows)]
fn installed_shells_in(env: &dyn Fn(&str) -> Option<OsString>) -> Vec<InstalledShell> {
    let search: Vec<PathBuf> = env("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .filter(|directory| directory.is_absolute())
        .collect();
    let on_path = |file: &str| {
        search
            .iter()
            .map(|directory| directory.join(file))
            .find(|path| path.is_file())
    };
    let under =
        |variable: &str, relative: &str| env(variable).map(|root| Path::new(&root).join(relative));
    let first = |candidates: Vec<Option<PathBuf>>| {
        candidates.into_iter().flatten().find(|path| path.is_file())
    };
    let system = |relative: &str| first(vec![under("SystemRoot", relative)]);
    // Git for Windows puts `git.exe` in `cmd`, beside the `bin` folder that holds its bash.
    let git_bash = || {
        on_path("git.exe").and_then(|git| {
            git.parent()?
                .parent()
                .map(|root| root.join(r"bin\bash.exe"))
        })
    };
    let shells = [
        (
            InstalledShellKind::PowerShell,
            first(vec![
                on_path("pwsh.exe"),
                under("ProgramFiles", r"PowerShell\7\pwsh.exe"),
            ]),
            &[][..],
        ),
        (
            InstalledShellKind::WindowsPowerShell,
            system(r"System32\WindowsPowerShell\v1.0\powershell.exe"),
            &[],
        ),
        (
            InstalledShellKind::CommandPrompt,
            system(r"System32\cmd.exe"),
            &[],
        ),
        (
            InstalledShellKind::GitBash,
            first(vec![
                under("ProgramFiles", r"Git\bin\bash.exe"),
                under("ProgramFiles(x86)", r"Git\bin\bash.exe"),
                under("LOCALAPPDATA", r"Programs\Git\bin\bash.exe"),
                git_bash(),
            ]),
            &["--login", "-i"],
        ),
    ];
    shells
        .into_iter()
        .filter_map(|(kind, path, arguments)| {
            Some(InstalledShell {
                kind,
                name: String::new(),
                executable: literal(&path?)?,
                arguments: arguments
                    .iter()
                    .map(|argument| (*argument).to_owned())
                    .collect(),
            })
        })
        .collect()
}

/// Shells Homebrew installs that `/etc/shells` often lacks, in the order they are listed.
#[cfg(not(windows))]
const HOMEBREW_SHELLS: [&str; 7] = ["zsh", "bash", "fish", "nu", "pwsh", "elvish", "xonsh"];

#[cfg(not(windows))]
fn installed_shells_in(_env: &dyn Fn(&str) -> Option<OsString>) -> Vec<InstalledShell> {
    let listed = std::fs::read_to_string("/etc/shells").unwrap_or_default();
    let homebrew = ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .flat_map(|directory| HOMEBREW_SHELLS.map(|shell| Path::new(directory).join(shell)));
    let candidates = listed
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .map(PathBuf::from)
        .chain(homebrew);
    let mut seen = std::collections::HashSet::new();
    let found: Vec<PathBuf> = candidates
        .filter(|path| executable_file(path))
        .filter(|path| seen.insert(path.canonicalize().unwrap_or_else(|_| path.clone())))
        .collect();
    let file_name = |path: &Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
    };
    found
        .iter()
        .filter_map(|path| {
            let name = file_name(path)?;
            let shared = found
                .iter()
                .filter(|other| file_name(other).as_ref() == Some(&name))
                .count()
                > 1;
            let name = match path.parent().and_then(Path::to_str) {
                Some(folder) if shared => format!("{name} ({folder})"),
                _ => name,
            };
            Some(InstalledShell {
                kind: InstalledShellKind::Named,
                name,
                executable: literal(path)?,
                arguments: Vec::new(),
            })
        })
        .collect()
}

#[cfg(not(windows))]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// The path as a profile stores it: absolute UTF-8 text within the executable bound.
fn literal(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (path.is_absolute() && text.len() <= 4096 && !text.contains('\0')).then(|| text.to_owned())
}
