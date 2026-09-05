use std::{
    fs, io,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};
const SERVER_GUARD_TIMEOUT: Duration = Duration::from_secs(1);
static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) struct FakeServer {
    _directory: TestDirectory,
    endpoint: PathBuf,
    thread: thread::JoinHandle<io::Result<()>>,
}

impl FakeServer {
    pub(crate) fn start(
        handler: impl FnOnce(UnixStream) -> io::Result<()> + Send + 'static,
    ) -> io::Result<Self> {
        let directory = TestDirectory::new()?;
        let endpoint = directory.path.join("client.sock");
        let listener = UnixListener::bind(&endpoint)?;
        let thread = thread::spawn(move || {
            let (stream, _address) = listener.accept()?;
            stream.set_read_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            stream.set_write_timeout(Some(SERVER_GUARD_TIMEOUT))?;
            handler(stream)
        });
        Ok(Self {
            _directory: directory,
            endpoint,
            thread,
        })
    }

    pub(crate) fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    pub(crate) fn finish(self) -> io::Result<()> {
        self.thread
            .join()
            .map_err(|_panic| io::Error::other("fake Server panicked"))?
    }
}

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new() -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-local-client-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique local Client test directory",
        ))
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn shutdown_handle(stream: UnixStream) -> super::LocalClientShutdown {
    super::LocalClientShutdown(stream)
}
