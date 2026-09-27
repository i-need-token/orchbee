use super::*;
use crate::{
    WindowsNamedSiteCatalog as Catalog, WindowsNamedSiteDeleteOutcome as Outcome,
    WindowsNamedSiteDeleteReason as Reason, WindowsSessionStore, WindowsSiteName,
};
use std::io;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn scope(name: &WindowsSiteName) -> String {
    name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap())
}
fn prepared(fixture: &Fixture, text: &str, stopped: bool) -> (WindowsSiteName, PathBuf, PathBuf) {
    let name = WindowsSiteName::parse(text).unwrap();
    let site = fixture.open(&scope(&name));
    let LockAttempt::Acquired(lock) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    site.register_session_name(&lock, &name).unwrap();
    site.commit_activation().unwrap();
    if stopped {
        site.retain_active_record()
            .unwrap()
            .complete([9; 16])
            .unwrap();
    }
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    store.publish(text.as_bytes()).unwrap();
    let runtime = site.runtime_directory().to_owned();
    let state = fixture
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    (name, runtime, state)
}
fn intent(fixture: &Fixture, name: &WindowsSiteName) -> PathBuf {
    fixture.0.join(format!("{}.deleting", scope(name)))
}

#[test]
fn inventory_is_read_only_sorted_and_case_sensitive() {
    let fixture = Fixture::new();
    let initial = Catalog::list(&fixture.0, deadline()).unwrap();
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].name.as_str(), "default");
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    let (_, upper, _) = prepared(&fixture, "Work", true);
    let (_, lower, _) = prepared(&fixture, "work", true);
    assert_ne!(upper, lower);
    let before = fs::read_dir(&fixture.0).unwrap().count();
    let entries = Catalog::list(&fixture.0, deadline()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "Work", "work"]
    );
    assert!(entries.iter().all(|entry| !entry.deleting));
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), before);
    fs::write(lower.join("session-name.json"), b"{}").unwrap();
    assert!(Catalog::list(&fixture.0, deadline()).is_err());
    assert_eq!(fs::read(lower.join("session-name.json")).unwrap(), b"{}");
}

#[test]
fn stopped_deletion_removes_only_selected_owned_storage_and_is_idempotent() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "work", true);
    let (_, peer, peer_state) = prepared(&fixture, "Work", true);
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    assert_eq!(
        Catalog::try_lock(&fixture.0).err().unwrap().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
    assert!(!state.exists());
    assert!(!intent(&fixture, &name).exists());
    assert!(peer.is_dir());
    assert_eq!(
        fs::read(peer_state.join("checkpoint.json")).unwrap(),
        b"Work"
    );
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Absent)
    );
    assert_eq!(
        catalog
            .delete(
                &WindowsSiteName::parse("default").unwrap(),
                &fixture.0,
                deadline(),
                || Ok(false)
            )
            .unwrap_err()
            .reason,
        Reason::DefaultSession
    );
    assert_eq!(Catalog::list(&fixture.0, deadline()).unwrap().len(), 2);
    drop(catalog);
    assert!(Catalog::try_lock(&fixture.0).is_ok());
}

#[test]
fn active_locked_or_listening_sessions_cannot_be_deleted() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "live", false);
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    let site = Rendezvous::from_runtime_directory(&runtime).unwrap();
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::RecoveryRequired
    );
    let LockAttempt::Acquired(lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(lifetime);
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(activation);
    site.retain_active_record()
        .unwrap()
        .complete([2; 16])
        .unwrap();
    let endpoint = orchbee_local_ipc::LocalListener::bind(site.client_socket_path()).unwrap();
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(endpoint);
    let store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    assert!(!intent(&fixture, &name).exists());
    assert_eq!(fs::read(state.join("checkpoint.json")).unwrap(), b"live");
    drop(store);
    drop(site);
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
}

#[test]
fn checkpoint_sharing_failure_fences_start_and_retry_completes() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "blocked", true);
    let held = permissions::open_file(&state.join("checkpoint.json"), false, false).unwrap();
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    let failure = catalog
        .delete(&name, &fixture.0, deadline(), || Ok(false))
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Filesystem);
    assert!(failure.pending);
    assert!(intent(&fixture, &name).is_file());
    assert_eq!(fs::read(state.join("checkpoint.json")).unwrap(), b"blocked");
    assert!(
        Rendezvous::prepare_mode_checked(&fixture.0, SiteScope::parse(&scope(&name)).unwrap())
            .is_err()
    );
    assert!(Catalog::list(&fixture.0, deadline()).unwrap()[1].deleting);
    drop(held);
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!state.exists());
    assert!(!runtime.exists());
    assert!(!intent(&fixture, &name).exists());
}

#[test]
fn retry_accepts_partial_runtime_without_identity_and_missing_runtime() {
    for remove_runtime in [false, true] {
        let fixture = Fixture::new();
        let (name, runtime, state) = prepared(&fixture, "partial", true);
        let held = permissions::open_directory(&runtime, false).unwrap();
        let catalog = Catalog::try_lock(&fixture.0).unwrap();
        assert!(
            catalog
                .delete(&name, &fixture.0, deadline(), || Ok(false))
                .unwrap_err()
                .pending
        );
        assert!(!state.exists());
        assert!(!runtime.join("session-name.json").exists());
        assert!(Catalog::list(&fixture.0, deadline()).unwrap()[1].deleting);
        drop(held);
        if remove_runtime {
            fs::remove_dir(&runtime).unwrap();
        }
        assert_eq!(
            catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
            Ok(Outcome::Deleted)
        );
        assert!(!runtime.exists());
        assert!(!intent(&fixture, &name).exists());
    }
}

#[test]
fn unknown_files_and_hard_links_refuse_before_intent_and_preserve_bytes() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "unknown", true);
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    let foreign = runtime.join("user-data");
    fs::write(&foreign, b"do not delete").unwrap();
    assert!(
        !catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .pending
    );
    assert_eq!(fs::read(&foreign).unwrap(), b"do not delete");
    fs::remove_file(foreign).unwrap();
    let checkpoint = state.join("checkpoint.json");
    let alias = fixture.0.join("linked-user-data");
    fs::hard_link(&checkpoint, &alias).unwrap();
    assert!(
        !catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .pending
    );
    assert_eq!(fs::read(&checkpoint).unwrap(), b"unknown");
    assert!(!intent(&fixture, &name).exists());
    fs::remove_file(alias).unwrap();
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, Instant::now(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::TimedOut
    );
    assert!(!intent(&fixture, &name).exists());
}

#[test]
fn orphaned_checkpoint_and_malformed_intent_are_not_cleanup_authority() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "orphan", true);
    // Exact test-owned files, after every runtime/store owner has ended.
    for entry in fs::read_dir(&runtime).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    fs::remove_dir(&runtime).unwrap();
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    assert_eq!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::RecoveryRequired
    );
    assert_eq!(fs::read(state.join("checkpoint.json")).unwrap(), b"orphan");
    fs::write(
        intent(&fixture, &name),
        br#"{"version":1,"name":"different","stopped_request_id":null}"#,
    )
    .unwrap();
    assert!(
        catalog
            .delete(&name, &fixture.0, deadline(), || Ok(false))
            .is_err()
    );
    assert!(Catalog::list(&fixture.0, deadline()).is_err());
    assert!(
        Rendezvous::prepare_mode_checked(&fixture.0, SiteScope::parse(&scope(&name)).unwrap())
            .is_err()
    );
    assert_eq!(fs::read(state.join("checkpoint.json")).unwrap(), b"orphan");
}

#[test]
fn validated_stale_staging_is_replaced_under_namespace_ownership() {
    let fixture = Fixture::new();
    let (name, _, _) = prepared(&fixture, "staging", true);
    let staging = fixture.0.join(format!("{}.deleting.pending", scope(&name)));
    fs::write(&staging, b"interrupted staging").unwrap();
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!staging.exists());
    assert!(!intent(&fixture, &name).exists());
}

#[test]
fn interruption_before_and_after_intent_preserves_honest_recovery_state() {
    let fixture = Fixture::new();
    let (name, runtime, state) = prepared(&fixture, "interrupt", true);
    let catalog = Catalog::try_lock(&fixture.0).unwrap();
    let failure = catalog
        .delete(&name, &fixture.0, deadline(), || Ok(true))
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(!failure.pending);
    assert!(!intent(&fixture, &name).exists());
    assert_eq!(
        fs::read(state.join("checkpoint.json")).unwrap(),
        b"interrupt"
    );
    let failure = catalog
        .delete(&name, &fixture.0, deadline(), || {
            Ok(intent(&fixture, &name).exists())
        })
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    assert_eq!(
        fs::read(state.join("checkpoint.json")).unwrap(),
        b"interrupt"
    );
    let interrupted_retry = catalog
        .delete(&name, &fixture.0, deadline(), || Ok(true))
        .unwrap_err();
    assert_eq!(interrupted_retry.reason, Reason::Interrupted);
    assert!(
        interrupted_retry.pending,
        "an existing intent survives early retry cancellation"
    );
    let failure = catalog
        .delete(&name, &fixture.0, deadline(), || Ok(!state.exists()))
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    assert!(!state.exists());
    assert!(runtime.is_dir());
    assert_eq!(
        catalog.delete(&name, &fixture.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!runtime.exists());
    assert!(!intent(&fixture, &name).exists());
}
