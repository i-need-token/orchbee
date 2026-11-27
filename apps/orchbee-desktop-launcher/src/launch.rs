//! The Windows and Linux launcher's run: the pointer read, the wait for the window that asked, each
//! window start and its watch, and the fallback. Every window it starts outlives it: the launcher
//! creates no Job, keeps a window's handle only while it watches that window, and lets it go when it
//! exits, so the window and its own children are never ended with the launcher. On Linux each window
//! leads a process group of its own, so an interrupt in the terminal that started the launcher
//! leaves the window.
use crate::decision::{self, Failure, Next, Plan, Start, Watched};
use std::{
    ffi::OsStr,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

/// The largest pointer the launcher reads, the bound of the store's own reader.
const POINTER_LIMIT: u64 = 16 * 1024;
/// How long it waits for the window that asked for a relaunch, which holds the single-instance
/// lock until it ends. Past it, the new window hands its command line to the old one and ends with
/// status 0, which breaks nothing.
const RELAUNCH_WAIT: Duration = Duration::from_secs(30);
/// How long it watches an active window that has a previous version to fall back to.
const WATCH: Duration = Duration::from_secs(120);
/// How often the watch looks at the window.
const WATCH_POLL: Duration = Duration::from_millis(100);

/// The launcher's parent, confirmed to have started no later than the launcher, so a reused process
/// id never names an unrelated process.
pub(crate) trait Parent {
    fn process_id(&self) -> u32;
    /// Waits until the parent ends or `limit` passes; a wait that fails is a wait that ended.
    fn wait(&self, limit: Duration);
}

/// Starts the active window version, and the previous one when the active one fails inside the
/// watch, and returns once a window stands or ended by its own choice. `parent` is confirmed before
/// the decision so the process waited for is the one confirmed. Every start receives `switches`
/// before its own arguments.
///
/// # Errors
///
/// The failure to report when no window starts.
pub(crate) fn launch(parent: Option<impl Parent>, switches: &[&OsStr]) -> Result<(), Failure> {
    let launcher = std::env::current_exe()
        .map_err(|_| Failure::NoActiveVersion(PathBuf::from(decision::POINTER)))?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let Plan {
        wait_for,
        first,
        fallback,
    } = decision::plan(
        &launcher,
        read_pointer,
        arguments,
        parent.as_ref().map(Parent::process_id),
        Path::is_file,
    )?;
    if let (Some(_), Some(parent)) = (wait_for, &parent) {
        parent.wait(RELAUNCH_WAIT);
    }
    drop(parent);
    let watched = match start(&first, switches) {
        None => Watched::NotStarted,
        Some(mut window) if fallback.is_some() => watch(&mut window),
        Some(_) => Watched::Running,
    };
    let next = match decision::after(watched, fallback.is_some()) {
        Next::Fallback => {
            let watched = fallback
                .as_ref()
                .and_then(|fallback| start(fallback, switches))
                .map_or(Watched::NotStarted, |_| Watched::Running);
            decision::after(watched, false)
        }
        next => next,
    };
    match next {
        Next::Done => Ok(()),
        Next::Fallback | Next::Unavailable => Err(Failure::ClientUnavailable(first.window)),
    }
}

/// Starts one window with `switches` first, or `None` when no process started.
fn start(start: &Start, switches: &[&OsStr]) -> Option<Child> {
    let mut command = Command::new(&start.window);
    command
        .args(switches)
        .args(&start.arguments)
        .current_dir(&start.folder);
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    if start.announced {
        command.env(decision::PROTOCOL_VARIABLE, std::process::id().to_string());
    } else {
        command.env_remove(decision::PROTOCOL_VARIABLE);
    }
    command.spawn().ok()
}

/// Watches `window` until it ends or `WATCH` passes. A status the launcher cannot read counts as a
/// window that still runs, so an unknown outcome never starts a second window beside it.
fn watch(window: &mut Child) -> Watched {
    let deadline = Instant::now() + WATCH;
    loop {
        match window.try_wait() {
            Ok(Some(status)) => return ended(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(WATCH_POLL),
            Ok(None) | Err(_) => return Watched::Running,
        }
    }
}

/// How a window that ended inside the watch came out; on Linux a window a signal ended reads as 128
/// plus the signal's number, as a shell reports it, so a crash gives way as it does on Windows.
fn ended(status: ExitStatus) -> Watched {
    #[cfg(unix)]
    let code = {
        use std::os::unix::process::ExitStatusExt as _;
        status
            .code()
            .or_else(|| status.signal().map(|signal| 128 + signal))
    };
    #[cfg(windows)]
    let code = status.code();
    code.map_or(Watched::Running, Watched::Ended)
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
