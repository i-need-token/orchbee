use super::*;
use crate::CommandFailure;
use std::{
    io::{Read, Write},
    os::windows::process::ExitStatusExt,
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_local_ipc::ChildOutputReader;
use process_wrap::std::KillOnJobClose;

#[test]
fn captured_commands_settle_on_work_deadline_or_caller_interruption() {
    for cancelled in [false, true] {
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 30",
        ]);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(150);
        let result = run_windows_command(command, (!cancelled).then_some(deadline), || {
            Ok(cancelled && Instant::now() >= deadline)
        });
        assert_eq!(
            result.failure,
            Some(if cancelled {
                CommandFailure::Cancelled
            } else {
                CommandFailure::Timeout
            })
        );
        assert!(started.elapsed() < Duration::from_secs(6));
        assert_eq!(result.exit_code, None);
    }
}
use winsafe::{HPROCESS, co};

fn tree_command() -> (Command, ChildOutputReader) {
    let executable = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let (output, writer) = ChildOutputReader::pipe().unwrap();
    let mut command = Command::new(executable);
    command.args([
        "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
        "$ErrorActionPreference='Stop'; $s=New-Object System.Diagnostics.ProcessStartInfo; $s.FileName=$env:SystemRoot+'\\System32\\ping.exe'; $s.Arguments='-n 30 127.0.0.1'; $s.UseShellExecute=$false; $s.CreateNoWindow=$true; $s.RedirectStandardOutput=$true; $c=[System.Diagnostics.Process]::Start($s); [Console]::WriteLine($c.Id); [Console]::Out.Flush(); $null=[Console]::ReadLine(); exit 7",
    ]).stdin(Stdio::piped()).stdout(writer).stderr(Stdio::null());
    (command, output)
}

fn tree() -> (CommandChild, ChildOutputReader) {
    let (command, output) = tree_command();
    (CommandChild::spawn(command).unwrap(), output)
}

fn announced_child(reader: &mut ChildOutputReader) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut output = Vec::new();
    while Instant::now() < deadline {
        let mut bytes = [0; 64];
        match reader.read(&mut bytes) {
            Ok(0) => panic!("fixture exited before announcing its child"),
            Ok(count) => output.extend_from_slice(&bytes[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("fixture pipe failed: {error}"),
        }
        assert!(output.len() < 64);
        if output.contains(&b'\n') {
            return std::str::from_utf8(&output)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("fixture did not announce its child");
}

fn wait_for_process_exit(process: &HPROCESS, description: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let state = process.WaitForSingleObject(Some(0)).unwrap();
        if state == co::WAIT::OBJECT_0 {
            return;
        }
        assert_eq!(state, co::WAIT::TIMEOUT);
        assert!(
            Instant::now() < deadline,
            "{description} survived kill-on-close Job teardown"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn kill_on_job_close_drop_terminates_root_and_retained_descendant() {
    let (command, mut output) = tree_command();
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(KillOnJobClose)
        .wrap(JobObject);
    let child = wrapped.spawn().unwrap();
    drop(wrapped);

    let root = HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, child.id()).unwrap();
    let descendant_pid = announced_child(&mut output);
    let descendant =
        HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, descendant_pid).unwrap();
    assert_eq!(
        root.WaitForSingleObject(Some(0)).unwrap(),
        co::WAIT::TIMEOUT
    );
    assert_eq!(
        descendant.WaitForSingleObject(Some(0)).unwrap(),
        co::WAIT::TIMEOUT
    );

    // This is the policy oracle: no explicit start_kill call participates.
    // Unwinding before this point also drops the wrapper and closes the Job.
    drop(child);
    wait_for_process_exit(&root, "root process");
    wait_for_process_exit(&descendant, "descendant process");
}

#[test]
fn root_exit_and_cancellation_wait_for_retained_descendant_identity() {
    for root_exits in [true, false] {
        let (mut child, mut output) = tree();
        let pid = announced_child(&mut output);
        let descendant = HPROCESS::OpenProcess(co::PROCESS::SYNCHRONIZE, false, pid).unwrap();
        assert_eq!(
            descendant.WaitForSingleObject(Some(0)).unwrap(),
            co::WAIT::TIMEOUT
        );
        assert!(child.try_wait().unwrap().is_none());
        let status = if root_exits {
            child
                .child
                .stdin()
                .as_mut()
                .unwrap()
                .write_all(b"exit\n")
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(8);
            while child.child.inner_mut().try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "root failed to exit");
                thread::sleep(Duration::from_millis(10));
            }
            // A NEW_PROCESS or root EXIT_PROCESS notification must not turn
            // the still-live descendant into a completed Job result.
            assert!(child.child.try_wait().unwrap().is_none());
            assert_eq!(
                descendant.WaitForSingleObject(Some(0)).unwrap(),
                co::WAIT::TIMEOUT
            );
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                assert!(Instant::now() < deadline, "root failed to exit");
                thread::sleep(Duration::from_millis(10));
            }
        } else {
            child.settle().unwrap()
        };
        if root_exits {
            assert_eq!(status.code(), Some(7));
        }
        assert_eq!(
            descendant.WaitForSingleObject(Some(0)).unwrap(),
            co::WAIT::OBJECT_0
        );
        // A retained known result does not consume another completion message.
        assert_eq!(child.try_wait().unwrap(), Some(status));
        assert_eq!(child.settle().unwrap(), status);
    }
}

#[derive(Debug)]
struct FailedJobWait(Arc<AtomicUsize>);

impl ChildWrapper for FailedJobWait {
    fn inner(&self) -> &dyn ChildWrapper {
        self
    }
    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        self
    }
    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        self
    }
    fn start_kill(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
            Err(io::Error::other("Job wait failed after caching root exit"))
        } else {
            Ok(Some(ExitStatus::from_raw(0)))
        }
    }
}

#[test]
fn failed_job_wait_never_becomes_cached_root_success_on_retry() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut child = CommandChild {
        child: Box::new(FailedJobWait(Arc::clone(&calls))),
        settlement: Settlement::Running,
    };
    assert!(child.settle().is_err());
    assert!(child.settle().is_err());
    assert!(child.try_wait().is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[derive(Debug)]
struct RejectWrappedChild;

impl process_wrap::std::CommandWrapper for RejectWrappedChild {
    fn wrap_child(
        &mut self,
        mut child: Box<dyn ChildWrapper>,
        _: &CommandWrap,
    ) -> io::Result<Box<dyn ChildWrapper>> {
        // The fixture itself proves cleanup; the production adapter still only
        // knows that native creation preceded an error and must preserve that fact.
        child.start_kill()?;
        child.wait()?;
        Err(io::Error::other("controlled post-creation wrapper failure"))
    }
}

#[test]
fn spawn_failure_distinguishes_no_child_from_error_after_native_creation() {
    let missing =
        std::env::temp_dir().join(format!("orchbee-absent-git-{}.exe", std::process::id()));
    assert!(!missing.exists());
    let Err(failure) = CommandChild::spawn(Command::new(missing)) else {
        panic!("missing program spawned")
    };
    assert!(!failure.created);
    assert_eq!(failure.error.kind(), io::ErrorKind::NotFound);

    let mut command = Command::new(
        std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe"),
    );
    command
        .args(["/d", "/c", "exit", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(JobObject)
        .wrap(RejectWrappedChild);
    let Err(failure) = CommandChild::from_wrapped(wrapped) else {
        panic!("rejecting wrapper succeeded")
    };
    assert!(failure.created);
    assert_eq!(failure.error.kind(), io::ErrorKind::Other);
}
