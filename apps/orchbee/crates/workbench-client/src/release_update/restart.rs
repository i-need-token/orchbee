//! A click's Session restart for the active version, on Windows, where a restart keeps every
//! terminal. The newer side decides: the active version's own `orchbee` reads the Session's restart
//! tier with `status server`, and when that tier, or a failed join, asks for it, restarts the
//! Session's Server and Worker with `server restart`. Both run on one background thread, each as a
//! child without a window, bounded in time.
use crate::background_thread::BackgroundThread;
use orchbee_local_runtime::ActiveRelease;
use std::{
    io::Read,
    path::Path,
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

/// What the job reports, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    /// The restart started; the Session is unavailable for a moment.
    Restarting,
    /// The new version can serve this TUI: leave for it.
    Leave,
    /// Nothing changed; the update waits for the Session to move.
    Waiting,
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
        let (send, progress) = mpsc::channel();
        let thread = BackgroundThread::spawn("orchbee-update-restart", move || {
            for step in run(&executable, &session, joins) {
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

/// The reports of one job. A Session the active version joins without a tier, or whose tier does
/// not read, switches as before; a restart that does not answer `restarted` changes nothing the
/// TUI can use, so the update waits.
fn run(executable: &Path, session: &str, joins: bool) -> Vec<Progress> {
    if joins {
        let tier = command_json(
            executable,
            session,
            &["status", "server", "--format", "json"],
            STATUS_BUDGET,
        );
        let needed = tier
            .as_ref()
            .and_then(|document| document["result"]["server"]["restart"]["tier"].as_str())
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
