//! The person's Linux menu entry for the window store: `<data>/applications/<stem>.desktop`, whose
//! every `Exec=` line starts the store's window entry `<root>/desktop/<stem>-desktop` by absolute
//! path, the main entry with no argument and each action with the arguments the template gives it,
//! and its icons `<data>/icons/hicolor/<n>x<n>/apps/<stem>.png`, where `<data>` is
//! `XDG_DATA_HOME` when that is an absolute path and `~/.local/share` otherwise. The active window
//! version carries both as `resources/linux/<stem>.desktop` and `resources/linux/icons/<n>x<n>.png`;
//! the window layout copies them out after it places the entry, and uninstall removes them while the
//! menu entry still names this store. Only this build's default store root has a menu entry, so a
//! store at `ORCHBEE_INSTALL_DIR`, a test fixture among them, never adds one.
use crate::release_store::open_release_file;
use orchbee_local_runtime::environment_stem;
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// The largest menu entry a window version may carry.
const ENTRY_LIMIT: u64 = 64 * 1024;
/// The largest icon a window version may carry.
const ICON_LIMIT: u64 = 4 * 1024 * 1024;
/// The window version's folder of menu files.
const SOURCE: &str = "resources/linux";

/// Where the menu entry and icons of one store go, and the window entry they start.
pub(super) struct MenuEntry {
    /// `<data>`.
    data: PathBuf,
    /// `<root>/desktop/<stem>-desktop`.
    program: PathBuf,
}

impl MenuEntry {
    /// The menu entry of the store `root`, whose window entry is `program`; none for a root other
    /// than this build's default one, or when `HOME` names no absolute folder.
    pub(super) fn of(root: &Path, program: PathBuf) -> Option<Self> {
        let env = |name: &str| std::env::var_os(name);
        let default = orchbee_local_runtime::release_store_root(|name| {
            (name != "ORCHBEE_INSTALL_DIR").then(|| env(name)).flatten()
        })
        .ok()?;
        if default != root {
            return None;
        }
        let data = env("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|data| data.is_absolute())
            .or_else(|| {
                let home = PathBuf::from(env("HOME")?);
                home.is_absolute()
                    .then(|| home.join(".local").join("share"))
            })?;
        Some(Self { data, program })
    }

    /// `<data>/applications/<stem>.desktop`.
    pub(super) fn path(&self) -> PathBuf {
        self.data
            .join("applications")
            .join(format!("{}.desktop", environment_stem()))
    }

    /// Writes the menu entry and icons the window version `version` carries, each only when its bytes
    /// differ; a version that carries none, or a file that cannot be written, leaves that file as it
    /// is.
    pub(super) fn place(&self, version: &Path) {
        let stem = environment_stem();
        if let Some(entry) = read_bounded(version, &format!("{SOURCE}/{stem}.desktop"), ENTRY_LIMIT)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .and_then(|template| self.entry_text(&template))
        {
            let _ = write_changed(&self.path(), entry.as_bytes());
        }
        let Ok(listing) = fs::read_dir(version.join(SOURCE).join("icons")) else {
            return;
        };
        for name in listing.filter_map(|entry| entry.ok()?.file_name().into_string().ok()) {
            let Some(size) = icon_size(&name) else {
                continue;
            };
            if let Some(bytes) =
                read_bounded(version, &format!("{SOURCE}/icons/{name}"), ICON_LIMIT)
            {
                let _ = write_changed(&self.icon(size), &bytes);
            }
        }
    }

    /// Whether the menu entry is a file whose every `Exec=` line starts this store's window entry.
    pub(super) fn names_store(&self) -> bool {
        let Some(own) = exec_line(&self.program) else {
            return false;
        };
        let Ok(file) = File::open(self.path()) else {
            return false;
        };
        let mut text = String::new();
        if file.take(ENTRY_LIMIT).read_to_string(&mut text).is_err() {
            return false;
        }
        let mut lines = text.lines().filter(|line| line.starts_with("Exec="));
        lines.clone().next().is_some()
            && lines.all(|line| {
                line.strip_prefix(own.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
            })
    }

    /// Removes the icons the menu entry names, once the entry itself is gone; each one that cannot
    /// be removed stays.
    pub(super) fn remove_icons(&self) {
        let Ok(sizes) = fs::read_dir(self.data.join("icons").join("hicolor")) else {
            return;
        };
        for size in sizes.filter_map(|entry| entry.ok()?.file_name().into_string().ok()) {
            if icon_size(&format!("{size}.png")).is_some() {
                let _ = fs::remove_file(self.icon(&size));
            }
        }
    }

    /// `<data>/icons/hicolor/<size>/apps/<stem>.png`.
    fn icon(&self, size: &str) -> PathBuf {
        self.data
            .join("icons")
            .join("hicolor")
            .join(size)
            .join("apps")
            .join(format!("{}.png", environment_stem()))
    }

    /// `template` with the program of every `Exec=` line, its first word, replaced by the window
    /// entry and the arguments after it kept; none when the entry's path cannot be written in a menu
    /// entry or the template has no `Exec=` line.
    fn entry_text(&self, template: &str) -> Option<String> {
        let own = exec_line(&self.program)?;
        let mut found = false;
        let mut text = String::with_capacity(template.len() + own.len());
        for line in template.lines() {
            if let Some(command) = line.strip_prefix("Exec=") {
                found = true;
                text.push_str(&own);
                if let Some((_, arguments)) = command.split_once(' ') {
                    text.push(' ');
                    text.push_str(arguments);
                }
            } else {
                text.push_str(line);
            }
            text.push('\n');
        }
        found.then_some(text)
    }
}

/// The `Exec=` line that starts `program`: its path quoted as the Desktop Entry Specification
/// quotes an argument, inside a string value, with `%` doubled; none for a path that is not UTF-8
/// or holds a control character.
fn exec_line(program: &Path) -> Option<String> {
    let path = program.to_str()?;
    if path.chars().any(char::is_control) {
        return None;
    }
    let mut line = String::from("Exec=\"");
    for character in path.chars() {
        match character {
            // The quoting rule escapes these with one backslash, which the string rule then
            // escapes again.
            '"' | '`' | '$' => line.push_str("\\\\"),
            '\\' => line.push_str("\\\\\\"),
            '%' => line.push('%'),
            _ => {}
        }
        line.push(character);
    }
    line.push('"');
    Some(line)
}

/// The hicolor size folder of an icon file named `<n>x<n>.png`.
fn icon_size(name: &str) -> Option<&str> {
    let size = name.strip_suffix(".png")?;
    let (width, height) = size.split_once('x')?;
    let digits =
        |side: &str| (1..=4).contains(&side.len()) && side.bytes().all(|b| b.is_ascii_digit());
    (digits(width) && width == height).then_some(size)
}

/// The bytes of the release file `relative` of `version`, when it is at most `limit` bytes.
fn read_bounded(version: &Path, relative: &str, limit: u64) -> Option<Vec<u8>> {
    let file = open_release_file(version, relative).ok()?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// Replaces `path` with `bytes` through a flushed copy beside it, unless it holds them already.
fn write_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let folder = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(folder)?;
    let mut name = OsString::from(".");
    name.push(path.file_name().ok_or(io::ErrorKind::InvalidInput)?);
    name.push(format!(
        ".new-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let copy = folder.join(name);
    let written = File::create_new(&copy).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    let placed = written.and_then(|()| fs::rename(&copy, path));
    if placed.is_err() {
        let _ = fs::remove_file(&copy);
    }
    placed
}
