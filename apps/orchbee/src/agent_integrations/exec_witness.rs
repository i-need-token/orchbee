//! One real OS process-tree witness for `exec::run_command` teardown —
//! no scripted injection: a fake provider script spawns a descendant that
//! inherits stdout and ignores SIGTERM while the leader exits on TERM. The
//! group must still take the KILL path; completion stays bounded and the
//! descendant's death is observed, never assumed.
//!
//! Only fake executables inside an owned temporary root participate — no
//! real provider binary, no real HOME, no process-name based killing.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use nix::{sys::signal::kill, unistd::Pid};

use super::exec::{self, CommandResult};
use crate::termination_signal::TerminationSource;

/// A `TerminationSource` backed by a plain flag — the witness arms it from
/// a helper thread after the command is in flight.
struct FlagCancel(Arc<AtomicBool>);

impl TerminationSource for FlagCancel {
    fn requested(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

fn alive(pid: i32) -> bool {
    // Signal-0 probe: Ok while the process exists (zombie included), ESRCH
    // once the group kill plus reparenting finished collecting it.
    kill(Pid::from_raw(pid), None).is_ok()
}

#[test]
fn cancelled_command_group_still_takes_the_kill_path() {
    // Owned temporary root: the fake executable, its home cwd, and the
    // descendant pidfile all live under it; failing paths still remove it.
    let root = std::env::temp_dir().join(format!(
        "orchbee-exec-witness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or(0)
    ));
    let bin = root.join("bin");
    let home = root.join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    let script = bin.join("fake-provider");
    let pidfile = root.join("descendant.pid");
    // The descendant subshell ignores TERM and execs sleep while holding
    // the inherited stdout write end; the leader execs sleep with the
    // default disposition and dies on the group's first TERM.
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n( trap '' TERM; exec sleep 60 ) &\necho $! > \"{}\"\nexec sleep 60\n",
            pidfile.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();

    // No scripted plan — the spawn below is a real OS process tree.
    super::inject::reset();

    let flag = Arc::new(AtomicBool::new(false));
    let cancel = FlagCancel(flag.clone());
    {
        let flag = flag.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::Relaxed);
        });
    }

    let start = Instant::now();
    let result = exec::run_command(
        &script,
        &[],
        &home,
        start + Duration::from_secs(30),
        Some(&cancel),
    );
    let elapsed = start.elapsed();

    // The cancel fires ~300ms in; teardown owes TERM + grace + KILL + reap
    // — under two seconds even on a loaded host, far under the slot.
    assert!(
        matches!(result, CommandResult::Cancelled),
        "mid-flight cancel ends in the typed Cancelled result: {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "bounded teardown: {elapsed:?}"
    );

    // The TERM-ignoring descendant must be gone — the group KILL did the
    // work the leader's exit could not. Allow a bounded window for the
    // reparented zombie to be collected.
    let descendant: i32 = fs::read_to_string(&pidfile)
        .expect("descendant recorded its pid")
        .trim()
        .parse()
        .expect("pidfile is a pid");
    let collect_deadline = Instant::now() + Duration::from_secs(5);
    while alive(descendant) {
        assert!(
            Instant::now() < collect_deadline,
            "descendant {descendant} survived the group KILL"
        );
        thread::sleep(Duration::from_millis(50));
    }

    let _ = fs::remove_dir_all(&root);
}
