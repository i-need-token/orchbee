use std::path::{Path, PathBuf};

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
