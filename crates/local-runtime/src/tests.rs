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
#[cfg(unix)]
mod posix_recovery;
pub(crate) mod record_migration;
#[cfg(windows)]
mod rendezvous;
mod session_listing;
mod session_name;
mod session_store;
mod version_record;

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

/// A folder in the temporary directory whose access list also lets the built-in Users group read it
/// and everything created below it, as a profile folder another account shares does.
#[cfg(windows)]
pub(crate) struct SharedFolder(pub(crate) PathBuf);
#[cfg(windows)]
impl SharedFolder {
    pub(crate) fn new() -> Self {
        use windows_permissions::{
            LocalBox, SecurityDescriptor,
            constants::{SeObjectType, SecurityInformation},
            wrappers,
        };
        let path = crate::test_temporary_directory().join(format!(
            "orchbee-shared-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let mut handle = permissions::open_directory(&path, true).unwrap();
        let descriptor: LocalBox<SecurityDescriptor> = format!(
            "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;GR;;;BU)",
            orchbee_local_ipc::current_user_sid().unwrap()
        )
        .parse()
        .unwrap();
        wrappers::SetSecurityInfo(
            &mut handle,
            SeObjectType::SE_FILE_OBJECT,
            SecurityInformation::Dacl | SecurityInformation::ProtectedDacl,
            None,
            None,
            descriptor.dacl(),
            None,
        )
        .unwrap();
        Self(path)
    }
}
#[cfg(windows)]
impl Drop for SharedFolder {
    fn drop(&mut self) {
        let parent = crate::test_temporary_directory().canonicalize().unwrap();
        let path = self.0.canonicalize().unwrap();
        assert_eq!(path.parent(), Some(parent.as_path()));
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-shared-test-")
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
