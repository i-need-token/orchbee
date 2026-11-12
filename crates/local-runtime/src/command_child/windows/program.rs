//! Resolves a Windows command operand using its literal path, caller cwd, and absolute PATH
//! entries with PATHEXT candidates, and names programs that ship in the Windows system folder.
//! It returns a program path without starting a process or admitting an artifact as an Orchbee
//! cohort executable.

use std::{
    io,
    path::{Path, PathBuf},
};

/// The full path of a program that ships in the Windows system folder, such as `cmd.exe` or
/// `WindowsPowerShell/v1.0/powershell.exe`. The folder comes from the operating system, never from
/// `PATH` or `SystemRoot`, so a same-named program earlier on the search path cannot stand in.
pub fn windows_system_program(relative: &str) -> io::Result<PathBuf> {
    let system = winsafe::GetSystemDirectory().map_err(io::Error::other)?;
    Ok(PathBuf::from(system).join(relative))
}

pub fn resolve_windows_program(value: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        return path.into();
    }
    if value.contains(['/', '\\']) || cwd.join(path).is_file() {
        return cwd.join(path);
    }
    // Prefer executable resolution; a batch file is passed directly to Rust's
    // Windows Command adapter, which quotes its argv for cmd.exe safely.
    if let Some(paths) = std::env::var_os("PATH") {
        let extensions =
            std::env::var_os("PATHEXT").unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into());
        for dir in std::env::split_paths(&paths).filter(|p| p.is_absolute()) {
            if path.extension().is_some() {
                let candidate = dir.join(path);
                if candidate.is_file() {
                    return candidate;
                }
            } else {
                for ext in extensions.to_string_lossy().split(';').filter(|s| {
                    s.starts_with('.') && s[1..].bytes().all(|c| c.is_ascii_alphanumeric())
                }) {
                    let candidate = dir.join(format!("{value}{ext}"));
                    if candidate.is_file() {
                        return candidate;
                    }
                }
            }
        }
    }
    path.into()
}
