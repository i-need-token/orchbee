use super::*;
use crate::{
    NamedSessionDeleteOutcome as Outcome, NamedSessionDeleteReason as Reason,
    RetainedSessionRecord as Retained, SessionCatalog as Catalog, SessionName, SessionStore,
};
use std::io;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn assert_checkpoint(path: &std::path::Path, payload: &[u8]) {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.get(..5), Some(b"OBCK\x01".as_slice()));
    assert_eq!(&bytes[45..], payload);
}
/// One activated Session whose runtime base and data root share `fixture`: its name, runtime
/// folder and Session folder.
fn prepared(fixture: &Fixture, text: &str, stopped: bool) -> (SessionName, PathBuf, PathBuf) {
    let name = SessionName::parse(text).unwrap();
    let root = fixture.root();
    let catalog = Catalog::try_lock(&fixture.0, &root).unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = fixture.open(name.as_str());
    let LockAttempt::Acquired(_lock) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    catalog.begin_activation(&session, [1; 16]).unwrap();
    session.commit_activation().unwrap();
    if stopped {
        stop(&root, &session, 9);
    }
    let mut store = SessionStore::open(&root, &session, 64).unwrap();
    store.publish(text.as_bytes()).unwrap();
    let runtime = session.runtime_directory().to_owned();
    (name, runtime, fixture.0.join("sessions").join(text))
}
/// A clean Server stop: the catalog record first, then the runtime marker.
fn stop(root: &crate::DataRoot, session: &Rendezvous, id: u8) {
    Retained::retain(root, session)
        .unwrap()
        .complete([id; 16])
        .unwrap();
    session
        .retain_active_record()
        .unwrap()
        .complete([id; 16])
        .unwrap();
}
/// Whether the Session's record carries the durable deletion fence.
fn deleting(state: &std::path::Path) -> bool {
    match fs::read(state.join("session.json")) {
        Ok(bytes) => {
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["lifecycle"]["state"]
                == "deleting"
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => panic!("record read: {error}"),
    }
}

#[test]
fn inventory_is_read_only_and_sorted_without_letter_case() {
    let fixture = Fixture::new();
    let initial = Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap();
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].name.as_str(), "default");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    let (_, _, beta) = prepared(&fixture, "beta", true);
    prepared(&fixture, "Alpha", true);
    let before = fs::read_dir(&fixture.0).unwrap().count();
    let entries = Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "Alpha", "beta"]
    );
    assert!(entries.iter().all(|entry| !entry.deleting));
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), before);
    fs::write(beta.join("session.json"), b"{}").unwrap();
    let entries = Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap();
    assert!(entries[2].unreadable);
    assert_eq!(fs::read(beta.join("session.json")).unwrap(), b"{}");
}

#[test]
fn stopped_deletion_removes_only_selected_owned_storage_and_is_idempotent() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "work", true);
    let (_, peer, peer_state) = prepared(&fixture, "peer", true);
    let session = fixture.open(name.as_str());
    let mut store = SessionStore::open(&fixture.root(), &session, 64).unwrap();
    store.publish(b"second").unwrap();
    store.publish(b"third").unwrap();
    drop(store);
    drop(session);
    for (leaf, bytes) in [
        (
            "checkpoint.corrupt-2026-11-08T10-00-00Z.raw",
            b"retained original".as_slice(),
        ),
        ("checkpoint.pending", b"uncommitted"),
        ("checkpoint.restore-notice", b"notice"),
        ("checkpoint.restore-notice.pending", b"uncommitted notice"),
        ("tui-position.json", b"position"),
        ("tui-position.pending", b"uncommitted position"),
    ] {
        fs::write(state.join(leaf), bytes).unwrap();
    }
    // Earlier layouts an earlier build kept.
    fs::create_dir(state.join("layouts")).unwrap();
    fs::write(
        state.join("layouts").join("1970-01-01T00-00-01Z.json"),
        b"earlier layout",
    )
    .unwrap();
    fs::create_dir(state.join("terminal-output")).unwrap();
    fs::write(state.join("terminal-output").join("1.bin"), b"output").unwrap();
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    assert_eq!(
        Catalog::try_lock(&fixture.0, &fixture.root())
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
    assert!(!state.exists());
    assert!(peer.is_dir());
    assert_checkpoint(&peer_state.join("checkpoint.json"), b"peer");
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Absent)
    );
    assert_eq!(
        catalog
            .delete(&SessionName::parse("default").unwrap(), deadline(), || {
                Ok(false)
            })
            .unwrap_err()
            .reason,
        Reason::DefaultSession
    );
    assert_eq!(
        Catalog::list(&fixture.0, &fixture.root(), deadline())
            .unwrap()
            .len(),
        2
    );
    drop(catalog);
    assert!(Catalog::try_lock(&fixture.0, &fixture.root()).is_ok());
}

#[test]
fn deletion_by_another_letter_case_removes_the_session_as_created() {
    let fixture = Fixture::new();
    let (_, runtime, state) = prepared(&fixture, "Work", true);
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    assert_eq!(
        catalog.delete(&SessionName::parse("wORK").unwrap(), deadline(), || Ok(
            false
        )),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
    assert!(!state.exists());
    assert_eq!(fs::read_dir(fixture.0.join("sessions")).unwrap().count(), 0);
}

#[test]
fn active_locked_or_listening_sessions_cannot_be_deleted() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "live", false);
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    let session = Rendezvous::from_runtime_directory(&runtime).unwrap();
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::RecoveryRequired
    );
    let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime")
    };
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(lifetime);
    let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(activation);
    stop(&fixture.root(), &session, 2);
    let endpoint = orchbee_local_ipc::LocalListener::bind(session.client_socket_path()).unwrap();
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(endpoint);
    let store = SessionStore::open(&fixture.root(), &session, 64).unwrap();
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    assert!(!deleting(&state));
    assert_checkpoint(&state.join("checkpoint.json"), b"live");
    drop(store);
    drop(session);
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
}

#[test]
fn checkpoint_sharing_failure_fences_start_and_retry_completes() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "blocked", true);
    let held = permissions::open_file(&state.join("checkpoint.json"), false, false).unwrap();
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    let failure = catalog.delete(&name, deadline(), || Ok(false)).unwrap_err();
    assert_eq!(failure.reason, Reason::Filesystem);
    assert!(failure.pending);
    assert!(deleting(&state));
    assert_checkpoint(&state.join("checkpoint.json"), b"blocked");
    assert_eq!(
        catalog.prepare_activation(&name).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap()[1].deleting);
    drop(held);
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!state.exists());
    assert!(!runtime.exists());
}

#[test]
fn retry_accepts_partial_runtime_and_missing_runtime() {
    for remove_runtime in [false, true] {
        let fixture = Fixture::new();
        let (name, runtime, state) = prepared(&fixture, "partial", true);
        let held = permissions::open_directory(&runtime, false).unwrap();
        let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
        assert!(
            catalog
                .delete(&name, deadline(), || Ok(false))
                .unwrap_err()
                .pending
        );
        assert!(!state.join("checkpoint.json").exists());
        assert!(!runtime.join("activation.started").exists());
        assert!(Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap()[1].deleting);
        drop(held);
        if remove_runtime {
            fs::remove_dir(&runtime).unwrap();
        }
        assert_eq!(
            catalog.delete(&name, deadline(), || Ok(false)),
            Ok(Outcome::Deleted)
        );
        assert!(!runtime.exists());
        assert!(!state.exists());
    }
}

#[test]
fn unknown_files_and_hard_links_refuse_before_the_fence_and_preserve_bytes() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "unknown", true);
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    for foreign in [runtime.join("user-data"), state.join("user-data")] {
        fs::write(&foreign, b"do not delete").unwrap();
        assert!(
            !catalog
                .delete(&name, deadline(), || Ok(false))
                .unwrap_err()
                .pending
        );
        assert_eq!(fs::read(&foreign).unwrap(), b"do not delete");
        fs::remove_file(foreign).unwrap();
    }
    let checkpoint = state.join("checkpoint.json");
    let alias = fixture.0.join("linked-user-data");
    fs::hard_link(&checkpoint, &alias).unwrap();
    assert!(
        !catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .pending
    );
    assert_checkpoint(&checkpoint, b"unknown");
    assert!(!deleting(&state));
    fs::remove_file(alias).unwrap();
    assert_eq!(
        catalog
            .delete(&name, Instant::now(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::TimedOut
    );
    assert!(!deleting(&state));
}

#[test]
fn orphaned_checkpoint_without_a_record_is_not_cleanup_authority() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "orphan", true);
    // Exact test-owned files, after every runtime/store owner has ended.
    for entry in fs::read_dir(&runtime).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    fs::remove_dir(&runtime).unwrap();
    for leaf in ["session.json", "session.lock"] {
        fs::remove_file(state.join(leaf)).unwrap();
    }
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::RecoveryRequired
    );
    assert_checkpoint(&state.join("checkpoint.json"), b"orphan");
    let entries = Catalog::list(&fixture.0, &fixture.root(), deadline()).unwrap();
    assert_eq!(entries[1].name, name);
    assert!(!entries[1].deleting);
    assert_checkpoint(&state.join("checkpoint.json"), b"orphan");
}

#[test]
fn stale_record_staging_is_replaced_under_namespace_ownership() {
    let fixture = Fixture::new();
    let (name, _, state) = prepared(&fixture, "staging", true);
    let staging = state.join("session.pending");
    fs::write(&staging, b"interrupted staging").unwrap();
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!staging.exists());
    assert!(!state.exists());
}

#[test]
fn interruption_before_and_after_the_fence_preserves_honest_recovery_state() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "interrupt", true);
    let catalog = Catalog::try_lock(&fixture.0, &fixture.root()).unwrap();
    let failure = catalog.delete(&name, deadline(), || Ok(true)).unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(!failure.pending);
    assert!(!deleting(&state));
    assert_checkpoint(&state.join("checkpoint.json"), b"interrupt");
    let failure = catalog
        .delete(&name, deadline(), || Ok(deleting(&state)))
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    assert_checkpoint(&state.join("checkpoint.json"), b"interrupt");
    let interrupted_retry = catalog.delete(&name, deadline(), || Ok(true)).unwrap_err();
    assert_eq!(interrupted_retry.reason, Reason::Interrupted);
    assert!(
        interrupted_retry.pending,
        "an existing fence survives early retry cancellation"
    );
    let failure = catalog
        .delete(&name, deadline(), || {
            Ok(!state.join("checkpoint.json").exists())
        })
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    assert!(!state.join("checkpoint.json").exists());
    assert!(runtime.is_dir());
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
    assert!(!state.exists());
}
