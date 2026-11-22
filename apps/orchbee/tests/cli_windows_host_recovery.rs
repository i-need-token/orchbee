//! Host recovery across a real managed Session (`proposals/platform/host-handover.md`, H2
//! acceptance): the CLI, Server, Worker and Zig Host under test, and a terminal that counts once a
//! second. A Host ended by its process ID, or whose dispatch thread stops, is replaced by a recovery
//! Host that takes the terminal back from the Session's Keeper with its program still running.
//! Past the cap the Server gives up and the program runs on in the Keeper; the confirmed stop then
//! ends it. The oracle is what the CLI reads and lists and the program's own process; nothing
//! imports Server internals.
//!
//! A Host holds only a terminal on the product's ConPTY in the Keeper, so these witnesses run only
//! when asked for, with `ORCHBEE_TEST_CONPTY_DIR` naming a verified bundle.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use session::{Fixture, system_program};
use std::{
    fs,
    process::Command,
    thread,
    time::{Duration, Instant},
};
use winsafe::{HPROCESS, HPROCESSLIST, co, guard::CloseHandleGuard};

const LABEL: &str = "ticker";
/// Bounds a recovery from the Host's end until a new Worker serves: the lost Worker's exit, up to
/// 7 s proving it gone, 20 s admission and one 20 s response window, though a healthy one takes a
/// few seconds.
const RECOVERED: Duration = Duration::from_secs(45);
/// Bounds a hung Host's recovery: 10 s without dispatch progress, the next two-second probe, its
/// end, and then a recovery.
const HANG_RECOVERED: Duration = Duration::from_secs(70);
/// Bounds the count's return in the recovered terminal.
const RETURNED: Duration = Duration::from_secs(20);
/// How long a Session past the cap is watched for a Worker it must not start.
const GIVEN_UP: Duration = Duration::from_secs(10);
/// Bounds the confirmed stop's end of the program the Keeper kept.
const STOPPED: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(250);
/// Replacement attempts allowed in ten minutes.
const CAP: usize = 3;

#[test]
#[ignore = "needs the product's ConPTY in ORCHBEE_TEST_CONPTY_DIR and four recoveries; run explicitly"]
fn a_killed_host_is_recovered_until_the_cap_and_a_confirmed_stop_ends_the_program() {
    require_product_conpty();
    let fixture = Fixture::new("orchbee-host-recovery-", &[]);
    let _logs = Logs(&fixture);
    let (terminal, mut shown) = ticker(&fixture);
    let host = host_of(worker(&fixture).expect("a Worker serves"));
    let program = program_of(&host);

    for kill in 1..=CAP {
        let lost = worker(&fixture).unwrap_or_else(|| panic!("kill {kill}: no Worker serves"));
        let host = host_of(lost);
        let killed = Instant::now();
        host.TerminateProcess(1).unwrap();
        assert!(
            signaled(&host, Duration::from_secs(10)),
            "kill {kill}: the Host did not end"
        );
        until(
            &format!("kill {kill}: a new Worker serves"),
            RECOVERED,
            || worker(&fixture).filter(|serving| *serving != lost),
        );
        assert!(
            !signaled(&program, Duration::ZERO),
            "kill {kill}: the program ended with its Host"
        );
        shown = until(&format!("kill {kill}: the count goes on"), RETURNED, || {
            last_tick(&read_terminal(&fixture, &terminal)).filter(|tick| *tick > shown)
        });
        eprintln!("kill {kill}: recovered in {:?}", killed.elapsed());
    }

    // A fourth loss inside ten minutes: the Server gives up, and the program runs on in the Keeper.
    let lost = worker(&fixture).expect("the last recovery serves");
    let host = host_of(lost);
    host.TerminateProcess(1).unwrap();
    assert!(
        signaled(&host, Duration::from_secs(10)),
        "the last Host did not end"
    );
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
        !signaled(&program, Duration::ZERO),
        "the program ended when the Server gave up"
    );

    // The confirmed stop ends the Keeper, and the program with it.
    let stopped = fixture.run(&["server", "stop", "--confirm", "--session", "default"]);
    assert_eq!(stopped.code, Some(0), "{stopped}");
    assert!(
        signaled(&program, STOPPED),
        "the confirmed stop left the program running"
    );
    // The Session starts again from what the stop left, and the fixture stops it at its end.
    let started = fixture.run(&["server", "start", "--session", "default"]);
    assert_eq!(started.code, Some(0), "{started}");
}

#[test]
#[ignore = "needs the product's ConPTY in ORCHBEE_TEST_CONPTY_DIR and a hung Host; run explicitly"]
fn a_host_whose_dispatch_stops_is_ended_and_recovered() {
    require_product_conpty();
    let fixture = Fixture::new("orchbee-host-hang-", &[]);
    let _logs = Logs(&fixture);
    let (terminal, shown) = ticker(&fixture);
    let lost = worker(&fixture).expect("a Worker serves");
    let host = host_of(lost);
    let program = program_of(&host);
    let stopped = Instant::now();
    suspend_dispatch_thread(&host);
    // Nothing here asks the Host anything until the Worker has replaced it; the attached TUI's
    // viewer requests may meet the hung Host before the Worker's own check does.
    until("a new Worker serves", HANG_RECOVERED, || {
        worker(&fixture).filter(|serving| *serving != lost)
    });
    assert!(
        signaled(&host, Duration::ZERO),
        "the hung Host still runs after its recovery"
    );
    assert!(
        !signaled(&program, Duration::ZERO),
        "the program ended with its hung Host"
    );
    until("the count goes on", RETURNED, || {
        last_tick(&read_terminal(&fixture, &terminal)).filter(|tick| *tick > shown)
    });
    eprintln!("hang: recovered in {:?}", stopped.elapsed());
}

/// Prints the fixture's diagnostic logs when its test fails, before the fixture removes them.
struct Logs<'a>(&'a Fixture);

impl Drop for Logs<'_> {
    fn drop(&mut self) {
        if !thread::panicking() {
            return;
        }
        // The data root, and with it `logs/`, sits in the fixture's home folder.
        let mut folders = vec![std::path::PathBuf::from(self.0.path("home"))];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(&folder).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
                    && path.parent().and_then(|parent| parent.file_name()) == Some("logs".as_ref())
                    && let Ok(text) = fs::read_to_string(&path)
                {
                    eprintln!("log {}:\n{text}", path.display());
                }
            }
        }
    }
}

/// The system ConHost's terminals stay unheld, so without the product's ConPTY nothing recovers.
fn require_product_conpty() {
    assert!(
        std::env::var_os("ORCHBEE_TEST_CONPTY_DIR").is_some(),
        "ORCHBEE_TEST_CONPTY_DIR must name a verified ConPTY bundle"
    );
}

/// Opens the only Workspace of the default Session, running a script that counts once a second
/// through cmd, and returns its terminal and a count it showed once the Keeper holds it.
fn ticker(fixture: &Fixture) -> (String, u64) {
    let script = fixture.path("ticker.cmd");
    fs::write(
        &script,
        "@echo off\r\nfor /l %%i in (1,1,1000000) do (\r\n  echo tick %%i\r\n  ping -n 2 127.0.0.1 >nul\r\n)\r\n",
    )
    .unwrap();
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
        &script,
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
    let terminal = listed.document["result"]["terminals"]
        .as_array()
        .and_then(|terminals| {
            terminals
                .iter()
                .find(|terminal| terminal["boundTo"]["panelId"] == panel.as_str())
        })
        .and_then(|terminal| terminal["terminalId"].as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .to_owned();
    // The Keeper starts once the Launch is answered; a few counts later it holds the terminal.
    let shown = until("the terminal counts", RETURNED, || {
        last_tick(&read_terminal(fixture, &terminal)).filter(|tick| *tick >= 3)
    });
    (terminal, shown)
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

/// The Host `worker` serves, which `status server` does not list: the first Terminal Host process
/// `worker` started. A first Worker starts the Keeper from the same program after its Host, and a
/// recovering Worker starts only its recovery Host.
fn host_of(worker: u32) -> CloseHandleGuard<HPROCESS> {
    let parent = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, worker)
        .unwrap_or_else(|error| panic!("open Worker {worker}: {error}"));
    children(&parent, "terminal-host.exe")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("Worker {worker} started no Host"))
}

/// The ticker's cmd process, which `host` started.
fn program_of(host: &HPROCESS) -> CloseHandleGuard<HPROCESS> {
    let mut programs = children(host, "cmd.exe").into_iter();
    let program = programs.next().expect("the Host started no program");
    assert!(programs.next().is_none(), "the Host started two programs");
    program
}

/// The live child processes of `parent` whose program file ends with `program`, oldest first, each
/// opened to query and wait. A child that started before `parent` belongs to an earlier process
/// with the same ID.
fn children(parent: &HPROCESS, program: &str) -> Vec<CloseHandleGuard<HPROCESS>> {
    let pid = parent.GetProcessId().unwrap();
    let after = creation(parent);
    let mut list = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None).unwrap();
    let ids: Vec<u32> = list
        .iter_processes()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.th32ParentProcessID == pid
                && entry.szExeFile().to_ascii_lowercase().ends_with(program)
        })
        .map(|entry| entry.th32ProcessID)
        .collect();
    let mut found: Vec<_> = ids
        .into_iter()
        .filter_map(|id| {
            let child = HPROCESS::OpenProcess(
                co::PROCESS::QUERY_LIMITED_INFORMATION
                    | co::PROCESS::SYNCHRONIZE
                    | co::PROCESS::TERMINATE,
                false,
                id,
            )
            .ok()?;
            (creation(&child) >= after).then_some(child)
        })
        .collect();
    found.sort_by_key(|child| creation(child));
    found
}

fn creation(process: &HPROCESS) -> u64 {
    let (created, ..) = process.GetProcessTimes().unwrap();
    (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime)
}

/// Suspends the first thread of `host`, which runs its dispatch loop, through Windows PowerShell:
/// the open handle pins the process, so its ID names no other.
fn suspend_dispatch_thread(host: &HPROCESS) {
    let pid = host.GetProcessId().unwrap();
    let script = format!(
        "$ErrorActionPreference = 'Stop'\n\
         Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; \
         public static class DispatchThread {{ \
         [DllImport(\"kernel32.dll\", SetLastError = true)] public static extern IntPtr OpenThread(int access, bool inherit, int id); \
         [DllImport(\"kernel32.dll\", SetLastError = true)] public static extern int SuspendThread(IntPtr thread); }}'\n\
         $thread = (Get-Process -Id {pid}).Threads | Sort-Object StartTime | Select-Object -First 1\n\
         $handle = [DispatchThread]::OpenThread(2, $false, $thread.Id)\n\
         if ($handle -eq [IntPtr]::Zero) {{ exit 2 }}\n\
         if ([DispatchThread]::SuspendThread($handle) -lt 0) {{ exit 3 }}\n"
    );
    let shell = system_program("WindowsPowerShell\\v1.0\\powershell.exe");
    let output = Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "suspend the Host's dispatch thread: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
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
    text.lines()
        .filter_map(|line| {
            let rest = &line[line.find("tick ")? + "tick ".len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .max()
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
