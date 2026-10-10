//! Real OS process witnesses for `exec::run_command` — no scripted
//! injection. One fake provider spawns a descendant that inherits stdout
//! and ignores SIGTERM while the leader exits on TERM; another remains live
//! until the runner's own timeout. Both pin the launched process identity,
//! observe bounded completion, and verify exit rather than assuming it.
//!
//! The descendant's identity is the parent's `$!` — a subshell's `$$`
//! would report the parent shell's own pid, which cannot prove anything
//! about the TERM-ignoring child. Both identities are pinned by pidfd
//! while `/proc` still proves fixture ownership (cwd inside the owned
//! root, the spawned group relationship), so no assertion or cleanup can
//! ever signal a recycled pid.
//!
//! Only fake executables inside an owned temporary root participate — no
//! real provider binary, no real HOME, no process-name based killing. The
//! guard below owns the failure paths: a panic mid-assertion still gets a
//! bounded KILL against the pidfd-pinned identities this test recorded,
//! and the temporary root always leaves with the test.

use std::{
    fs, io,
    os::{
        fd::{AsFd, OwnedFd},
        unix::fs::PermissionsExt,
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use nix::{
    poll::{PollFd, PollFlags, PollTimeout, poll},
    unistd::{Pid, getpgid},
};
use rustix::process::{Pid as RustixPid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use super::exec::{self, CommandResult};
use crate::termination_signal::TerminationSource;

/// A `TerminationSource` backed by a plain flag — the witness arms it only
/// after the descendant's own ready barrier proves the TERM trap is live.
struct FlagCancel(Arc<AtomicBool>);

impl TerminationSource for FlagCancel {
    fn requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Kernel-level exit witness for a pinned process: pidfd readability means
/// the process exited. That is the honest claim — a reparented process may
/// linger as an unreaped zombie and this test neither assumes nor requires
/// init's reaping to prove termination.
fn pidfd_exited(pidfd: &OwnedFd) -> io::Result<bool> {
    let mut descriptor = [PollFd::new(pidfd.as_fd(), PollFlags::POLLIN)];
    poll(&mut descriptor, PollTimeout::ZERO).map_err(io::Error::from)?;
    let events = descriptor[0].revents().unwrap_or(PollFlags::POLLNVAL);
    if events.contains(PollFlags::POLLNVAL) {
        return Err(io::Error::other("pinned pidfd became invalid"));
    }
    Ok(events.intersects(PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR))
}

/// A fixture process pinned by pidfd. The descriptor binds the process
/// itself rather than a numeric id, so a recycled pid can never receive
/// its signals and readability is the kernel-level exit witness.
struct PinnedProcess {
    pidfd: OwnedFd,
}

impl PinnedProcess {
    fn exited(&self) -> io::Result<bool> {
        pidfd_exited(&self.pidfd)
    }

    /// SIGKILL through the pinned descriptor — `SRCH` means the process
    /// was already gone, which is the desired end state.
    fn kill(&self) {
        let _ = pidfd_send_signal(&self.pidfd, Signal::KILL);
    }
}

/// Pin `raw_pid` while `/proc` still proves this is the fixture-owned
/// process: alive, its cwd inside the owned temporary root, and its
/// process group equal to `expected_group` — the relationship the spawn
/// established. The descriptor is checked non-exited before AND after the
/// `/proc` reads so a mid-check exit cannot slip a recycled pid in.
fn pin_verified(raw_pid: i32, cwd: &Path, expected_group: i32) -> io::Result<PinnedProcess> {
    let Some(pid) = RustixPid::from_raw(raw_pid) else {
        return Err(io::Error::other("fixture wrote an invalid pid"));
    };
    let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(rustix::io::Errno::SRCH) => {
            return Err(io::Error::other("fixture process exited before pin"));
        }
        Err(error) => return Err(error.into()),
    };
    if pidfd_exited(&pidfd)? {
        return Err(io::Error::other("fixture process exited before pin"));
    }
    let proc = PathBuf::from(format!("/proc/{raw_pid}"));
    let actual_cwd = fs::canonicalize(proc.join("cwd"))?;
    let group = getpgid(Some(Pid::from_raw(raw_pid)))
        .map_err(|error| io::Error::other(format!("getpgid {raw_pid}: {error}")))?
        .as_raw();
    if pidfd_exited(&pidfd)? {
        return Err(io::Error::other("fixture process exited during pin"));
    }
    if actual_cwd != cwd || group != expected_group {
        return Err(io::Error::other(format!(
            "pid {raw_pid} is not the fixture-owned process (cwd {actual_cwd:?}, group {group})"
        )));
    }
    Ok(PinnedProcess { pidfd })
}

/// Scoped cleanup for the pinned owned processes and the temporary root.
/// Failure paths signal only the pidfd handles recorded below — a pidfd
/// cannot target a recycled pid, and there is no raw numeric-pid or group
/// fallback. Identities are recorded the moment each pin succeeds, before
/// the barrier and every assertion that could unwind.
struct TeardownGuard {
    root: PathBuf,
    /// The leader — `run_command` spawns with `process_group(0)`, so its
    /// pid is also the owned group's id.
    leader: Option<PinnedProcess>,
    /// The TERM-ignoring descendant — the parent's `$!`, pinned while its
    /// group membership still proves the spawn relationship.
    descendant: Option<PinnedProcess>,
}

impl TeardownGuard {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            leader: None,
            descendant: None,
        }
    }
}

impl Drop for TeardownGuard {
    fn drop(&mut self) {
        for pinned in [&self.descendant, &self.leader].into_iter().flatten() {
            pinned.kill();
        }
        // Bounded settle on the kernel exit witness — `exited` is what the
        // cleanup claims; it does not wait for a reparented zombie to be
        // reaped by init.
        let settle = Instant::now() + Duration::from_millis(500);
        while Instant::now() < settle {
            let pending = [&self.descendant, &self.leader]
                .into_iter()
                .flatten()
                .any(|pinned| matches!(pinned.exited(), Ok(false)));
            if !pending {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Arms the cancel flag on unwind — a failed pin or barrier assertion must
/// not leave the scoped `run_command` running until its current slot closes
/// while the test tears down.
struct ArmFlag<'a>(&'a AtomicBool);

impl Drop for ArmFlag<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Read a pidfile the child wrote — bounded, since the file lands only
/// after the process is actually running.
fn read_pidfile(path: &Path, within: Duration) -> i32 {
    let deadline = Instant::now() + within;
    loop {
        if let Ok(content) = fs::read_to_string(path)
            && let Ok(pid) = content.trim().parse::<i32>()
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "pidfile {} never appeared",
            path.display()
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn owned_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "orchbee-exec-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or(0)
    ))
}

#[test]
fn cancelled_command_group_still_takes_the_kill_path() {
    // Owned temporary root: the fake executable, its home cwd, and the
    // pidfiles all live under it; the guard removes it on every path.
    let root = owned_root("cancel-witness");
    let mut guard = TeardownGuard::new(root.clone());
    let bin = root.join("bin");
    let home = root.join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    let home = fs::canonicalize(&home).unwrap();
    let script = bin.join("fake-provider");
    let leader_pidfile = root.join("leader.pid");
    let descendant_pidfile = root.join("descendant.pid");
    let ready = root.join("descendant.ready");
    // The leader records its own pid (== the group id). The descendant
    // subshell installs its TERM-ignore trap, raises the ready barrier,
    // and execs `sleep` holding the inherited stdout write end — while the
    // PARENT records the descendant's real pid through `$!` (a subshell's
    // `$$` would be the parent's own pid and prove nothing). The leader
    // exits on the group's first TERM — the early-leader-exit KILL branch
    // is what this witness pins.
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > \"{leader}\"\n( trap '' TERM; : > \"{ready}\"; exec sleep 60 ) &\necho $! > \"{desc}\"\nexec sleep 60\n",
            leader = leader_pidfile.display(),
            desc = descendant_pidfile.display(),
            ready = ready.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();

    // No scripted plan — the spawn below is a real OS process tree.
    super::inject::reset();

    let flag = Arc::new(AtomicBool::new(false));
    let cancel = FlagCancel(flag.clone());

    // Spawn in a scoped thread so `run_command` can borrow the cancel
    // source; the barrier below gates the arm on the descendant's own
    // ready file — a fixed sleep is not proof the trap is installed. The
    // `ArmFlag` keeps an unwinding assertion from waiting for the current
    // command slot to close: the pinned identities die through the command's
    // own group-teardown path.
    let (result, elapsed, leader, descendant) = thread::scope(|scope| {
        // `ArmFlag` drops on every path out of this closure — a barrier
        // panic arms the flag before `scope` joins the command thread.
        let _arm = ArmFlag(&flag);
        let command = scope.spawn(|| {
            exec::run_command(
                &script,
                &[],
                &home,
                Instant::now() + exec::SLOT,
                Some(&cancel),
            )
        });
        // Pin each identity the moment its pidfile lands — before the
        // barrier and before any assertion that could unwind.
        let leader = read_pidfile(&leader_pidfile, Duration::from_secs(5));
        guard.leader =
            Some(pin_verified(leader, &home, leader).expect("leader is the owned group leader"));
        let descendant = read_pidfile(&descendant_pidfile, Duration::from_secs(5));
        guard.descendant = Some(
            pin_verified(descendant, &home, leader).expect("descendant is in the owned group"),
        );
        assert_ne!(
            leader, descendant,
            "the descendant must be a distinct process — `$$` inside a \
             subshell would collapse both assertions onto the leader"
        );
        // The TERM-ignoring child emits the barrier itself, after its trap
        // is installed — never stdout EOF, never a fixed sleep.
        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            assert!(
                Instant::now() < ready_deadline,
                "descendant ready barrier never fired"
            );
            thread::sleep(Duration::from_millis(10));
        }
        // The premise the teardown contract rests on: the leader is alive
        // until the group TERM lands — observed on the pinned pidfd, not
        // assumed from signal-0 on a bare pid.
        assert!(
            !guard.leader.as_ref().unwrap().exited().unwrap(),
            "leader {leader} exited before the group TERM"
        );
        assert!(
            !guard.descendant.as_ref().unwrap().exited().unwrap(),
            "descendant {descendant} exited before the cancel"
        );
        let start = Instant::now();
        flag.store(true, Ordering::Relaxed);
        (
            command.join().expect("run_command thread must not panic"),
            start.elapsed(),
            leader,
            descendant,
        )
    });

    assert!(
        matches!(result, CommandResult::Cancelled),
        "mid-flight cancel ends in the typed Cancelled result: {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "bounded teardown: {elapsed:?}"
    );

    // The leader took the group TERM — the premise the KILL branch exists
    // for. `exited` on the pinned pidfd is the kernel-level claim.
    let leader_deadline = Instant::now() + Duration::from_secs(5);
    while !guard.leader.as_ref().unwrap().exited().unwrap() {
        assert!(
            Instant::now() < leader_deadline,
            "leader {leader} survived the group TERM"
        );
        thread::sleep(Duration::from_millis(25));
    }
    // The TERM-ignoring descendant must have exited — the group KILL did
    // the work the leader's exit could not. `exited` is the claim; an
    // unreaped reparented zombie is not hidden or required to be gone.
    let collect_deadline = Instant::now() + Duration::from_secs(5);
    while !guard.descendant.as_ref().unwrap().exited().unwrap() {
        assert!(
            Instant::now() < collect_deadline,
            "descendant {descendant} survived the group KILL"
        );
        thread::sleep(Duration::from_millis(25));
    }
    guard.descendant = None;
    guard.leader = None;
}

#[test]
fn timed_out_command_is_launched_and_reaped_inside_its_slot() {
    let root = owned_root("timeout-witness");
    let mut guard = TeardownGuard::new(root.clone());
    let bin = root.join("bin");
    let home = root.join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    let home = fs::canonicalize(&home).unwrap();
    let script = bin.join("fake-provider");
    let leader_pidfile = root.join("leader.pid");
    let ready = root.join("leader.ready");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > \"{leader}\"\n: > \"{ready}\"\nexec sleep 60\n",
            leader = leader_pidfile.display(),
            ready = ready.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();

    // No scripted plan: the ready file and pinned pidfd prove the timeout
    // result came from a launched native process, not pre-spawn admission.
    super::inject::reset();
    let (result, elapsed, leader) = thread::scope(|scope| {
        let command = scope.spawn(|| {
            let start = Instant::now();
            let result = exec::run_command(&script, &[], &home, Instant::now() + exec::SLOT, None);
            (result, start.elapsed())
        });
        let leader = read_pidfile(&leader_pidfile, Duration::from_secs(2));
        guard.leader =
            Some(pin_verified(leader, &home, leader).expect("leader is the owned group leader"));
        let ready_deadline = Instant::now() + Duration::from_secs(2);
        while !ready.exists() {
            assert!(
                Instant::now() < ready_deadline,
                "timeout witness ready barrier never fired"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let (result, elapsed) = command.join().expect("run_command thread must not panic");
        (result, elapsed, leader)
    });

    assert!(
        matches!(result, CommandResult::TimedOut),
        "launched timeout witness returned {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "one command exceeded its 3.6s slot: {elapsed:?}"
    );
    assert!(
        guard.leader.as_ref().unwrap().exited().unwrap(),
        "timed-out leader {leader} was still alive after run_command returned"
    );
    guard.leader = None;
}
