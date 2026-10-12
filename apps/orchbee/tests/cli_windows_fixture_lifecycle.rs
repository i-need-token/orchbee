//! Failure settlement for the shared Windows CLI process/peer owner.
#![cfg(windows)]
#[allow(dead_code)]
#[path = "support/windows_agent_peer.rs"]
mod peer_support;
use orchbee_local_ipc::LocalListener;
use peer_support::{Fixture, peer, peer_with_budget, process::FailureKind};
use std::{
    io::Read,
    os::windows::process::CommandExt,
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use winsafe::{HPROCESS, co};

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .expect("fixture panic must include a diagnostic")
}

fn powershell(script: &str) -> Command {
    let path = std::path::PathBuf::from(winsafe::GetSystemDirectory().unwrap())
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(path);
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW for the controlled fixture.
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]);
    command
}

#[test]
fn fixture_command_deadline_refuses_a_child_that_outlives_its_budget() {
    let fixture = Fixture::new();
    let started = Instant::now();
    let failure = fixture
        .try_run_command(
            powershell("Start-Sleep -Seconds 30"),
            None,
            Duration::from_millis(40),
            1024,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Deadline, "{failure}");
    assert!(
        failure.status.is_some(),
        "deadline must settle the Job: {failure}"
    );
    assert!(started.elapsed() < Duration::from_secs(15));
}

#[test]
fn fixture_command_output_limit_refuses_a_large_completed_document() {
    let fixture = Fixture::new();
    let failure = fixture
        .try_run_command(
            powershell("[Console]::OpenStandardOutput().Write([byte[]]::new(65536),0,65536)"),
            None,
            Duration::from_secs(10),
            1024,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::OutputLimit, "{failure}");
    assert!(
        failure.status.is_some(),
        "output refusal must settle the Job: {failure}"
    );
    assert_eq!(failure.stdout.len() + failure.stderr.len(), 1024);
}

#[test]
fn fixture_full_duplex_keeps_large_output_input_eof_and_nonzero_status() {
    let fixture = Fixture::new();
    let input: Vec<_> = (0..1024 * 1024).map(|n| (n % 251) as u8).collect();
    let output = fixture.try_run_command(
        powershell("$o=[Console]::OpenStandardOutput();$o.Write([byte[]]::new(3145728),0,3145728);[Console]::OpenStandardInput().CopyTo($o);[Console]::Error.Write('done');exit 42"),
        Some(&input), Duration::from_secs(15), 5 * 1024 * 1024,
    ).unwrap();
    assert_eq!(output.status.code(), Some(42));
    assert_eq!(output.stdout.len(), 4 * 1024 * 1024);
    assert!(
        output.stdout[..3 * 1024 * 1024]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(&output.stdout[3 * 1024 * 1024..], input);
    assert_eq!(output.stderr, b"done");
}

#[test]
fn fixture_early_stdin_close_preserves_the_child_rejection() {
    let fixture = Fixture::new();
    let output = fixture
        .try_run_command(
            powershell("[Console]::Out.Write('rejected');exit 23"),
            Some(&vec![0; 1024 * 1024]),
            Duration::from_secs(10),
            1024,
        )
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert_eq!(output.stdout, b"rejected");
}

#[test]
fn dropping_an_unlaunched_peer_joins_and_releases_the_next_invocation() {
    for unwind in [false, true] {
        let fixture = Fixture::new();
        let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
        let handler = peer(&fixture, listener, |_| {
            panic!("a cancelled peer must never accept")
        });
        let started = Instant::now();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let guard = handler;
            assert!(!unwind, "controlled parent assertion failure");
            drop(guard);
        }));
        assert_eq!(outcome.is_err(), unwind);
        if let Err(payload) = outcome {
            assert_eq!(
                panic_message(payload.as_ref()),
                "controlled parent assertion failure"
            );
        }
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(fixture.run(&["--help"]).status.success());
    }
}

#[test]
fn peer_admission_timeout_cancels_and_settles_a_stalled_command() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer_with_budget(&fixture, listener, Duration::from_millis(40), |_| {
        panic!("the controlled child never connects");
    });
    let failure = fixture
        .try_run_command(
            powershell("Start-Sleep -Seconds 30"),
            Some(&vec![0; 1024 * 1024]),
            Duration::from_secs(10),
            1024,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Cancelled, "{failure}");
    assert!(failure.status.is_some(), "{failure}");
    let panic = handler.join().unwrap_err();
    assert!(panic_message(panic.as_ref()).starts_with("CLI admission timeout:"));
}

#[test]
fn dropping_an_accepted_peer_settles_the_command_and_joins_its_reader() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let (admitted, admission) = mpsc::channel();
    let (closed, closure) = mpsc::channel();
    let handler = peer(&fixture, listener, move |mut stream| {
        admitted.send(()).unwrap();
        // Drain the CLI request, then observe the exact client's close after cancellation.
        let mut bytes = [0; 1024];
        loop {
            if stream.read(&mut bytes).unwrap() == 0 {
                break;
            }
        }
        closed.send(()).unwrap();
    });
    thread::scope(|scope| {
        let runner = scope.spawn(|| {
            fixture.try_run_command(
                fixture.command(&["agent", "names", "--format=json"], None, None, &[]),
                None,
                Duration::from_secs(20),
                1024 * 1024,
            )
        });
        admission.recv_timeout(Duration::from_secs(10)).unwrap();
        let started = Instant::now();
        drop(handler);
        assert!(started.elapsed() < Duration::from_secs(15));
        assert!(
            closure.try_recv().is_ok(),
            "Drop returned before joining the accepted reader"
        );
        let failure = runner.join().unwrap().unwrap_err();
        assert_eq!(failure.kind, FailureKind::Cancelled, "{failure}");
        assert!(failure.status.is_some(), "{failure}");
    });
}

#[test]
fn peer_drop_cancels_blocked_stdin_and_reaps_the_owned_descendant() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |_| {
        panic!("the controlled child never connects")
    });
    let announced = fixture.0.join("descendant.pid");
    let mut command = powershell(
        "$s=New-Object System.Diagnostics.ProcessStartInfo;$s.FileName=$env:SystemRoot+'\\System32\\ping.exe';$s.Arguments='-n 30 127.0.0.1';$s.UseShellExecute=$false;$s.CreateNoWindow=$true;$s.RedirectStandardOutput=$true;$c=[System.Diagnostics.Process]::Start($s);[Console]::Out.Write('before-cancel');[Console]::Out.Flush();[IO.File]::WriteAllText($env:ORCHBEE_TEST_CHILD_PID,[string]$c.Id);Start-Sleep -Seconds 30",
    );
    command.env("ORCHBEE_TEST_CHILD_PID", &announced);
    thread::scope(|scope| {
        let runner = scope.spawn(|| {
            fixture.try_run_command(
                command,
                Some(&vec![0; 1024 * 1024]),
                Duration::from_secs(20),
                1024,
            )
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(&announced)
                && let Ok(pid) = text.parse::<u32>()
            {
                break pid;
            }
            assert!(
                Instant::now() < deadline,
                "child did not announce its descendant"
            );
            thread::sleep(Duration::from_millis(5));
        };
        let descendant = HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, pid).unwrap();
        assert_eq!(
            descendant.WaitForSingleObject(Some(0)).unwrap(),
            co::WAIT::TIMEOUT
        );
        drop(handler);
        let failure = runner.join().unwrap().unwrap_err();
        assert_eq!(failure.kind, FailureKind::Cancelled, "{failure}");
        assert!(failure.status.is_some(), "{failure}");
        assert_eq!(
            descendant.WaitForSingleObject(Some(0)).unwrap(),
            co::WAIT::OBJECT_0
        );
        assert_eq!(failure.stdout, b"before-cancel");
        assert!(failure.stdout.len() + failure.stderr.len() <= 1024);
    });
}

#[test]
fn spawn_failure_wakes_the_unlaunched_peer_without_a_job_receipt() {
    let fixture = Fixture::new();
    let listener = LocalListener::bind(fixture.rendezvous().client_socket_path()).unwrap();
    let handler = peer(&fixture, listener, |_| panic!("no child can connect"));
    let failure = fixture
        .try_run_command(
            Command::new(fixture.0.join("missing.exe")),
            None,
            Duration::from_secs(10),
            1024,
        )
        .unwrap_err();
    assert_eq!(failure.kind, FailureKind::Spawn, "{failure}");
    assert!(failure.status.is_none(), "no Job was created: {failure}");
    let panic = handler.join().unwrap_err();
    assert_eq!(
        panic_message(panic.as_ref()),
        "CLI launch cancelled before admission"
    );
}
