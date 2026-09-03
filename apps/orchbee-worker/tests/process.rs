#![cfg(unix)]

use std::{
    error::Error,
    fs, io,
    io::Read,
    num::NonZeroU64,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_local_peer_credentials::require_same_effective_user;
use orchbee_local_site_protocol::{
    LocalSiteMessage, RejectionReason, WorkerHello, WorkerId, read_message, write_message,
};

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_WORKER_ID: WorkerId = WorkerId::from_bytes(*b"orchbee-local-v1");
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn rejects_every_invalid_private_invocation_without_output() -> TestResult {
    for arguments in [
        Vec::<&str>::new(),
        vec!["--version"],
        vec!["--local-endpoint"],
        vec!["--local-endpoint", "relative.sock"],
        vec!["--local-endpoint", "/tmp/orchbee.sock", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_orchbee-worker"))
            .args(arguments)
            .output()?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    Ok(())
}

#[test]
fn registers_exact_identity_emits_readiness_and_exits_on_server_loss() -> TestResult {
    let fixture = ListenerFixture::new()?;
    let endpoint = fixture.endpoint().to_owned();
    let (disconnect_sender, disconnect_receiver) = mpsc::channel();
    let server = thread::spawn(move || -> io::Result<WorkerHello> {
        let mut stream = accept_with_deadline(fixture.listener())?;
        require_same_effective_user(&stream)?;
        stream.set_read_timeout(Some(ACCEPT_TIMEOUT))?;
        stream.set_write_timeout(Some(ACCEPT_TIMEOUT))?;
        let LocalSiteMessage::WorkerHello(hello) =
            read_message(&mut stream).map_err(protocol_io)?
        else {
            return Err(io::Error::other("Worker sent the wrong handshake message"));
        };
        let generation = NonZeroU64::new(7)
            .ok_or_else(|| io::Error::other("test generation must be nonzero"))?;
        write_message(
            &mut stream,
            &LocalSiteMessage::WorkerAccepted { generation },
        )
        .map_err(protocol_io)?;
        disconnect_receiver
            .recv_timeout(ACCEPT_TIMEOUT)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("timed out waiting to disconnect Worker: {error}"),
                )
            })?;
        Ok(hello)
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_orchbee-worker"))
        .arg("--local-endpoint")
        .arg(&endpoint)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Worker stdout pipe is unavailable"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("Worker stderr pipe is unavailable"))?;
    let mut readiness = [0_u8; 6];
    stdout.read_exact(&mut readiness)?;
    disconnect_sender
        .send(())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let status = child.wait()?;
    let mut trailing_stdout = Vec::new();
    stdout.read_to_end(&mut trailing_stdout)?;
    let mut captured_stderr = Vec::new();
    stderr.read_to_end(&mut captured_stderr)?;
    let hello = server
        .join()
        .map_err(|_panic| io::Error::other("fake Server thread panicked"))??;
    assert_eq!(hello.cohort(), env!("CARGO_PKG_VERSION"));
    assert_eq!(hello.worker_id(), LOCAL_WORKER_ID);
    assert_eq!(status.code(), Some(1));
    assert_eq!(readiness, *b"ready\n");
    assert!(trailing_stdout.is_empty());
    assert!(captured_stderr.is_empty());
    Ok(())
}

#[test]
fn exits_silently_when_server_rejects_registration() -> TestResult {
    let fixture = ListenerFixture::new()?;
    let endpoint = fixture.endpoint().to_owned();
    let server = thread::spawn(move || -> io::Result<()> {
        let mut stream = accept_with_deadline(fixture.listener())?;
        require_same_effective_user(&stream)?;
        stream.set_read_timeout(Some(ACCEPT_TIMEOUT))?;
        stream.set_write_timeout(Some(ACCEPT_TIMEOUT))?;
        let LocalSiteMessage::WorkerHello(_hello) =
            read_message(&mut stream).map_err(protocol_io)?
        else {
            return Err(io::Error::other("Worker sent the wrong handshake message"));
        };
        write_message(
            &mut stream,
            &LocalSiteMessage::Rejected {
                reason: RejectionReason::IncompatibleCohort,
            },
        )
        .map_err(protocol_io)
    });

    let output = run_worker(&endpoint)?;
    server
        .join()
        .map_err(|_panic| io::Error::other("fake Server thread panicked"))??;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    Ok(())
}

fn run_worker(endpoint: &Path) -> io::Result<std::process::Output> {
    Command::new(env!("CARGO_BIN_EXE_orchbee-worker"))
        .arg("--local-endpoint")
        .arg(endpoint)
        .output()
}

fn accept_with_deadline(listener: &UnixListener) -> io::Result<std::os::unix::net::UnixStream> {
    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _address)) => return Ok(stream),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for Worker connection",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn protocol_io(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

struct ListenerFixture {
    _directory: TestDirectory,
    endpoint: PathBuf,
    listener: UnixListener,
}

impl ListenerFixture {
    fn new() -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        let endpoint = directory.path().join("site.sock");
        let listener = UnixListener::bind(&endpoint)?;
        fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            _directory: directory,
            endpoint,
            listener,
        })
    }

    fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    fn listener(&self) -> &UnixListener {
        &self.listener
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-worker-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique Worker-test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}
