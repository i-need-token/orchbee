#![cfg(target_os = "linux")]

#[path = "local_startup_process/native_close.rs"]
mod native_close;
#[path = "local_startup_process/native_cohort.rs"]
mod native_cohort;
#[path = "local_startup_process/native_context.rs"]
mod native_context;
#[path = "local_startup_process/native_location_labels.rs"]
mod native_location_labels;
#[path = "local_startup_process/native_workspace_fixture.rs"]
mod native_workspace_fixture;
#[path = "local_startup_process/native_workspace_setup.rs"]
mod native_workspace_setup;
#[path = "local_startup_process/pty_client.rs"]
mod pty_client;

use native_cohort::{Fixture, ManagedProcess};
use orchbee_local_client_protocol::ClientSessionConnection;
use orchbee_local_site_runtime::{LockAttempt, NativeCohort};
use pty_client::{Client, ReadyMode, wait_file};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn Error>>;

const STARTUP_OR_CREATE_TIMEOUT: Duration = Duration::from_secs(22);
const SWITCH_TIMEOUT: Duration = Duration::from_secs(10);
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(5);
const EXIT_TIMEOUT: Duration = Duration::from_secs(3);

// Bound process-witness scheduling: unrelated cohort hashing and launches must not compete
// inside these tests' deadlines. Each test acquires this before creating fixtures or deadlines.
static PROCESS_WITNESS_LOCK: Mutex<()> = Mutex::new(());

fn process_witness_guard() -> MutexGuard<'static, ()> {
    // Only scheduling is shared; a failed witness must not prevent later independent fixtures.
    PROCESS_WITNESS_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

#[test]
fn native_rename_visible_editor_updates_canonical_label_and_preserves_live_shell() {
    let _guard = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    let identities = create_three_shells(&fixture, &mut client);
    let checkpoint = client.checkpoint();
    client.send(b"\x02,\x15renamed shell");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "Rename Tab",
    );
    assert!(client.contains("renamed shell"));
    assert!(client.contains("Enter"));
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    client.wait_ordered_ready_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["1", "2", "renamed shell"],
        3,
        ReadyMode::Controlling,
    );
    // Respect the accepted post-submit quiet handoff before sending new unframed shell input.
    thread::sleep(Duration::from_millis(75));
    assert_shell_survives(&fixture, &mut client, &identities[2], "renamed-shell");
    let checkpoint = client.checkpoint();
    client.send(b"\x02w");
    client.wait_ordered_navigator_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &["1", "2", "renamed shell"],
        3,
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
}

#[test]
fn two_tabs_keep_distinct_shells_across_previous_next_and_numbered_switches() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(0, startup_deadline, 1, "1", ReadyMode::Controlling);

    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "one", "tab-1-before", true).as_bytes());
    let tab_1_before = wait_file(
        &fixture.base.join("tab-1-before"),
        &mut [&mut client],
        shell_deadline,
    );
    let tab_1_pid = require_shell_identity(&tab_1_before, "one");

    let create_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let checkpoint = client.checkpoint();
    client.send(b"\x02c");
    client.wait_ready_after(checkpoint, create_deadline, 2, "2", ReadyMode::Controlling);
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "two", "tab-2-before", true).as_bytes());
    let tab_2_before = wait_file(
        &fixture.base.join("tab-2-before"),
        &mut [&mut client],
        shell_deadline,
    );
    let tab_2_pid = require_shell_identity(&tab_2_before, "two");
    assert_ne!(
        tab_1_pid, tab_2_pid,
        "the two Tabs must own distinct shells"
    );

    switch_and_wait(&mut client, b"\x02p", 1, "1");
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "one", "tab-1-after-previous", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("tab-1-after-previous"),
            &mut [&mut client],
            shell_deadline,
        ),
        tab_1_before
    );

    switch_and_wait(&mut client, b"\x02n", 2, "2");
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "two", "tab-2-after-next", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("tab-2-after-next"),
            &mut [&mut client],
            shell_deadline,
        ),
        tab_2_before
    );

    switch_and_wait(&mut client, b"\x021", 1, "1");
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "one", "tab-1-after-number", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("tab-1-after-number"),
            &mut [&mut client],
            shell_deadline,
        ),
        tab_1_before
    );

    switch_and_wait(&mut client, b"\x022", 2, "2");
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(shell_identity_command(&fixture, "two", "tab-2-after-number", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("tab-2-after-number"),
            &mut [&mut client],
            shell_deadline,
        ),
        tab_2_before
    );

    let tab_row = client.current_row_containing("[*2 2]");
    assert_eq!(tab_row.matches("[ 1 1]").count(), 1, "{tab_row:?}");
    assert_eq!(tab_row.matches("[*2 2]").count(), 1, "{tab_row:?}");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
}

#[test]
fn second_live_client_refreshes_shared_tabs_and_preserves_independent_selection() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut creator = Client::spawn(&fixture, "en_US.UTF-8");
    creator.wait_ready_after(0, startup_deadline, 1, "1", ReadyMode::Controlling);
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    creator.send(shell_identity_command(&fixture, "one", "shared-tab-1", true).as_bytes());
    let tab_1 = wait_file(
        &fixture.base.join("shared-tab-1"),
        &mut [&mut creator],
        shell_deadline,
    );
    require_shell_identity(&tab_1, "one");

    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut peer = Client::spawn(&fixture, "en_US.UTF-8");
    peer.wait_ready_after(0, startup_deadline, 1, "1", ReadyMode::ViewOnly);

    let create_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let peer_checkpoint = peer.checkpoint();
    let checkpoint = creator.checkpoint();
    creator.send(b"\x02c");
    creator.wait_ready_after(checkpoint, create_deadline, 2, "2", ReadyMode::Controlling);
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    creator.send(shell_identity_command(&fixture, "two", "shared-tab-2", true).as_bytes());
    let tab_2 = wait_file(
        &fixture.base.join("shared-tab-2"),
        &mut [&mut creator, &mut peer],
        shell_deadline,
    );
    let tab_1_pid = require_shell_identity(&tab_1, "one");
    let tab_2_pid = require_shell_identity(&tab_2, "two");
    assert_ne!(
        tab_1_pid, tab_2_pid,
        "the two Tabs must own distinct shells"
    );

    // Committed publications propagate to the live peer without an explicit refresh,
    // while the peer keeps its independent selection on the first Tab.
    peer.wait_ordered_ready_after(
        peer_checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        &["1", "2"],
        1,
        ReadyMode::ViewOnly,
    );

    let refresh_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02w");
    let revision = peer.wait_navigator_refreshed_after(checkpoint, refresh_deadline);
    assert!(revision > 0, "a refreshed snapshot has a positive revision");

    let close_navigator_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let checkpoint = peer.checkpoint();
    peer.send(b"\x02w");
    peer.wait_navigator_closed_after(
        checkpoint,
        close_navigator_deadline,
        1,
        "1",
        ReadyMode::ViewOnly,
    );

    switch_and_wait(&mut creator, b"\x02p", 1, "1");
    switch_and_wait(&mut peer, b"\x02n", 2, "2");

    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    creator.send(shell_identity_command(&fixture, "one", "shared-tab-1-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("shared-tab-1-after"),
            &mut [&mut creator, &mut peer],
            shell_deadline,
        ),
        tab_1
    );
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    peer.send(shell_identity_command(&fixture, "two", "shared-tab-2-after", false).as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("shared-tab-2-after"),
            &mut [&mut creator, &mut peer],
            shell_deadline,
        ),
        tab_2
    );

    peer.close_before(Instant::now() + EXIT_TIMEOUT);
    creator.close_before(Instant::now() + EXIT_TIMEOUT);
}

#[test]
fn moving_tabs_in_both_directions_and_across_boundaries_preserves_shells_and_resize() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    let identities = create_three_shells(&fixture, &mut client);
    assert_shell_survives(&fixture, &mut client, &identities[2], "before-first-move");
    let original_size = shell_size(&fixture, &mut client, "size-before-move");

    // Each expected order is explicit: interior swaps and both wrap directions must preserve C.
    for (step, (command, labels, active_position)) in [
        (b"\x02<", ["1", "3", "2"], 2),
        (b"\x02<", ["3", "1", "2"], 1),
        (b"\x02<", ["1", "2", "3"], 3),
        (b"\x02>", ["3", "1", "2"], 1),
        (b"\x02>", ["1", "3", "2"], 2),
        (b"\x02>", ["1", "2", "3"], 3),
    ]
    .into_iter()
    .enumerate()
    {
        let checkpoint = client.checkpoint();
        let deadline = Instant::now() + SWITCH_TIMEOUT;
        client.send(command);
        client.wait_ordered_ready_after(
            checkpoint,
            deadline,
            &labels,
            active_position,
            ReadyMode::Controlling,
        );
        assert_shell_survives(
            &fixture,
            &mut client,
            &identities[2],
            &format!("move-{step}"),
        );
    }

    let checkpoint = client.checkpoint();
    let resize_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.resize_rows(30);
    client.wait_resized_ready_after(checkpoint, resize_deadline, 30, 3);
    let resized = shell_size(&fixture, &mut client, "size-after-move");
    assert_eq!(resized, (original_size.0 - 2, original_size.1));
    assert_shell_survives(&fixture, &mut client, &identities[2], "resized-moved-shell");

    for (index, command) in [b"\x021", b"\x022"].into_iter().enumerate() {
        select_ordered_tab(&mut client, command, &["1", "2", "3"], index + 1);
        assert_shell_survives(
            &fixture,
            &mut client,
            &identities[index],
            &format!("unmoved-{index}"),
        );
    }
    client.close_before(Instant::now() + EXIT_TIMEOUT);
}

#[test]
fn moved_order_refresh_preserves_peer_selection_and_reopen_selects_new_canonical_first() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut mover = Client::spawn(&fixture, "en_US.UTF-8");
    let identities = create_three_shells(&fixture, &mut mover);
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut peer = Client::spawn(&fixture, "en_US.UTF-8");
    peer.wait_ordered_ready_after(
        0,
        startup_deadline,
        &["1", "2", "3"],
        1,
        ReadyMode::Controlling,
    );
    assert_shell_survives(&fixture, &mut peer, &identities[0], "peer-before-move");

    let checkpoint = mover.checkpoint();
    let move_deadline = Instant::now() + SWITCH_TIMEOUT;
    mover.send(b"\x02>");
    mover.wait_ordered_ready_after(
        checkpoint,
        move_deadline,
        &["3", "1", "2"],
        1,
        ReadyMode::Controlling,
    );
    assert_shell_survives(&fixture, &mut mover, &identities[2], "mover-after-wrap");

    // Another Client learns order only through explicit refresh, keeping its selected A by ID.
    let stale_row = peer.current_row_containing("[*1 1]");
    assert!(stale_row.contains("[*1 1] [ 2 2] [ 3 3]"), "{stale_row:?}");
    let checkpoint = peer.checkpoint();
    let refresh_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    peer.send(b"\x02w");
    let revision =
        peer.wait_ordered_navigator_after(checkpoint, refresh_deadline, &["3", "1", "2"], 2);
    assert!(revision > 0);
    let checkpoint = peer.checkpoint();
    let close_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    peer.send(b"\x02w");
    peer.wait_navigator_closed_after(checkpoint, close_deadline, 2, "1", ReadyMode::Controlling);
    assert_shell_survives(&fixture, &mut peer, &identities[0], "peer-after-refresh");
    assert_shell_survives(
        &fixture,
        &mut mover,
        &identities[2],
        "mover-after-peer-refresh",
    );

    // Open still retains bootstrap A, now second. Fresh startup must attach snapshot head C.
    let server = one_process(&fixture, "orchbee-server");
    let worker = one_process(&fixture, "orchbee-worker");
    let host = one_process(&fixture, "orchbee-terminal-host");
    peer.close_before(Instant::now() + EXIT_TIMEOUT);
    mover.close_before(Instant::now() + EXIT_TIMEOUT);
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut reopened = Client::spawn(&fixture, "en_US.UTF-8");
    reopened.wait_ordered_ready_after(
        0,
        startup_deadline,
        &["3", "1", "2"],
        1,
        ReadyMode::Controlling,
    );
    assert_shell_survives(
        &fixture,
        &mut reopened,
        &identities[2],
        "reopened-canonical-head",
    );
    assert_process_retained(&fixture, "orchbee-server", &server);
    assert_process_retained(&fixture, "orchbee-worker", &worker);
    assert_process_retained(&fixture, "orchbee-terminal-host", &host);
    select_ordered_tab(&mut reopened, b"\x02n", &["3", "1", "2"], 2);
    assert_shell_survives(
        &fixture,
        &mut reopened,
        &identities[0],
        "reopened-bootstrap-second",
    );
    select_ordered_tab(&mut reopened, b"\x023", &["3", "1", "2"], 3);
    assert_shell_survives(
        &fixture,
        &mut reopened,
        &identities[1],
        "reopened-third-position",
    );
    reopened.close_before(Instant::now() + EXIT_TIMEOUT);
}

fn create_three_shells(fixture: &Fixture, client: &mut Client) -> [String; 3] {
    client.wait_ordered_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        &["1"],
        1,
        ReadyMode::Controlling,
    );
    let identities = std::array::from_fn(|index| {
        if index > 0 {
            let checkpoint = client.checkpoint();
            let deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
            client.send(b"\x02c");
            client.wait_ordered_ready_after(
                checkpoint,
                deadline,
                &["1", "2", "3"][..=index],
                index + 1,
                ReadyMode::Controlling,
            );
        }
        let marker = ["one", "two", "three"][index];
        let filename = format!("move-shell-{index}-before");
        let deadline = Instant::now() + OBSERVATION_TIMEOUT;
        client.send(shell_identity_command(fixture, marker, &filename, true).as_bytes());
        let identity = wait_file(&fixture.base.join(filename), &mut [client], deadline);
        require_shell_identity(&identity, marker);
        identity
    });
    for first in 0..identities.len() {
        for second in first + 1..identities.len() {
            assert_ne!(
                identities[first].split_once(':').unwrap().0,
                identities[second].split_once(':').unwrap().0,
                "three Tabs must own three distinct shell processes"
            );
        }
    }
    identities
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

fn shell_size(fixture: &Fixture, client: &mut Client, filename: &str) -> (u16, u16) {
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    client.send(
        format!(
            "OB_TEST_SIZE=$(stty size); printf '%s' \"$OB_TEST_SIZE\" > {}\r",
            fixture.base.join(filename).display()
        )
        .as_bytes(),
    );
    let size = wait_file(&fixture.base.join(filename), &mut [client], deadline);
    let (rows, columns) = size
        .split_once(' ')
        .unwrap_or_else(|| panic!("invalid shell size {size:?}"));
    (rows.parse().unwrap(), columns.parse().unwrap())
}

fn select_ordered_tab(client: &mut Client, command: &[u8], labels: &[&str], position: usize) {
    let checkpoint = client.checkpoint();
    let deadline = Instant::now() + SWITCH_TIMEOUT;
    client.send(command);
    client.wait_ordered_ready_after(
        checkpoint,
        deadline,
        labels,
        position,
        ReadyMode::Controlling,
    );
}

fn switch_and_wait(client: &mut Client, command: &[u8], tab_number: usize, safe_label: &str) {
    assert_eq!(command.len(), 2, "one decoded prefix action per switch");
    let deadline = Instant::now() + SWITCH_TIMEOUT;
    let checkpoint = client.checkpoint();
    client.send(command);
    client.wait_ready_after(
        checkpoint,
        deadline,
        tab_number,
        safe_label,
        ReadyMode::Controlling,
    );
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

#[test]
fn simultaneous_bare_clients_share_one_site_and_reopen_the_same_shell() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut first = Client::spawn(&fixture, "en_US.UTF-8");
    let mut second = Client::spawn(&fixture, "en_US.UTF-8");
    let first_mode = first.ready_before(startup_deadline);
    let second_mode = second.ready_before(startup_deadline);
    assert_ne!(first_mode, second_mode);
    let controller = if first_mode == ReadyMode::Controlling {
        &mut first
    } else {
        &mut second
    };
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    controller.send(
        format!(
            "OB_RETAINED=alive; printf '%s' \"$$\" > {}/pid-before\r",
            fixture.base.display()
        )
        .as_bytes(),
    );
    let original_pid = wait_file(
        &fixture.base.join("pid-before"),
        &mut [&mut first, &mut second],
        shell_deadline,
    );
    let process_names = ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"];
    let retained_processes = process_names.map(|name| one_process(&fixture, name));
    first.close_before(Instant::now() + EXIT_TIMEOUT);
    second.close_before(Instant::now() + EXIT_TIMEOUT);
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut reopened = Client::spawn(&fixture, "en_US.UTF-8");
    reopened.wait_ready_after(0, startup_deadline, 1, "1", ReadyMode::Controlling);
    for (name, expected) in process_names.into_iter().zip(retained_processes) {
        assert_process_retained(&fixture, name, &expected);
    }
    let shell_deadline = Instant::now() + OBSERVATION_TIMEOUT;
    reopened.send(
        format!(
            "printf '%s:%s' \"$$\" \"$OB_RETAINED\" > {}/pid-after\r",
            fixture.base.display()
        )
        .as_bytes(),
    );
    assert_eq!(
        wait_file(
            &fixture.base.join("pid-after"),
            &mut [&mut reopened],
            shell_deadline,
        ),
        format!("{original_pid}:alive")
    );
    reopened.close_before(Instant::now() + EXIT_TIMEOUT);
}

#[test]
fn server_and_worker_readiness_does_not_start_a_host() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let site = fixture.site();
    site.commit_activation()?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let cohort = NativeCohort::load(&fixture.cohort, deadline)
        .map_err(|error| format!("fixture cohort admission: {error}"))?;
    let mut server = Command::new(fixture.cohort.join("orchbee-server"))
        .arg("--local-site")
        .arg(fixture.runtime())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    loop {
        if site.client_socket_exists()? {
            break;
        }
        assert!(server.try_wait()?.is_none());
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    let connection = ClientSessionConnection::connect_before(
        site.client_socket_path(),
        cohort.identity(),
        deadline,
    )
    .map_err(|error| format!("managed Client admission: {error}"))?;
    assert_eq!(fixture.processes("orchbee-worker").len(), 1);
    assert!(fixture.processes("orchbee-terminal-host").is_empty());
    assert!(!fixture.runtime().join("host").exists());
    // Managed admission is permanently fenced after the initial Worker.
    assert!(std::os::unix::net::UnixStream::connect(site.worker_socket_path()).is_err());
    drop(connection);
    server.kill()?;
    server.wait()?;
    Ok(())
}

#[test]
fn committed_failed_startup_requires_recovery_and_does_not_launch_again() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    fixture.site().commit_activation().unwrap();
    for locale in ["en_US.UTF-8", "zh_CN.UTF-8"] {
        let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
        let mut client = Client::spawn(&fixture, locale);
        let error = client.exit_message_before(startup_deadline);
        assert!(error.contains(if locale.starts_with("en") {
            "requires explicit recovery"
        } else {
            "需要显式恢复"
        }));
        assert!(fixture.processes("orchbee-server").is_empty());
        assert!(!fixture.runtime().join("worker.sock").exists());
    }
}

#[test]
fn activation_contention_uses_one_deadline_and_delayed_progress() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let site = fixture.site();
    let LockAttempt::Acquired(_guard) = site.try_acquire_activation_lock().unwrap() else {
        panic!("fixture lock")
    };
    let started = Instant::now();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    let error = client.exit_message_before(started + Duration::from_secs(24));
    assert!(
        started.elapsed() >= Duration::from_secs(19) && started.elapsed() < Duration::from_secs(24)
    );
    assert!(error.contains("Starting the local terminal"));
    assert!(error.contains("startup timed out"));
    assert!(!site.activation_started().unwrap());
    assert!(fixture.processes("orchbee-server").is_empty());
}

#[test]
fn changed_cohort_fails_before_runtime_creation() {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    fs::write(fixture.cohort.join("orchbee-terminal-host"), b"mixed Host").unwrap();
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    assert!(
        client
            .exit_message_before(startup_deadline)
            .contains("cohort is missing, changed, or invalid")
    );
    assert!(!fixture.runtime().exists());
    assert!(fixture.processes("orchbee-server").is_empty());
}

#[test]
fn lazy_worker_rejects_invalid_launches_then_preserves_its_first_dispatch_and_replay() -> TestResult
{
    let _process_witness = process_witness_guard();
    use orchbee_capability_node_protocol::{
        AuthorizedTerminalLaunch, CapabilityNodeMessage, LeaseExpiresAtUnixMilliseconds, LeaseId,
        OperationId, TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId,
        TerminalLaunchMode, TerminalLaunchOutcome, TerminalOperationAuthorization,
        TerminalOperationLease, WorkerGeneration, WorkerId, read_message as read_obcn,
        write_message as write_obcn,
    };
    use orchbee_local_site_protocol::{LocalSiteMessage, read_message, write_message};
    use std::{
        num::NonZeroU64,
        os::unix::net::UnixListener,
        time::{SystemTime, UNIX_EPOCH},
    };
    let fixture = Fixture::new();
    let site = fixture.site();
    site.commit_activation()?;
    let listener = UnixListener::bind(site.worker_socket_path())?;
    fs::set_permissions(site.worker_socket_path(), fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let cohort_identity =
        NativeCohort::load(&fixture.cohort, Instant::now() + Duration::from_secs(20))?
            .identity()
            .to_owned();
    let mut worker = Command::new(fixture.cohort.join("orchbee-worker"))
        .arg("--local-site")
        .arg(fixture.runtime())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut server = loop {
        match listener.accept() {
            Ok((server, _)) => break server,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
        assert!(worker.try_wait()?.is_none());
        assert!(Instant::now() < deadline);
        thread::yield_now();
    };
    server.set_read_timeout(Some(Duration::from_secs(7)))?;
    server.set_write_timeout(Some(Duration::from_secs(2)))?;
    let LocalSiteMessage::WorkerHello(hello) = read_message(&mut server)? else {
        panic!("Worker hello")
    };
    assert_eq!(hello.cohort(), cohort_identity);
    write_message(
        &mut server,
        &LocalSiteMessage::WorkerAccepted {
            generation: NonZeroU64::MIN,
        },
    )?;
    let expiry = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())? + 60_000;
    let launch = |id: u8, generation: u64, expiry: u64| -> Result<_, Box<dyn Error>> {
        Ok(CapabilityNodeMessage::AuthorizedTerminalLaunch(
            AuthorizedTerminalLaunch::new(
                TerminalOperationAuthorization::new(
                    TerminalId::from_bytes([id; 16])?,
                    OperationId::from_bytes([id; 16])?,
                    WorkerId::from_bytes(*b"orchbee-local-v1")?,
                    WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
                    TerminalFence::new(NonZeroU64::MIN),
                    TerminalOperationLease::new(
                        LeaseId::from_bytes([id; 16])?,
                        LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(expiry).unwrap()),
                    ),
                    TERMINAL_LAUNCH_CAPABILITY_VERSION,
                ),
                None,
                TerminalLaunchMode::Deferred,
            ),
        ))
    };
    for request in [launch(1, 1, 1)?, launch(2, 2, expiry)?] {
        write_obcn(&mut server, &request)?;
        let CapabilityNodeMessage::TerminalLaunchResult(result) = read_obcn(&mut server)? else {
            panic!("Launch result")
        };
        assert!(matches!(
            result.outcome(),
            TerminalLaunchOutcome::Rejected(_)
        ));
        assert!(fixture.processes("orchbee-terminal-host").is_empty());
        assert!(!fixture.runtime().join("host").exists());
    }
    let request = launch(3, 1, expiry)?;
    write_obcn(&mut server, &request)?;
    let result = read_obcn(&mut server)?;
    assert!(
        matches!(&result, CapabilityNodeMessage::TerminalLaunchResult(result)
        if matches!(result.outcome(), TerminalLaunchOutcome::HostAccepted(_)))
    );
    write_obcn(&mut server, &request)?;
    assert_eq!(read_obcn(&mut server)?, result);
    assert_eq!(fixture.processes("orchbee-terminal-host").len(), 1);
    drop(server);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = worker.try_wait()? {
            assert_eq!(status.code(), Some(1));
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(fixture.processes("orchbee-terminal-host").len(), 1);
    Ok(())
}

#[test]
fn managed_process_loss_never_starts_a_replacement() -> TestResult {
    let _process_witness = process_witness_guard();
    for failed_role in ["orchbee-server", "orchbee-worker", "orchbee-terminal-host"] {
        eprintln!("managed process loss partition: {failed_role}");
        let fixture = Fixture::new();
        let mut client = Client::spawn(&fixture, "en_US.UTF-8");
        client.ready_before(Instant::now() + STARTUP_OR_CREATE_TIMEOUT);
        client.close_before(Instant::now() + EXIT_TIMEOUT);
        let previous_server = one_process(&fixture, "orchbee-server");
        let previous_host = one_process(&fixture, "orchbee-terminal-host");
        let failed = one_process(&fixture, failed_role);
        failed.terminate()?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if failed.exited()? && fixture.processes("orchbee-worker").is_empty() {
                assert!(Instant::now() < deadline, "managed processes retired late");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "failed managed process did not retire"
            );
            thread::yield_now();
        }
        // A reopening Client may legitimately contend for activation within the original 20 s
        // startup deadline. Holding this existing lock beyond 5 s exposes a premature observer.
        let site = fixture.site();
        let activation = if failed_role == "orchbee-server" {
            let LockAttempt::Acquired(guard) = site.try_acquire_activation_lock()? else {
                panic!("fixture activation lock")
            };
            Some(guard)
        } else {
            None
        };
        let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
        let mut reopened = Client::spawn(&fixture, "en_US.UTF-8");
        let error = thread::scope(|scope| {
            if let Some(guard) = activation {
                scope.spawn(move || {
                    thread::sleep(Duration::from_secs(6));
                    drop(guard);
                });
            }
            reopened.exit_message_before(startup_deadline)
        });
        if failed_role == "orchbee-server" {
            assert!(error.contains("requires explicit recovery"));
            assert!(fixture.processes("orchbee-server").is_empty());
        } else {
            assert!(error.contains("terminal is unavailable"));
            assert_process_retained(&fixture, "orchbee-server", &previous_server);
        }
        assert!(fixture.processes("orchbee-worker").is_empty());
        if failed_role == "orchbee-terminal-host" {
            assert!(fixture.processes("orchbee-terminal-host").is_empty());
        } else {
            assert_process_retained(&fixture, "orchbee-terminal-host", &previous_host);
        }
    }
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
    assert!(
        !expected.exited().unwrap(),
        "original {name} process exited during identity verification"
    );
}

#[test]
fn help_version_and_non_tty_use_no_runtime_or_processes() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    for arguments in [vec!["--help"], vec!["--version"], vec![]] {
        let output = Command::new(fixture.cohort.join("orchbee"))
            .args(&arguments)
            .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
            .env("LC_ALL", "en_US.UTF-8")
            .output()?;
        assert_eq!(
            output.status.code(),
            Some(if arguments.is_empty() { 1 } else { 0 })
        );
        assert!(!fixture.runtime().exists());
        assert!(fixture.processes("orchbee-server").is_empty());
    }
    Ok(())
}

#[test]
fn incompatible_warm_cohort_is_rejected_without_replacing_the_running_site() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut first = Client::spawn(&fixture, "en_US.UTF-8");
    first.ready_before(Instant::now() + STARTUP_OR_CREATE_TIMEOUT);
    first.close_before(Instant::now() + EXIT_TIMEOUT);
    let server = one_process(&fixture, "orchbee-server");
    let host = one_process(&fixture, "orchbee-terminal-host");
    let other = Fixture::new();
    fs::OpenOptions::new()
        .append(true)
        .open(other.cohort.join("orchbee-terminal-host"))?
        .write_all(b"another-build")?;
    other.manifest();
    let startup_deadline = Instant::now() + STARTUP_OR_CREATE_TIMEOUT;
    let mut second = Client::spawn_at(&other, "en_US.UTF-8", &fixture.base);
    assert!(
        second
            .exit_message_before(startup_deadline)
            .contains("incompatible with this development cohort")
    );
    assert_process_retained(&fixture, "orchbee-server", &server);
    assert_process_retained(&fixture, "orchbee-terminal-host", &host);
    assert!(other.processes("orchbee-server").is_empty());
    Ok(())
}
