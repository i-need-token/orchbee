//! One real OS process-tree witness for `exec::run_command` teardown —
//! no scripted injection: a fake provider script spawns a descendant that
//! inherits stdout and ignores SIGTERM while the leader exits on TERM. The
//! group must still take the KILL path; completion stays bounded and the
//! descendant's death is observed, never assumed.
//!
//! Only fake executables inside an owned temporary root participate — no
//! real provider binary, no real HOME, no process-name based killing. The
//! guard below owns the failure paths: a panic mid-assertion still gets a
//! bounded group KILL against the identities this test itself recorded,
//! and the temporary root always leaves with the test.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use nix::{
    sys::signal::{Signal, kill, killpg},
    unistd::Pid,
};

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

/// Signal-0 liveness probe: ONLY `ESRCH` proves disappearance. `EPERM`
/// means the pid exists but is not ours to signal; every other error is
/// not affirmative evidence the process is gone — keep it on the live
/// side and let the bounded wait decide.
fn alive(pid: i32) -> bool {
    !matches!(
        kill(Pid::from_raw(pid), None),
        Err(nix::errno::Errno::ESRCH)
    )
}

/// Scoped cleanup for the owned process group and the temporary root. The
/// identities come from pidfiles the spawned processes wrote themselves —
/// `killpg`/`kill` never run against a guessed or name-matched process.
struct TeardownGuard {
    root: PathBuf,
    /// The leader's pid — `run_command` spawns with `process_group(0)`, so
    /// the child pid is the group id.
    group: Option<i32>,
    /// The TERM-ignoring descendant's pid, recorded by the descendant.
    descendant: Option<i32>,
}

impl TeardownGuard {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            group: None,
            descendant: None,
        }
    }
}

impl Drop for TeardownGuard {
    fn drop(&mut self) {
        // Kill only while an owned identity still answers signal-0 — a
        // fully reaped group is left alone, so a stale pidfile can never
        // aim a signal at an unrelated reused pid.
        let descendant_live = self.descendant.is_some_and(alive);
        if let Some(group) = self.group
            && (alive(group) || descendant_live)
        {
            let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
        }
        if let Some(pid) = self.descendant.filter(|pid| alive(*pid)) {
            let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
        }
        // Bounded settle for the reaper on panic paths — the cleanup claim
        // covers the owned processes, not an unbounded reap.
        let settle = Instant::now() + Duration::from_millis(500);
        while self.descendant.is_some_and(alive) && Instant::now() < settle {
            thread::sleep(Duration::from_millis(25));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Arms the cancel flag on unwind — a failed barrier assertion must not
/// leave the scoped `run_command` running until its real deadline while
/// the test tears down.
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

#[test]
fn cancelled_command_group_still_takes_the_kill_path() {
    // Owned temporary root: the fake executable, its home cwd, and the
    // pidfiles all live under it; the guard removes it on every path.
    let root = std::env::temp_dir().join(format!(
        "orchbee-exec-witness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or(0)
    ));
    let mut guard = TeardownGuard::new(root.clone());
    let bin = root.join("bin");
    let home = root.join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    let script = bin.join("fake-provider");
    let leader_pidfile = root.join("leader.pid");
    let descendant_pidfile = root.join("descendant.pid");
    let ready = root.join("descendant.ready");
    // The leader records its own pid (== the group id), then the
    // descendant installs its TERM-ignore trap, records its pid, raises
    // the ready barrier, and only then blocks on sleep holding the
    // inherited stdout write end. The leader exits on the group's first
    // TERM — the early-leader-exit KILL branch is what this witness pins.
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho $$ > \"{leader}\"\n( trap '' TERM; echo $$ > \"{desc}\"; : > \"{ready}\"; exec sleep 60 ) &\nexec sleep 60\n",
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
    // `ArmFlag` guard keeps an unwinding assertion from stranding the
    // command at its real 30s deadline.
    let (result, elapsed, leader, descendant) = thread::scope(|scope| {
        // `ArmFlag` drops on every path out of this closure — a barrier
        // panic arms the flag before `scope` joins the command thread.
        let _arm = ArmFlag(&flag);
        let command = scope.spawn(|| {
            exec::run_command(
                &script,
                &[],
                &home,
                Instant::now() + Duration::from_secs(30),
                Some(&cancel),
            )
        });
        let descendant = read_pidfile(&descendant_pidfile, Duration::from_secs(5));
        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            assert!(
                Instant::now() < ready_deadline,
                "descendant ready barrier never fired"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let leader = read_pidfile(&leader_pidfile, Duration::from_secs(5));
        guard.descendant = Some(descendant);
        guard.group = Some(leader);
        // The premise the teardown contract rests on: the leader is alive
        // until the group TERM lands — observed, not assumed.
        assert!(
            alive(leader),
            "leader {leader} exited before the group TERM"
        );
        assert!(
            alive(descendant),
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
    // for. ESRCH is the only affirmative disappearance.
    let leader_deadline = Instant::now() + Duration::from_secs(5);
    while alive(leader) {
        assert!(
            Instant::now() < leader_deadline,
            "leader {leader} survived the group TERM"
        );
        thread::sleep(Duration::from_millis(25));
    }
    // The TERM-ignoring descendant must be gone — the group KILL did the
    // work the leader's exit could not. ESRCH-only probing, bounded wait.
    let collect_deadline = Instant::now() + Duration::from_secs(5);
    while alive(descendant) {
        assert!(
            Instant::now() < collect_deadline,
            "descendant {descendant} survived the group KILL"
        );
        thread::sleep(Duration::from_millis(25));
    }
    guard.descendant = None;
    guard.group = None;
}
