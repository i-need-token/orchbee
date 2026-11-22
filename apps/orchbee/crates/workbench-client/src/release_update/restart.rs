//! A click's Session restart for the active version, on Windows, where a restart keeps every
//! terminal. The newer side decides: the active version's own `orchbee` reads the Session's restart
//! tier with `status server`, and when that tier, or a failed join, asks for it, restarts the
//! Session's Server and Worker with `server restart`. Both run on one background thread, each as a
//! child without a window, bounded in time. A Terminal Host that version's Worker cannot adopt rules
//! out a restart that keeps terminals: the person confirms first, and the confirmed replacement stops
//! the Session through this TUI's own connection, so the active version starts it again with every
//! terminal restarted from the Session's final capture.
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

/// What the job reports, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    /// The restart started; the Session is unavailable for a moment.
    Restarting,
    /// The new version can serve this TUI: leave for it.
    Leave,
    /// Nothing changed; the update waits for the Session to move.
    Waiting,
    /// A Terminal Host of the Session must be replaced, which restarts its terminals: the person
    /// confirms first.
    HostRequired,
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
        Self::spawn(move || run(&executable, &session, joins))
    }

    /// Starts the confirmed Host replacement: stops the Session whose runtime folder is `runtime`
    /// through this TUI's own connection and reports [`Progress::Leave`] once the stop ended, so the
    /// active version starts the Session again, or [`Progress::Waiting`] when it did not.
    pub(super) fn replace_host(runtime: PathBuf) -> Option<Self> {
        Self::spawn(move || {
            vec![
                Progress::Restarting,
                if stop(&runtime) {
                    Progress::Leave
                } else {
                    Progress::Waiting
                },
            ]
        })
    }

    fn spawn(steps: impl FnOnce() -> Vec<Progress> + Send + 'static) -> Option<Self> {
        let (send, progress) = mpsc::channel();
        let thread = BackgroundThread::spawn("orchbee-update-restart", move || {
            for step in steps() {
                if send.send(step).is_err() {
                    return;
                }
            }
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

/// The reports of one job. A Host whose own tier is `required` asks the person first, since the
/// restart's successor could not adopt it. A Session the active version joins without a tier, or
/// whose tier does not read, switches as before; a restart that does not answer `restarted` changes
/// nothing the TUI can use, so the update waits.
fn run(executable: &Path, session: &str, joins: bool) -> Vec<Progress> {
    let status = command_json(
        executable,
        session,
        &["status", "server", "--format", "json"],
        STATUS_BUDGET,
    );
    let restart = status
        .as_ref()
        .map(|document| &document["result"]["server"]["restart"]);
    if restart
        .and_then(|restart| restart["hosts"].as_array())
        .is_some_and(|hosts| hosts.iter().any(|host| host["tier"] == "required"))
    {
        return vec![Progress::HostRequired];
    }
    if joins {
        let needed = restart
            .and_then(|restart| restart["tier"].as_str())
            .is_some_and(|tier| matches!(tier, "recommended" | "required"));
        if !needed {
            return vec![Progress::Leave];
        }
    }
    let restarted = command_json(
        executable,
        session,
        &["server", "restart", "--format", "json"],
        RESTART_BUDGET,
    )
    .is_some_and(|document| document["result"]["outcome"] == "restarted");
    vec![
        Progress::Restarting,
        if restarted {
            Progress::Leave
        } else {
            Progress::Waiting
        },
    ]
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
