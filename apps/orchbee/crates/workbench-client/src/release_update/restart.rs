//! A click's Session restart for the active version, which keeps every terminal. The newer side
//! decides: the active version's own `orchbee` reads the Session's restart tier with
//! `status server`, and when that tier, or a failed join, asks for it, restarts the Session's
//! Server and Worker with `server restart`. Both run on one background thread, each as a
//! child without a window, bounded in time. A Terminal Host that version's Worker cannot adopt rules
//! out a restart that keeps terminals: the person confirms first, and the confirmed replacement stops
//! the Session through this TUI's own connection, so the active version starts it again with every
//! terminal restarted from the Session's final capture. A Host that version must replace but whose
//! terminals its Worker moves asks only once the moves settle, about the terminals left on it.
//!
//! A TUI that joined a Session running an older version moves it onto its own version the same
//! way, with its own program and whatever the Session's tier says, since that tier reads the
//! store's active version, which a macOS bundle may still be installing. A Host that version must
//! replace asks first, and the confirmed replacement starts the Session again with this program.
use super::SessionWait;
use crate::background_thread::BackgroundThread;
use orchbee_local_client_protocol::{
    SessionStopRequest, SessionStopStatus, StopAction, StopId, exchange_session_stop_before,
};
use orchbee_local_runtime::{
    ActivationState, ActiveRelease, Cohort, ModeCheckedLocalSessionRendezvous,
    SessionLifetimeObservation, SessionScope,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

/// How long the tier read may take.
const STATUS_BUDGET: Duration = Duration::from_secs(15);
/// How long the restart may take: its successor's own budget and the wait for its endpoint.
const RESTART_BUDGET: Duration = Duration::from_secs(120);
/// The most standard output read from either command; a status document is a few kilobytes.
const OUTPUT_LIMIT: u64 = 256 * 1024;
/// How long the stop's request may take to be answered.
const STOP_REQUEST_BUDGET: Duration = Duration::from_secs(10);
/// How long a replacement waits for the stop to end: the Server spends at most 60 seconds on one
/// attempt, then writes its record.
const STOP_BUDGET: Duration = Duration::from_secs(75);
/// How often the moves off a Host the active version must replace are read: the Worker moves one
/// idle terminal per two-second turn.
const SETTLE_PERIOD: Duration = Duration::from_secs(2);
/// How long the count of terminals left on such Hosts stays unchanged before the rest count as
/// unable to move.
const SETTLE_STEADY: Duration = Duration::from_secs(6);
/// How long the moves may take before the terminals left are asked about.
const SETTLE_BUDGET: Duration = Duration::from_secs(180);

/// What the job reports, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    /// The restart started; the Session is unavailable for a moment.
    Restarting,
    /// The new version can serve this TUI: leave for it.
    Leave,
    /// Nothing changed; the update waits for the Session to move.
    Waiting,
    /// The Session refused to move onto this Client's version, for the reason named; only an
    /// older Session's move reports it.
    Refused(SessionWait),
    /// A Terminal Host of the Session must be replaced, which restarts its terminals: the person
    /// confirms first. `None` before the restart, when the active version's Worker cannot adopt
    /// it; `Some` counts the terminals that could not move from it once the Session runs that
    /// version.
    HostRequired(Option<u32>),
}

pub(super) struct Job {
    progress: Receiver<Progress>,
    _thread: BackgroundThread,
}

impl Job {
    /// Starts the job for the Session `session` with the active version's program, which shares
    /// this program's file name; `None` when that program is missing, so the click decides as
    /// before. `joins` is whether that version can join the Session at all.
    pub(super) fn start(active: &ActiveRelease, session: String, joins: bool) -> Option<Self> {
        let name = std::env::current_exe().ok()?.file_name()?.to_owned();
        let executable = active.directory.join(name);
        if !executable.is_file() {
            return None;
        }
        Self::spawn(move |report| run(&executable, &session, joins, report))
    }

    /// Starts the confirmed Host replacement: stops the Session whose runtime folder is `runtime`
    /// through this TUI's own connection and reports [`Progress::Leave`] once the stop ended, so the
    /// active version starts the Session again, or [`Progress::Waiting`] when it did not.
    pub(super) fn replace_host(runtime: PathBuf) -> Option<Self> {
        Self::spawn(move |report| {
            report(Progress::Restarting);
            report(if stop(&runtime) {
                Progress::Leave
            } else {
                Progress::Waiting
            });
        })
    }

    /// Starts the move of the Session `session`, which runs an older version, onto this Client's
    /// own version: [`Progress::Restarting`] at once, then [`Progress::Leave`] once it runs this
    /// version, [`Progress::HostRequired`] when a Host must be replaced first, or
    /// [`Progress::Refused`].
    pub(super) fn move_session(session: &str) -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        let session = session.to_owned();
        Self::spawn(move |report| run_move(&executable, &session, report))
    }

    /// Starts the confirmed Host replacement for an older Session's move: stops the Session whose
    /// runtime folder is `runtime`, named `session`, as [`Self::replace_host`] does, then starts it
    /// with this Client's program, so it runs this version with every terminal restarted. This
    /// TUI's connection waits for that Server and joins it.
    pub(super) fn replace_host_for_move(runtime: PathBuf, session: String) -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        Self::spawn(move |report| {
            report(Progress::Restarting);
            let started = stop(&runtime)
                && command_json(
                    &executable,
                    &session,
                    &["server", "start", "--format", "json"],
                    RESTART_BUDGET,
                )
                .is_some_and(|document| {
                    matches!(
                        document["result"]["outcome"].as_str(),
                        Some("started" | "already-running")
                    )
                });
            report(if started {
                Progress::Leave
            } else {
                Progress::Refused(SessionWait::Other)
            });
        })
    }

    /// Reports [`Progress::Leave`] at once: the person kept the terminals that could not move on
    /// their Host, and the TUI leaves for the version the Session runs.
    pub(super) fn leave() -> Option<Self> {
        Self::spawn(|report| report(Progress::Leave))
    }

    /// Runs `steps` on the job's thread; each report reaches the TUI as it is made, so Restarting
    /// shows while the restart or stop runs. A TUI that dropped the job hears nothing more.
    fn spawn(steps: impl FnOnce(&dyn Fn(Progress)) + Send + 'static) -> Option<Self> {
        let (send, progress) = mpsc::channel();
        let thread = BackgroundThread::spawn("orchbee-update-restart", move || {
            steps(&|step| {
                let _ = send.send(step);
            });
        })
        .ok()?;
        Some(Self {
            progress,
            _thread: thread,
        })
    }

    /// The next report, once; a job that ended without a last word waits.
    pub(super) fn poll(&self) -> Option<Progress> {
        match self.progress.try_recv() {
            Ok(progress) => Some(progress),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Progress::Waiting),
        }
    }
}

/// Reports one job's steps. A Host whose own tier is `required` and whose terminals the active
/// version's Worker does not move asks the person first, since the restart's successor could not
/// adopt it. A Session the active version joins without a tier, or whose tier does not read,
/// switches as before; a restart that does not answer `restarted` changes nothing the TUI can use,
/// so the update waits. Once the Session runs that version, a `required` Host whose terminals move
/// is watched until the moves settle, and the terminals left on it are asked about.
fn run(executable: &Path, session: &str, joins: bool, report: &dyn Fn(Progress)) {
    let status = command_json(
        executable,
        session,
        &["status", "server", "--format", "json"],
        STATUS_BUDGET,
    );
    let restart = status
        .as_ref()
        .map(|document| &document["result"]["server"]["restart"]);
    let hosts = restart.and_then(|restart| restart["hosts"].as_array());
    if hosts.is_some_and(|hosts| {
        hosts
            .iter()
            .any(|host| host["tier"] == "required" && host["moves"] != true)
    }) {
        report(Progress::HostRequired(None));
        return;
    }
    let moving = restart.is_some_and(|restart| staying(restart) > 0);
    if joins {
        let needed = restart
            .and_then(|restart| restart["tier"].as_str())
            .is_some_and(|tier| matches!(tier, "recommended" | "required"));
        if !needed {
            report(if moving {
                settle(executable, session)
            } else {
                Progress::Leave
            });
            return;
        }
    }
    report(Progress::Restarting);
    let restarted = command_json(
        executable,
        session,
        &["server", "restart", "--format", "json"],
        RESTART_BUDGET,
    )
    .is_some_and(|document| document["result"]["outcome"] == "restarted");
    report(match (restarted, moving) {
        (false, _) => Progress::Waiting,
        (true, false) => Progress::Leave,
        (true, true) => settle(executable, session),
    });
}

/// Reports one older Session's move onto this Client's version. A Host this version's Worker
/// cannot adopt asks the person first; otherwise the Session restarts whatever its tier says, and
/// the move ends once the restart answers, since the terminals its Worker could not move from a
/// Host keep running there.
fn run_move(executable: &Path, session: &str, report: &dyn Fn(Progress)) {
    report(Progress::Restarting);
    let status = command_json(
        executable,
        session,
        &["status", "server", "--format", "json"],
        STATUS_BUDGET,
    );
    let hosts = status
        .as_ref()
        .and_then(|document| document["result"]["server"]["restart"]["hosts"].as_array());
    if hosts.is_some_and(|hosts| {
        hosts
            .iter()
            .any(|host| host["tier"] == "required" && host["moves"] != true)
    }) {
        report(Progress::HostRequired(None));
        return;
    }
    let answer = command_json(
        executable,
        session,
        &["server", "restart", "--format", "json"],
        RESTART_BUDGET,
    );
    let result = answer.as_ref().map(|document| &document["result"]);
    report(
        match (
            result.and_then(|result| result["outcome"].as_str()),
            result.and_then(|result| result["reason"].as_str()),
        ) {
            (Some("restarted"), _) => Progress::Leave,
            (Some("refused"), Some("background-programs")) => {
                Progress::Refused(SessionWait::BackgroundPrograms)
            }
            (Some("refused"), Some("clients")) => Progress::Refused(SessionWait::Clients),
            _ => Progress::Refused(SessionWait::Other),
        },
    );
}

/// Watches the Session's Worker move terminals off the Hosts the active version must replace:
/// reads `status server` every [`SETTLE_PERIOD`] until those Hosts run no terminal, which leaves,
/// or their count has not changed for [`SETTLE_STEADY`], or [`SETTLE_BUDGET`] passed, which asks
/// about the terminals left. A read that fails changes nothing.
fn settle(executable: &Path, session: &str) -> Progress {
    let began = Instant::now();
    let mut last: Option<(u32, Instant)> = None;
    loop {
        let read = command_json(
            executable,
            session,
            &["status", "server", "--format", "json"],
            STATUS_BUDGET,
        )
        .and_then(|document| {
            let restart = &document["result"]["server"]["restart"];
            restart.is_object().then(|| staying(restart))
        });
        let now = Instant::now();
        match (read, last) {
            (Some(0), _) => return Progress::Leave,
            (Some(count), Some((previous, since))) if count == previous => {
                if now.saturating_duration_since(since) >= SETTLE_STEADY {
                    return Progress::HostRequired(Some(count));
                }
            }
            (Some(count), _) => last = Some((count, now)),
            (None, _) => {}
        }
        if now.saturating_duration_since(began) >= SETTLE_BUDGET {
            return last.map_or(Progress::Leave, |(count, _)| {
                Progress::HostRequired(Some(count))
            });
        }
        thread::sleep(SETTLE_PERIOD);
    }
}

/// The terminals the Hosts `restart` names `required` run.
fn staying(restart: &serde_json::Value) -> u32 {
    restart["hosts"].as_array().map_or(0, |hosts| {
        hosts
            .iter()
            .filter(|host| host["tier"] == "required")
            .filter_map(|host| host["terminals"].as_u64())
            .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
            .fold(0, u32::saturating_add)
    })
}

/// Stops the Session whose runtime folder is `runtime` as a TUI's stop does, under a fresh
/// identity, and waits until its record says that stop ended and its lifetime lock is free: the
/// final capture is written and a start may run. False when the stop was not recorded, failed or
/// outlived its bound.
fn stop(runtime: &Path) -> bool {
    let deadline = Instant::now() + STOP_BUDGET;
    let (Some(base), Some(scope)) = (
        runtime.parent(),
        runtime
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| SessionScope::parse(name).ok()),
    ) else {
        return false;
    };
    let Ok(session) = ModeCheckedLocalSessionRendezvous::validate_mode_checked(base, scope) else {
        return false;
    };
    let Ok(cohort) = Cohort::current(deadline) else {
        return false;
    };
    let mut bytes = [0; 16];
    let Some(id) = getrandom::fill(&mut bytes)
        .ok()
        .and_then(|()| StopId::new(bytes).ok())
    else {
        return false;
    };
    let offer = crate::local_client::client_offer(Some(cohort.identity()));
    let request = SessionStopRequest {
        id,
        action: StopAction::Start,
    };
    match exchange_session_stop_before(
        session.client_socket_path(),
        &offer,
        request,
        Instant::now() + STOP_REQUEST_BUDGET,
        None,
    ) {
        Ok(result) if matches!(result.status, SessionStopStatus::Pending { .. }) => {}
        _ => return false,
    }
    loop {
        let stopped = matches!(
            session.activation_state(),
            Ok(ActivationState::Stopped { request_id }) if request_id == id.bytes()
        ) && matches!(
            session.observe_session_lifetime_lock(),
            Ok(SessionLifetimeObservation::Free)
        );
        if stopped {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Runs `executable --session <session> <arguments>` without a window or input and reads its
/// standard output as one JSON document; `None` when it cannot start, outlives `budget`, which
/// ends it, or prints no document.
fn command_json(
    executable: &Path,
    session: &str,
    arguments: &[&str],
    budget: Duration,
) -> Option<serde_json::Value> {
    let mut command = Command::new(executable);
    command
        .arg("--session")
        .arg(session)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let reader = thread::Builder::new()
        .name("orchbee-update-output".to_owned())
        .spawn(move || {
            let mut bytes = Vec::new();
            let _read = stdout.take(OUTPUT_LIMIT).read_to_end(&mut bytes);
            bytes
        })
        .ok();
    let deadline = Instant::now() + budget;
    let finished = loop {
        match child.try_wait() {
            Ok(Some(_)) => break true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            _ => {
                let _killed = child.kill();
                let _waited = child.wait();
                break false;
            }
        }
    };
    let bytes = reader?.join().ok()?;
    finished
        .then(|| serde_json::from_slice(&bytes).ok())
        .flatten()
}
