#![cfg(target_os = "linux")]
// The shared process/PTY harness covers more surfaces than this witness uses.
#![allow(dead_code)]

//! Real process/PTY witness for peer-publication refresh (D1-R). A second live Client must
//! install the snapshot a peer committed — with no manual refresh and no second mutation —
//! and keep its own input path correct afterwards. Scripted Busy/Full admission evidence
//! lives in the deterministic unit suites; this file covers the real Linux PTY +
//! Server/Worker/Host two-Client path only.

#[path = "support/inherited_environment.rs"]
mod inherited_environment;
#[path = "local_startup_process/native_cohort.rs"]
mod native_cohort;
#[path = "local_startup_process/pty_client.rs"]
mod pty_client;
#[path = "support/mod.rs"]
mod support;
#[path = "support/tui_observer.rs"]
mod tui_observer;
#[path = "local_startup_process/workbench_peer.rs"]
mod workbench_peer;

use native_cohort::{Fixture, ManagedProcess};
use pty_client::{Client, ReadyMode, wait_file};
use std::{
    error::Error,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn Error>>;

const STARTUP_OR_CREATE_TIMEOUT: Duration = Duration::from_secs(22);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(8);
const EXIT_TIMEOUT: Duration = Duration::from_secs(3);

// Bound process-witness scheduling inside this target; unrelated cohort hashing and launches
// must not compete inside these tests' deadlines.
static PROCESS_WITNESS_LOCK: Mutex<()> = Mutex::new(());

fn process_witness_guard() -> MutexGuard<'static, ()> {
    PROCESS_WITNESS_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// One committed peer publication reaches the observer's native UI on its own: the observer
/// never opens the navigator, never sends `w`, and commits nothing itself. After the install
/// its input still lands on its own selected Tab, and both Clients' shells stay distinct.
#[test]
fn peer_publication_installs_on_observer_without_manual_refresh() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();

    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut creator = Client::spawn(&fixture, "en_US.UTF-8");
    creator.wait_ready_after(0, startup_deadline, 1, "1", ReadyMode::Controlling);
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    creator.send(shell_identity_command(&fixture, "one", "pub-tab-1", true).as_bytes());
    let tab_1 = wait_file(
        &fixture.base.join("pub-tab-1"),
        &mut [&mut creator],
        shell_deadline,
    );
    require_shell_identity(&tab_1, "one");

    // The creator moves onto a second Tab so the observer attaches to the uncontrolled first
    // Tab and holds Controlling on it.
    let create_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let checkpoint = creator.checkpoint();
    creator.send(b"\x02c");
    creator.wait_ordered_ready_after(
        checkpoint,
        create_deadline,
        &["1", "2"],
        2,
        ReadyMode::Controlling,
    );
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    creator.send(shell_identity_command(&fixture, "two", "pub-tab-2", true).as_bytes());
    let tab_2 = wait_file(
        &fixture.base.join("pub-tab-2"),
        &mut [&mut creator],
        shell_deadline,
    );
    assert_ne!(
        require_shell_identity(&tab_1, "one"),
        require_shell_identity(&tab_2, "two"),
        "the two Tabs must own distinct shells"
    );

    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut observer = Client::spawn(&fixture, "en_US.UTF-8");
    observer.wait_ordered_ready_after(0, startup_deadline, &["1", "2"], 1, ReadyMode::Controlling);
    let server = one_process(&fixture, "orchbee-server");
    let worker = one_process(&fixture, "orchbee-worker");
    let host = one_process(&fixture, "orchbee-terminal-host");

    // The peer commits exactly one structural change. The checkpoint precedes it, so only
    // output produced after the commit can satisfy the observer's membership rows.
    let observer_checkpoint = observer.checkpoint();
    let create_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let creator_checkpoint = creator.checkpoint();
    creator.send(b"\x02c");
    creator.wait_ordered_ready_after(
        creator_checkpoint,
        create_deadline,
        &["1", "2", "3"],
        3,
        ReadyMode::Controlling,
    );

    // The observer installs the publication itself: no navigator open, no `w`, no second
    // mutation. Its selection stays on its own Tab while the committed membership appears.
    observer.wait_ordered_ready_after(
        observer_checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["1", "2", "3"],
        1,
        ReadyMode::Controlling,
    );

    // Input still lands on the observer's selected Tab after the install.
    assert_shell_survives(&fixture, &mut observer, &tab_1, "pub-observer-input");
    assert_process_retained(&fixture, "orchbee-server", &server);
    assert_process_retained(&fixture, "orchbee-worker", &worker);
    assert_process_retained(&fixture, "orchbee-terminal-host", &host);

    eprintln!(
        "publication witness: server={} worker={} host={} tab1={} tab2={} observer-input={:?}",
        server.pid(),
        worker.pid(),
        host.pid(),
        tab_1.trim(),
        tab_2.trim(),
        std::fs::read_to_string(fixture.base.join("pub-observer-input"))
            .unwrap_or_default()
            .trim()
    );

    observer.close_before(Instant::now() + EXIT_TIMEOUT);
    creator.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

fn one_process(fixture: &Fixture, name: &str) -> ManagedProcess {
    let mut processes = fixture.process_handles(name).unwrap();
    assert_eq!(processes.len(), 1, "expected one exact {name} process");
    processes.pop().unwrap()
}

fn assert_process_retained(fixture: &Fixture, name: &str, expected: &ManagedProcess) {
    assert!(
        !expected.exited().unwrap(),
        "original {name} process exited"
    );
    let current = one_process(fixture, name);
    assert_eq!(
        current.pid(),
        expected.pid(),
        "warm operation replaced the exact {name} process"
    );
}

fn assert_shell_survives(
    fixture: &Fixture,
    client: &mut Client,
    expected: &str,
    checkpoint_name: &str,
) {
    let checkpoint = client.checkpoint();
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    // Split the output token so terminal echo of the command cannot satisfy the frame witness.
    client.send(
        format!(
            "{}; printf '%s%s\\n' 'OB_MOVE_' '{checkpoint_name}'\r",
            shell_identity_command(fixture, "", checkpoint_name, false).trim_end_matches('\r')
        )
        .as_bytes(),
    );
    assert_eq!(
        wait_file(&fixture.base.join(checkpoint_name), &mut [client], deadline),
        expected
    );
    client.wait_terminal_output_after(checkpoint, deadline, &format!("OB_MOVE_{checkpoint_name}"));
}

fn shell_identity_command(fixture: &Fixture, marker: &str, filename: &str, assign: bool) -> String {
    let assignment = if assign {
        format!("OB_TEST_TAB={marker}; ")
    } else {
        String::new()
    };
    format!(
        "{assignment}printf '%s:%s' \"$$\" \"$OB_TEST_TAB\" > {}\r",
        fixture.base.join(filename).display()
    )
}

fn require_shell_identity(value: &str, expected_marker: &str) -> u32 {
    let (pid, marker) = value
        .split_once(':')
        .unwrap_or_else(|| panic!("invalid shell identity {value:?}"));
    assert_eq!(marker, expected_marker, "shell-local marker changed");
    let pid = pid
        .parse::<u32>()
        .unwrap_or_else(|error| panic!("invalid shell PID {pid:?}: {error}"));
    assert_ne!(pid, 0, "shell PID must be positive");
    pid
}
