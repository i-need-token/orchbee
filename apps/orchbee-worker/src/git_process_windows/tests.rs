use super::*;
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
use winsafe::{HPROCESS, co};

fn tree() -> (GitChild, ChildOutputReader) {
    let executable = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let (output, writer) = ChildOutputReader::pipe().unwrap();
    let mut command = Command::new(executable);
    command.args([
        "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
        "$ErrorActionPreference='Stop'; $s=New-Object System.Diagnostics.ProcessStartInfo; $s.FileName=$env:SystemRoot+'\\System32\\ping.exe'; $s.Arguments='-n 30 127.0.0.1'; $s.UseShellExecute=$false; $s.CreateNoWindow=$true; $s.RedirectStandardOutput=$true; $c=[System.Diagnostics.Process]::Start($s); [Console]::WriteLine($c.Id); [Console]::Out.Flush(); $null=[Console]::ReadLine(); exit 7",
    ]).stdin(Stdio::piped()).stdout(writer).stderr(Stdio::null());
    (GitChild::spawn(command).unwrap(), output)
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
    let mut child = GitChild {
        child: Box::new(FailedJobWait(Arc::clone(&calls))),
        settlement: Settlement::Running,
    };
    assert!(child.settle().is_err());
    assert!(child.settle().is_err());
    assert!(child.try_wait().is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
