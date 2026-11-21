//! Worker replacement across a real managed Session: the CLI, Server, Worker and Zig Host under
//! test, a terminal that counts once a second, and a TUI Client of that Session running in a
//! terminal of a second Session. A Worker killed by its process ID is replaced, the Host and its
//! program keep running, and the Client shows the count again by itself without repeating a line.
//! Past the cap the Server gives up and leaves the Host running; the confirmed stop then ends it,
//! and the next start restores the layout. The oracle is what the CLI reads and lists; nothing
//! imports Server internals.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use session::{Fixture, system_program};
use std::{
    fs, thread,
    time::{Duration, Instant},
};
use winsafe::{HPROCESS, HPROCESSLIST, co, guard::CloseHandleGuard};

const COLUMNS: u16 = 100;
const ROWS: u16 = 30;
const LABEL: &str = "ticker";
/// Bounds a replacement from the kill until a new Worker serves: up to 7 s proving the lost one
/// gone, 20 s admission and one 20 s response window, though a healthy one takes a few seconds.
const REPLACED: Duration = Duration::from_secs(45);
/// Bounds the Client's return to a count after the replacement serves.
const RETURNED: Duration = Duration::from_secs(20);
/// How long a Session past the cap is watched for a Worker it must not start.
const GIVEN_UP: Duration = Duration::from_secs(10);
/// Bounds the confirmed stop's end of the Host and the next start's restore.
const STOPPED: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(250);
/// Replacement attempts allowed in ten minutes.
const CAP: usize = 3;

#[test]
fn a_killed_worker_is_replaced_until_the_cap_and_a_confirmed_stop_ends_the_host() {
    let fixture = Fixture::new("orchbee-worker-replacement-", &[]);
    let ticker = root_file(
        &fixture,
        "ticker.cmd",
        "@echo off\r\nfor /l %%i in (1,1,1000000) do (\r\n  echo tick %%i\r\n  ping -n 2 127.0.0.1 >nul\r\n)\r\n",
    );
    let terminal = ticker_terminal(&fixture, &ticker);
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut shown = until("the Client shows a count", RETURNED, || {
        last_tick(&fixture.screen(&tui.terminal))
    });
    let first = worker(&fixture).expect("a Worker serves");
    let host_handle = host_of(first);

    for kill in 1..=CAP {
        let lost = worker(&fixture).unwrap_or_else(|| panic!("kill {kill}: no Worker serves"));
        let killed = Instant::now();
        terminate_listed_worker(&fixture, lost);
        until(
            &format!("kill {kill}: a new Worker serves"),
            REPLACED,
            || worker(&fixture).filter(|serving| *serving != lost),
        );
        assert!(
            !signaled(&host_handle, Duration::ZERO),
            "kill {kill}: the Host was not kept"
        );
        eprintln!("kill {kill}: replaced in {:?}", killed.elapsed());
        // The program kept counting in the Host, and the Client shows it again by itself.
        let read = until(&format!("kill {kill}: the count goes on"), RETURNED, || {
            last_tick(&read_terminal(&fixture, &terminal)).filter(|tick| *tick > shown)
        });
        let returned = until(
            &format!("kill {kill}: the Client returns"),
            RETURNED,
            || last_tick(&fixture.screen(&tui.terminal)).filter(|tick| *tick > shown),
        );
        fixture.settle_ready(&tui.client);
        let screen = fixture.screen(&tui.terminal);
        assert_ascending(&screen);
        assert!(
            read >= returned - 1,
            "kill {kill}: the Client is ahead of the Host"
        );
        shown = returned;
    }

    // Past the cap the Server gives up: no Worker is started, and the Host keeps its program.
    let lost = worker(&fixture).expect("the last replacement serves");
    terminate_listed_worker(&fixture, lost);
    // The listing may name the ended Worker until the Server sees its connection close.
    let watched = Instant::now();
    while watched.elapsed() < GIVEN_UP {
        assert_eq!(
            worker(&fixture).filter(|serving| *serving != lost),
            None,
            "a Worker was started past the cap"
        );
        thread::sleep(POLL);
    }
    assert!(
        !signaled(&host_handle, Duration::ZERO),
        "the Host ended when the Server gave up"
    );

    // The confirmed stop ends the Host, and the next start restores the layout.
    let stopped = fixture.run(&["server", "stop", "--confirm", "--session", "default"]);
    assert_eq!(stopped.code, Some(0), "{stopped}");
    assert!(
        signaled(&host_handle, STOPPED),
        "the confirmed stop left the Host running"
    );
    let started = fixture.run(&["server", "start", "--session", "default"]);
    assert_eq!(started.code, Some(0), "{started}");
    // The window's Update proves that a version joins this Session by this name.
    assert!(
        started.document["result"]["negotiatedCapabilities"]
            .as_array()
            .is_some_and(|names| names.iter().any(|name| name == "compatibility-window")),
        "{started}"
    );
    until("the layout is restored", STOPPED, || {
        let listed = fixture.run(&["workspace", "list"]);
        listed.document["result"]["workspaces"]
            .as_array()?
            .iter()
            .any(|workspace| workspace["label"] == LABEL)
            .then_some(())
    });
}

/// Writes `text` to `name` in the fixture's root and returns its path.
fn root_file(fixture: &Fixture, name: &str, text: &str) -> String {
    let path = fixture.path(name);
    fs::write(&path, text).unwrap();
    path
}

/// Opens the only Workspace of the default Session, running `script` through cmd, and returns its
/// terminal.
fn ticker_terminal(fixture: &Fixture, script: &str) -> String {
    let shell = system_program("cmd.exe");
    let work = fixture.path("work");
    let mut words = vec![
        "workspace",
        "create",
        "--label",
        LABEL,
        "--cwd",
        &work,
        "--no-focus",
        "--exec",
        &shell,
        "--arg",
        "/d",
        "--arg",
        "/c",
        "--arg",
        script,
    ];
    let environment = fixture.terminal_environment();
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    let panel = created.document["result"]["workspace"]["result"]["panelId"]
        .as_str()
        .unwrap_or_else(|| panic!("{created}"))
        .to_owned();
    let listed = fixture.run(&["terminal", "list"]);
    listed.document["result"]["terminals"]
        .as_array()
        .and_then(|terminals| {
            terminals
                .iter()
                .find(|terminal| terminal["boundTo"]["panelId"] == panel.as_str())
        })
        .and_then(|terminal| terminal["terminalId"].as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .to_owned()
}

/// The default Session's serving Worker, by process ID, as `status server` lists it.
fn worker(fixture: &Fixture) -> Option<u32> {
    let status = fixture.run(&["status", "server", "--session", "default"]);
    assert_eq!(status.code, Some(0), "{status}");
    status.document["result"]["server"]["processes"]
        .as_array()?
        .iter()
        .find(|process| process["role"] == "worker")
        .and_then(|process| process["processId"].as_u64())
        .and_then(|pid| u32::try_from(pid).ok())
}

/// The Host `worker` started, which `status server` does not list: the Host process whose parent
/// is the live `worker` and which started after it.
fn host_of(worker: u32) -> CloseHandleGuard<HPROCESS> {
    let parent = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, worker)
        .unwrap_or_else(|error| panic!("open Worker {worker}: {error}"));
    let after = creation(&parent);
    let mut list = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).unwrap();
    let ids: Vec<u32> = list
        .iter_processes()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.th32ParentProcessID == worker
                && entry
                    .szExeFile()
                    .to_ascii_lowercase()
                    .ends_with("terminal-host.exe")
        })
        .map(|entry| entry.th32ProcessID)
        .collect();
    let mut hosts = ids.into_iter().filter_map(|pid| {
        let host = HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
            false,
            pid,
        )
        .ok()?;
        (creation(&host) >= after).then_some(host)
    });
    let host = hosts
        .next()
        .unwrap_or_else(|| panic!("Worker {worker} started no Host"));
    assert!(hosts.next().is_none(), "Worker {worker} started two Hosts");
    host
}

fn creation(process: &HPROCESS) -> u64 {
    let (created, ..) = process.GetProcessTimes().unwrap();
    (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime)
}

/// Ends the Worker this fixture's Session lists as `pid`. The open handle pins that process, so
/// once the Session still lists `pid` as its Worker the handle cannot name another process.
fn terminate_listed_worker(fixture: &Fixture, pid: u32) {
    let process = HPROCESS::OpenProcess(
        co::PROCESS::TERMINATE | co::PROCESS::SYNCHRONIZE,
        false,
        pid,
    )
    .unwrap_or_else(|error| panic!("open Worker {pid}: {error}"));
    assert_eq!(
        worker(fixture),
        Some(pid),
        "the Session no longer lists Worker {pid}"
    );
    process.TerminateProcess(1).unwrap();
    assert!(
        signaled(&process, Duration::from_secs(10)),
        "Worker {pid} did not end"
    );
}

fn signaled(process: &HPROCESS, within: Duration) -> bool {
    let milliseconds = u32::try_from(within.as_millis()).unwrap_or(u32::MAX);
    process
        .WaitForSingleObject(Some(milliseconds))
        .is_ok_and(|event| event == co::WAIT::OBJECT_0)
}

fn read_terminal(fixture: &Fixture, terminal: &str) -> String {
    let read = fixture.run(&["terminal", "read", terminal, "--source", "visible"]);
    read.document["result"]["content"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// The highest count `text` shows.
fn last_tick(text: &str) -> Option<u64> {
    ticks(text).max()
}

fn ticks(text: &str) -> impl Iterator<Item = u64> + '_ {
    text.lines().filter_map(|line| {
        let rest = &line[line.find("tick ")? + "tick ".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// Each count shows once, in order: the Client repeated no output it had already shown.
fn assert_ascending(screen: &str) {
    let counts: Vec<u64> = ticks(screen).collect();
    assert!(
        counts.windows(2).all(|pair| pair[0] < pair[1]),
        "counts repeat or go back: {counts:?}\n{screen}"
    );
}

fn until<T>(what: &str, within: Duration, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + within;
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "{what}: not within {within:?}");
        thread::sleep(POLL);
    }
}
