//! The Session Server's automatic-update schedule: a Server that runs from a store version folder
//! runs the store's active `orchbee --automatic-update-v1` one minute after the Session is ready and
//! then every hour, reading `update.automatic` before each run. The helper runs at low priority, in
//! a Job this process holds on Windows and in a process group of its own on macOS, with bounded
//! output, on the schedule's own thread; no Session work waits on it, its failure only writes the
//! outcome record, and it ends with the Server. Installing, verifying and cleaning up stay with the
//! helper; this owner never touches the store. An admitted Client's `workbench.update-check` asks
//! for a run now through the shared [`UpdateJob`]: this thread runs it as it runs a scheduled one,
//! whatever `update.automatic` says, and the hourly grid stays. It also answers the
//! [idle move](crate::idle_move) whether the helper runs and whether the store's active version has
//! passed this Server.
use crate::{
    local_terminal_service::LocalTerminalService,
    update_job::{UpdateJob, Wake},
};
use orchbee_local_runtime::{EnvironmentKind, LogLevel, LogValue, log_event};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod helper;
mod outcome;
use outcome::Outcome;

/// The wait from the Session's ready moment to the first run.
const FIRST_RUN: Duration = Duration::from_secs(60);
/// The wait from one scheduled moment to the next.
const PERIOD: Duration = Duration::from_secs(60 * 60);
/// The variable that names another update feed; a test build schedules only while it is set.
const FEED_VARIABLE: &str = "ORCHBEE_UPDATE_FEED";
/// The longest a macOS stop waits for a running helper's process group to settle: the settlement's
/// own budget, which ends the group with `SIGKILL`, with room to spare.
#[cfg(target_os = "macos")]
const HELPER_SETTLEMENT: Duration = Duration::from_secs(6);

/// The running schedule. Dropping it stops the schedule without waiting.
pub(crate) struct AutomaticUpdates {
    job: Arc<UpdateJob>,
    thread: Option<JoinHandle<()>>,
    /// Whether the helper runs now.
    helper: Arc<AtomicBool>,
    /// The name of the store version folder this Server runs from.
    own_version: String,
}

impl AutomaticUpdates {
    /// Starts the schedule on `job` when this Server schedules automatic updates, and records the
    /// decision when the build would schedule at all. None means nothing runs: the build or the
    /// Server's place rules it out, or the schedule's thread could not start; `job` then answers
    /// every Client as unavailable.
    pub(crate) fn start(terminals: Arc<LocalTerminalService>, job: Arc<UpdateJob>) -> Option<Self> {
        let started = Self::schedule(terminals, Arc::clone(&job));
        if started.is_none() {
            job.end();
        }
        started
    }

    fn schedule(terminals: Arc<LocalTerminalService>, job: Arc<UpdateJob>) -> Option<Self> {
        let own_directory = std::env::current_exe()
            .ok()
            .and_then(|executable| executable.parent().map(Path::to_path_buf));
        let decision = rule(
            orchbee_local_runtime::environment_kind(),
            orchbee_local_runtime::automatic_updates_live(),
            std::env::var_os(FEED_VARIABLE).is_some_and(|value| !value.is_empty()),
            own_directory.as_deref(),
        );
        match decision {
            Decision::Scheduled => {}
            Decision::Unscheduled(Unscheduled::OutsideStore) => {
                record_schedule(false, Some("outside-store"));
                return None;
            }
            // A development build, a test build without a feed and a release build before the
            // switch leave no trace, so a dark build writes nothing about automatic updates.
            Decision::Unscheduled(_) => return None,
        }
        // The rule schedules only a Server that runs from a store version folder, which a valid
        // version names.
        let own_version = own_directory
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())?
            .to_owned();
        let owner = Arc::clone(&job);
        let helper = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&helper);
        let spawned = thread::Builder::new()
            .name("orchbee-automatic-update".into())
            .spawn(move || {
                // However the thread ends, a panic included, no Client waits on a run that will
                // never start.
                let _ended = Ended(&owner);
                serve(
                    &Schedule(&owner),
                    || terminals.automatic_updates_enabled(),
                    || {
                        // A helper that panics leaves the flag set, so the Session never moves
                        // under a helper whose end nothing observed.
                        running.store(true, Ordering::Release);
                        owner.begin();
                        let outcome = helper::run(&owner);
                        owner.finish(outcome.finish());
                        running.store(false, Ordering::Release);
                        outcome
                    },
                    Outcome::record,
                );
            });
        match spawned {
            Ok(thread) => {
                record_schedule(true, None);
                Some(Self {
                    job,
                    thread: Some(thread),
                    helper,
                    own_version,
                })
            }
            Err(_) => {
                record_schedule(false, Some("thread-unavailable"));
                None
            }
        }
    }

    /// Stops the schedule: no run starts after this, and a running helper is ended at the
    /// schedule's next check, within its settlement budget. The helper holds nothing a Session
    /// stop needs, so the stop never fails because of it. On Windows it returns at once, as a
    /// helper still settling when this process ends is ended by the Job closing with it. macOS has
    /// no such Job, so there it waits up to [`HELPER_SETTLEMENT`] for the helper's group to settle
    /// before the Server goes on.
    pub(crate) fn stop(mut self) {
        self.job.end();
        #[cfg(target_os = "macos")]
        if let Some(thread) = &self.thread {
            let settled_by = Instant::now() + HELPER_SETTLEMENT;
            while !thread.is_finished() && Instant::now() < settled_by {
                thread::sleep(Duration::from_millis(10));
            }
        }
        if let Some(thread) = self.thread.take_if(|thread| thread.is_finished()) {
            // A panic already reached the log through the panic hook; nothing is left to settle.
            let _ = thread.join();
        }
    }

    /// Whether the helper runs now. It holds the active version folder and may start its Client.
    pub(crate) fn helper_running(&self) -> bool {
        self.helper.load(Ordering::Acquire)
    }

    /// The store's active version, found as a run and the next Client start find it, when it is
    /// newer than the version folder this Server runs from. A store whose active version cannot be
    /// read answers `None`.
    pub(crate) fn superseding(&self) -> Option<String> {
        let root = orchbee_local_runtime::release_store_root(|name| std::env::var_os(name)).ok()?;
        orchbee_local_runtime::active_release(&root)
            .ok()
            .flatten()
            .map(|active| active.version)
            .filter(|version| crate::idle_move::older(&self.own_version, version))
    }
}

impl Drop for AutomaticUpdates {
    fn drop(&mut self) {
        self.job.end();
    }
}

/// Ends the job when the schedule's thread ends.
struct Ended<'a>(&'a UpdateJob);

impl Drop for Ended<'_> {
    fn drop(&mut self) {
        self.0.end();
    }
}

/// Whether a Server schedules automatic updates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    Scheduled,
    Unscheduled(Unscheduled),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unscheduled {
    /// A development build never schedules.
    Development,
    /// A test build schedules only while [`FEED_VARIABLE`] names a feed.
    NoFeed,
    /// A release build schedules only once the automatic-update switch is on.
    SwitchedOff,
    /// The Server does not run from a store version folder, so there is no store it belongs to.
    OutsideStore,
}

/// The schedule rule over the build's environment, the automatic-update switch, whether a feed is
/// named, and the folder this Server runs from.
fn rule(kind: EnvironmentKind, live: bool, feed: bool, own_directory: Option<&Path>) -> Decision {
    let refusal = match kind {
        EnvironmentKind::Development => Some(Unscheduled::Development),
        EnvironmentKind::Test if !feed => Some(Unscheduled::NoFeed),
        _ if !live => Some(Unscheduled::SwitchedOff),
        _ if !own_directory.is_some_and(orchbee_local_runtime::is_release_version_directory) => {
            Some(Unscheduled::OutsideStore)
        }
        _ => None,
    };
    refusal.map_or(Decision::Scheduled, Decision::Unscheduled)
}

fn record_schedule(scheduled: bool, reason: Option<&'static str>) {
    let mut fields = vec![("scheduled", LogValue::Flag(scheduled))];
    if let Some(reason) = reason {
        fields.push(("reason", LogValue::Token(reason)));
    }
    log_event(LogLevel::Info, "automatic-update-schedule", &fields);
}

/// Time as the schedule sees it, its owner's stop, and Clients' requests.
trait Pace {
    fn now(&self) -> Instant;
    /// Waits until `due` or a Client's request and answers which came first, or `None` once the
    /// owner has stopped.
    fn wait_until(&self, due: Instant) -> Option<Wake>;
}

/// Runs the schedule until `pace` reports the stop: the first moment [`FIRST_RUN`] after it starts,
/// then one every [`PERIOD`] on the same grid. Each moment reads `enabled` and runs nothing while it
/// is off; a Client's request runs at once whatever `enabled` says and leaves the grid as it was. A
/// run happens on this thread and returns before the next wait, so runs never overlap, and a moment
/// that passes during a run is skipped rather than caught up.
fn serve(
    pace: &impl Pace,
    enabled: impl Fn() -> bool,
    mut run: impl FnMut() -> Outcome,
    mut record: impl FnMut(&Outcome),
) {
    let mut due = pace.now() + FIRST_RUN;
    while let Some(wake) = pace.wait_until(due) {
        if wake == Wake::Requested || enabled() {
            record(&run());
        }
        let now = pace.now();
        if now >= due {
            due = next_moment(due, now);
        }
    }
}

/// The first moment on the grid of `due` that lies after `now`.
fn next_moment(due: Instant, now: Instant) -> Instant {
    let mut next = due + PERIOD;
    while next <= now {
        next += PERIOD;
    }
    next
}

/// The schedule's clock over the shared job, whose end is the schedule's stop.
struct Schedule<'a>(&'a UpdateJob);

impl Pace for Schedule<'_> {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn wait_until(&self, due: Instant) -> Option<Wake> {
        self.0.wait_until(due)
    }
}

#[cfg(test)]
mod tests;
