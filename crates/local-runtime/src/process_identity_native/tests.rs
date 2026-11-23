//! Boundary evidence for the reviewed libproc adapter: a child's start time falls within its spawn
//! and stays put, an ended child reads as gone before and after it is reaped, and another account's
//! process reads as denied while its program stays readable.
use super::{Started, program_file_name, program_path, started};
use std::{
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn now() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(since.as_micros()).unwrap()
}

#[test]
fn a_child_starts_within_its_spawn_and_reads_gone_once_ended() {
    let before = now();
    let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let after = now();
    let Started::At(start) = started(child.id()).unwrap() else {
        panic!("the child runs");
    };
    // One second of slack covers a clock adjustment between the readings.
    assert!(before.saturating_sub(1_000_000) <= start && start <= after + 1_000_000);
    assert_eq!(started(child.id()).unwrap(), Started::At(start));
    assert_eq!(program_file_name(child.id()).unwrap(), "sleep");
    child.kill().unwrap();
    // Killed but not yet reaped, the child is a zombie.
    let until = Instant::now() + Duration::from_secs(5);
    while started(child.id()).unwrap() != Started::Gone {
        assert!(
            Instant::now() < until,
            "the killed child still reads as running"
        );
        thread::sleep(Duration::from_millis(10));
    }
    child.wait().unwrap();
    assert_eq!(started(child.id()).unwrap(), Started::Gone);
}

#[test]
fn another_accounts_process_is_denied_but_names_its_program() {
    // Process 1 is root's launchd; these tests never run as root.
    assert_eq!(started(1).unwrap(), Started::Denied);
    assert_eq!(program_path(1).as_deref(), Some(Path::new("/sbin/launchd")));
}
