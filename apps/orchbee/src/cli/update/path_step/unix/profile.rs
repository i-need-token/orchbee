//! The macOS reach of the PATH step: a new Mac's shells do not search `~/.local/bin`, so the step
//! also adds two lines that put it on `PATH` to the startup file of the person's shell, and
//! uninstall removes exactly those lines. zsh, the default, reads `~/.zshenv` in every shell; bash
//! reads the first of `~/.bash_profile`, `~/.bash_login` and `~/.profile` in a login shell, which
//! Terminal and iTerm2 start. Another shell gets no file and `install` prints the line to add. The
//! edit is best effort: a file that cannot be read or written leaves the link in place, and the
//! install goes on.
use orchbee_local_runtime::environment_stem;
use std::{
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Puts `~/.local/bin` first on `PATH` unless it is there already, in zsh and bash alike.
const LINE: &str = r#"case ":${PATH}:" in *":$HOME/.local/bin:"*) ;; *) export PATH="$HOME/.local/bin:$PATH" ;; esac"#;

/// The line before [`LINE`] that marks the pair as this environment's, so each install adds and
/// each uninstall removes only its own.
fn marker() -> String {
    let stem = environment_stem();
    format!(
        "# Added by `{stem} install` to put ~/.local/bin on PATH; `{stem} uninstall` removes these two lines."
    )
}

/// The startup file the shell `shell` names reads, under `home`: zsh's when none is named, and
/// none for a shell other than zsh or bash.
pub(super) fn target(home: &Path, shell: Option<&OsStr>) -> Option<PathBuf> {
    match shell
        .map(Path::new)
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
    {
        None | Some("zsh") => Some(home.join(".zshenv")),
        Some("bash") => Some(
            [".bash_profile", ".bash_login"]
                .into_iter()
                .map(|name| home.join(name))
                .find(|file| file.exists())
                .unwrap_or_else(|| home.join(".profile")),
        ),
        Some(_) => None,
    }
}

/// The text without the first copy of this environment's two lines, or none when it holds none.
fn without(text: &str) -> Option<String> {
    fn bare(line: &str) -> &str {
        line.trim_end_matches(['\n', '\r'])
    }
    let marker = marker();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let at = lines.windows(2).position(
        |pair| matches!(pair, [first, second] if bare(first) == marker && bare(second) == LINE),
    )?;
    Some(
        lines
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != at && *index != at + 1)
            .map(|(_, line)| *line)
            .collect(),
    )
}

/// Adds this environment's two lines to the startup file of the shell `shell` names, unless that
/// file holds them already, keeping every line it had; the file that holds them, or none for
/// another shell or when the file could not be read or written.
pub(super) fn add(home: &Path, shell: Option<&OsStr>) -> Option<PathBuf> {
    let file = target(home, shell)?;
    let text = match fs::read(&file) {
        Ok(bytes) => String::from_utf8(bytes).ok()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(_) => return None,
    };
    if without(&text).is_some() {
        return Some(file);
    }
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let block = format!("{separator}{}\n{LINE}\n", marker());
    // Appending writes through a link a dotfile manager placed, which stays a link.
    let mut writer = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .ok()?;
    writer.write_all(block.as_bytes()).ok()?;
    Some(file)
}

/// Every file that holds this environment's two lines, whichever shell they were added for.
pub(super) fn holders(home: &Path) -> Vec<PathBuf> {
    [".zshenv", ".bash_profile", ".bash_login", ".profile"]
        .into_iter()
        .map(|name| home.join(name))
        .filter(|file| fs::read_to_string(file).is_ok_and(|text| without(&text).is_some()))
        .collect()
}

/// Removes every copy of this environment's two lines from each file that holds them and keeps the
/// rest of each file as it was; a plain file left empty is removed. Every file is tried; the first
/// failure is returned.
pub(super) fn remove(home: &Path) -> io::Result<()> {
    let mut failure = None;
    for file in holders(home) {
        if let Err(error) = remove_from(&file) {
            failure.get_or_insert(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn remove_from(file: &Path) -> io::Result<()> {
    let mut text = fs::read_to_string(file)?;
    while let Some(kept) = without(&text) {
        text = kept;
    }
    let linked = fs::symlink_metadata(file)?.file_type().is_symlink();
    if text.is_empty() && !linked {
        return fs::remove_file(file);
    }
    // The file a link names is rewritten, so the link stays; a rename keeps the file whole.
    let target = if linked {
        fs::canonicalize(file)?
    } else {
        file.to_owned()
    };
    let folder = target.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let name = target.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let staged = folder.join(format!(
        ".{}.orchbee-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let permissions = fs::metadata(&target)?.permissions();
    let written = fs::write(&staged, text.as_bytes())
        .and_then(|()| fs::set_permissions(&staged, permissions))
        .and_then(|()| fs::rename(&staged, &target));
    if written.is_err() {
        let _ = fs::remove_file(&staged);
    }
    written
}

#[cfg(test)]
mod tests;
