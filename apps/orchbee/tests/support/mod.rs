//! Bounded child output and private runtime fixtures for Unix CLI tests.
//! The aggregate integration target shares one module; process-isolated targets
//! retain their own module instance.

pub use crate::inherited_environment::orchbee_command;

use std::{
    fs,
    io::{ErrorKind, Read},
    os::fd::AsFd,
    os::unix::fs::PermissionsExt,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nix::fcntl::{FcntlArg, OFlag, fcntl};
use orchbee_local_runtime::ModeCheckedLocalSessionRendezvous;
use sha2::{Digest, Sha256};

/// Wall-clock backstop on every spawned CLI child. A healthy run retires on
/// the CLI's own 5s network budget; this bound only guards the suite against
/// a child that never exits at all, leaving ample slack for a loaded machine.
pub const RUN_WALL_CLOCK_BACKSTOP: Duration = Duration::from_secs(45);

/// Extra drain window after a killed child: enough to collect whatever the
/// pipes already buffered, short enough that the timeout stays decisive.
const POST_KILL_DRAIN: Duration = Duration::from_millis(100);

/// Total bytes retained across both captured streams — one budget shared by
/// stdout, stderr, and the post-kill drain. Sized at twice the largest
/// legitimate capture in the suite (the 2,000,000B-per-stream witnesses);
/// the wall-clock backstop alone cannot stop a fast writer from exhausting
/// runner memory first.
const CAPTURE_BYTE_BUDGET: usize = 8 * 1024 * 1024;

/// The bounded wait failed. `status` is always the reaped direct-child state;
/// `stdout`/`stderr` hold whatever was captured up to the failure and may be
/// partial — they never pretend to be a complete normal `Output`.
pub enum BoundedChildFailure {
    /// The direct child was still running at the deadline: SIGKILL + reaped.
    ChildTimeout {
        budget: Duration,
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// The child exited (normally or in the kill race) but a pipe stayed open
    /// past the deadline — some other process still holds a write end.
    DrainTimeout {
        budget: Duration,
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// A pipe read failed; the child was killed if it was still running.
    ReadError {
        budget: Duration,
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        stream: &'static str,
        error: std::io::Error,
    },
    /// Combined captured output crossed the shared byte budget. `stdout` and
    /// `stderr` hold the truncated prefix actually retained (their total is
    /// at most `limit`); `status` is the real reaped state — the child was
    /// killed if it was still running, and a natural exit in the race is
    /// reported as its own status.
    OutputLimit {
        limit: usize,
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
}

// Debug delegates to Display: the captured byte buffers only appear as
// counts, so a failure report stays a line, not a memory dump.
impl std::fmt::Debug for BoundedChildFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for BoundedChildFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChildTimeout {
                budget,
                status,
                stdout,
                stderr,
            } => write!(
                formatter,
                "cli child exceeded the {budget:?} wall-clock backstop; killed \
                 and reaped ({status}) with {out}B stdout / {err}B stderr captured",
                out = stdout.len(),
                err = stderr.len()
            ),
            Self::DrainTimeout {
                budget,
                status,
                stdout,
                stderr,
            } => write!(
                formatter,
                "cli child exited ({status}) but a pipe stayed open past the \
                 {budget:?} backstop — a descendant still holds a write end; \
                 {out}B stdout / {err}B stderr captured",
                out = stdout.len(),
                err = stderr.len()
            ),
            Self::ReadError {
                budget,
                status,
                stdout,
                stderr,
                stream,
                error,
            } => write!(
                formatter,
                "cli child {stream} read failed ({error}) inside the {budget:?} \
                 backstop; child reaped ({status}) with {out}B stdout / \
                 {err}B stderr captured",
                out = stdout.len(),
                err = stderr.len()
            ),
            Self::OutputLimit {
                limit,
                status,
                stdout,
                stderr,
            } => write!(
                formatter,
                "cli child output crossed the {limit}B shared capture budget; \
                 child reaped ({status}) with a truncated prefix of \
                 {out}B stdout / {err}B stderr retained",
                out = stdout.len(),
                err = stderr.len()
            ),
        }
    }
}

/// What the guard needs from a captured stream: nonblocking flaggable reads.
trait CapturablePipe: Read + AsFd {}
impl<T: Read + AsFd> CapturablePipe for T {}

/// A captured pipe switched to `O_NONBLOCK`: reads return `WouldBlock`
/// instead of blocking, so draining never waits on a write end held open by
/// a descendant and no reader thread can be left blocked after the deadline.
struct Pipe {
    reader: Box<dyn CapturablePipe>,
    captured: Vec<u8>,
    open: bool,
}

enum DrainFault {
    /// The end-to-end deadline arrived mid-drain.
    Deadline,
    /// A read observed bytes beyond the shared capture budget — the retained
    /// prefix stops at the limit and nothing past it is appended.
    Limit,
    /// The fd reported a real read error.
    Read(std::io::Error),
}

impl Pipe {
    fn new<R: Read + AsFd + 'static>(reader: R) -> Self {
        fcntl(&reader, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))
            .expect("mark cli child pipe nonblocking");
        Self {
            reader: Box::new(reader),
            captured: Vec::new(),
            open: true,
        }
    }

    /// Pull available bytes until EOF, `WouldBlock`, `until`, or the shared
    /// capture budget. Bounded in every path — the deadline check sits inside
    /// the read loop so a continuously writing child cannot starve it, and a
    /// read that observes the first byte past `remaining` fails `Limit`
    /// instead of growing `captured` without bound.
    fn drain(&mut self, until: Instant, remaining: &mut usize) -> Result<(), DrainFault> {
        let mut chunk = [0u8; 8192];
        while self.open {
            if Instant::now() >= until {
                return Err(DrainFault::Deadline);
            }
            match self.reader.read(&mut chunk) {
                Ok(0) => self.open = false,
                Ok(n) => {
                    let take = n.min(*remaining);
                    self.captured.extend_from_slice(&chunk[..take]);
                    *remaining -= take;
                    if take < n {
                        return Err(DrainFault::Limit);
                    }
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(DrainFault::Read(error)),
            }
        }
        Ok(())
    }
}

/// Single finish engine behind both public entries: one end-to-end deadline
/// covers the direct child's run *and* the pipe-EOF wait. On expiry a still
/// running direct child gets an exact `Child::kill` + `wait` reap; a child
/// that already exited but whose pipes stay open (a descendant holds a write
/// end) is a distinct `DrainTimeout`, never a fabricated normal `Output`.
fn finish(mut child: Child, budget: Duration) -> Result<Output, BoundedChildFailure> {
    let deadline = Instant::now() + budget;
    let mut stdout = child.stdout.take().map(Pipe::new);
    let mut stderr = child.stderr.take().map(Pipe::new);
    let mut observed: Option<ExitStatus> = None;
    let mut remaining = CAPTURE_BYTE_BUDGET;
    loop {
        if observed.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => observed = Some(status),
                Ok(None) => {}
                Err(error) => panic!("cli child wait failed: {error}"),
            }
        }
        if observed.is_none() && Instant::now() >= deadline {
            match child.kill() {
                Ok(()) => {}
                // Exited between the last poll and the kill: nothing to
                // signal, `wait` reaps the real exit status below.
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => panic!("cli child kill failed: {error}"),
            }
            let status = child.wait().expect("reap cli child");
            observed = Some(status);
            if status.signal().is_some() {
                // Real kill: one bounded pass collects what the pipes hold,
                // still inside the same shared capture budget.
                let grace = Instant::now() + POST_KILL_DRAIN;
                for pipe in [&mut stdout, &mut stderr].into_iter().flatten() {
                    let _ = pipe.drain(grace, &mut remaining);
                }
                return Err(BoundedChildFailure::ChildTimeout {
                    budget,
                    status,
                    stdout: stdout.map_or_else(Vec::new, |pipe| pipe.captured),
                    stderr: stderr.map_or_else(Vec::new, |pipe| pipe.captured),
                });
            }
            // No signal: the child exited on its own in the kill race — fall
            // through and finish draining under the same deadline.
        }
        let mut read_fault: Option<(&'static str, std::io::Error)> = None;
        let mut limit_hit = false;
        for (pipe, stream) in [(stdout.as_mut(), "stdout"), (stderr.as_mut(), "stderr")] {
            let Some(pipe) = pipe else { continue };
            match pipe.drain(deadline, &mut remaining) {
                Ok(()) => {}
                // Data may still be mid-flight; let the top of the loop apply
                // the child-kill or drain-timeout branch with a fresh clock.
                Err(DrainFault::Deadline) => break,
                Err(DrainFault::Limit) => {
                    limit_hit = true;
                    break;
                }
                Err(DrainFault::Read(error)) => {
                    read_fault = Some((stream, error));
                    break;
                }
            }
        }
        if limit_hit {
            // The point of the limit is to stop growth now: kill the direct
            // child if it is still running (a natural exit in the race is a
            // legal reaped status), and skip the post-kill drain — there is
            // no budget left to spend and nothing more worth collecting.
            if observed.is_none() {
                let _ = child.kill();
                observed = Some(child.wait().expect("reap cli child"));
            }
            return Err(BoundedChildFailure::OutputLimit {
                limit: CAPTURE_BYTE_BUDGET,
                status: observed.expect("child status reaped"),
                stdout: stdout.map_or_else(Vec::new, |pipe| pipe.captured),
                stderr: stderr.map_or_else(Vec::new, |pipe| pipe.captured),
            });
        }
        if let Some((stream, error)) = read_fault {
            if observed.is_none() {
                let _ = child.kill();
                observed = Some(child.wait().expect("reap cli child"));
            }
            return Err(BoundedChildFailure::ReadError {
                budget,
                status: observed.expect("child status reaped"),
                stdout: stdout.map_or_else(Vec::new, |pipe| pipe.captured),
                stderr: stderr.map_or_else(Vec::new, |pipe| pipe.captured),
                stream,
                error,
            });
        }
        let drained = stdout.as_ref().is_none_or(|pipe| !pipe.open)
            && stderr.as_ref().is_none_or(|pipe| !pipe.open);
        if let Some(status) = observed {
            if drained {
                return Ok(Output {
                    status,
                    stdout: stdout.map_or_else(Vec::new, |pipe| pipe.captured),
                    stderr: stderr.map_or_else(Vec::new, |pipe| pipe.captured),
                });
            }
            if Instant::now() >= deadline {
                return Err(BoundedChildFailure::DrainTimeout {
                    budget,
                    status,
                    stdout: stdout.map_or_else(Vec::new, |pipe| pipe.captured),
                    stderr: stderr.map_or_else(Vec::new, |pipe| pipe.captured),
                });
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
}

/// `Command::output` with a deadline. Owns the stdio configuration — stdin
/// closed, both pipes captured — and drains them while the child runs, so a
/// child writing more than the pipe buffer cannot deadlock the wait.
pub fn bounded_output(
    command: &mut Command,
    budget: Duration,
) -> Result<Output, BoundedChildFailure> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().expect("spawn cli child");
    finish(child, budget)
}

/// Deadline wait on an already-spawned child whose stdio the caller owns —
/// e.g. a `/dev/full` stdout that cannot route through `bounded_output`.
/// Whichever pipes exist are drained while the child runs under the same
/// single end-to-end deadline; unpiped streams are not treated as an error.
pub fn bounded_wait_with_output(
    child: Child,
    budget: Duration,
) -> Result<Output, BoundedChildFailure> {
    finish(child, budget)
}

/// Spawn the staged CLI binary under the wall-clock backstop with the shared
/// deterministic-locale environment both suites use.
pub fn run(binary: &Path, base: &Path, arguments: &[&str]) -> Output {
    run_lang(binary, base, arguments, "en_US.UTF-8")
}

/// The same spawn under a caller-selected `LANG` — locale-comparison evidence
/// needs the zh-CN pass to differ from the en-US default in exactly one
/// environment variable.
pub fn run_lang(binary: &Path, base: &Path, arguments: &[&str], lang: &str) -> Output {
    let mut command = orchbee_command(binary);
    command
        .args(arguments)
        .env("ORCHBEE_RUNTIME_BASE", base)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", lang);
    bounded_output(&mut command, RUN_WALL_CLOCK_BACKSTOP)
        .unwrap_or_else(|timeout| panic!("{timeout}"))
}

// ---- short runtime base -----------------------------------------------------

/// Session scope directory prefix the rendezvous validator expects.
const SCOPE_PREFIX: &str = "orchbee-native-";

/// Leaf-name length minted by `runtime_base`: `"oc-"` + 8 hex chars. Fixed
/// and short so the socket-margin check below bounds the canonical TMPDIR
/// this suite can run under.
const BASE_LEAF_LEN: usize = 11;

/// Usable `sockaddr_un.sun_path` capacity for a socket pathname: the field
/// length minus the trailing NUL std reserves (`SocketAddr` rejects
/// `len >= sun_path.len()`). The field offset — not `sizeof(sa_family_t)` —
/// accounts for every header member: Linux (`sun_family:u16` + `sun_path[108]`)
/// yields 110−2−1 = 107; BSD/Darwin (`sun_len:u8` + `sun_family:u8` +
/// `sun_path[104]`) yields 106−2−1 = 103. Neither layout has tail padding:
/// `sun_path` is a `c_char` array and `sockaddr_un` ends with it at its own
/// alignment.
pub fn sun_path_limit() -> usize {
    std::mem::size_of::<nix::libc::sockaddr_un>()
        - std::mem::offset_of!(nix::libc::sockaddr_un, sun_path)
        - 1
}

/// Mint a short, exclusively-created, mode-0700 runtime base under the
/// canonical TMPDIR so `<base>/<scope>/client.sock` stays inside `sun_path`
/// even when TMPDIR itself is long. `tag` feeds the name entropy only — it is
/// not a uniqueness guarantee; exclusive `create_dir` plus bounded retries
/// are. Fails fast with a clear message when the canonical TMPDIR leaves no
/// socket margin at all instead of hanging or silently skipping.
pub fn runtime_base(tag: &str) -> PathBuf {
    let tmp = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize TMPDIR for the cli runtime base");
    let uid = nix::unistd::geteuid().as_raw();
    // Worst socket the rendezvous can derive is the Host endpoint
    // `<scope>/host/<endpoint-id>.s` — deeper than client/worker/pending.
    let probe_scope = tmp
        .join("x".repeat(BASE_LEAF_LEN))
        .join(format!("{SCOPE_PREFIX}{uid}"));
    let worst_len = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&probe_scope)
        .as_os_str()
        .as_encoded_bytes()
        .len();
    let limit = sun_path_limit();
    let tmp_len = tmp.as_os_str().as_encoded_bytes().len();
    assert!(
        worst_len <= limit,
        "canonical TMPDIR {tmp:?} ({tmp_len}B) leaves no unix-socket margin for \
         the deepest derived socket address ({worst_len}B against a \
         {limit}B sun_path limit) — point TMPDIR at a shorter real directory",
    );
    let pid = std::process::id();
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    for _ in 0..32 {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let digest = Sha256::digest(format!("obcli:{tag}:{pid}:{nanos}:{sequence}"));
        let leaf = format!(
            "oc-{:02x}{:02x}{:02x}{:02x}",
            digest[0], digest[1], digest[2], digest[3]
        );
        let directory = tmp.join(leaf);
        match fs::create_dir(&directory) {
            Ok(()) => {
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                    .expect("chmod runtime base");
                return directory;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                panic!("create cli runtime base {}: {error}", directory.display())
            }
        }
    }
    panic!(
        "could not mint a unique short runtime dir under {} after 32 attempts",
        tmp.display()
    );
}

/// The existing-only Session scope path inside a runtime base.
pub fn scope_dir(base: &Path) -> PathBuf {
    base.join(format!("{SCOPE_PREFIX}{}", nix::unistd::geteuid()))
}

/// Create the Session scope directory the rendezvous validator expects (mode
/// 0700, owned by the caller).
pub fn stage_scope(base: &Path) -> PathBuf {
    let scope = scope_dir(base);
    fs::create_dir(&scope).expect("create scope directory");
    fs::set_permissions(&scope, fs::Permissions::from_mode(0o700)).expect("chmod scope");
    scope
}
