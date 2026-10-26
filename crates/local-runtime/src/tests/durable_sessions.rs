use super::*;
use crate::{
    ActivationState, NamedSessionDeleteOutcome as Outcome, NamedSessionDeleteReason as Reason,
    RetainedSessionRecord as Retained, SessionCatalog as Catalog, SessionName, SessionStore,
    durable_sessions::{Directory, Lifecycle, Record},
};
use std::{io, path::Path};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}
fn scope(name: &SessionName) -> String {
    name.scope_for_user(&orchbee_local_ipc::current_user_sid().unwrap())
}
fn record_path(base: &Fixture, local: &Fixture, name: &SessionName) -> PathBuf {
    local
        .0
        .join("Orchbee/session-catalog")
        .join(Rendezvous::host_endpoint_id(
            &base.0.canonicalize().unwrap(),
        ))
        .join(format!("{}.json", scope(name)))
}
fn record_version(base: &Fixture, local: &Fixture, name: &SessionName) -> u64 {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(record_path(base, local, name)).unwrap()).unwrap();
    value["version"].as_u64().unwrap()
}
fn rewrite_record_version(base: &Fixture, local: &Fixture, name: &SessionName, version: u8) {
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
) -> (SessionName, Rendezvous, Retained) {
    let name = SessionName::parse(text).unwrap();
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert_eq!(
        catalog
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .unwrap(),
        Some(name.clone())
    );
    let session = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime")
    };
    session.register_session_name(&activation, &name).unwrap();
    catalog
        .begin_activation(&session, [generation; 16])
        .unwrap();
    session.commit_activation().unwrap();
    let mut store = SessionStore::open(&local.0, &session, 64).unwrap();
    if store.read().unwrap().is_none() {
        store.publish(text.as_bytes()).unwrap();
    }
    let owner = Retained::retain(&local.0, &session).unwrap().unwrap();
    (name, session, owner)
}
fn stop(session: &Rendezvous, owner: &mut Retained, id: u8) {
    owner.complete([id; 16]).unwrap();
    session
        .retain_active_record()
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
        "session-lifetime.lock",
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
fn obsolete_named_identity_cannot_become_an_absent_new_session() {
    use std::io::Write;
    for retained_record in [false, true] {
        let base = Fixture::new();
        let local = Fixture::new();
        let name = SessionName::parse("work").unwrap();
        let sid = orchbee_local_ipc::current_user_sid().unwrap();
        let mut hash = Sha256::new();
        hash.update(b"orchbee.windows.site-name.v1\0");
        hash.update(sid.as_bytes());
        hash.update(b"\0work");
        let old_scope = format!("orchbee-session-{:x}", hash.finalize())[..63].to_owned();
        assert_ne!(old_scope, scope(&name));
        let old_runtime = base.0.canonicalize().unwrap().join(&old_scope);

        let (evidence, bytes) = if retained_record {
            let (_, session, owner) = start(&base, &local, name.as_str(), 1);
            let runtime = session.runtime_directory().to_path_buf();
            drop(owner);
            drop(session);
            remove_runtime(&base, &runtime);
            let current = record_path(&base, &local, &name);
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&current).unwrap()).unwrap();
            value["runtime"] = serde_json::json!(old_runtime.to_str().unwrap());
            let bytes = serde_json::to_vec(&value).unwrap();
            fs::write(&current, &bytes).unwrap();
            let old = current.with_file_name(format!("{old_scope}.json"));
            fs::rename(&current, &old).unwrap();
            (old, bytes)
        } else {
            drop(base.open(&old_scope));
            let path = old_runtime.join("session-name.json");
            let bytes = br#"{"version":1,"name":"work"}"#.to_vec();
            let mut file = permissions::open_file(&path, true, true).unwrap();
            file.write_all(&bytes).unwrap();
            file.sync_all().unwrap();
            (path, bytes)
        };
        let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
        assert_eq!(
            catalog
                .prepare_activation(&scope(&name), Some(&name), deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(!base.0.join(scope(&name)).exists());
        assert_eq!(fs::read(evidence).unwrap(), bytes);
    }
}

#[test]
fn restore_notice_reads_while_server_owns_store_and_only_projects_the_active_generation() {
    use crate::CheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    for name in ["default", "notice-test"] {
        let (_, session, owner) = start(&base, &local, name, 1);
        let store = SessionStore::open(&local.0, &session, 64).unwrap();
        assert_eq!(
            SessionStore::open(&local.0, &session, 64)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        for notice in [Some(Notice::Empty), Some(Notice::Recovered), None] {
            store.publish_restore_notice(&owner, notice).unwrap();
            assert_eq!(session.checkpoint_restore_notice(&local.0).unwrap(), notice);
        }
        let path = store.directory.join(crate::checkpoint_restore_notice::FILE);
        let stale = b"orchbee-checkpoint-restore-v1:02020202020202020202020202020202:E";
        fs::write(&path, stale).unwrap();
        assert_eq!(session.checkpoint_restore_notice(&local.0).unwrap(), None);
        assert_eq!(fs::read(&path).unwrap(), stale);
        let invalid = [b'X'; 128];
        fs::write(&path, invalid).unwrap();
        assert_eq!(
            session
                .checkpoint_restore_notice(&local.0)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&path).unwrap(), invalid);
        store
            .publish_restore_notice(&owner, Some(Notice::Recovered))
            .unwrap();
        assert_eq!(
            session.checkpoint_restore_notice(&local.0).unwrap(),
            Some(Notice::Recovered)
        );
    }
}

#[test]
fn blocked_notice_replacement_preserves_current_feedback_and_reclaims_pending() {
    use crate::CheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    let (_, session, owner) = start(&base, &local, "notice-blocked", 1);
    let store = SessionStore::open(&local.0, &session, 64).unwrap();
    store
        .publish_restore_notice(&owner, Some(Notice::Empty))
        .unwrap();
    let path = store.directory.join(crate::checkpoint_restore_notice::FILE);
    let before = fs::read(&path).unwrap();
    let consumer = permissions::open_snapshot_file(&path).unwrap();
    assert!(store.publish_restore_notice(&owner, None).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(
        !store
            .directory
            .join(crate::checkpoint_restore_notice::PENDING)
            .exists()
    );
    drop(consumer);
    store.publish_restore_notice(&owner, None).unwrap();
    assert_eq!(session.checkpoint_restore_notice(&local.0).unwrap(), None);
}

#[test]
fn default_and_exact_names_remain_discoverable_and_restart_after_runtime_loss() {
    let base = Fixture::new();
    let local = Fixture::new();
    for text in ["default", "work", "Work"] {
        let (_, session, mut owner) = start(&base, &local, text, 1);
        stop(&session, &mut owner, 9);
        let runtime = session.runtime_directory().to_owned();
        drop(session);
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
        let (_, session, mut owner) = start(&base, &local, text, 2);
        let store = SessionStore::open(&local.0, &session, 64).unwrap();
        assert_eq!(store.read().unwrap().unwrap(), text.as_bytes());
        drop(store);
        stop(&session, &mut owner, 8);
    }
}

#[test]
fn begin_activation_publishes_current_record_before_checkpoint_storage_can_write() {
    for text in ["default", "catalog-first"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let name = SessionName::parse(text).unwrap();
        let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
        assert_eq!(
            catalog
                .prepare_activation(
                    &scope(&name),
                    Some(&name),
                    std::time::Instant::now() + std::time::Duration::from_secs(5)
                )
                .unwrap(),
            Some(name.clone())
        );
        let session = base.open(&scope(&name));
        let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap()
        else {
            panic!("activation")
        };
        let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
        else {
            panic!("lifetime")
        };
        session.register_session_name(&activation, &name).unwrap();
        let state = local
            .0
            .join("Orchbee/native-sessions")
            .join(Rendezvous::host_endpoint_id(session.runtime_directory()));
        assert!(!state.exists());

        catalog.begin_activation(&session, [1; 16]).unwrap();

        assert_eq!(record_version(&base, &local, &name), 1);
        assert!(!state.exists());
    }
}

#[test]
fn unsupported_catalog_refuses_activation_and_recovery_without_changing_retained_bytes() {
    for text in ["default", "future-catalog"] {
        for stopped in [false, true] {
            let base = Fixture::new();
            let local = Fixture::new();
            let (name, session, mut owner) = start(&base, &local, text, 1);
            if stopped {
                stop(&session, &mut owner, 9);
            }
            rewrite_record_version(&base, &local, &name, 6);
            let record = record_path(&base, &local, &name);
            let marker = session.runtime_directory().join("activation.started");
            let checkpoint = local
                .0
                .join("Orchbee/native-sessions")
                .join(Rendezvous::host_endpoint_id(session.runtime_directory()))
                .join("checkpoint.json");
            let retained = [&record, &marker, &checkpoint].map(|path| fs::read(path).unwrap());
            let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
            assert!(
                catalog
                    .prepare_activation(
                        &scope(&name),
                        Some(&name),
                        std::time::Instant::now() + std::time::Duration::from_secs(5)
                    )
                    .is_err()
            );
            assert!(catalog.begin_activation(&session, [2; 16]).is_err());
            let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap()
            else {
                panic!("activation")
            };
            let LockAttempt::Acquired(lifetime) =
                session.try_acquire_session_lifetime_lock().unwrap()
            else {
                panic!("lifetime")
            };
            assert!(
                catalog
                    .recover_inactive(&session, &activation, &lifetime)
                    .is_err()
            );
            for (path, bytes) in [&record, &marker, &checkpoint].into_iter().zip(retained) {
                assert_eq!(fs::read(path).unwrap(), bytes);
            }
        }
    }
}

fn assert_framed_stopped_checkpoint_evidence(leaf: &str, text: &str) {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, text, 1);
    stop(&session, &mut owner, 9);
    let runtime = session.runtime_directory().to_owned();
    let state = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    drop(session);
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
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .unwrap(),
        Some(name.clone())
    );
    let session = base.open(&scope(&name));
    let LockAttempt::Acquired(activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime")
    };
    session.register_session_name(&activation, &name).unwrap();
    catalog.begin_activation(&session, [2; 16]).unwrap();
    assert_eq!(record_version(&base, &local, &name), 1);
    assert_eq!(fs::read(state.join(leaf)).unwrap(), evidence);
}

#[test]
fn framed_stopped_recovery_accepts_backup_only_checkpoint_evidence() {
    assert_framed_stopped_checkpoint_evidence("checkpoint.previous-1.json", "backup-evidence");
}

#[test]
fn framed_stopped_recovery_accepts_raw_only_checkpoint_evidence() {
    assert_framed_stopped_checkpoint_evidence(
        "checkpoint.corrupt-00000000000000000000000000000001.raw",
        "raw-evidence",
    );
}

#[test]
fn stopped_runtime_loss_allows_delete_and_preserves_the_other_base() {
    let first = Fixture::new();
    let second = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&first, &local, "work", 1);
    stop(&session, &mut owner, 4);
    let runtime = session.runtime_directory().to_owned();
    drop(session);
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
    let (name, session, mut old) = start(&base, &local, "generation", 1);
    stop(&session, &mut old, 4);
    assert!(old.complete([4; 16]).is_ok());
    assert!(old.complete([5; 16]).is_err());
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    assert!(catalog.begin_activation(&session, [1; 16]).is_err());
    assert!(catalog.begin_activation(&session, [0; 16]).is_err());
    drop(catalog);
    drop(session);
    let (_, session, mut current) = start(&base, &local, "generation", 2);
    let path = record_path(&base, &local, &name);
    let active = fs::read(&path).unwrap();
    assert!(old.complete([4; 16]).is_err());
    assert_eq!(fs::read(&path).unwrap(), active);
    stop(&session, &mut current, 6);
}

#[test]
fn failed_stop_publication_preserves_active_bytes_and_can_be_retried() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "publication", 1);
    let path = record_path(&base, &local, &name);
    let active = fs::read(&path).unwrap();
    let held = permissions::open_file(&path, false, false).unwrap();
    assert!(owner.complete([2; 16]).is_err());
    assert_eq!(fs::read(&path).unwrap(), active);
    assert!(!path.with_extension("pending").exists());
    assert_eq!(session.activation_state().unwrap(), ActivationState::Active);
    drop(held);
    stop(&session, &mut owner, 2);
}

#[test]
fn deletion_survives_loss_of_temporary_intent_and_runtime() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "delete", 1);
    stop(&session, &mut owner, 3);
    let runtime = session.runtime_directory().to_owned();
    drop(session);
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
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
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
    let (name, session, mut owner) = start(&base, &local, "admission", 1);
    stop(&session, &mut owner, 3);
    let runtime = session.runtime_directory().to_owned();
    let state_path = local
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(&runtime));
    drop(session);
    remove_runtime(&base, &runtime);
    let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
    let held = permissions::open_file(&state_path.join("state.lock"), false, true).unwrap();
    held.try_lock().unwrap();
    assert!(
        catalog
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
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
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
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
            .prepare_activation(
                &scope(&name),
                Some(&name),
                std::time::Instant::now() + std::time::Duration::from_secs(5)
            )
            .is_ok()
    );
}

#[test]
fn malformed_oversized_mismatched_and_linked_records_are_never_repaired() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "strict", 1);
    stop(&session, &mut owner, 3);
    let path = record_path(&base, &local, &name);
    let original = fs::read(&path).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let mut cases = vec![vec![b' '; 8193], br#"{"version":1,"version":1}"#.to_vec()];
    for (field, replacement) in [
        ("version", serde_json::json!(6)),
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
        if let Ok(entries) = Catalog::list(&base.0, &local.0, deadline()) {
            assert!(
                entries
                    .iter()
                    .find(|entry| entry.name == name)
                    .unwrap()
                    .unreadable
            );
        }
        assert!(directory.read(&scope(&name)).is_err());
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

#[test]
fn incompatible_records_are_listed_unreadable_and_only_explicit_guarded_deletion_removes_them() {
    for missing_processes in [false, true] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (name, session, mut owner) = start(&base, &local, "unreadable", 1);
        let path = record_path(&base, &local, &name);
        stop(&session, &mut owner, 3);
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if missing_processes {
            record.as_object_mut().unwrap().remove("processes");
        } else {
            record["version"] = serde_json::json!(5);
        }
        let bytes = serde_json::to_vec(&record).unwrap();
        fs::write(&path, &bytes).unwrap();
        let entries = Catalog::list(&base.0, &local.0, deadline()).unwrap();
        let entry = entries.iter().find(|entry| entry.name == name).unwrap();
        assert!(entry.unreadable);
        assert!(!entry.cold_recoverable().unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let catalog = Catalog::try_lock(&base.0, &local.0).unwrap();
        assert!(
            catalog
                .prepare_activation(
                    &scope(&name),
                    Some(&name),
                    std::time::Instant::now() + std::time::Duration::from_secs(5)
                )
                .is_err()
        );
        let LockAttempt::Acquired(lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
        else {
            panic!("lifetime")
        };
        assert_eq!(
            catalog
                .delete(&name, &local.0, deadline(), || Ok(false))
                .unwrap_err()
                .reason,
            Reason::InUse
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(lifetime);
        drop(owner);
        drop(session);
        assert_eq!(
            catalog
                .delete(&name, &local.0, deadline(), || Ok(false))
                .unwrap(),
            Outcome::Deleted
        );
        assert!(!path.exists());
    }
}
