//! The window launcher's effects: its own path and arguments, the parent check and wait, the pointer
//! read, each window start and its watch, the message box, and the Windows display language. Every
//! window it starts outlives it: the launcher creates no Job, keeps a window's handle only while it
//! watches that window, and closes it when it exits, so the window and its own children are never
//! ended with the launcher.
use crate::decision::{self, Failure, Next, Plan, Start, Watched};
use crate::localization::{self, Locale};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use winsafe::{self as w, co, prelude::*};

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
/// The registry values holding the Windows display language: the person's choice when they made
/// one, then the system's, as Windows caches it for the person.
const DISPLAY_LANGUAGE: [(&str, &str); 2] = [
    (r"Control Panel\Desktop", "PreferredUILanguages"),
    (
        r"Control Panel\Desktop\MuiCached",
        "MachinePreferredUILanguages",
    ),
];

/// Starts the active window version, and the previous one when the active one fails inside the
/// watch, and returns once a window stands or ended by its own choice.
///
/// # Errors
///
/// The failure to show when no window starts.
pub(crate) fn launch() -> Result<(), Failure> {
    let launcher = std::env::current_exe()
        .map_err(|_| Failure::NoActiveVersion(PathBuf::from("active.json")))?;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    // The parent is opened before the decision so the process waited for is the one confirmed.
    let parent = Parent::confirmed();
    let Plan {
        wait_for,
        first,
        fallback,
    } = decision::plan(
        &launcher,
        read_pointer,
        arguments,
        parent.as_ref().map(|parent| parent.process_id),
        Path::is_file,
    )?;
    if let (Some(_), Some(parent)) = (wait_for, &parent) {
        parent.wait(RELAUNCH_WAIT);
    }
    drop(parent);
    let watched = match start(&first) {
        None => Watched::NotStarted,
        Some(mut window) if fallback.is_some() => watch(&mut window),
        Some(_) => Watched::Running,
    };
    let next = match decision::after(watched, fallback.is_some()) {
        Next::Fallback => {
            let watched = fallback
                .as_ref()
                .and_then(start)
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

/// Shows `failure` in a message box in the Windows display language. Nothing else can report it:
/// the launcher has no console, and a message box that cannot open leaves only the exit code.
pub(crate) fn show(failure: &Failure) {
    let text = localization::failure(display_locale(), failure);
    let _ = w::HWND::NULL.MessageBox(&text, localization::CAPTION, co::MB::OK | co::MB::ICONERROR);
}

/// The launcher's parent, open for the wait, when it started no later than the launcher; a later
/// start means its process id was reused, so no unrelated process is waited for.
struct Parent {
    process_id: u32,
    process: w::guard::CloseHandleGuard<w::HPROCESS>,
}

impl Parent {
    fn confirmed() -> Option<Self> {
        let own = std::process::id();
        let mut snapshot =
            w::HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).ok()?;
        let process_id = snapshot
            .iter_processes()
            .filter_map(Result::ok)
            .find(|entry| entry.th32ProcessID == own)?
            .th32ParentProcessID;
        let process = w::HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
            false,
            process_id,
        )
        .ok()?;
        let (parent_started, ..) = process.GetProcessTimes().ok()?;
        let (own_started, ..) = w::HPROCESS::GetCurrentProcess().GetProcessTimes().ok()?;
        (u64::from(parent_started) <= u64::from(own_started)).then_some(Self {
            process_id,
            process,
        })
    }

    /// Waits until the parent ends or `limit` passes; a wait that fails is a wait that ended.
    fn wait(&self, limit: Duration) {
        let milliseconds = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX);
        let _ = self.process.WaitForSingleObject(Some(milliseconds));
    }
}

/// Starts one window, or `None` when no process started.
fn start(start: &Start) -> Option<Child> {
    let mut command = Command::new(&start.window);
    command.args(&start.arguments).current_dir(&start.folder);
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
            Ok(Some(status)) => {
                return status.code().map_or(Watched::Running, Watched::Ended);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(WATCH_POLL),
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

/// The locale of the first Windows display language the registry names, or `en-US` without one.
fn display_locale() -> Locale {
    let tag = DISPLAY_LANGUAGE.iter().find_map(|(key, value)| {
        let found = w::HKEY::CURRENT_USER
            .RegGetValue(Some(key), Some(value), co::RRF::RT_REG_MULTI_SZ)
            .ok()?;
        match found {
            w::RegistryValue::MultiSz(languages) => {
                languages.into_iter().find(|language| !language.is_empty())
            }
            _ => None,
        }
    });
    localization::locale(tag.as_deref())
}
