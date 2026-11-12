use crate::{
    LockAttempt, ModeCheckedLocalSessionRendezvous as Rendezvous, SessionScope,
    private_filesystem as permissions,
};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
mod child_registration;
#[cfg(windows)]
mod crash_recovery;
mod durable_sessions;
#[cfg(windows)]
mod named_catalog;
pub(crate) mod record_migration;
#[cfg(windows)]
mod rendezvous;
mod session_name;
mod session_store;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = crate::test_temporary_directory().join(format!(
            "orchbee-runtime-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let mut directory = permissions::open_directory(&path, true).unwrap();
        permissions::protect_new(&mut directory).unwrap();
        Self(path)
    }
    fn open(&self, scope: &str) -> Rendezvous {
        Rendezvous::prepare_mode_checked(&self.0, SessionScope::parse(scope).unwrap()).unwrap()
    }
    /// This folder as a data root, whose `sessions/` holds every Session folder.
    fn root(&self) -> crate::DataRoot {
        crate::DataRoot::at(self.0.clone())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let parent = crate::test_temporary_directory().canonicalize().unwrap();
        let path = self.0.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(parent.as_path()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-runtime-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

/// Writes a fixture file as storage writes one, private to the user: Windows files inherit the
/// fixture's private ACL, and a POSIX file is created as 0600.
fn write_private(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes.as_ref())
}
