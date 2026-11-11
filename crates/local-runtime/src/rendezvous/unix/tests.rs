use std::{
    error::Error,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, PermissionsExt, symlink},
    },
    path::{Path, PathBuf},
    sync::{
        Barrier, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use super::*;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);
pub(crate) static FILESYSTEM_TEST_LOCK: Mutex<()> = Mutex::new(());

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn rejects_non_normal_runtime_base_forms_before_filesystem_resolution() -> TestResult {
    let fixture = TestDirectory::new("runtime-base-lexical")?;
    fs::create_dir(fixture.path().join("nested"))?;
    let scope = SessionScope::parse("orchbee-lexical")?;
    let mut double_root_bytes = vec![b'/'];
    double_root_bytes.extend_from_slice(fixture.path().as_os_str().as_bytes());
    let invalid_paths = [
        path_with_suffix(fixture.path(), b"/"),
        path_with_suffix(fixture.path(), b"/."),
        path_with_suffix(fixture.path(), b"/nested/./child"),
        path_with_suffix(fixture.path(), b"//nested"),
        path_with_suffix(fixture.path(), b"/nested/.."),
        path_with_suffix(fixture.path(), b"/bad\0component"),
        PathBuf::from(OsString::from_vec(double_root_bytes)),
    ];

    for invalid_path in invalid_paths {
        assert!(matches!(
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
                &invalid_path,
                scope.clone()
            ),
            Err(RendezvousError::RuntimeBaseNotNormalized { path }) if path == invalid_path
        ));
    }
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(Path::new("relative"), scope.clone()),
        Err(RendezvousError::RuntimeBaseNotAbsolute { path }) if path == Path::new("relative")
    ));
    assert!(!fixture.path().join(scope.as_str()).exists());
    assert!(!fixture.path().join("nested").join(scope.as_str()).exists());
    Ok(())
}

#[test]
fn creates_and_validates_an_owned_exact_mode_runtime_directory() -> TestResult {
    let fixture = TestDirectory::new("runtime-directory")?;
    let scope = SessionScope::parse("orchbee-local")?;
    let rendezvous =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;

    assert_eq!(
        rendezvous.runtime_directory(),
        fixture.path().join(scope.as_str())
    );
    assert_eq!(
        mode(rendezvous.runtime_directory())?,
        RUNTIME_DIRECTORY_MODE
    );
    assert_eq!(
        fs::metadata(rendezvous.runtime_directory())?.uid(),
        effective_uid()
    );

    let validated =
        ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope)?;
    assert_eq!(
        validated.runtime_directory(),
        rendezvous.runtime_directory()
    );
    Ok(())
}

#[test]
fn initializes_a_new_scope_under_a_fully_restrictive_umask() -> TestResult {
    let fixture = TestDirectory::new("restrictive-umask")?;
    let scope = SessionScope::parse("orchbee-umask")?;
    let runtime_directory = fixture.path().join(scope.as_str());

    let rendezvous = {
        let _umask = TestUmask::set(0o777);
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?
    };
    assert_eq!(rendezvous.runtime_directory(), runtime_directory);
    assert_eq!(mode(&runtime_directory)?, RUNTIME_DIRECTORY_MODE);
    drop(rendezvous);

    let validated =
        ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope)?;
    assert_eq!(validated.runtime_directory(), runtime_directory);
    assert_eq!(mode(&runtime_directory)?, RUNTIME_DIRECTORY_MODE);
    Ok(())
}

#[test]
fn concurrent_preparation_converges_on_one_initialized_scope() -> TestResult {
    let fixture = TestDirectory::new("concurrent-creation")?;
    let runtime_base = fixture.path().to_path_buf();
    let scope = SessionScope::parse("orchbee-concurrent")?;
    let barrier = Barrier::new(2);

    let (first, second) = {
        let _umask = TestUmask::set(0);
        thread::scope(|threads| -> TestResult<_> {
            let first_base = runtime_base.clone();
            let first_scope = scope.clone();
            let first_barrier = &barrier;
            let first = threads.spawn(move || {
                first_barrier.wait();
                ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&first_base, first_scope)
            });
            let second_base = runtime_base.clone();
            let second_scope = scope.clone();
            let second_barrier = &barrier;
            let second = threads.spawn(move || {
                second_barrier.wait();
                ModeCheckedLocalSessionRendezvous::prepare_mode_checked(&second_base, second_scope)
            });
            let first = first
                .join()
                .map_err(|_| io::Error::other("first preparation thread panicked"))??;
            let second = second
                .join()
                .map_err(|_| io::Error::other("second preparation thread panicked"))??;
            Ok((first, second))
        })?
    };

    assert_eq!(first.runtime_directory(), second.runtime_directory());
    assert_eq!(mode(first.runtime_directory())?, RUNTIME_DIRECTORY_MODE);
    assert_eq!(
        fs::read_dir(fixture.path())?
            .collect::<Result<Vec<_>, _>>()?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn validation_does_not_create_a_missing_scope() -> TestResult {
    let fixture = TestDirectory::new("validate-missing")?;
    let scope = SessionScope::parse("orchbee-local")?;
    let expected_path = fixture.path().join(scope.as_str());

    assert!(
        ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope).is_err()
    );
    assert!(!expected_path.exists());
    Ok(())
}

#[test]
fn rejects_runtime_base_and_scope_symlinks_without_following_them() -> TestResult {
    let fixture = TestDirectory::new("directory-symlinks")?;
    let actual_base = fixture.path().join("actual-base");
    fs::create_dir(&actual_base)?;
    fs::create_dir(actual_base.join("child"))?;
    let linked_base = fixture.path().join("linked-base");
    symlink(&actual_base, &linked_base)?;

    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            &linked_base,
            SessionScope::parse("orchbee-final")?
        ),
        Err(RendezvousError::DirectorySymlink { path }) if path == linked_base
    ));
    assert!(!actual_base.join("orchbee-final").exists());

    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            &linked_base.join("child"),
            SessionScope::parse("orchbee-intermediate")?
        ),
        Err(RendezvousError::DirectorySymlink { path }) if path == linked_base
    ));
    assert!(!actual_base.join("child/orchbee-intermediate").exists());

    let linked_scope = SessionScope::parse("orchbee-linked")?;
    let target = fixture.path().join("scope-target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(RUNTIME_DIRECTORY_MODE))?;
    let linked_scope_path = fixture.path().join(linked_scope.as_str());
    symlink(&target, &linked_scope_path)?;
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), linked_scope),
        Err(RendezvousError::DirectorySymlink { path }) if path == linked_scope_path
    ));
    assert_eq!(mode(&target)?, RUNTIME_DIRECTORY_MODE);
    Ok(())
}

#[test]
fn rejects_non_directory_wrong_owner_and_wrong_mode_scopes() -> TestResult {
    let fixture = TestDirectory::new("directory-metadata")?;
    let file_base = fixture.path().join("file-base");
    fs::write(&file_base, b"preserve base\n")?;
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
            &file_base,
            SessionScope::parse("orchbee-local")?
        ),
        Err(RendezvousError::NotDirectory { .. })
    ));
    assert_eq!(fs::read(&file_base)?, b"preserve base\n");

    let file_scope = SessionScope::parse("orchbee-file")?;
    fs::write(fixture.path().join(file_scope.as_str()), b"preserve\n")?;
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), file_scope),
        Err(RendezvousError::NotDirectory { .. })
    ));

    let owner_scope = SessionScope::parse("orchbee-owner")?;
    let different_uid = different_uid(effective_uid());
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked_for_user(
            fixture.path(),
            owner_scope,
            different_uid
        ),
        Err(RendezvousError::UnexpectedOwner {
            expected,
            actual,
            ..
        }) if expected == different_uid && actual == effective_uid()
    ));

    let mode_scope = SessionScope::parse("orchbee-mode")?;
    let mode_path = fixture.path().join(mode_scope.as_str());
    fs::create_dir(&mode_path)?;
    fs::set_permissions(&mode_path, fs::Permissions::from_mode(0o750))?;
    assert!(matches!(
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), mode_scope),
        Err(RendezvousError::UnexpectedMode {
            expected: RUNTIME_DIRECTORY_MODE,
            actual: 0o750,
            ..
        })
    ));
    assert_eq!(mode(&mode_path)?, 0o750);
    Ok(())
}

#[test]
fn lock_files_reject_links_non_regular_entries_and_insecure_modes() -> TestResult {
    let fixture = TestDirectory::new("lock-metadata")?;
    let rendezvous = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        fixture.path(),
        SessionScope::parse("orchbee-local")?,
    )?;
    let target_path = rendezvous.runtime_directory().join("target");
    fs::write(&target_path, b"preserve target\n")?;
    fs::set_permissions(&target_path, fs::Permissions::from_mode(0o640))?;
    symlink(&target_path, &rendezvous.paths.activation_lock)?;

    assert!(matches!(
        rendezvous.try_acquire_activation_lock(),
        Err(RendezvousError::LockFileSymlink {
            kind: LockKind::Activation,
            ..
        })
    ));
    assert_eq!(fs::read(&target_path)?, b"preserve target\n");
    assert_eq!(mode(&target_path)?, 0o640);

    fs::remove_file(&rendezvous.paths.activation_lock)?;
    fs::create_dir(&rendezvous.paths.activation_lock)?;
    assert!(matches!(
        rendezvous.try_acquire_activation_lock(),
        Err(RendezvousError::NotRegularLockFile {
            kind: LockKind::Activation,
            ..
        })
    ));

    fs::remove_dir(&rendezvous.paths.activation_lock)?;
    nix::unistd::mkfifo(
        &rendezvous.paths.activation_lock,
        exact_mode(LOCK_FILE_MODE),
    )?;
    assert!(matches!(
        rendezvous.try_acquire_activation_lock(),
        Err(RendezvousError::NotRegularLockFile {
            kind: LockKind::Activation,
            ..
        })
    ));

    fs::remove_file(&rendezvous.paths.activation_lock)?;
    let lock = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&rendezvous.paths.activation_lock)?;
    lock.set_permissions(fs::Permissions::from_mode(0o640))?;
    drop(lock);
    assert!(matches!(
        rendezvous.try_acquire_activation_lock(),
        Err(RendezvousError::UnexpectedMode {
            expected: LOCK_FILE_MODE,
            actual: 0o640,
            ..
        })
    ));
    assert_eq!(mode(&rendezvous.paths.activation_lock)?, 0o640);

    fs::remove_file(&rendezvous.paths.activation_lock)?;
    let hard_link_target = rendezvous.runtime_directory().join("hard-link-target");
    fs::write(&hard_link_target, b"preserve hard link\n")?;
    fs::set_permissions(
        &hard_link_target,
        fs::Permissions::from_mode(LOCK_FILE_MODE),
    )?;
    fs::hard_link(&hard_link_target, &rendezvous.paths.activation_lock)?;
    assert!(matches!(
        rendezvous.try_acquire_activation_lock(),
        Err(RendezvousError::UnexpectedLockLinkCount {
            kind: LockKind::Activation,
            actual: 2,
            ..
        })
    ));
    assert_eq!(fs::read(&hard_link_target)?, b"preserve hard link\n");

    fs::remove_file(&rendezvous.paths.activation_lock)?;
    fs::remove_file(&hard_link_target)?;
    fs::write(&rendezvous.paths.activation_lock, b"")?;
    fs::set_permissions(
        &rendezvous.paths.activation_lock,
        fs::Permissions::from_mode(LOCK_FILE_MODE),
    )?;
    let expected_uid = different_uid(effective_uid());
    assert!(matches!(
        rendezvous.try_acquire_for_user(LockKind::Activation, expected_uid),
        Err(RendezvousError::UnexpectedOwner {
            expected,
            actual,
            ..
        }) if expected == expected_uid && actual == effective_uid()
    ));
    Ok(())
}

#[test]
fn both_locks_are_independent_durable_and_nonblocking_under_contention() -> TestResult {
    let fixture = TestDirectory::new("lock-contention")?;
    let scope = SessionScope::parse("orchbee-local")?;
    let first =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    let second = ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope)?;

    let activation = acquired(first.try_acquire_activation_lock()?)?;
    let lifetime = acquired(first.try_acquire_session_lifetime_lock()?)?;
    assert!(matches!(
        second.try_acquire_activation_lock()?,
        LockAttempt::Contended
    ));
    assert!(matches!(
        second.try_acquire_session_lifetime_lock()?,
        LockAttempt::Contended
    ));
    assert_eq!(mode(&first.paths.activation_lock)?, LOCK_FILE_MODE);
    assert_eq!(mode(&first.paths.session_lifetime_lock)?, LOCK_FILE_MODE);

    drop(activation);
    assert!(matches!(
        second.try_acquire_activation_lock()?,
        LockAttempt::Acquired(_)
    ));
    assert!(matches!(
        second.try_acquire_session_lifetime_lock()?,
        LockAttempt::Contended
    ));
    drop(lifetime);
    assert!(matches!(
        second.try_acquire_session_lifetime_lock()?,
        LockAttempt::Acquired(_)
    ));
    Ok(())
}

#[test]
fn accepts_stale_lock_files_without_truncating_or_replacing_them() -> TestResult {
    let fixture = TestDirectory::new("stale-locks")?;
    let rendezvous = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        fixture.path(),
        SessionScope::parse("orchbee-local")?,
    )?;

    for (kind, path, contents) in [
        (
            LockKind::Activation,
            &rendezvous.paths.activation_lock,
            b"stale activation\n".as_slice(),
        ),
        (
            LockKind::SessionLifetime,
            &rendezvous.paths.session_lifetime_lock,
            b"stale lifetime\n".as_slice(),
        ),
    ] {
        let mut stale = OpenOptions::new().create_new(true).write(true).open(path)?;
        stale.write_all(contents)?;
        stale.set_permissions(fs::Permissions::from_mode(LOCK_FILE_MODE))?;
        drop(stale);
        let before = fs::metadata(path)?.ino();

        let guard = acquired(rendezvous.try_acquire(kind)?)?;
        assert_eq!(fs::metadata(path)?.ino(), before);
        assert_eq!(fs::read(path)?, contents);
        drop(guard);
        assert!(path.exists());
    }
    Ok(())
}

#[test]
fn obsolete_lifetime_identity_is_refused_without_creating_a_second_lock() -> TestResult {
    let fixture = TestDirectory::new("obsolete-lifetime")?;
    let scope = SessionScope::parse("orchbee-local")?;
    let rendezvous =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    let runtime = rendezvous.runtime_directory().to_path_buf();
    drop(rendezvous);
    let obsolete = runtime.join("site-lifetime.lock");
    fs::write(&obsolete, b"retained old owner\n")?;
    fs::set_permissions(&obsolete, fs::Permissions::from_mode(LOCK_FILE_MODE))?;
    let before = fs::metadata(&obsolete)?.ino();

    for create in [false, true] {
        let result = if create {
            ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())
        } else {
            ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope.clone())
        };
        assert!(matches!(result, Err(RendezvousError::System { source, .. })
            if source.kind() == io::ErrorKind::InvalidData));
        assert!(!runtime.join("session-lifetime.lock").exists());
        assert_eq!(fs::metadata(&obsolete)?.ino(), before);
        assert_eq!(fs::read(&obsolete)?, b"retained old owner\n");
    }
    Ok(())
}

#[test]
fn dropping_rendezvous_resources_releases_only_locks_and_removes_nothing() -> TestResult {
    let fixture = TestDirectory::new("cleanup-ownership")?;
    let scope = SessionScope::parse("orchbee-local")?;
    let rendezvous =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    fs::write(rendezvous.client_socket_path(), b"client sentinel\n")?;
    fs::write(rendezvous.worker_socket_path(), b"worker sentinel\n")?;
    let activation_path = rendezvous.paths.activation_lock.clone();
    let lifetime_path = rendezvous.paths.session_lifetime_lock.clone();
    let runtime_directory = rendezvous.runtime_directory().to_path_buf();
    let client_socket = rendezvous.client_socket_path().to_path_buf();
    let worker_socket = rendezvous.worker_socket_path().to_path_buf();
    let activation = acquired(rendezvous.try_acquire_activation_lock()?)?;
    let lifetime = acquired(rendezvous.try_acquire_session_lifetime_lock()?)?;

    drop(activation);
    drop(lifetime);
    drop(rendezvous);

    assert!(runtime_directory.is_dir());
    assert!(activation_path.is_file());
    assert!(lifetime_path.is_file());
    assert_eq!(fs::read(&client_socket)?, b"client sentinel\n");
    assert_eq!(fs::read(&worker_socket)?, b"worker sentinel\n");
    let reopened = ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope)?;
    assert!(matches!(
        reopened.try_acquire_activation_lock()?,
        LockAttempt::Acquired(_)
    ));
    assert!(matches!(
        reopened.try_acquire_session_lifetime_lock()?,
        LockAttempt::Acquired(_)
    ));
    fs::remove_file(&client_socket)?;
    fs::remove_file(&worker_socket)?;
    Ok(())
}

fn acquired(attempt: LockAttempt) -> TestResult<ModeCheckedSessionLock> {
    match attempt {
        LockAttempt::Acquired(lock) => Ok(lock),
        LockAttempt::Contended => {
            Err(io::Error::other("test lock was unexpectedly contended").into())
        }
    }
}

fn effective_uid() -> u32 {
    geteuid().as_raw()
}

fn different_uid(uid: u32) -> u32 {
    if uid == u32::MAX { uid - 1 } else { uid + 1 }
}

fn mode(path: &Path) -> io::Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
}

struct TestDirectory {
    path: PathBuf,
    _filesystem_lock: MutexGuard<'static, ()>,
}

impl TestDirectory {
    fn new(label: &str) -> io::Result<Self> {
        let filesystem_lock = FILESYSTEM_TEST_LOCK
            .lock()
            .map_err(|_| io::Error::other("local Session filesystem test lock is poisoned"))?;
        let temporary_root = if cfg!(target_os = "macos") {
            Path::new("/private/tmp")
        } else {
            Path::new("/tmp")
        };
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = temporary_root.join(format!(
                "orchbee-{label}-{}-{fixture_id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        _filesystem_lock: filesystem_lock,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique local Session rendezvous test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

struct TestUmask(Mode);

impl TestUmask {
    fn set(mode: u32) -> Self {
        Self(nix::sys::stat::umask(exact_mode(mode)))
    }
}

impl Drop for TestUmask {
    fn drop(&mut self) {
        let _replaced = nix::sys::stat::umask(self.0);
    }
}

fn path_with_suffix(path: &Path, suffix: &[u8]) -> PathBuf {
    let mut bytes = path.as_os_str().as_bytes().to_vec();
    bytes.extend_from_slice(suffix);
    PathBuf::from(OsString::from_vec(bytes))
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn activation_marker_is_exclusive_immutable_and_validated() -> TestResult {
    let fixture = TestDirectory::new("marker")?;
    let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        fixture.path(),
        SessionScope::parse("orchbee-marker")?,
    )?;
    assert!(!session.activation_started()?);
    session.commit_activation()?;
    assert!(session.activation_started()?);
    assert_eq!(
        session.commit_activation().unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    let marker = session.runtime_directory().join("activation.started");
    assert_eq!(fs::metadata(&marker)?.permissions().mode() & 0o7777, 0o600);
    fs::write(&marker, b"corrupt")?;
    assert_eq!(
        session.activation_started().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    Ok(())
}

#[test]
fn activation_marker_aliases_and_socket_impostors_are_rejected() -> TestResult {
    let fixture = TestDirectory::new("marker-alias")?;
    let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        fixture.path(),
        SessionScope::parse("orchbee-marker")?,
    )?;
    fs::write(session.client_socket_path(), b"not a socket")?;
    assert_eq!(
        session.client_socket_exists().unwrap_err().kind(),
        io::ErrorKind::PermissionDenied
    );
    let marker = session.runtime_directory().join("activation.started");
    symlink("client.sock", &marker)?;
    assert!(session.activation_started().is_err());
    assert_eq!(
        session.commit_activation().unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    Ok(())
}

/// `server diagnose` descriptors: every named observation answers raw facts
/// through the validated directory descriptor — absent, present-with-kind,
/// symlinks never followed — and creates nothing of its own.
#[test]
fn named_observations_report_raw_facts_and_create_nothing() -> TestResult {
    let fixture = TestDirectory::new("observe")?;
    let session = ModeCheckedLocalSessionRendezvous::prepare_mode_checked(
        fixture.path(),
        SessionScope::parse("orchbee-observe")?,
    )?;
    let runtime = session.runtime_directory().to_path_buf();
    let client_socket = session.client_socket_path().to_path_buf();
    let worker_socket = session.worker_socket_path().to_path_buf();
    let pending_socket = ModeCheckedLocalSessionRendezvous::client_pending_socket_path(&runtime);
    let host_endpoint = ModeCheckedLocalSessionRendezvous::host_endpoint_path(&runtime);

    // An empty validated directory answers `Absent` everywhere.
    for observation in [
        session.observe_activation_marker(),
        session.observe_activation_lock(),
        session.observe_client_socket(),
        session.observe_worker_socket(),
        session.observe_client_pending_socket(),
        session.observe_agent_observation_socket(),
        session.observe_host_directory(),
        session.observe_host_endpoint(),
    ] {
        assert_eq!(observation?, EntryObservation::Absent);
    }
    assert_eq!(
        session.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Absent
    );

    // A committed marker reports its own facts: regular, owned, 0600,
    // empty, single link.
    session.commit_activation()?;
    assert_eq!(
        session.observe_activation_marker()?,
        EntryObservation::Present {
            kind: EntryKind::RegularFile,
            owner_matches: true,
            mode: 0o600,
            size: 0,
            link_count: 1,
        }
    );

    // A real socket leaf reports `Socket`; a regular file reports its own
    // kind — the verdict stays with the caller.
    let listener = std::os::unix::net::UnixListener::bind(&client_socket)?;
    fs::set_permissions(&client_socket, fs::Permissions::from_mode(0o600))?;
    assert_eq!(
        session.observe_client_socket()?,
        EntryObservation::Present {
            kind: EntryKind::Socket,
            owner_matches: true,
            mode: 0o600,
            size: 0,
            link_count: 1,
        }
    );
    fs::write(&pending_socket, b"pending sentinel\n")?;
    fs::set_permissions(&pending_socket, fs::Permissions::from_mode(0o644))?;
    assert_eq!(
        session.observe_client_pending_socket()?,
        EntryObservation::Present {
            kind: EntryKind::RegularFile,
            owner_matches: true,
            mode: 0o644,
            size: 17,
            link_count: 1,
        }
    );

    // A symlink is reported as a symlink — never followed, never opened.
    symlink(&client_socket, &worker_socket)?;
    let observed = session.observe_worker_socket()?;
    assert!(matches!(
        observed,
        EntryObservation::Present {
            kind: EntryKind::Symlink,
            owner_matches: true,
            ..
        }
    ));
    assert_eq!(fs::read_link(&worker_socket)?, client_socket);

    // The Host directory is observed in the runtime directory and its endpoint in the socket
    // folder.
    let host_directory = runtime.join(HOST_SCOPE_NAME);
    fs::create_dir(&host_directory)?;
    fs::set_permissions(&host_directory, fs::Permissions::from_mode(0o700))?;
    assert_eq!(
        session.observe_host_directory()?,
        EntryObservation::Present {
            kind: EntryKind::Directory,
            owner_matches: true,
            mode: 0o700,
            size: fs::metadata(&host_directory)?.len(),
            link_count: fs::metadata(&host_directory)?.nlink(),
        }
    );
    assert_eq!(session.observe_host_endpoint()?, EntryObservation::Absent);
    let host_listener = std::os::unix::net::UnixListener::bind(&host_endpoint)?;
    assert!(matches!(
        session.observe_host_endpoint()?,
        EntryObservation::Present {
            kind: EntryKind::Socket,
            owner_matches: true,
            ..
        }
    ));
    drop(host_listener);
    drop(listener);
    fs::remove_file(&host_endpoint)?;

    // A hard-linked or non-empty lock still reports facts — `Unsafe` is the
    // verdict, never an open attempt on an unadmitted name.
    fs::write(&session.paths.session_lifetime_lock, b"stale\n")?;
    fs::set_permissions(
        &session.paths.session_lifetime_lock,
        fs::Permissions::from_mode(0o600),
    )?;
    assert_eq!(
        session.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Free
    );
    fs::remove_file(&session.paths.session_lifetime_lock)?;
    fs::create_dir(&session.paths.session_lifetime_lock)?;
    assert_eq!(
        session.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Unsafe
    );
    fs::remove_dir(&session.paths.session_lifetime_lock)?;
    symlink("activation.started", &session.paths.session_lifetime_lock)?;
    assert_eq!(
        session.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Unsafe
    );
    fs::remove_file(&session.paths.session_lifetime_lock)?;

    // No observation created anything: exactly the leaves the test itself
    // made remain, with their bytes intact.
    let mut leaves: Vec<String> = fs::read_dir(&runtime)?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, io::Error>>()?;
    leaves.sort();
    assert_eq!(leaves, ["activation.started", "host"].map(str::to_owned));
    assert_eq!(fs::read(&pending_socket)?, b"pending sentinel\n");
    for socket in [&client_socket, &worker_socket, &pending_socket] {
        fs::remove_file(socket)?;
    }
    Ok(())
}

/// A contended lifetime lock observes `Held` without acquiring; releasing
/// the owner answers `Free` — a fact about the lock, never about a process.
#[test]
fn lifetime_observation_reports_held_free_and_unsafe() -> TestResult {
    let fixture = TestDirectory::new("observe-lifetime")?;
    let scope = SessionScope::parse("orchbee-lifetime")?;
    let owner =
        ModeCheckedLocalSessionRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    let observer = ModeCheckedLocalSessionRendezvous::validate_mode_checked(fixture.path(), scope)?;

    let held = acquired(owner.try_acquire_session_lifetime_lock()?)?;
    assert_eq!(
        observer.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Held
    );
    // The observer never takes the lock itself: probing it twice still
    // answers Held, and the owner's guard is untouched.
    assert_eq!(
        observer.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Held
    );
    drop(held);
    assert_eq!(
        observer.observe_session_lifetime_lock()?,
        SessionLifetimeObservation::Free
    );
    Ok(())
}
