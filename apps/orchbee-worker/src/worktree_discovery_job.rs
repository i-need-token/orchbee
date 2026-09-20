use std::{
    ffi::OsString,
    io::{self, Read},
    os::unix::io::{AsFd, AsRawFd},
    path::Path,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    poll::{PollFd, PollFlags, PollTimeout, poll},
};

const JOB_TTL: Duration = Duration::from_secs(5);
const RECEIPT_TTL: Duration = Duration::from_secs(60);
const STDOUT_CAP: usize = 64 * 1024;
const STDERR_RETAIN: usize = 4 * 1024;
const POLL_SLICE: Duration = Duration::from_millis(25);
const DRAIN_QUANTUM: usize = 8;
const THREAD_NAME: &str = "orchbee-discovery-job";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JobId(pub(crate) [u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stage {
    Probe,
    List,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JobOutcome {
    Completed {
        common_dir: Vec<u8>,
        listing: Vec<u8>,
        stderr: Vec<u8>,
    },
    GitFailed {
        stage: Stage,
        status: Option<i32>,
        stderr: Vec<u8>,
    },
    SpawnUnavailable {
        stage: Stage,
    },
    SpawnFailed {
        stage: Stage,
    },
    TooLarge {
        stage: Stage,
    },
    Timeout,
    Cancelled,
    OwnerIo {
        stage: Option<Stage>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StartOutcome {
    Accepted,
    Busy,
    Dead,
    InvalidDirectory,
    SpawnFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PollOutcome {
    Pending,
    Completed(JobOutcome),
    PhysicalUnknown,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CancelOutcome {
    Registered,
    AlreadyTerminal(JobOutcome),
    PhysicalUnknown,
    Unknown,
}

struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
}

impl CommandSpec {
    fn spawn(&self) -> io::Result<Child> {
        Command::new(&self.program)
            .args(&self.args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

struct JobCommands {
    probe: CommandSpec,
    list: CommandSpec,
    test_hooks: Option<TestHooks>,
}

struct TestHooks {
    faults: Faults,
    witness: Arc<Mutex<JobWitness>>,
}

#[derive(Clone, Copy, Default)]
struct Faults {
    nonblock: bool,
    drain: bool,
    tail: bool,
}

#[derive(Default)]
struct JobWitness {
    spawned: Vec<u32>,
    // (fd number, /proc/self/fd symlink target) — survives fd reuse.
    pipe_ends: Vec<(i32, String)>,
    reaped: Vec<u32>,
}

fn git_commands(directory: &Path) -> JobCommands {
    let directory: OsString = directory.as_os_str().to_owned();
    JobCommands {
        probe: CommandSpec {
            program: "git".into(),
            args: vec![
                "--no-optional-locks".into(),
                "-C".into(),
                directory.clone(),
                "rev-parse".into(),
                "--path-format=absolute".into(),
                "--git-common-dir".into(),
            ],
        },
        list: CommandSpec {
            program: "git".into(),
            args: vec![
                "--no-optional-locks".into(),
                "-C".into(),
                directory,
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
                "-z".into(),
            ],
        },
        test_hooks: None,
    }
}

struct OwnerReport {
    outcome: JobOutcome,
    child_reaped: bool,
    pipes_closed: bool,
}

struct Physical {
    job_id: JobId,
    cancel: Arc<AtomicBool>,
    join: JoinHandle<OwnerReport>,
}

struct Receipt {
    job_id: JobId,
    outcome: JobOutcome,
    expires: Instant,
}

struct SlotInner {
    physical: Option<Physical>,
    dead: Option<JobId>,
    receipt: Option<Receipt>,
}

pub(crate) struct WorktreeDiscoverySlot {
    inner: Mutex<SlotInner>,
}

impl WorktreeDiscoverySlot {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(SlotInner {
                physical: None,
                dead: None,
                receipt: None,
            }),
        }
    }

    pub(crate) fn start(&self, job_id: JobId, directory: &Path) -> StartOutcome {
        self.start_with(job_id, directory, JOB_TTL, git_commands(directory))
    }

    fn start_with(
        &self,
        job_id: JobId,
        directory: &Path,
        ttl: Duration,
        commands: JobCommands,
    ) -> StartOutcome {
        if directory.as_os_str().is_empty()
            || !directory.is_absolute()
            || directory.as_os_str().as_encoded_bytes().contains(&0)
        {
            return StartOutcome::InvalidDirectory;
        }
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        settle(&mut inner);
        purge_expired_receipt(&mut inner);
        if inner.dead.is_some() {
            return StartOutcome::Dead;
        }
        if inner.physical.is_some() {
            return StartOutcome::Busy;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + ttl;
        let thread_cancel = Arc::clone(&cancel);
        let spawned = thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || run_owner(commands, thread_cancel, deadline));
        match spawned {
            Err(_error) => StartOutcome::SpawnFailed,
            Ok(join) => {
                inner.receipt = None;
                inner.physical = Some(Physical {
                    job_id,
                    cancel,
                    join,
                });
                StartOutcome::Accepted
            }
        }
    }

    pub(crate) fn poll(&self, job_id: JobId) -> PollOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        settle(&mut inner);
        purge_expired_receipt(&mut inner);
        if let Some(receipt) = &inner.receipt
            && receipt.job_id == job_id
        {
            return PollOutcome::Completed(receipt.outcome.clone());
        }
        if let Some(physical) = &inner.physical
            && physical.job_id == job_id
        {
            return PollOutcome::Pending;
        }
        if inner.dead == Some(job_id) {
            return PollOutcome::PhysicalUnknown;
        }
        PollOutcome::Unknown
    }

    pub(crate) fn cancel(&self, job_id: JobId) -> CancelOutcome {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        settle(&mut inner);
        purge_expired_receipt(&mut inner);
        if let Some(physical) = &inner.physical
            && physical.job_id == job_id
        {
            physical.cancel.store(true, Ordering::Relaxed);
            return CancelOutcome::Registered;
        }
        if let Some(receipt) = &inner.receipt
            && receipt.job_id == job_id
        {
            return CancelOutcome::AlreadyTerminal(receipt.outcome.clone());
        }
        if inner.dead == Some(job_id) {
            return CancelOutcome::PhysicalUnknown;
        }
        CancelOutcome::Unknown
    }
}

impl Drop for WorktreeDiscoverySlot {
    fn drop(&mut self) {
        if let Ok(inner) = self.inner.get_mut()
            && let Some(physical) = &inner.physical
        {
            physical.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn settle(inner: &mut SlotInner) {
    let Some(physical) = &inner.physical else {
        return;
    };
    if !physical.join.is_finished() {
        return;
    }
    let physical = inner.physical.take().expect("physical checked above");
    let job_id = physical.job_id;
    match physical.join.join() {
        Ok(report) if report.child_reaped && report.pipes_closed => {
            inner.receipt = Some(Receipt {
                job_id,
                outcome: report.outcome,
                expires: Instant::now() + RECEIPT_TTL,
            });
        }
        _ => {
            inner.dead = Some(job_id);
        }
    }
}

fn purge_expired_receipt(inner: &mut SlotInner) {
    if let Some(receipt) = &inner.receipt
        && Instant::now() >= receipt.expires
    {
        inner.receipt = None;
    }
}

enum StageEnd {
    Exited {
        status_code: Option<i32>,
        stdout: Vec<u8>,
    },
    Terminated {
        outcome: JobOutcome,
        reaped: bool,
    },
}

fn run_owner(commands: JobCommands, cancel: Arc<AtomicBool>, deadline: Instant) -> OwnerReport {
    let hooks = commands.test_hooks.as_ref();
    let mut stdout_total = 0usize;
    let mut stderr = Vec::new();
    let mut probe_stdout: Option<Vec<u8>> = None;
    let mut list_stdout: Option<Vec<u8>> = None;
    for (stage, spec) in [
        (Stage::Probe, &commands.probe),
        (Stage::List, &commands.list),
    ] {
        if cancel.load(Ordering::Relaxed) {
            return owner_report(JobOutcome::Cancelled);
        }
        if Instant::now() >= deadline {
            return owner_report(JobOutcome::Timeout);
        }
        let child = match spec.spawn() {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return owner_report(JobOutcome::SpawnUnavailable { stage });
            }
            Err(_error) => return owner_report(JobOutcome::SpawnFailed { stage }),
            Ok(child) => child,
        };
        let mut stage_stdout = Vec::new();
        let mut ctx = StageCtx {
            stage,
            cancel: &cancel,
            deadline,
            stdout_total: &mut stdout_total,
            stdout: &mut stage_stdout,
            stderr: &mut stderr,
            hooks,
        };
        match drain_child(child, &mut ctx) {
            StageEnd::Exited {
                status_code,
                stdout,
            } => {
                if status_code == Some(0) {
                    match stage {
                        Stage::Probe => probe_stdout = Some(stdout),
                        Stage::List => list_stdout = Some(stdout),
                    }
                } else {
                    return owner_report(JobOutcome::GitFailed {
                        stage,
                        status: status_code,
                        stderr,
                    });
                }
            }
            StageEnd::Terminated { outcome, reaped } => {
                return OwnerReport {
                    outcome,
                    child_reaped: reaped,
                    pipes_closed: true,
                };
            }
        }
    }
    owner_report(JobOutcome::Completed {
        common_dir: probe_stdout.unwrap_or_default(),
        listing: list_stdout.unwrap_or_default(),
        stderr,
    })
}

fn owner_report(outcome: JobOutcome) -> OwnerReport {
    OwnerReport {
        outcome,
        child_reaped: true,
        pipes_closed: true,
    }
}

fn witness_record(hooks: Option<&TestHooks>, record: impl FnOnce(&mut JobWitness)) {
    if let Some(hooks) = hooks {
        let mut witness = hooks
            .witness
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        record(&mut witness);
    }
}

fn set_nonblocking(file: &dyn AsFd, hooks: Option<&TestHooks>) -> io::Result<()> {
    if hooks.is_some_and(|hooks| hooks.faults.nonblock) {
        return Err(io::Error::other("injected nonblocking-setup fault"));
    }
    let flags = fcntl(file.as_fd(), FcntlArg::F_GETFL)
        .map(|flags| OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK)
        .map_err(io::Error::from)?;
    fcntl(file.as_fd(), FcntlArg::F_SETFL(flags)).map_err(io::Error::from)?;
    Ok(())
}

struct StageCtx<'a> {
    stage: Stage,
    cancel: &'a AtomicBool,
    deadline: Instant,
    stdout_total: &'a mut usize,
    stdout: &'a mut Vec<u8>,
    stderr: &'a mut Vec<u8>,
    hooks: Option<&'a TestHooks>,
}

impl StageCtx<'_> {
    fn faults(&self) -> Faults {
        self.hooks
            .map_or_else(Faults::default, |hooks| hooks.faults)
    }
}

fn drain_child(mut child: Child, ctx: &mut StageCtx<'_>) -> StageEnd {
    let stage = ctx.stage;
    let cancel = ctx.cancel;
    let deadline = ctx.deadline;
    let hooks = ctx.hooks;
    let faults = ctx.faults();
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    witness_record(hooks, |witness| {
        witness.spawned.push(child.id());
        for file in [
            out.as_ref().map(|file| file as &dyn AsFd),
            err.as_ref().map(|file| file as &dyn AsFd),
        ]
        .into_iter()
        .flatten()
        {
            let fd = file.as_fd().as_raw_fd();
            let target = std::fs::read_link(format!("/proc/self/fd/{fd}"))
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            witness.pipe_ends.push((fd, target));
        }
    });
    let setup_failed = [
        out.as_ref().map(|file| file as &dyn AsFd),
        err.as_ref().map(|file| file as &dyn AsFd),
    ]
    .into_iter()
    .flatten()
    .any(|file| set_nonblocking(file, hooks).is_err());
    if setup_failed {
        return kill_reap(
            child,
            out.take(),
            err.take(),
            JobOutcome::OwnerIo { stage: Some(stage) },
            hooks,
        );
    }
    let mut buffer = [0u8; 8192];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return kill_reap(child, out.take(), err.take(), JobOutcome::Cancelled, hooks);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return kill_reap(child, out.take(), err.take(), JobOutcome::Timeout, hooks);
        }
        let timeout = match PollTimeout::try_from(remaining.min(POLL_SLICE).as_millis() + 1) {
            Ok(timeout) => timeout,
            Err(_error) => {
                return kill_reap(
                    child,
                    out.take(),
                    err.take(),
                    JobOutcome::OwnerIo { stage: Some(stage) },
                    hooks,
                );
            }
        };
        let mut descriptors = Vec::with_capacity(2);
        if let Some(file) = &out {
            descriptors.push(PollFd::new(file.as_fd(), PollFlags::POLLIN));
        }
        if let Some(file) = &err {
            descriptors.push(PollFd::new(file.as_fd(), PollFlags::POLLIN));
        }
        if !descriptors.is_empty() {
            match poll(&mut descriptors, timeout) {
                Ok(_) => {}
                Err(Errno::EINTR) => continue,
                Err(_error) => {
                    return kill_reap(
                        child,
                        out.take(),
                        err.take(),
                        JobOutcome::OwnerIo { stage: Some(stage) },
                        hooks,
                    );
                }
            }
        }
        let out_events = descriptors
            .first()
            .and_then(|fd| fd.revents())
            .unwrap_or_else(PollFlags::empty);
        let err_index = usize::from(out.is_some());
        let err_events = descriptors
            .get(err_index)
            .and_then(|fd| fd.revents())
            .unwrap_or_else(PollFlags::empty);
        drop(descriptors);
        if let Some(file) = &mut out
            && out_events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)
        {
            match drain_fd(
                file,
                &mut buffer,
                ctx.stdout_total,
                ctx.stdout,
                faults.drain,
            ) {
                Ok(DrainResult::Overflow) => {
                    return kill_reap(
                        child,
                        out.take(),
                        err.take(),
                        JobOutcome::TooLarge { stage },
                        hooks,
                    );
                }
                Ok(DrainResult::Eof) => {
                    out = None;
                }
                Ok(DrainResult::Idle | DrainResult::Limited) => {}
                Err(_error) => {
                    return kill_reap(
                        child,
                        out.take(),
                        err.take(),
                        JobOutcome::OwnerIo { stage: Some(stage) },
                        hooks,
                    );
                }
            }
        }
        if let Some(file) = &mut err
            && err_events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)
        {
            match drain_stderr(file, &mut buffer, ctx.stderr, faults.drain) {
                Ok(DrainResult::Eof) => {
                    err = None;
                }
                Ok(DrainResult::Idle | DrainResult::Limited | DrainResult::Overflow) => {}
                Err(_error) => {
                    return kill_reap(
                        child,
                        out.take(),
                        err.take(),
                        JobOutcome::OwnerIo { stage: Some(stage) },
                        hooks,
                    );
                }
            }
        }
        match child.try_wait() {
            Err(_error) => {
                return kill_reap(
                    child,
                    out.take(),
                    err.take(),
                    JobOutcome::OwnerIo { stage: Some(stage) },
                    hooks,
                );
            }
            Ok(Some(status)) => {
                witness_record(hooks, |witness| witness.reaped.push(child.id()));
                return drain_tail(status.code(), out.take(), err.take(), ctx);
            }
            Ok(None) => {}
        }
        if out.is_none() && err.is_none() {
            // Both pipes at EOF but try_wait saw no exit yet; loop back
            // so try_wait is re-observed instead of blocking on pipes.
            thread::sleep(POLL_SLICE);
        }
    }
}

// The child is already reaped. Drain only what is still buffered: a
// WouldBlock closes that end because all pre-exit output is already in
// the pipe, while a descendant-held write end is the admitted limit and
// is never awaited. Each turn is quantum-bounded and still honors
// cancellation and the shared deadline.
fn drain_tail(
    status_code: Option<i32>,
    mut out: Option<ChildStdout>,
    mut err: Option<ChildStderr>,
    ctx: &mut StageCtx<'_>,
) -> StageEnd {
    let stage = ctx.stage;
    let cancel = ctx.cancel;
    let deadline = ctx.deadline;
    let faults = ctx.faults();
    let mut buffer = [0u8; 8192];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return detached(JobOutcome::Cancelled, out, err);
        }
        if deadline.saturating_duration_since(Instant::now()).is_zero() {
            return detached(JobOutcome::Timeout, out, err);
        }
        if let Some(file) = &mut out {
            match drain_fd(file, &mut buffer, ctx.stdout_total, ctx.stdout, faults.tail) {
                Ok(DrainResult::Overflow) => {
                    return detached(JobOutcome::TooLarge { stage }, out, err);
                }
                Ok(DrainResult::Eof | DrainResult::Idle) => {
                    out = None;
                }
                Ok(DrainResult::Limited) => {}
                Err(_error) => {
                    return detached(JobOutcome::OwnerIo { stage: Some(stage) }, out, err);
                }
            }
        }
        if let Some(file) = &mut err {
            match drain_stderr(file, &mut buffer, ctx.stderr, faults.tail) {
                Ok(DrainResult::Eof | DrainResult::Idle) => {
                    err = None;
                }
                Ok(DrainResult::Limited | DrainResult::Overflow) => {}
                Err(_error) => {
                    return detached(JobOutcome::OwnerIo { stage: Some(stage) }, out, err);
                }
            }
        }
        if out.is_none() && err.is_none() {
            return StageEnd::Exited {
                status_code,
                stdout: std::mem::take(ctx.stdout),
            };
        }
    }
}

fn detached(outcome: JobOutcome, out: Option<ChildStdout>, err: Option<ChildStderr>) -> StageEnd {
    drop(out);
    drop(err);
    StageEnd::Terminated {
        outcome,
        reaped: true,
    }
}

enum DrainResult {
    Idle,
    Limited,
    Eof,
    Overflow,
}

fn drain_fd(
    file: &mut impl Read,
    buffer: &mut [u8],
    total: &mut usize,
    retained: &mut Vec<u8>,
    inject_error: bool,
) -> io::Result<DrainResult> {
    if inject_error {
        return Err(io::Error::other("injected drain fault"));
    }
    let mut budget = DRAIN_QUANTUM;
    loop {
        if budget == 0 {
            return Ok(DrainResult::Limited);
        }
        budget -= 1;
        match file.read(buffer) {
            Ok(0) => return Ok(DrainResult::Eof),
            Ok(read) => {
                *total = total.saturating_add(read);
                if *total > STDOUT_CAP {
                    return Ok(DrainResult::Overflow);
                }
                retained.extend_from_slice(&buffer[..read]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(DrainResult::Idle);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

fn drain_stderr(
    file: &mut impl Read,
    buffer: &mut [u8],
    retained: &mut Vec<u8>,
    inject_error: bool,
) -> io::Result<DrainResult> {
    if inject_error {
        return Err(io::Error::other("injected drain fault"));
    }
    let mut budget = DRAIN_QUANTUM;
    loop {
        if budget == 0 {
            return Ok(DrainResult::Limited);
        }
        budget -= 1;
        match file.read(buffer) {
            Ok(0) => return Ok(DrainResult::Eof),
            Ok(read) => {
                let remaining = STDERR_RETAIN.saturating_sub(retained.len());
                if remaining > 0 {
                    let keep = read.min(remaining);
                    retained.extend_from_slice(&buffer[..keep]);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(DrainResult::Idle);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

fn kill_reap(
    mut child: Child,
    out: Option<ChildStdout>,
    err: Option<ChildStderr>,
    outcome: JobOutcome,
    hooks: Option<&TestHooks>,
) -> StageEnd {
    let _ = child.kill();
    let reaped = child.wait().is_ok();
    if reaped {
        witness_record(hooks, |witness| witness.reaped.push(child.id()));
    }
    drop(out);
    drop(err);
    StageEnd::Terminated { outcome, reaped }
}

#[cfg(test)]
mod tests;
