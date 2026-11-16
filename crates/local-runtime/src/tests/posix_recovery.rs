use super::*;
use crate::{
    ActivationState, RetainedSessionRecord as Retained, SessionCatalog as Catalog, SessionName,
    durable_sessions::{Directory, Lifecycle},
};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// A Server that ended without its stop, by `SIGKILL` or a lost machine, leaves its marker, its
/// Active record and its socket files. The next activation recovers it unless a Terminal Host
/// still holds its lock, and a clean stop needs only its receipt.
#[test]
fn stale_sockets_never_block_recovery_and_a_held_host_lock_does() {
    let base = Fixture::new();
    let local = Fixture::new();
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    let name = SessionName::parse("default").unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = base.open(name.as_str());
    let lifetime = {
        let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap()
        else {
            panic!("activation");
        };
        catalog.begin_activation(&session, [1; 16]).unwrap();
        session.commit_activation().unwrap();
        let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
        else {
            panic!("lifetime");
        };
        lifetime
    };
    let runtime = session.runtime_directory().to_path_buf();
    let [client, worker] = Rendezvous::socket_paths(&runtime);
    for path in [&client, &worker] {
        drop(std::os::unix::net::UnixListener::bind(path).unwrap());
    }
    let host = runtime.join(crate::HOST_SCOPE_NAME);
    fs::create_dir(&host).unwrap();
    fs::set_permissions(&host, fs::Permissions::from_mode(0o700)).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(host.join(format!("{}.l", Rendezvous::host_endpoint_id(&runtime))))
        .unwrap();
    held.try_lock().unwrap();
    // The Server is gone.
    drop(lifetime);

    let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime");
    };
    let refused = catalog
        .recover_inactive(&session, &activation, &lifetime)
        .unwrap_err();
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(session.activation_state().unwrap(), ActivationState::Active);
    let directory = Directory::open(&local.root()).unwrap().unwrap();
    assert_eq!(
        directory.read("default").unwrap().unwrap().lifecycle,
        Lifecycle::Active
    );

    drop(held);
    assert!(
        catalog
            .recover_inactive(&session, &activation, &lifetime)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        session.activation_state().unwrap(),
        ActivationState::Crashed {
            activation_id: [1; 16]
        }
    );
    catalog.begin_activation(&session, [2; 16]).unwrap();
    session.commit_activation().unwrap();

    // A clean stop publishes no checkpoint on POSIX, and its receipt admits the next start.
    let mut record = Retained::retain(&local.root(), &session).unwrap();
    record.complete([3; 16]).unwrap();
    session
        .retain_active_record()
        .unwrap()
        .complete([3; 16])
        .unwrap();
    drop((lifetime, activation));
    let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation");
    };
    catalog.begin_activation(&session, [4; 16]).unwrap();
    session.commit_activation().unwrap();
    for path in [&client, &worker] {
        fs::remove_file(path).unwrap();
    }
}

/// A Terminal Host leaves its lock file in `host` when it exits. Deleting a stopped Session
/// removes that file, and refuses while a Host still holds its lock.
#[test]
fn deletion_removes_an_exited_host_lock_and_refuses_a_held_one() {
    use crate::{NamedSessionDeleteOutcome as Outcome, NamedSessionDeleteReason as Reason};
    let fixture = Fixture::new();
    let root = fixture.root();
    let catalog = Catalog::try_lock(&fixture.0, &root).unwrap();
    let name = SessionName::parse("work").unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = fixture.open(name.as_str());
    {
        let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap()
        else {
            panic!("activation");
        };
        catalog.begin_activation(&session, [1; 16]).unwrap();
        session.commit_activation().unwrap();
        Retained::retain(&root, &session)
            .unwrap()
            .complete([9; 16])
            .unwrap();
        session
            .retain_active_record()
            .unwrap()
            .complete([9; 16])
            .unwrap();
    }
    let runtime = session.runtime_directory().to_path_buf();
    drop(session);
    let host = runtime.join(crate::HOST_SCOPE_NAME);
    fs::create_dir(&host).unwrap();
    fs::set_permissions(&host, fs::Permissions::from_mode(0o700)).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(host.join(format!("{}.l", Rendezvous::host_endpoint_id(&runtime))))
        .unwrap();
    held.try_lock().unwrap();
    let deadline = || Instant::now() + Duration::from_secs(5);
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(held);
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
}
