//! The TUI's Update control under the automatic-update proposal: whether the release store names a
//! version newer than this Client active, whether that version can join the running Session, and
//! how the TUI leaves for it. The store is read on a background thread; the switch itself is the
//! TUI's exit: a launcher that announced the relaunch protocol starts the active version; without
//! one a macOS TUI replaces itself with that version, and elsewhere the TUI names the command that
//! opens it. On Windows and macOS a click first restarts the Session for the active version when
//! that version asks for it, keeping every terminal; see [`restart`]. There, too, a TUI that joined
//! a Session running an older version moves that Session onto its own version by itself, keeping
//! every terminal, and Update offers the move again only when it could not happen. About's Check
//! for updates asks the Session Server to run its update job now; see [`check`].

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
}

impl Build {
    const fn current() -> Self {
        Self {
            live: orchbee_local_runtime::automatic_updates_live(),
            kind: orchbee_local_runtime::environment_kind(),
        }
    }

    /// Whether any store state could make the control Ready: never behind the switch or in a
    /// development build, which has no store.
    const fn offers_updates(self) -> bool {
        self.live && !matches!(self.kind, EnvironmentKind::Development)
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

/// Whether Update offering `version` moves the Session rather than switching this Client: only
/// that move offers this Client's own version, since a version to switch to is always newer.
pub(crate) fn moves_session(version: &str) -> bool {
    version == crate::local_client::CLIENT_VERSION
}

/// Why the move of an older Session onto this Client's version waits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionWait {
    /// A configured command or plugin job runs in the Session, which the move would end.
    BackgroundPrograms,
    /// An older Client that cannot come back to a new Server by itself is connected.
    Clients,
    /// A Terminal Host must be replaced, which restarts every terminal: the person decides.
    HostRequired,
    /// Another reason, such as the restart failing.
    Other,
}

/// What a click on Update came to, or what its restart reached since.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateSwitch {
    /// The control is no longer Ready, so the click is refused.
    Unavailable,
    /// The active version cannot join this Session yet; the Session must move first.
    Waiting,
    /// The TUI leaves for the active version.
    Leave,
    /// The active version decides on a restart first; its progress follows.
    Started,
    /// The Session's Server and Worker restart for the active version, keeping every terminal.
    Restarting,
    /// A Terminal Host of the Session must be replaced, which restarts the terminals: the TUI asks
    /// the person, and only a confirmed [`Updater::replace_host`] goes on. `Some` counts the
    /// terminals that could not move once the Session runs the new version.
    HostReplacement(Option<u32>),
    /// The Session runs an older version than this Client, which moves it onto its own version,
    /// keeping every terminal.
    SessionUpdating,
    /// The Session runs this Client's version now.
    SessionUpdated,
    /// The Session still runs an older version; Update offers the move again.
    SessionWaiting(SessionWait),
}

/// The move of an older Session onto this Client's version: made once by itself, then on a click.
#[cfg(any(windows, target_os = "macos"))]
#[derive(Default)]
enum SessionMove {
    /// No move began: the Session is not known to run an older version.
    #[default]
    Untried,
    Running(restart::Job),
    /// The move could not happen, so Update offers it.
    Waiting(SessionWait),
    /// A click asked the person whether to restart every terminal.
    Asking,
    /// The Session runs this Client's version.
    Done,
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
    /// The restart a click started, with the version it leaves for.
    #[cfg(any(windows, target_os = "macos"))]
    restart: Option<(String, restart::Job)>,
    /// The version a Host replacement leaves for while the person has not answered, and whether
    /// the Session already runs it.
    #[cfg(any(windows, target_os = "macos"))]
    replacing: Option<(String, bool)>,
    #[cfg(any(windows, target_os = "macos"))]
    session_move: SessionMove,
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
            #[cfg(any(windows, target_os = "macos"))]
            restart: None,
            #[cfg(any(windows, target_os = "macos"))]
            replacing: None,
            #[cfg(any(windows, target_os = "macos"))]
            session_move: SessionMove::Untried,
        }
    }

    /// A Ready control for `version` without a watcher, as a test's store would show it.
    #[cfg(test)]
    pub(crate) fn ready(version: &str, generations: ClientGenerations) -> Self {
        let mut updater = Self {
            build: Build {
                live: true,
                kind: EnvironmentKind::Test,
            },
            watcher: None,
            active: None,
            control: UpdateControl::Absent,
            generations,
            leaving: UpdateDeparture::default(),
            check: check::Check::default(),
            store_read: false,
            #[cfg(any(windows, target_os = "macos"))]
            restart: None,
            #[cfg(any(windows, target_os = "macos"))]
            replacing: None,
            #[cfg(any(windows, target_os = "macos"))]
            session_move: SessionMove::Untried,
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

    /// The version Update offers: the store's active version while the control is Ready, else,
    /// while the move of an older Session waits, this Client's own version, which only the Session
    /// changes to.
    pub(crate) fn offered(&self) -> Option<&str> {
        if let UpdateControl::Ready { version } = &self.control {
            return Some(version);
        }
        #[cfg(any(windows, target_os = "macos"))]
        if matches!(self.session_move, SessionMove::Waiting(_)) {
            return Some(crate::local_client::CLIENT_VERSION);
        }
        None
    }

    /// Moves the local Session `session` onto this Client's version by itself, once, when it runs
    /// `older`, an older version; [`Self::take_restart`] reports the move. Windows and macOS only,
    /// where a restart keeps every terminal.
    pub(crate) fn move_older_session(&mut self, older: bool, session: Option<&str>) {
        #[cfg(any(windows, target_os = "macos"))]
        if older
            && matches!(self.session_move, SessionMove::Untried)
            && let Some(job) = session.and_then(restart::Job::move_session)
        {
            self.session_move = SessionMove::Running(job);
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = (older, session);
    }

    /// Runs a click against the current control and the Session's handshake answer. On Windows,
    /// with the local Session `session` named, the active version decides on a restart first and
    /// [`Self::take_restart`] reports what followed; a click during that restart changes nothing.
    /// A switch records the version the TUI leaves for in its [`UpdateDeparture`].
    pub(crate) fn switch(
        &mut self,
        marker: bool,
        negotiated: Option<Generation>,
        session: Option<&str>,
    ) -> UpdateSwitch {
        #[cfg(any(windows, target_os = "macos"))]
        if matches!(
            self.session_move,
            SessionMove::Running(_) | SessionMove::Asking
        ) {
            return UpdateSwitch::Started;
        }
        let (UpdateControl::Ready { version }, Some(active)) = (&self.control, &self.active) else {
            return self.switch_session(session);
        };
        let joins = joins(marker, negotiated, (self.generations)(active));
        #[cfg(any(windows, target_os = "macos"))]
        {
            if self.restart.is_some() || self.replacing.is_some() {
                return UpdateSwitch::Started;
            }
            if let Some(job) =
                session.and_then(|session| restart::Job::start(active, session.to_owned(), joins))
            {
                self.restart = Some((version.clone(), job));
                return UpdateSwitch::Started;
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = session;
        if !joins {
            return UpdateSwitch::Waiting;
        }
        self.leaving.set(version.clone());
        UpdateSwitch::Leave
    }

    /// A click while no newer version is offered: moves an older Session again when its move
    /// waits, asking first when that restarts every terminal.
    fn switch_session(&mut self, session: Option<&str>) -> UpdateSwitch {
        #[cfg(any(windows, target_os = "macos"))]
        match self.session_move {
            SessionMove::Waiting(SessionWait::HostRequired) => {
                self.session_move = SessionMove::Asking;
                return UpdateSwitch::HostReplacement(None);
            }
            SessionMove::Waiting(_) => {
                if let Some(job) = session.and_then(restart::Job::move_session) {
                    self.session_move = SessionMove::Running(job);
                    return UpdateSwitch::Started;
                }
            }
            SessionMove::Untried
            | SessionMove::Running(_)
            | SessionMove::Asking
            | SessionMove::Done => {}
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        let _ = session;
        UpdateSwitch::Unavailable
    }

    /// The next report of the restart a click started, once: [`UpdateSwitch::Restarting`] when it
    /// began, then [`UpdateSwitch::Leave`] with the version recorded, or [`UpdateSwitch::Waiting`]
    /// when nothing changed; else the next report of an older Session's move.
    pub(crate) fn take_restart(&mut self) -> Option<UpdateSwitch> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            let Some((_, job)) = &self.restart else {
                return self.take_session_move();
            };
            let progress = job.poll()?;
            Some(match progress {
                restart::Progress::Restarting => UpdateSwitch::Restarting,
                restart::Progress::Leave => {
                    let (version, _) = self.restart.take()?;
                    self.leaving.set(version);
                    UpdateSwitch::Leave
                }
                restart::Progress::Waiting | restart::Progress::Refused(_) => {
                    self.restart = None;
                    UpdateSwitch::Waiting
                }
                restart::Progress::HostRequired(staying) => {
                    let (version, _) = self.restart.take()?;
                    self.replacing = Some((version, staying.is_some()));
                    UpdateSwitch::HostReplacement(staying)
                }
            })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        None
    }

    /// The next report of an older Session's move, once. A move that left terminals on a Host it
    /// must replace still moved the Session; those terminals keep running on their Host.
    #[cfg(any(windows, target_os = "macos"))]
    fn take_session_move(&mut self) -> Option<UpdateSwitch> {
        let SessionMove::Running(job) = &self.session_move else {
            return None;
        };
        let (next, report) = match job.poll()? {
            restart::Progress::Restarting => return Some(UpdateSwitch::SessionUpdating),
            restart::Progress::Leave | restart::Progress::HostRequired(Some(_)) => {
                (SessionMove::Done, UpdateSwitch::SessionUpdated)
            }
            restart::Progress::HostRequired(None) => (
                SessionMove::Waiting(SessionWait::HostRequired),
                UpdateSwitch::SessionWaiting(SessionWait::HostRequired),
            ),
            restart::Progress::Refused(wait) => (
                SessionMove::Waiting(wait),
                UpdateSwitch::SessionWaiting(wait),
            ),
            restart::Progress::Waiting => (
                SessionMove::Waiting(SessionWait::Other),
                UpdateSwitch::SessionWaiting(SessionWait::Other),
            ),
        };
        self.session_move = next;
        Some(report)
    }

    /// The person's answer to [`UpdateSwitch::HostReplacement`]. Confirmed, it stops the local
    /// Session whose runtime folder is `runtime` and reports through [`Self::take_restart`] as a
    /// restart does. Declined once the Session runs the new version, the terminals that could not
    /// move stay on their Host and the TUI leaves for that version; declined before, or with no
    /// question asked, nothing changes and the update waits. Answers [`UpdateSwitch::Started`]
    /// when a job started, else what to say instead.
    pub(crate) fn replace_host(
        &mut self,
        confirmed: bool,
        runtime: Option<&std::path::Path>,
    ) -> UpdateSwitch {
        #[cfg(any(windows, target_os = "macos"))]
        {
            if matches!(self.session_move, SessionMove::Asking) {
                return self.replace_host_for_move(confirmed, runtime);
            }
            let Some((version, restarted)) = self.replacing.take() else {
                return UpdateSwitch::Waiting;
            };
            if !confirmed && restarted {
                let Some(job) = restart::Job::leave() else {
                    return UpdateSwitch::Waiting;
                };
                self.restart = Some((version, job));
                return UpdateSwitch::Started;
            }
            let Some(job) = runtime
                .filter(|_| confirmed)
                .and_then(|runtime| restart::Job::replace_host(runtime.to_owned()))
            else {
                return UpdateSwitch::Waiting;
            };
            self.restart = Some((version, job));
            UpdateSwitch::Started
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (confirmed, runtime);
            UpdateSwitch::Waiting
        }
    }

    /// The person's answer when moving an older Session restarts every terminal. Confirmed, it
    /// stops the local Session whose runtime folder is `runtime` and starts it again with this
    /// Client's version; declined, or when that cannot begin, the move waits.
    #[cfg(any(windows, target_os = "macos"))]
    fn replace_host_for_move(
        &mut self,
        confirmed: bool,
        runtime: Option<&std::path::Path>,
    ) -> UpdateSwitch {
        let job = runtime.filter(|_| confirmed).and_then(|runtime| {
            let session = runtime.file_name()?.to_str()?.to_owned();
            restart::Job::replace_host_for_move(runtime.to_owned(), session)
        });
        let Some(job) = job else {
            self.session_move = SessionMove::Waiting(SessionWait::HostRequired);
            return UpdateSwitch::SessionWaiting(SessionWait::HostRequired);
        };
        self.session_move = SessionMove::Running(job);
        UpdateSwitch::Started
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
    /// `ORCHBEE_RELAUNCH` names this process's parent.
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
#[cfg(any(windows, target_os = "macos"))]
mod restart;
#[cfg(test)]
mod tests;
