//! Boundary evidence for the `/proc` reads: a child's start stays put, is listed among this
//! process's children and reads as gone once ended, before and after it is reaped; another
//! account's process keeps its program unread; and a command name that holds `)` and spaces
//! leaves the fields after it intact.
use super::{Fields, Started, children, program_file_name, program_path, started};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn a_child_keeps_its_start_and_reads_gone_once_ended() {
    let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let Started::At(start) = started(child.id()).unwrap() else {
        panic!("the child runs");
    };
    assert_eq!(started(child.id()).unwrap(), Started::At(start));
    assert_ne!(started(std::process::id()).unwrap(), Started::At(start));
    assert_eq!(program_file_name(child.id()).unwrap(), "sleep");
    assert!(children(std::process::id()).unwrap().contains(&child.id()));
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
fn another_accounts_process_keeps_its_program_unread() {
    // Process 1 is root's init; these tests never run as root.
    assert!(matches!(started(1).unwrap(), Started::At(_)));
    assert_eq!(program_path(1), None);
}

#[test]
fn a_command_name_with_a_parenthesis_and_spaces_keeps_the_fields_after_it() {
    let mut stat = b"4242 (a) b (c) S 17".to_vec();
    stat.extend(b" 0".repeat(17));
    stat.extend(b" 123456 0 0\n");
    assert_eq!(
        Fields::parse(&stat).unwrap(),
        Fields {
            ended: false,
            parent: 17,
            start_ticks: 123_456,
        }
    );
}
