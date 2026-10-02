use super::*;
use crate::{
    ActivationState, WindowsNamedSiteCatalog as Catalog, WindowsNamedSiteDeleteOutcome as Outcome,
    WindowsNamedSiteDeleteReason as Reason, WindowsRetainedSessionRecord as Retained,
    WindowsSessionStore, WindowsSiteName,
    durable_sessions_windows::{Directory, Lifecycle, Record},
};
use std::{io, path::Path};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn scope(name: &WindowsSiteName) -> String {
    name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap())
}
fn record_path(base: &Fixture, local: &Fixture, name: &WindowsSiteName) -> PathBuf {
    local
        .0
        .join("Orchbee/session-catalog")
        .join(Rendezvous::host_endpoint_id(
            &base.0.canonicalize().unwrap(),
        ))
        .join(format!("{}.json", scope(name)))
}
fn record_version(base: &Fixture, local: &Fixture, name: &WindowsSiteName) -> u64 {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(record_path(base, local, name)).unwrap()).unwrap();
    value["version"].as_u64().unwrap()
}
fn rewrite_record_version(base: &Fixture, local: &Fixture, name: &WindowsSiteName, version: u8) {
    let path = record_path(base, local, name);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["version"] = serde_json::json!(version);
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}
fn start(
    base: &Fixture,
    local: &Fixture,
    text: &str,
    generation: u8,
) -> (WindowsSiteName, Rendezvous, Retained) {
    let name = WindowsSiteName::parse(text).unwrap();
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert_eq!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .unwrap(),
        Some(name.clone())
    );
    let site = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [generation; 16]).unwrap();
    site.commit_activation().unwrap();
    let mut store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
    if store.read().unwrap().is_none() {
        store.publish(text.as_bytes()).unwrap();
    }
    let owner = Retained::retain(&local.0, &site).unwrap().unwrap();
    (name, site, owner)
}
fn stop(site: &Rendezvous, owner: &mut Retained, id: u8) {
    owner.complete([id; 16]).unwrap();
    site.retain_active_record()
        .unwrap()
        .complete([id; 16])
        .unwrap();
}
fn remove_runtime(base: &Fixture, runtime: &Path) {
    assert_eq!(
        runtime.parent(),
        Some(base.0.canonicalize().unwrap().as_path())
    );
    // This helper only removes exact fixture-owned leaves after all runtime handles end.
    for leaf in [
        "session-name.json",
        "activation.started",
        "activation.lock",
        "site-lifetime.lock",
    ] {
        match fs::remove_file(runtime.join(leaf)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("fixture removal: {error}"),
        }
    }
    assert_eq!(fs::read_dir(runtime).unwrap().count(), 0);
    fs::remove_dir(runtime).unwrap();
}

#[test]
fn restore_notice_reads_while_server_owns_store_and_only_projects_the_active_generation() {
    use crate::WindowsCheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    for name in ["default", "notice-test"] {
        let (_, site, owner) = start(&base, &local, name, 1);
        let store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
        assert_eq!(
            WindowsSessionStore::open(&local.0, &site, 64)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        for notice in [Some(Notice::Empty), Some(Notice::Recovered), None] {
            store.publish_restore_notice(&owner, notice).unwrap();
            assert_eq!(site.checkpoint_restore_notice(&local.0).unwrap(), notice);
        }
        let path = store
            .directory
            .join(crate::checkpoint_restore_notice_windows::FILE);
        let stale = b"orchbee-checkpoint-restore-v1:02020202020202020202020202020202:E";
        fs::write(&path, stale).unwrap();
        assert_eq!(site.checkpoint_restore_notice(&local.0).unwrap(), None);
        assert_eq!(fs::read(&path).unwrap(), stale);
        let invalid = [b'X'; 128];
        fs::write(&path, invalid).unwrap();
        assert_eq!(
            site.checkpoint_restore_notice(&local.0).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&path).unwrap(), invalid);
        store
            .publish_restore_notice(&owner, Some(Notice::Recovered))
            .unwrap();
        assert_eq!(
            site.checkpoint_restore_notice(&local.0).unwrap(),
            Some(Notice::Recovered)
        );
    }
}

#[test]
fn blocked_notice_replacement_preserves_current_feedback_and_reclaims_pending() {
    use crate::WindowsCheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    let (_, site, owner) = start(&base, &local, "notice-blocked", 1);
    let store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
    store
        .publish_restore_notice(&owner, Some(Notice::Empty))
        .unwrap();
    let path = store
        .directory
        .join(crate::checkpoint_restore_notice_windows::FILE);
    let before = fs::read(&path).unwrap();
    let consumer = permissions::open_snapshot_file(&path).unwrap();
    assert!(store.publish_restore_notice(&owner, None).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        !store
            .directory
            .join(crate::checkpoint_restore_notice_windows::PENDING)
            .exists()
    );
    drop(consumer);
    store.publish_restore_notice(&owner, None).unwrap();
    assert_eq!(site.checkpoint_restore_notice(&local.0).unwrap(), None);
}

#[test]
fn default_and_exact_names_remain_discoverable_and_restart_after_runtime_loss() {
    let base = Fixture::new();
    let local = Fixture::new();
    for text in ["default", "work", "Work"] {
        let (_, site, mut owner) = start(&base, &local, text, 1);
        stop(&site, &mut owner, 9);
        let runtime = site.runtime_directory().to_owned();
        drop(site);
        remove_runtime(&base, &runtime);
    }
    let before = fs::read_dir(&base.0).unwrap().count();
    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "Work", "work"]
    );
    for entry in &entries {
        assert!(!entry.runtime.exists());
        assert_eq!(
            entry.absent_runtime_state().unwrap(),
            Some(ActivationState::Stopped {
                request_id: [9; 16]
            })
        );
    }
    assert_eq!(fs::read_dir(&base.0).unwrap().count(), before);
    for text in ["default", "work", "Work"] {
        let (_, site, mut owner) = start(&base, &local, text, 2);
        let store = WindowsSessionStore::open(&local.0, &site, 64).unwrap();
        assert_eq!(store.read().unwrap().unwrap(), text.as_bytes());
        drop(store);
        stop(&site, &mut owner, 8);
    }
}

#[test]
fn begin_activation_publishes_v3_before_checkpoint_storage_can_write() {
    let base = Fixture::new();
    let local = Fixture::new();
    let name = WindowsSiteName::parse("catalog-v3-first").unwrap();
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert_eq!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .unwrap(),
        Some(name.clone())
    );
    let site = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    site.register_session_name(&activation, &name).unwrap();
    let state = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(site.runtime_directory()));
    assert!(!state.exists());

    catalog.begin_activation(&site, [1; 16]).unwrap();

    assert_eq!(record_version(&base, &local, &name), 3);
    assert!(!state.exists());
}

fn assert_v3_stopped_checkpoint_evidence(leaf: &str, text: &str) {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, text, 1);
    stop(&site, &mut owner, 9);
    rewrite_record_version(&base, &local, &name, 3);
    let runtime = site.runtime_directory().to_owned();
    let state = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    drop(site);
    remove_runtime(&base, &runtime);
    fs::rename(state.join("checkpoint.json"), state.join(leaf)).unwrap();
    let evidence = fs::read(state.join(leaf)).unwrap();

    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry.name == name)
        .expect("retained named session");
    assert_eq!(
        entry.absent_runtime_state().unwrap(),
        Some(ActivationState::Stopped {
            request_id: [9; 16]
        })
    );

    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert_eq!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .unwrap(),
        Some(name.clone())
    );
    let site = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [2; 16]).unwrap();
    assert_eq!(record_version(&base, &local, &name), 3);
    assert_eq!(fs::read(state.join(leaf)).unwrap(), evidence);
}

#[test]
fn v3_stopped_recovery_accepts_backup_only_checkpoint_evidence() {
    assert_v3_stopped_checkpoint_evidence("checkpoint.previous-1.json", "backup-evidence");
}

#[test]
fn v3_stopped_recovery_accepts_raw_only_checkpoint_evidence() {
    assert_v3_stopped_checkpoint_evidence(
        "checkpoint.corrupt-00000000000000000000000000000001.raw",
        "raw-evidence",
    );
}

fn assert_v2_stopped_recovery_requires_primary_checkpoint(leaf: &str, text: &str) {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, text, 1);
    stop(&site, &mut owner, 9);
    rewrite_record_version(&base, &local, &name, 2);
    let record_before = fs::read(record_path(&base, &local, &name)).unwrap();
    let runtime = site.runtime_directory().to_owned();
    let state = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    fs::write(state.join("checkpoint.json"), b"plain-v4").unwrap();
    drop(site);
    remove_runtime(&base, &runtime);
    fs::rename(state.join("checkpoint.json"), state.join(leaf)).unwrap();

    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    assert_eq!(
        fs::read(record_path(&base, &local, &name)).unwrap(),
        record_before
    );
    let entry = entries
        .iter()
        .find(|entry| entry.name == name)
        .expect("retained named session");
    assert_eq!(
        entry.absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_err()
    );
    assert_eq!(
        fs::read(record_path(&base, &local, &name)).unwrap(),
        record_before
    );

    fs::rename(state.join(leaf), state.join("checkpoint.json")).unwrap();
    assert_eq!(
        entry.absent_runtime_state().unwrap(),
        Some(ActivationState::Stopped {
            request_id: [9; 16]
        })
    );
    assert_eq!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .unwrap(),
        Some(name.clone())
    );
    assert_eq!(record_version(&base, &local, &name), 2);
    let site = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [2; 16]).unwrap();
    assert_eq!(record_version(&base, &local, &name), 3);
}

#[test]
fn v2_stopped_recovery_requires_primary_checkpoint() {
    assert_v2_stopped_recovery_requires_primary_checkpoint(
        "checkpoint.previous-1.json",
        "v2-backup-is-not-primary",
    );
    assert_v2_stopped_recovery_requires_primary_checkpoint(
        "checkpoint.corrupt-00000000000000000000000000000001.raw",
        "v2-raw-is-not-primary",
    );
}

#[test]
fn v1_stopped_recovery_requires_primary_checkpoint() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, "legacy-stopped", 1);
    stop(&site, &mut owner, 9);
    let runtime = site.runtime_directory().to_owned();
    let state = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    let directory = Directory::open(&local.0, &base.0.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    directory
        .publish(
            &scope(&name),
            &Record::new(
                &base.0.canonicalize().unwrap(),
                &name,
                Some([1; 16]),
                Lifecycle::Stopped {
                    request_id: [9; 16],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let record_before = fs::read(record_path(&base, &local, &name)).unwrap();
    fs::write(state.join("checkpoint.json"), b"plain-v4").unwrap();
    drop(directory);
    drop(site);
    remove_runtime(&base, &runtime);
    fs::rename(
        state.join("checkpoint.json"),
        state.join("checkpoint.previous-1.json"),
    )
    .unwrap();

    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    assert_eq!(
        fs::read(record_path(&base, &local, &name)).unwrap(),
        record_before
    );
    let entry = entries
        .iter()
        .find(|entry| entry.name == name)
        .expect("retained named session");
    assert_eq!(
        entry.absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_err()
    );
    assert_eq!(
        fs::read(record_path(&base, &local, &name)).unwrap(),
        record_before
    );

    fs::rename(
        state.join("checkpoint.previous-1.json"),
        state.join("checkpoint.json"),
    )
    .unwrap();
    assert_eq!(
        entry.absent_runtime_state().unwrap(),
        Some(ActivationState::Stopped {
            request_id: [9; 16]
        })
    );
    assert_eq!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .unwrap(),
        Some(name.clone())
    );
    assert_eq!(record_version(&base, &local, &name), 1);
    let site = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = site.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = site.try_acquire_site_lifetime_lock().unwrap() else {
        panic!("lifetime")
    };
    site.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&site, [2; 16]).unwrap();
    assert_eq!(record_version(&base, &local, &name), 3);
}

#[test]
fn stopped_runtime_loss_allows_delete_and_preserves_the_other_base() {
    let first = Fixture::new();
    let second = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&first, &local, "work", 1);
    stop(&site, &mut owner, 4);
    let runtime = site.runtime_directory().to_owned();
    drop(site);
    remove_runtime(&first, &runtime);
    let (_, peer, mut peer_owner) = start(&second, &local, "work", 1);
    stop(&peer, &mut peer_owner, 5);
    let peer_bytes = fs::read(record_path(&second, &local, &name)).unwrap();
    let catalog = Catalog::try_lock(&first.0, &local.0).unwrap();
    assert_eq!(
        catalog.delete(&name, &local.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!record_path(&first, &local, &name).exists());
    assert_eq!(
        fs::read(record_path(&second, &local, &name)).unwrap(),
        peer_bytes
    );
    assert_eq!(
        catalog.delete(&name, &local.0, deadline(), || Ok(false)),
        Ok(Outcome::Absent)
    );
    assert_eq!(
        Catalog::list(&first.0, &local.0, deadline()).unwrap().len(),
        1
    );
}

#[test]
fn old_stop_owner_cannot_settle_a_new_generation_or_reuse_its_nonce() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut old) = start(&base, &local, "generation", 1);
    stop(&site, &mut old, 4);
    assert!(old.complete([4; 16]).is_ok());
    assert!(old.complete([5; 16]).is_err());
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert!(catalog.begin_activation(&site, [1; 16]).is_err());
    assert!(catalog.begin_activation(&site, [0; 16]).is_err());
    drop(catalog);
    drop(site);
    let (_, site, mut current) = start(&base, &local, "generation", 2);
    let path = record_path(&base, &local, &name);
    let active = fs::read(&path).unwrap();
    assert!(old.complete([4; 16]).is_err());
    assert_eq!(fs::read(&path).unwrap(), active);
    stop(&site, &mut current, 6);
}

#[test]
fn failed_stop_publication_preserves_active_bytes_and_can_be_retried() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, "publication", 1);
    let path = record_path(&base, &local, &name);
    let active = fs::read(&path).unwrap();
    let held = permissions::open_file(&path, false, false).unwrap();
    assert!(owner.complete([2; 16]).is_err());
    assert_eq!(fs::read(&path).unwrap(), active);
    assert!(!path.with_extension("pending").exists());
    assert_eq!(site.activation_state().unwrap(), ActivationState::Active);
    drop(held);
    stop(&site, &mut owner, 2);
}

#[test]
fn legacy_active_state_refuses_lost_runtime_and_conflicting_runtime_receipt() {
    for remove in [false, true] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (name, site, _owner) = start(&base, &local, "uncertain", 1);
        let directory = Directory::open(&local.0, &base.0.canonicalize().unwrap())
            .unwrap()
            .unwrap();
        directory
            .publish(
                &scope(&name),
                &Record::new(
                    &base.0.canonicalize().unwrap(),
                    &name,
                    Some([1; 16]),
                    Lifecycle::Active,
                )
                .unwrap(),
            )
            .unwrap();
        // An ephemeral stopped marker alone cannot settle the durable Active generation.
        site.retain_active_record()
            .unwrap()
            .complete([7; 16])
            .unwrap();
        let runtime = site.runtime_directory().to_owned();
        drop(site);
        if remove {
            remove_runtime(&base, &runtime);
        }
        let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
        assert!(!entries[1].accepts_runtime_state(ActivationState::Stopped {
            request_id: [7; 16]
        }));
        if remove {
            assert_eq!(
                entries[1].absent_runtime_state().unwrap(),
                Some(ActivationState::Active)
            );
        }
        let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
        let before = fs::read(record_path(&base, &local, &name)).unwrap();
        let failure = catalog
            .delete(&name, &local.0, deadline(), || Ok(false))
            .unwrap_err();
        assert_eq!(failure.reason, Reason::RecoveryRequired);
        assert!(!failure.pending);
        assert_eq!(fs::read(record_path(&base, &local, &name)).unwrap(), before);
        if remove {
            assert!(
                catalog
                    .prepare_activation(&scope(&name), Some(&name))
                    .is_err()
            );
        }
    }
}

#[test]
fn deletion_survives_loss_of_temporary_intent_and_runtime() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, "delete", 1);
    stop(&site, &mut owner, 3);
    let runtime = site.runtime_directory().to_owned();
    drop(site);
    let path = record_path(&base, &local, &name);
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    let failure = catalog
        .delete(&name, &local.0, deadline(), || {
            let value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?).unwrap();
            Ok(value["lifecycle"]["state"] == "deleting")
        })
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    let ephemeral = base.0.join(format!("{}.deleting", scope(&name)));
    assert!(
        !ephemeral.exists(),
        "interrupted between durable and ephemeral intent"
    );
    remove_runtime(&base, &runtime);
    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    assert!(entries[1].deleting);
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_err()
    );
    assert_eq!(
        catalog.delete(&name, &local.0, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!path.exists());
    assert!(!ephemeral.exists());
}

#[test]
fn checkpoint_locks_endpoints_and_foreign_storage_roots_cannot_bypass_admission() {
    let base = Fixture::new();
    let local = Fixture::new();
    let foreign = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, "admission", 1);
    stop(&site, &mut owner, 3);
    let runtime = site.runtime_directory().to_owned();
    let state_path = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    drop(site);
    remove_runtime(&base, &runtime);
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    let held = permissions::open_file(&state_path.join("state.lock"), false, true).unwrap();
    held.try_lock().unwrap();
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_err()
    );
    let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
    assert_eq!(
        entries[1].absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    assert_eq!(
        catalog
            .delete(&name, &local.0, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(held);
    let listener =
        orchbee_local_ipc::LocalListener::bind(&Rendezvous::socket_paths(&runtime)[0]).unwrap();
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_err()
    );
    assert_eq!(
        entries[1].absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    drop(listener);
    let before = fs::read(record_path(&base, &local, &name)).unwrap();
    assert!(
        catalog
            .delete(&name, &foreign.0, deadline(), || Ok(false))
            .is_err()
    );
    assert_eq!(fs::read(record_path(&base, &local, &name)).unwrap(), before);
    assert!(state_path.join("checkpoint.json").exists());
    assert_eq!(fs::read_dir(&foreign.0).unwrap().count(), 0);
    assert!(
        catalog
            .prepare_activation(&scope(&name), Some(&name))
            .is_ok()
    );
}

#[test]
fn malformed_oversized_mismatched_and_linked_records_are_never_repaired() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, site, mut owner) = start(&base, &local, "strict", 1);
    stop(&site, &mut owner, 3);
    let path = record_path(&base, &local, &name);
    let original = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mut cases = vec![vec![b' '; 8193], br#"{"version":1,"version":1}"#.to_vec()];
    for (field, replacement) in [
        ("version", serde_json::json!(4)),
        ("name", serde_json::json!("other")),
        ("runtime", serde_json::json!(local.0.to_str().unwrap())),
        ("activation_id", serde_json::json!(vec![0; 16])),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut corrupt = value.clone();
        corrupt[field] = replacement;
        cases.push(serde_json::to_vec(&corrupt).unwrap());
    }
    let directory = Directory::open(&local.0, &base.0.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    let valid = Record::new(
        &base.0.canonicalize().unwrap(),
        &name,
        Some([2; 16]),
        Lifecycle::Active,
    )
    .unwrap();
    for bytes in cases {
        fs::write(&path, &bytes).unwrap();
        assert!(Catalog::list(&base.0, &local.0, deadline()).is_err());
        assert!(directory.publish(&scope(&name), &valid).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, &original).unwrap();
    let alias = local.0.join("foreign-link");
    fs::hard_link(&path, &alias).unwrap();
    assert!(directory.read(&scope(&name)).is_err());
    assert!(directory.publish(&scope(&name), &valid).is_err());
    assert_eq!(fs::read(&alias).unwrap(), original);
    fs::remove_file(alias).unwrap();
    assert!(directory.read(&scope(&name)).is_ok());
}
