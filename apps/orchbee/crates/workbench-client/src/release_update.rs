//! The TUI's Update control under the automatic-update proposal: whether the release store names a
//! version newer than this Client active, whether that version can join the running Session, and
//! how the TUI leaves for it. The store is read on a background thread; the switch itself is the
//! TUI's exit: a launcher that announced the relaunch protocol starts the active version; without
//! one a macOS TUI replaces itself with that version, and elsewhere the TUI names the command that
//! opens it. About's Check for updates asks the Session Server to run its update job now; see
//! [`check`].

use crate::background_thread::BackgroundThread;
use crate::localization::Locale;
pub(crate) use check::{CheckContext, CheckUnavailable, UpdateCheck};
use orchbee_connection_handshake::{COMPATIBILITY_WINDOW, Generation, GenerationRange};
use orchbee_local_runtime::{ActiveRelease, EnvironmentKind};
use std::{
    cmp::Ordering,
    sync::{
        Arc, Mutex, PoisonError,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    time::Duration,
};

/// How often the watcher reads the store's pointer. A background install moves it at most once an
/// hour, so the control appears within this interval of it.
const STORE_POLL: Duration = Duration::from_secs(10);

/// The status with which a TUI started by an announcing launcher asks that launcher to start the
/// store's active version in the same console.
pub const RELAUNCH_STATUS: u8 = 75;

/// What the Update control shows. Switching has no state of its own: the TUI leaves at once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateControl {
    Absent,
    /// The store names this version active, newer than this Client's own.
    Ready {
        version: String,
    },
}

/// The facts about this build that decide whether the control can ever show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Build {
    /// The automatic-update switch, `automatic_updates_live`.
    pub(crate) live: bool,
    pub(crate) kind: EnvironmentKind,
    /// Whether `orchbee update` writes this platform's release store: Windows and macOS, not Linux.
    pub(crate) store: bool,
}

impl Build {
    const fn current() -> Self {
        Self {
            live: orchbee_local_runtime::automatic_updates_live(),
            kind: orchbee_local_runtime::environment_kind(),
            store: cfg!(any(windows, target_os = "macos")),
        }
    }

    /// Whether any store state could make the control Ready: never behind the switch, in a
    /// development build, which has no store, or where no update writes the store.
    const fn offers_updates(self) -> bool {
        self.live && self.store && !matches!(self.kind, EnvironmentKind::Development)
    }
}

/// The control for `build`, this Client's version `own`, and the version the store names active.
/// A held version is never the active one, so it needs no check here.
pub(crate) fn control(build: Build, own: &str, active: Option<&str>) -> UpdateControl {
    match active {
        Some(version)
            if build.offers_updates()
                && orchbee_local_runtime::compare_product_versions(version, own)
                    == Ordering::Greater =>
        {
            UpdateControl::Ready {
                version: version.to_owned(),
            }
        }
        _ => UpdateControl::Absent,
    }
}

/// Whether the active version can join the running Session, as step 1 of a click checks: the
/// Session runs the first automatic-update release or later, which its answer's
/// `compatibility-window` capability marks, and the active manifest's `client` component lists the
/// `client-server` generation this connection negotiated.
pub(crate) fn joins(
    marker: bool,
    negotiated: Option<Generation>,
    client: Option<GenerationRange>,
) -> bool {
    marker
        && negotiated
            .zip(client)
            .is_some_and(|(negotiated, client)| client.contains(negotiated))
}

/// Reads the `client-server` generations the active version's manifest lists for its `client`
/// component; `None` when the manifest cannot be read or lists none. The application supplies it,
/// since it owns the manifest format.
pub type ClientGenerations = Box<dyn Fn(&ActiveRelease) -> Option<GenerationRange> + Send>;

/// What a press on About's Updates row asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateRequest {
    /// Ask the Session Server to run its update job now.
    Check,
    /// Run the Update control's switch, as a click on it does.
    Switch,
}

/// What a click on Update came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateSwitch {
    /// The control is no longer Ready, so the click is refused.
    Unavailable,
    /// The active version cannot join this Session yet; the Session must move first.
    Waiting,
    /// The TUI leaves for the active version.
    Leave,
}

/// The store watcher and the switch state of one TUI.
pub struct Updater {
    build: Build,
    watcher: Option<StoreWatcher>,
    active: Option<ActiveRelease>,
    control: UpdateControl,
    generations: ClientGenerations,
    leaving: UpdateDeparture,
    check: check::Check,
    /// A store reading arrived since the check last looked.
    store_read: bool,
}

/// The version a click chose to leave for, readable after the TUI that owns the [`Updater`] ended.
#[derive(Clone, Debug, Default)]
pub struct UpdateDeparture(Arc<Mutex<Option<String>>>);

impl UpdateDeparture {
    /// The version, once.
    pub fn take(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }

    /// Whether a click chose a version not yet taken.
    pub fn chosen(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    fn set(&self, version: String) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(version);
    }
}

impl Updater {
    /// Starts watching the store when this build can offer updates at all; otherwise the control
    /// stays Absent and nothing is read.
    pub fn start(generations: ClientGenerations) -> Self {
        let build = Build::current();
        let watcher = build
            .offers_updates()
            .then(StoreWatcher::start)
            .and_then(Result::ok);
        Self {
            build,
            watcher,
            active: None,
            control: UpdateControl::Absent,
            generations,
            leaving: UpdateDeparture::default(),
            check: check::Check::default(),
            store_read: false,
        }
    }

    /// A Ready control for `version` without a watcher, as a test's store would show it.
    #[cfg(test)]
    pub(crate) fn ready(version: &str, generations: ClientGenerations) -> Self {
        let mut updater = Self {
            build: Build {
                live: true,
                kind: EnvironmentKind::Test,
                store: true,
            },
            watcher: None,
            active: None,
            control: UpdateControl::Absent,
            generations,
            leaving: UpdateDeparture::default(),
            check: check::Check::default(),
            store_read: false,
        };
        updater.observe(Some(ActiveRelease {
            version: version.to_owned(),
            directory: std::path::PathBuf::new(),
        }));
        updater
    }

    /// Takes the watcher's newest reading. Answers whether the control changed.
    pub(crate) fn tick(&mut self) -> bool {
        let Some(watcher) = &self.watcher else {
            return false;
        };
        let Some(active) = watcher.latest() else {
            return false;
        };
        self.store_read = true;
        self.observe(active)
    }

    /// What About's Updates row reports.
    pub(crate) const fn check(&self) -> UpdateCheck {
        self.check.shown()
    }

    /// A press on Check for updates.
    pub(crate) fn start_check(&mut self, context: &CheckContext<'_>) {
        self.check.start(self.build, context);
    }

    /// Follows the check on every tick; after a run installed a version, the store is read at
    /// once rather than at its next poll.
    pub(crate) fn drive_check(&mut self, context: &CheckContext<'_>) {
        let store = check::Store {
            read: std::mem::take(&mut self.store_read),
            ready: matches!(self.control, UpdateControl::Ready { .. }),
        };
        if self.check.drive(self.build, context, store)
            && let Some(watcher) = &self.watcher
        {
            watcher.read_now();
        }
    }

    fn observe(&mut self, active: Option<ActiveRelease>) -> bool {
        let control = control(
            self.build,
            crate::local_client::CLIENT_VERSION,
            active.as_ref().map(|release| release.version.as_str()),
        );
        self.active = active;
        let changed = control != self.control;
        self.control = control;
        changed
    }

    pub(crate) fn control(&self) -> &UpdateControl {
        &self.control
    }

    /// Runs a click against the current control and the Session's handshake answer. A switch
    /// records the version the TUI leaves for in its [`UpdateDeparture`].
    pub(crate) fn switch(&mut self, marker: bool, negotiated: Option<Generation>) -> UpdateSwitch {
        let (UpdateControl::Ready { version }, Some(active)) = (&self.control, &self.active) else {
            return UpdateSwitch::Unavailable;
        };
        if !joins(marker, negotiated, (self.generations)(active)) {
            return UpdateSwitch::Waiting;
        }
        self.leaving.set(version.clone());
        UpdateSwitch::Leave
    }

    /// Leaves for the store's active version when it is newer than this Client, as a click does,
    /// once the Session this Client showed moved to it: no Session is left to join.
    pub(crate) fn leave_for_move(&mut self) -> bool {
        let UpdateControl::Ready { version } = &self.control else {
            return false;
        };
        self.leaving.set(version.clone());
        true
    }

    /// Where a click records the version it leaves for.
    pub fn departure(&self) -> UpdateDeparture {
        self.leaving.clone()
    }
}

/// The Session's handshake facts a click checks: whether the answer listed the slice 1 marker,
/// and the generation it chose.
pub(crate) fn session_facts(
    origin: Option<&crate::local_client::LocalClientOrigin>,
) -> (bool, Option<Generation>) {
    origin.map_or((false, None), |origin| {
        (
            origin
                .session_capabilities()
                .is_some_and(|answer| answer.contains(COMPATIBILITY_WINDOW)),
            origin.session_generation(),
        )
    })
}

/// Whether the launcher that started this TUI announced the relaunch protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Launcher {
    /// `ORCHBEE_RELAUNCH_V1` names this process's parent.
    Announcing,
    Absent,
}

/// How the process ends after it left for `version`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Departure {
    pub status: u8,
    /// One line for standard error.
    pub line: Option<String>,
}

/// Under an announcing launcher the TUI asks it, by status 75, to start the active version;
/// without one it exits normally and names the command that opens that version, which a macOS TUI
/// needs only when it could not replace itself.
pub fn depart(version: &str, launcher: Launcher, locale: Locale) -> Departure {
    match launcher {
        Launcher::Announcing => Departure {
            status: RELAUNCH_STATUS,
            line: None,
        },
        Launcher::Absent => Departure {
            status: 0,
            line: Some(locale.update_without_launcher(version)),
        },
    }
}

/// Reads the store's pointer every [`STORE_POLL`] on its own thread and keeps the newest reading.
struct StoreWatcher {
    readings: Receiver<Option<ActiveRelease>>,
    // Each message asks for a reading now; dropping the sender ends the thread at its next wait.
    wake: Sender<()>,
    _thread: BackgroundThread,
}

impl StoreWatcher {
    fn start() -> std::io::Result<Self> {
        let (wake, woken) = mpsc::channel::<()>();
        let (send, readings) = mpsc::channel();
        let thread = BackgroundThread::spawn("orchbee-update-store", move || {
            let mut last = None;
            let mut asked = false;
            loop {
                // A reading asked for is sent even when unchanged, so the asker learns it arrived.
                if let Some(reading) = read_store()
                    && (asked || last.as_ref() != Some(&reading))
                {
                    if send.send(reading.clone()).is_err() {
                        return;
                    }
                    last = Some(reading);
                }
                asked = match woken.recv_timeout(STORE_POLL) {
                    Err(RecvTimeoutError::Timeout) => false,
                    Ok(()) => true,
                    Err(RecvTimeoutError::Disconnected) => return,
                };
            }
        })?;
        Ok(Self {
            readings,
            wake,
            _thread: thread,
        })
    }

    /// Asks for a reading now instead of at the next poll.
    fn read_now(&self) {
        let _sent = self.wake.send(());
    }

    /// The newest reading since the last call, if any arrived.
    fn latest(&self) -> Option<Option<ActiveRelease>> {
        self.readings.try_iter().last()
    }
}

/// The active version, or none when the store has no root or pointer; `None` when the pointer
/// cannot be read now, as during its replacement, so the last reading stands until the next poll.
fn read_store() -> Option<Option<ActiveRelease>> {
    let Ok(root) = orchbee_local_runtime::release_store_root(|name| std::env::var_os(name)) else {
        return Some(None);
    };
    orchbee_local_runtime::active_release(&root).ok()
}

mod check;
#[cfg(test)]
mod tests;
