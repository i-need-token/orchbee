use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn site_scope_accepts_one_canonical_bounded_component() -> TestResult {
    for valid in [
        "orchbee-release",
        "orchbee-local",
        "orchbee-dev-a1b2c3",
        "a",
        &"a".repeat(MAX_SITE_SCOPE_BYTES),
    ] {
        assert_eq!(SiteScope::parse(valid)?.as_str(), valid);
    }

    for invalid in [
        "",
        "1orchbee",
        "Orchbee",
        "orchbee_dev",
        "orchbee--dev",
        "orchbee-",
        ".",
        "..",
        "orchbee/dev",
        &format!("a{}", "0".repeat(MAX_SITE_SCOPE_BYTES)),
    ] {
        assert!(matches!(
            SiteScope::parse(invalid),
            Err(RendezvousError::InvalidSiteScope)
        ));
    }
    Ok(())
}

#[test]
fn derives_exact_stable_paths_from_the_typed_scope() -> TestResult {
    let paths = LocalSitePaths::new(
        Path::new("/run/user/501"),
        &SiteScope::parse("orchbee-dev-a1b2c3")?,
    )?;

    assert_eq!(
        paths.runtime_directory,
        Path::new("/run/user/501/orchbee-dev-a1b2c3")
    );
    assert_eq!(
        paths.activation_lock,
        Path::new("/run/user/501/orchbee-dev-a1b2c3/activation.lock")
    );
    assert_eq!(
        paths.site_lifetime_lock,
        Path::new("/run/user/501/orchbee-dev-a1b2c3/site-lifetime.lock")
    );
    assert_eq!(
        paths.client_socket,
        Path::new("/run/user/501/orchbee-dev-a1b2c3/client.sock")
    );
    assert_eq!(
        paths.worker_socket,
        Path::new("/run/user/501/orchbee-dev-a1b2c3/worker.sock")
    );
    assert!(matches!(
        LocalSitePaths::new(Path::new("relative"), &SiteScope::parse("orchbee-local")?),
        Err(RendezvousError::RuntimeBaseNotAbsolute { .. })
    ));
    Ok(())
}

#[test]
fn creates_and_validates_an_owned_exact_mode_runtime_directory() -> TestResult {
    let fixture = TestDirectory::new("runtime-directory")?;
    let scope = SiteScope::parse("orchbee-local")?;
    let rendezvous =
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;

    assert_eq!(
        rendezvous.runtime_directory(),
        fixture.path().join(scope.as_str())
    );
    assert_eq!(
        rendezvous.client_socket_path(),
        rendezvous.runtime_directory().join(CLIENT_SOCKET_NAME)
    );
    assert_eq!(
        rendezvous.worker_socket_path(),
        rendezvous.runtime_directory().join(WORKER_SOCKET_NAME)
    );
    assert_eq!(
        mode(rendezvous.runtime_directory())?,
        RUNTIME_DIRECTORY_MODE
    );
    assert_eq!(
        fs::metadata(rendezvous.runtime_directory())?.uid(),
        effective_uid()
    );

    let validated = ModeCheckedLocalSiteRendezvous::validate_mode_checked(fixture.path(), scope)?;
    assert_eq!(
        validated.runtime_directory(),
        rendezvous.runtime_directory()
    );
    Ok(())
}

#[test]
fn validation_does_not_create_a_missing_scope() -> TestResult {
    let fixture = TestDirectory::new("validate-missing")?;
    let scope = SiteScope::parse("orchbee-local")?;
    let expected_path = fixture.path().join(scope.as_str());

    assert!(ModeCheckedLocalSiteRendezvous::validate_mode_checked(fixture.path(), scope).is_err());
    assert!(!expected_path.exists());
    Ok(())
}

#[test]
fn rejects_runtime_base_and_scope_symlinks_without_following_them() -> TestResult {
    let fixture = TestDirectory::new("directory-symlinks")?;
    let actual_base = fixture.path().join("actual-base");
    fs::create_dir(&actual_base)?;
    let linked_base = fixture.path().join("linked-base");
    symlink(&actual_base, &linked_base)?;

    assert!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
            &linked_base,
            SiteScope::parse("orchbee-local")?
        )
        .is_err()
    );
    assert!(!actual_base.join("orchbee-local").exists());

    let linked_scope = SiteScope::parse("orchbee-linked")?;
    let target = fixture.path().join("scope-target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(RUNTIME_DIRECTORY_MODE))?;
    symlink(&target, fixture.path().join(linked_scope.as_str()))?;
    assert!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), linked_scope).is_err()
    );
    assert_eq!(mode(&target)?, RUNTIME_DIRECTORY_MODE);
    Ok(())
}

#[test]
fn rejects_non_directory_wrong_owner_and_wrong_mode_scopes() -> TestResult {
    let fixture = TestDirectory::new("directory-metadata")?;
    let file_base = fixture.path().join("file-base");
    fs::write(&file_base, b"preserve base\n")?;
    assert!(matches!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
            &file_base,
            SiteScope::parse("orchbee-local")?
        ),
        Err(RendezvousError::NotDirectory { .. })
    ));
    assert_eq!(fs::read(&file_base)?, b"preserve base\n");

    let file_scope = SiteScope::parse("orchbee-file")?;
    fs::write(fixture.path().join(file_scope.as_str()), b"preserve\n")?;
    assert!(matches!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), file_scope),
        Err(RendezvousError::NotDirectory { .. })
    ));

    let owner_scope = SiteScope::parse("orchbee-owner")?;
    let different_uid = different_uid(effective_uid());
    assert!(matches!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked_for_user(
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

    let mode_scope = SiteScope::parse("orchbee-mode")?;
    let mode_path = fixture.path().join(mode_scope.as_str());
    fs::create_dir(&mode_path)?;
    fs::set_permissions(&mode_path, fs::Permissions::from_mode(0o750))?;
    assert!(matches!(
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), mode_scope),
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
    let rendezvous = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        fixture.path(),
        SiteScope::parse("orchbee-local")?,
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
    let scope = SiteScope::parse("orchbee-local")?;
    let first =
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    let second = ModeCheckedLocalSiteRendezvous::validate_mode_checked(fixture.path(), scope)?;

    let activation = acquired(first.try_acquire_activation_lock()?)?;
    let lifetime = acquired(first.try_acquire_site_lifetime_lock()?)?;
    assert!(matches!(
        second.try_acquire_activation_lock()?,
        LockAttempt::Contended
    ));
    assert!(matches!(
        second.try_acquire_site_lifetime_lock()?,
        LockAttempt::Contended
    ));
    assert_eq!(mode(&first.paths.activation_lock)?, LOCK_FILE_MODE);
    assert_eq!(mode(&first.paths.site_lifetime_lock)?, LOCK_FILE_MODE);

    drop(activation);
    assert!(matches!(
        second.try_acquire_activation_lock()?,
        LockAttempt::Acquired(_)
    ));
    assert!(matches!(
        second.try_acquire_site_lifetime_lock()?,
        LockAttempt::Contended
    ));
    drop(lifetime);
    assert!(matches!(
        second.try_acquire_site_lifetime_lock()?,
        LockAttempt::Acquired(_)
    ));
    Ok(())
}

#[test]
fn accepts_stale_lock_files_without_truncating_or_replacing_them() -> TestResult {
    let fixture = TestDirectory::new("stale-locks")?;
    let rendezvous = ModeCheckedLocalSiteRendezvous::prepare_mode_checked(
        fixture.path(),
        SiteScope::parse("orchbee-local")?,
    )?;

    for (kind, path, contents) in [
        (
            LockKind::Activation,
            &rendezvous.paths.activation_lock,
            b"stale activation\n".as_slice(),
        ),
        (
            LockKind::SiteLifetime,
            &rendezvous.paths.site_lifetime_lock,
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
fn dropping_rendezvous_resources_releases_only_locks_and_removes_nothing() -> TestResult {
    let fixture = TestDirectory::new("cleanup-ownership")?;
    let scope = SiteScope::parse("orchbee-local")?;
    let rendezvous =
        ModeCheckedLocalSiteRendezvous::prepare_mode_checked(fixture.path(), scope.clone())?;
    fs::write(rendezvous.client_socket_path(), b"client sentinel\n")?;
    fs::write(rendezvous.worker_socket_path(), b"worker sentinel\n")?;
    let activation_path = rendezvous.paths.activation_lock.clone();
    let lifetime_path = rendezvous.paths.site_lifetime_lock.clone();
    let runtime_directory = rendezvous.runtime_directory().to_path_buf();
    let activation = acquired(rendezvous.try_acquire_activation_lock()?)?;
    let lifetime = acquired(rendezvous.try_acquire_site_lifetime_lock()?)?;

    drop(activation);
    drop(lifetime);
    drop(rendezvous);

    assert!(runtime_directory.is_dir());
    assert!(activation_path.is_file());
    assert!(lifetime_path.is_file());
    assert_eq!(
        fs::read(runtime_directory.join(CLIENT_SOCKET_NAME))?,
        b"client sentinel\n"
    );
    assert_eq!(
        fs::read(runtime_directory.join(WORKER_SOCKET_NAME))?,
        b"worker sentinel\n"
    );
    let reopened = ModeCheckedLocalSiteRendezvous::validate_mode_checked(fixture.path(), scope)?;
    assert!(matches!(
        reopened.try_acquire_activation_lock()?,
        LockAttempt::Acquired(_)
    ));
    assert!(matches!(
        reopened.try_acquire_site_lifetime_lock()?,
        LockAttempt::Acquired(_)
    ));
    Ok(())
}

fn acquired(attempt: LockAttempt) -> TestResult<ModeCheckedSiteLock> {
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

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> io::Result<Self> {
        for _attempt in 0..100 {
            let fixture_id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchbee-{label}-{}-{fixture_id}",
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
            "could not allocate a unique local Site rendezvous test directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _result = fs::remove_dir_all(&self.0);
    }
}
