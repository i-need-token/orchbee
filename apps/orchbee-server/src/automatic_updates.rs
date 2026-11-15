//! The Session Server's automatic-update schedule: a Server that runs from a store version folder
//! runs the store's active `orchbee --automatic-update-v1` one minute after the Session is ready and
//! then every hour, reading `update.automatic` before each run. The helper runs in a Job this
//! process holds, at below-normal priority, with bounded output, on the schedule's own thread; no
//! Session work waits on it, its failure only writes the outcome record, and it ends with the
//! Server. Installing, verifying and cleaning up stay with the helper; this owner never touches the
//! store.
use crate::local_terminal_service::LocalTerminalService;
use orchbee_local_runtime::{EnvironmentKind, LogLevel, LogValue, log_event};
use std::{
    path::Path,
    sync::{Arc, Condvar, Mutex, PoisonError},
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

/// The running schedule. Dropping it stops the schedule without waiting.
pub(crate) struct AutomaticUpdates {
    stop: Arc<Stop>,
    thread: Option<JoinHandle<()>>,
}

impl AutomaticUpdates {
    /// Starts the schedule when this Server schedules automatic updates, and records the decision
    /// when the build would schedule at all. None means nothing runs: the build or the Server's
    /// place rules it out, or the schedule's thread could not start.
    pub(crate) fn start(terminals: Arc<LocalTerminalService>) -> Option<Self> {
        let decision = rule(
            orchbee_local_runtime::environment_kind(),
            orchbee_local_runtime::automatic_updates_live(),
            std::env::var_os(FEED_VARIABLE).is_some_and(|value| !value.is_empty()),
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(Path::parent),
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
        let stop = Arc::new(Stop::default());
        let owner = Arc::clone(&stop);
        let spawned = thread::Builder::new()
            .name("orchbee-automatic-update".into())
            .spawn(move || {
                serve(
                    &*owner,
                    || terminals.automatic_updates_enabled(),
                    || helper::run(&owner),
                    Outcome::record,
                );
            });
        match spawned {
            Ok(thread) => {
                record_schedule(true, None);
                Some(Self {
                    stop,
                    thread: Some(thread),
                })
            }
            Err(_) => {
                record_schedule(false, Some("thread-unavailable"));
                None
            }
        }
    }

    /// Stops the schedule and returns at once: no run starts after this, and a running helper is
    /// ended through its Job at the helper's next check, within its settlement budget. The helper
    /// holds nothing a Session stop needs, so the stop never waits on it; a helper still settling
    /// when this process ends is ended by the Job closing with it.
    pub(crate) fn stop(mut self) {
        self.stop.request();
        if let Some(thread) = self.thread.take_if(|thread| thread.is_finished()) {
            // A panic already reached the log through the panic hook; nothing is left to settle.
            let _ = thread.join();
        }
    }
}

impl Drop for AutomaticUpdates {
    fn drop(&mut self) {
        self.stop.request();
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

/// Time as the schedule sees it, and its owner's stop.
trait Pace {
    fn now(&self) -> Instant;
    /// Waits until `due` and answers true, or answers false once the owner has stopped.
    fn wait_until(&self, due: Instant) -> bool;
}

/// Runs the schedule until `pace` reports the stop: the first moment [`FIRST_RUN`] after it starts,
/// then one every [`PERIOD`] on the same grid. Each moment reads `enabled` and runs nothing while it
/// is off. A run happens on this thread and returns before the next moment is chosen, so runs never
/// overlap, and a moment that passes during a run is skipped rather than caught up.
fn serve(
    pace: &impl Pace,
    enabled: impl Fn() -> bool,
    mut run: impl FnMut() -> Outcome,
    mut record: impl FnMut(&Outcome),
) {
    let mut due = pace.now() + FIRST_RUN;
    while pace.wait_until(due) {
        if enabled() {
            record(&run());
        }
        due = next_moment(due, pace.now());
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

/// The schedule's stop. A poisoned lock reads as stopped, so a broken owner never starts a run.
#[derive(Default)]
struct Stop {
    stopped: Mutex<bool>,
    changed: Condvar,
}

impl Stop {
    fn request(&self) {
        *self.stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.changed.notify_all();
    }

    fn requested(&self) -> bool {
        self.stopped.lock().map_or(true, |stopped| *stopped)
    }
}

impl Pace for Stop {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn wait_until(&self, due: Instant) -> bool {
        let Ok(mut stopped) = self.stopped.lock() else {
            return false;
        };
        loop {
            if *stopped {
                return false;
            }
            let now = Instant::now();
            if now >= due {
                return true;
            }
            stopped = match self.changed.wait_timeout(stopped, due - now) {
                Ok((stopped, _)) => stopped,
                Err(_) => return false,
            };
        }
    }
}

#[cfg(test)]
mod tests;
