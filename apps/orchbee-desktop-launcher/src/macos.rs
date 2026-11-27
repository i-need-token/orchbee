//! The macOS window launcher's effects: its own path, arguments and parent, the wait for the asking
//! window, the pointer read, the replacement of the application with `ditto`, the replaced
//! application's access, `codesign` and two renames, each window start and its watch, and undoing the replacement when the new window fails
//! inside the watch. Every window it starts leads a process group of its own and outlives it; the
//! launcher keeps a window only while it watches it.
use crate::decision::{self, Next, Start, Watched};
use crate::replacement::{self, Relaunch, Source};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    os::unix::process::{CommandExt, ExitStatusExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// The largest pointer the launcher reads, the bound of the store's own reader.
const POINTER_LIMIT: u64 = 16 * 1024;
/// How long it waits for the asking window to end. A window that still runs keeps its application:
/// nothing is replaced under a running window.
const RELAUNCH_WAIT: Duration = Duration::from_secs(30);
/// How long it watches the new window before the application it replaced is removed.
const WATCH: Duration = Duration::from_secs(120);
/// How often the wait and the watch look.
const POLL: Duration = Duration::from_millis(100);
/// The folder an application goes to when the folder it ran from cannot be written, in `$HOME`.
const USER_APPLICATIONS: &str = "Applications";
const DITTO: &str = "/usr/bin/ditto";
const CODESIGN: &str = "/usr/bin/codesign";

/// Serves the relaunch its parent asked for: waits for that window to end, replaces the
/// application it ran from with the active window version, starts it, and puts the replaced
/// application back and starts it again when the new one cannot be put in place, cannot start, or
/// fails inside the watch. `None` when the request is no relaunch the launcher serves, or no window
/// starts.
pub(crate) fn launch() -> Option<()> {
    let launcher = std::env::current_exe().ok()?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let relaunch =
        replacement::relaunch(&launcher, arguments, std::os::unix::process::parent_id())?;
    let source = relaunch.source(read_pointer(&relaunch.pointer()), Path::is_file);
    if !parent_ended(relaunch.wait_for, RELAUNCH_WAIT) {
        return Some(());
    }
    let Some(Source { version, bundle }) = source else {
        return restart(&relaunch, None);
    };
    let Some(placed) = Placed::install(&bundle, &relaunch.application) else {
        return restart(&relaunch, Some(&version));
    };
    let first = window_start(
        &placed.application,
        &relaunch,
        relaunch.arguments.clone(),
        true,
    );
    let watched = start(&first).map_or(Watched::NotStarted, |mut window| watch(&mut window));
    match decision::after(watched, true) {
        Next::Fallback => {
            placed.undo();
            restart(&relaunch, Some(&version))
        }
        Next::Done | Next::Unavailable => {
            placed.commit();
            Some(())
        }
    }
}

/// Starts the application the window ran from again, unannounced, after `failed` could not take
/// its place; the start is not watched, since nothing remains to fall back to.
fn restart(relaunch: &Relaunch, failed: Option<&str>) -> Option<()> {
    let again = window_start(
        &relaunch.application,
        relaunch,
        relaunch.fallback_arguments(failed),
        false,
    );
    start(&again).map(drop)
}

fn window_start(
    application: &Path,
    relaunch: &Relaunch,
    arguments: Vec<OsString>,
    announced: bool,
) -> Start {
    let window = application.join(&relaunch.program);
    let folder = window
        .parent()
        .map_or_else(|| application.to_path_buf(), Path::to_path_buf);
    Start {
        window,
        folder,
        arguments,
        announced,
    }
}

/// Whether the launcher's parent, `parent` at its start, ended within `limit`: an orphaned process
/// is adopted by `launchd`, so its parent process id changes exactly when the parent ends.
fn parent_ended(parent: u32, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if std::os::unix::process::parent_id() != parent {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
}

/// An application put in place, with whatever stood at its path before, renamed aside in the same
/// folder until the new window has stood through the watch.
struct Placed {
    application: PathBuf,
    previous: Option<PathBuf>,
}

impl Placed {
    /// Copies `source` beside `original`, the application the window ran from, and renames it into
    /// place, or, when that folder cannot be written, into `~/Applications` under the same name.
    /// The copy takes the access of the application it replaces, as Sparkle does, so only the
    /// store's own copy stays private. `None` leaves every folder as it was.
    fn install(source: &Path, original: &Path) -> Option<Self> {
        let name = original.file_name()?;
        let folder = original.parent()?;
        let (folder, staging) = match staging_folder(folder, name) {
            Ok(staging) => (folder.to_path_buf(), staging),
            Err(error) if writing_refused(&error) => {
                let home = std::env::var_os("HOME").map(PathBuf::from)?;
                let user = home.join(USER_APPLICATIONS);
                if !home.is_absolute() || user == folder {
                    return None;
                }
                fs::create_dir_all(&user).ok()?;
                let staging = staging_folder(&user, name).ok()?;
                (user, staging)
            }
            Err(_) => return None,
        };
        let application = folder.join(name);
        // What the copy takes its access from: the application it replaces, or the one the window
        // ran from when it goes to `~/Applications` where none stands.
        let replaced = fs::symlink_metadata(&application)
            .or_else(|_| fs::symlink_metadata(original))
            .ok()
            .filter(fs::Metadata::is_dir);
        let copied = run(Command::new(DITTO).arg(source).arg(&staging))
            && match_access(&staging, replaced.as_ref()).is_ok()
            && run(Command::new(CODESIGN)
                .args(["--verify", "--deep", "--strict"])
                .arg(&staging));
        if !copied {
            let _ = fs::remove_dir_all(&staging);
            return None;
        }
        let previous = match fs::symlink_metadata(&application) {
            Ok(_) => {
                let previous = hidden(&folder, name, "previous");
                if fs::rename(&application, &previous).is_err() {
                    let _ = fs::remove_dir_all(&staging);
                    return None;
                }
                Some(previous)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(_) => {
                let _ = fs::remove_dir_all(&staging);
                return None;
            }
        };
        if fs::rename(&staging, &application).is_err() {
            if let Some(previous) = &previous {
                let _ = fs::rename(previous, &application);
            }
            let _ = fs::remove_dir_all(&staging);
            return None;
        }
        Some(Self {
            application,
            previous,
        })
    }

    /// Keeps the new application and removes the one it replaced.
    fn commit(self) {
        if let Some(previous) = self.previous {
            let _ = fs::remove_dir_all(previous);
        }
    }

    /// Puts back what stood at the path before, or removes the new application where nothing stood.
    fn undo(self) {
        let Some(previous) = self.previous else {
            let _ = fs::remove_dir_all(&self.application);
            return;
        };
        let (Some(folder), Some(name)) = (self.application.parent(), self.application.file_name())
        else {
            return;
        };
        let failed = hidden(folder, name, "failed");
        if fs::rename(&self.application, &failed).is_ok() {
            let _ = fs::rename(&previous, &self.application);
            let _ = fs::remove_dir_all(failed);
        }
    }
}

/// Gives the copy at `staging`, which `ditto` made from the store's private copy (folders and
/// programs 0700, other files 0600), the access of `replaced`, the application bundle it replaces:
/// its group, and its owner where the system allows that, then its folder's mode on every folder and
/// program and that mode without execute bits on every other file, always keeping the owner's read
/// and write. Without a replaced bundle the mode is 0755, an application folder's usual mode. Links
/// are left alone. A refused owner or group change keeps the copy's own; a refused mode fails.
fn match_access(staging: &Path, replaced: Option<&fs::Metadata>) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, lchown};
    const OWNER: u32 = 0o700;
    const EXECUTE: u32 = 0o111;
    let base = replaced.map_or(0o755, |replaced| replaced.mode() & 0o777);
    let mut pending = vec![staging.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if let Some(replaced) = replaced {
            let user = (replaced.uid() != metadata.uid()).then_some(replaced.uid());
            let group = (replaced.gid() != metadata.gid()).then_some(replaced.gid());
            if (user.is_some() || group.is_some()) && lchown(&path, user, group).is_err() {
                let _ = lchown(&path, None, group);
            }
        }
        let mode = if metadata.is_dir() {
            for entry in fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
            base | OWNER
        } else if metadata.mode() & 0o100 != 0 {
            base | OWNER
        } else {
            (base & !EXECUTE) | 0o600
        };
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

/// Creates the empty hidden folder the copy goes to, in `folder` beside the application `name`.
fn staging_folder(folder: &Path, name: &std::ffi::OsStr) -> io::Result<PathBuf> {
    let staging = hidden(folder, name, "update");
    fs::create_dir(&staging)?;
    Ok(staging)
}

/// `<folder>/.<name>.<purpose>-<the launcher's process id>`, a name no other launcher run uses.
fn hidden(folder: &Path, name: &std::ffi::OsStr, purpose: &str) -> PathBuf {
    let mut hidden = OsString::from(".");
    hidden.push(name);
    hidden.push(format!(".{purpose}-{}", std::process::id()));
    folder.join(hidden)
}

/// Whether creating in a folder failed because the folder cannot be written by this user.
fn writing_refused(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
    )
}

/// Runs a system tool without input or output and says whether it succeeded.
fn run(command: &mut Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Starts one window in a process group of its own, or `None` when no process started.
fn start(start: &Start) -> Option<Child> {
    let mut command = Command::new(&start.window);
    command
        .args(&start.arguments)
        .current_dir(&start.folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let variable = decision::PROTOCOL_VARIABLE;
    if start.announced {
        let process = std::process::id().to_string();
        command
            .env(variable.current, &process)
            .env(variable.earlier, &process);
    } else {
        command
            .env_remove(variable.current)
            .env_remove(variable.earlier);
    }
    command.spawn().ok()
}

/// Watches `window` until it ends or `WATCH` passes. A window ended by a signal, as a crashed window
/// ends, counts as a failure; a status the launcher cannot read counts as a window that still runs,
/// so an unknown outcome never starts a second window beside it.
fn watch(window: &mut Child) -> Watched {
    let deadline = Instant::now() + WATCH;
    loop {
        match window.try_wait() {
            Ok(Some(status)) => {
                let code = status
                    .code()
                    .or_else(|| status.signal().map(|signal| 128 + signal));
                return code.map_or(Watched::Running, Watched::Ended);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
            Ok(None) | Err(_) => return Watched::Running,
        }
    }
}

/// The pointer's bytes, or `None` when it is missing, unreadable, or larger than the bound.
fn read_pointer(path: &Path) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(POINTER_LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    u64::try_from(bytes.len())
        .is_ok_and(|length| length <= POINTER_LIMIT)
        .then_some(bytes)
}
