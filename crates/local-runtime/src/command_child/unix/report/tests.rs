use super::*;
use std::{
    os::{fd::OwnedFd, unix::process::ExitStatusExt},
    process::{Command, Stdio},
};

/// A group whose leader runs `script` in `/bin/sh`.
fn group(script: &str) -> CommandChild {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    CommandChild::spawn(command).unwrap()
}

#[test]
fn reported_groups_end_once_their_owner_ended_and_no_others() {
    let mut left = group("exec sleep 30");
    let mut stubborn = group("trap '' TERM; sleep 30 & wait");
    let mut settled = group("exec sleep 30");
    let mut other = group("exec sleep 30");
    // The owner runs until released, and its end is what the launcher waits for.
    let (gate, mut release) = std::io::pipe().unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "read -r gate"])
        .stdin(gate)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut owner = CommandChild::spawn(command).unwrap();
    let (reports, writer) = std::io::pipe().unwrap();
    let launcher = {
        let owner = owner.exact_identity().unwrap();
        thread::spawn(move || end_reported_groups(reports, owner))
    };
    let report = GroupReport {
        output: File::from(OwnedFd::from(writer)),
    };
    (&report.output).write_all(b"ready\n").unwrap();
    // A killed owner never reports these two settled.
    std::mem::forget(report.report(&left).unwrap().unwrap());
    std::mem::forget(report.report(&stubborn).unwrap().unwrap());
    drop(report.report(&settled).unwrap().unwrap());
    let (pid, start) = other.exact_identity().unwrap();
    report.write('+', (pid, start + 1)).unwrap();
    drop(report);

    // The report ended, but its owner runs on, and a running owner's groups stay its own.
    thread::sleep(WATCH_SLICE * 2);
    assert!(
        left.try_wait().unwrap().is_none(),
        "a running owner's group was signalled"
    );
    let ending = Instant::now();
    release.write_all(b"end\n").unwrap();
    let ended = launcher.join().unwrap().unwrap();
    assert_eq!(
        ended,
        EndedGroups {
            signalled: 2,
            left_out: 0
        }
    );
    assert!(
        ending.elapsed() >= TERM_GRACE,
        "a leader that ignores SIGTERM was killed before the grace passed"
    );
    let signal =
        |child: &mut CommandChild| child.try_wait().unwrap().and_then(|status| status.signal());
    assert_eq!(signal(&mut left), Some(nix::libc::SIGTERM));
    assert_eq!(signal(&mut stubborn), Some(nix::libc::SIGKILL));
    assert!(
        settled.try_wait().unwrap().is_none(),
        "a group reported settled was signalled"
    );
    assert!(
        other.try_wait().unwrap().is_none(),
        "a group reported with another start was signalled"
    );
    for child in [&mut settled, &mut other, &mut owner] {
        child.settle().unwrap();
    }
}
