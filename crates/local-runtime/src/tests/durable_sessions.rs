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
fn state_path(local: &Fixture, name: &SessionName) -> PathBuf {
    local.0.join("sessions").join(name.as_str())
}
fn record_path(local: &Fixture, name: &SessionName) -> PathBuf {
    state_path(local, name).join("session.json")
}
fn record_version(local: &Fixture, name: &SessionName) -> u64 {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(record_path(local, name)).unwrap()).unwrap();
    value["version"].as_u64().unwrap()
}
fn rewrite_record_version(local: &Fixture, name: &SessionName, version: u8) {
    let path = record_path(local, name);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["version"] = serde_json::json!(version);
    write_private(path, serde_json::to_vec(&value).unwrap()).unwrap();
}
/// Starts `text` the way activation does: spelled as its existing folder is.
fn start(
    base: &Fixture,
    local: &Fixture,
    text: &str,
    generation: u8,
) -> (SessionName, Rendezvous, Retained) {
    let root = local.root();
    let name = Catalog::spelling(&root, &SessionName::parse(text).unwrap()).unwrap();
    let catalog = Catalog::try_lock(&base.0, &root).unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = base.open(name.as_str());
    let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime")
    };
    catalog
        .begin_activation(&session, [generation; 16])
        .unwrap();
    session.commit_activation().unwrap();
    let mut store = SessionStore::open(&root, &session, 64).unwrap();
    if store.read().unwrap().is_none() {
        store.publish(text.as_bytes()).unwrap();
    }
    let owner = Retained::retain(&root, &session).unwrap();
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
fn unreadable_record_refuses_only_its_own_name() {
    use crate::{UnrecognizedRecord, UnrecognizedRecordKind as Kind, UnrecognizedRecordReason};
    let base = Fixture::new();
    let local = Fixture::new();
    let other = SessionName::parse("other").unwrap();
    let (name, session, mut owner) = start(&base, &local, "work", 1);
    stop(&session, &mut owner, 9);
    let runtime = session.runtime_directory().to_path_buf();
    drop(owner);
    drop(session);
    remove_runtime(&base, &runtime);
    let path = record_path(&local, &name);
    let bytes = br#"{"version":1,"name":"work""#.to_vec();
    write_private(&path, &bytes).unwrap();
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    assert_eq!(
        catalog.prepare_activation(&name).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!runtime.exists());
    catalog.prepare_activation(&other).unwrap();
    drop(catalog);
    let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.unreadable))
            .collect::<Vec<_>>(),
        [("default", false), ("work", true)]
    );
    // Diagnosis finds the record by any letter case of its name, and only for that name.
    assert_eq!(
        Catalog::unrecognized_records(&local.root(), "WORK", deadline()).unwrap(),
        [UnrecognizedRecord {
            kind: Kind::CatalogRecord,
            reason: UnrecognizedRecordReason::Unreadable,
            path: path.clone(),
        }]
    );
    assert!(
        Catalog::unrecognized_records(&local.root(), other.as_str(), deadline())
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    // A folder in `sessions/` whose name is no Session name fails the inventory explicitly.
    fs::create_dir(local.0.join("sessions").join("not a name")).unwrap();
    assert!(Catalog::list(&base.0, &local.root(), deadline()).is_err());
}

#[test]
fn restore_notice_reads_while_server_owns_store_and_only_projects_the_active_generation() {
    use crate::CheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    for name in ["default", "notice-test"] {
        let (_, session, owner) = start(&base, &local, name, 1);
        let store = SessionStore::open(&local.root(), &session, 64).unwrap();
        assert_eq!(
            SessionStore::open(&local.root(), &session, 64)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        for notice in [Some(Notice::Empty), Some(Notice::Recovered), None] {
            store.publish_restore_notice(&owner, notice).unwrap();
            assert_eq!(
                session.checkpoint_restore_notice(&local.root()).unwrap(),
                notice
            );
        }
        let path = store.directory.join(crate::checkpoint_restore_notice::FILE);
        let stale = b"orchbee-checkpoint-restore-v1:02020202020202020202020202020202:E";
        write_private(&path, stale).unwrap();
        assert_eq!(
            session.checkpoint_restore_notice(&local.root()).unwrap(),
            None
        );
        assert_eq!(fs::read(&path).unwrap(), stale);
        let invalid = [b'X'; 128];
        write_private(&path, invalid).unwrap();
        assert_eq!(
            session
                .checkpoint_restore_notice(&local.root())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&path).unwrap(), invalid);
        store
            .publish_restore_notice(&owner, Some(Notice::Recovered))
            .unwrap();
        assert_eq!(
            session.checkpoint_restore_notice(&local.root()).unwrap(),
            Some(Notice::Recovered)
        );
    }
}

// A Windows reader without delete sharing blocks the replacement; a POSIX reader does not.
#[cfg(windows)]
#[test]
fn blocked_notice_replacement_preserves_current_feedback_and_reclaims_pending() {
    use crate::CheckpointRestoreNotice as Notice;
    let base = Fixture::new();
    let local = Fixture::new();
    let (_, session, owner) = start(&base, &local, "notice-blocked", 1);
    let store = SessionStore::open(&local.root(), &session, 64).unwrap();
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
    assert_eq!(
        session.checkpoint_restore_notice(&local.root()).unwrap(),
        None
    );
}

#[test]
fn sessions_remain_discoverable_and_restart_by_any_letter_case_after_runtime_loss() {
    let base = Fixture::new();
    let local = Fixture::new();
    for text in ["default", "Work"] {
        let (_, session, mut owner) = start(&base, &local, text, 1);
        stop(&session, &mut owner, 9);
        let runtime = session.runtime_directory().to_owned();
        drop(session);
        remove_runtime(&base, &runtime);
    }
    let before = fs::read_dir(&base.0).unwrap().count();
    let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["default", "Work"]
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
    // `work` is the Session created as `Work`, which keeps its creation spelling.
    for (text, spelling, saved) in [("DEFAULT", "default", "default"), ("work", "Work", "Work")] {
        let (name, session, mut owner) = start(&base, &local, text, 2);
        assert_eq!(name.as_str(), spelling);
        let store = SessionStore::open(&local.root(), &session, 64).unwrap();
        assert_eq!(store.read().unwrap().unwrap(), saved.as_bytes());
        drop(store);
        stop(&session, &mut owner, 8);
    }
    let mut folders = fs::read_dir(local.0.join("sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    folders.sort();
    assert_eq!(folders, ["Work", "default"]);
}

#[test]
fn begin_activation_publishes_current_record_before_checkpoint_storage_can_write() {
    for text in ["default", "catalog-first"] {
        let base = Fixture::new();
        let local = Fixture::new();
        let name = SessionName::parse(text).unwrap();
        let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
        catalog.prepare_activation(&name).unwrap();
        let session = base.open(name.as_str());
        let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap()
        else {
            panic!("activation")
        };
        let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
        else {
            panic!("lifetime")
        };
        let state = state_path(&local, &name);
        assert!(!state.exists());

        catalog.begin_activation(&session, [1; 16]).unwrap();

        assert_eq!(record_version(&local, &name), 1);
        assert!(!state.join("state.lock").exists());
        assert!(!state.join("checkpoint.json").exists());
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
            rewrite_record_version(&local, &name, 6);
            let record = record_path(&local, &name);
            let marker = session.runtime_directory().join("activation.started");
            let checkpoint = state_path(&local, &name).join("checkpoint.json");
            let retained = [&record, &marker, &checkpoint].map(|path| fs::read(path).unwrap());
            let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
            assert!(catalog.prepare_activation(&name).is_err());
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
    let state = state_path(&local, &name);
    drop(session);
    remove_runtime(&base, &runtime);
    fs::rename(state.join("checkpoint.json"), state.join(leaf)).unwrap();
    let evidence = fs::read(state.join(leaf)).unwrap();

    let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
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

    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    catalog.prepare_activation(&name).unwrap();
    let session = base.open(name.as_str());
    let LockAttempt::Acquired(_activation) = session.try_acquire_activation_lock().unwrap() else {
        panic!("activation")
    };
    let LockAttempt::Acquired(_lifetime) = session.try_acquire_session_lifetime_lock().unwrap()
    else {
        panic!("lifetime")
    };
    catalog.begin_activation(&session, [2; 16]).unwrap();
    assert_eq!(record_version(&local, &name), 1);
    assert_eq!(fs::read(state.join(leaf)).unwrap(), evidence);
}

#[test]
fn framed_stopped_recovery_accepts_backup_only_checkpoint_evidence() {
    assert_framed_stopped_checkpoint_evidence("checkpoint.previous-1.json", "backup-evidence");
}

#[test]
fn framed_stopped_recovery_accepts_raw_only_checkpoint_evidence() {
    assert_framed_stopped_checkpoint_evidence(
        "checkpoint.corrupt-2026-11-08T10-00-00Z.raw",
        "raw-evidence",
    );
}

#[test]
fn a_record_naming_another_runtime_folder_starts_here_only_once_stopped() {
    let first = Fixture::new();
    let second = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&first, &local, "work", 1);
    let catalog = Catalog::try_lock(&second.0, &local.root()).unwrap();
    assert_eq!(
        catalog.prepare_activation(&name).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!second.0.join("work").exists());
    drop(catalog);
    stop(&session, &mut owner, 4);
    let runtime = session.runtime_directory().to_owned();
    drop(owner);
    drop(session);
    remove_runtime(&first, &runtime);
    let (_, peer, mut peer_owner) = start(&second, &local, "work", 2);
    assert_eq!(
        SessionStore::open(&local.root(), &peer, 64)
            .unwrap()
            .read()
            .unwrap()
            .unwrap(),
        b"work"
    );
    stop(&peer, &mut peer_owner, 5);
    let listed = Catalog::list(&first.0, &local.root(), deadline()).unwrap();
    assert_eq!(listed[1].runtime, peer.runtime_directory());
    let catalog = Catalog::try_lock(&second.0, &local.root()).unwrap();
    drop(peer_owner);
    drop(peer);
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!state_path(&local, &name).exists());
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Absent)
    );
    assert_eq!(
        Catalog::list(&first.0, &local.root(), deadline())
            .unwrap()
            .len(),
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
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    assert!(catalog.begin_activation(&session, [1; 16]).is_err());
    assert!(catalog.begin_activation(&session, [0; 16]).is_err());
    drop(catalog);
    drop(session);
    let (_, session, mut current) = start(&base, &local, "generation", 2);
    let path = record_path(&local, &name);
    let active = fs::read(&path).unwrap();
    assert!(old.complete([4; 16]).is_err());
    assert_eq!(fs::read(&path).unwrap(), active);
    stop(&session, &mut current, 6);
}

// A Windows reader without delete sharing blocks the replacement; a POSIX reader does not.
#[cfg(windows)]
#[test]
fn failed_stop_publication_preserves_active_bytes_and_can_be_retried() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "publication", 1);
    let path = record_path(&local, &name);
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
fn deletion_survives_loss_of_runtime() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "delete", 1);
    stop(&session, &mut owner, 3);
    let runtime = session.runtime_directory().to_owned();
    drop(session);
    let path = record_path(&local, &name);
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    let failure = catalog
        .delete(&name, deadline(), || {
            let value: serde_json::Value = serde_json::from_slice(&fs::read(&path)?).unwrap();
            Ok(value["lifecycle"]["state"] == "deleting")
        })
        .unwrap_err();
    assert_eq!(failure.reason, Reason::Interrupted);
    assert!(failure.pending);
    remove_runtime(&base, &runtime);
    let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
    assert!(entries[1].deleting);
    assert!(catalog.prepare_activation(&name).is_err());
    assert_eq!(
        catalog.delete(&name, deadline(), || Ok(false)),
        Ok(Outcome::Deleted)
    );
    assert!(!path.exists());
    assert!(!state_path(&local, &name).exists());
}

#[test]
fn checkpoint_locks_and_endpoints_cannot_bypass_admission() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "admission", 1);
    stop(&session, &mut owner, 3);
    let runtime = session.runtime_directory().to_owned();
    let state_path = state_path(&local, &name);
    drop(session);
    remove_runtime(&base, &runtime);
    let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
    let held = permissions::open_file(&state_path.join("state.lock"), false, true).unwrap();
    held.try_lock().unwrap();
    assert!(catalog.prepare_activation(&name).is_err());
    let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
    assert_eq!(
        entries[1].absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    assert_eq!(
        catalog
            .delete(&name, deadline(), || Ok(false))
            .unwrap_err()
            .reason,
        Reason::InUse
    );
    drop(held);
    let listener =
        orchbee_local_ipc::LocalListener::bind(&Rendezvous::socket_paths(&runtime)[0]).unwrap();
    assert!(catalog.prepare_activation(&name).is_err());
    assert_eq!(
        entries[1].absent_runtime_state().unwrap(),
        Some(ActivationState::Active)
    );
    drop(listener);
    // A POSIX socket outlives its listener until its owner removes it; a Windows pipe does not.
    #[cfg(unix)]
    fs::remove_file(&Rendezvous::socket_paths(&runtime)[0]).unwrap();
    assert!(state_path.join("checkpoint.json").exists());
    assert!(catalog.prepare_activation(&name).is_ok());
}

#[test]
fn malformed_oversized_mismatched_and_linked_records_are_never_repaired() {
    let base = Fixture::new();
    let local = Fixture::new();
    let (name, session, mut owner) = start(&base, &local, "strict", 1);
    stop(&session, &mut owner, 3);
    let path = record_path(&local, &name);
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
    let directory = Directory::open(&local.root()).unwrap().unwrap();
    let valid = Record::new(
        &base.0.canonicalize().unwrap().join(name.as_str()),
        &name,
        Some([2; 16]),
        Lifecycle::Active,
    )
    .unwrap();
    for bytes in cases {
        write_private(&path, &bytes).unwrap();
        if let Ok(entries) = Catalog::list(&base.0, &local.root(), deadline()) {
            assert!(
                entries
                    .iter()
                    .find(|entry| entry.name == name)
                    .unwrap()
                    .unreadable
            );
        }
        assert!(directory.read(name.as_str()).is_err());
        assert!(directory.publish(name.as_str(), &valid).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    write_private(&path, &original).unwrap();
    let alias = local.0.join("foreign-link");
    fs::hard_link(&path, &alias).unwrap();
    assert!(directory.read(name.as_str()).is_err());
    assert!(directory.publish(name.as_str(), &valid).is_err());
    assert_eq!(fs::read(&alias).unwrap(), original);
    fs::remove_file(alias).unwrap();
    assert!(directory.read(name.as_str()).is_ok());
}

#[test]
fn incompatible_records_are_listed_unreadable_and_only_explicit_guarded_deletion_removes_them() {
    for missing_processes in [false, true] {
        let base = Fixture::new();
        let local = Fixture::new();
        let (name, session, mut owner) = start(&base, &local, "unreadable", 1);
        let path = record_path(&local, &name);
        stop(&session, &mut owner, 3);
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if missing_processes {
            record.as_object_mut().unwrap().remove("processes");
        } else {
            // A record a newer version saved is listed unreadable too, but no deletion fence
            // replaces it: it is never rewritten.
            record["version"] = serde_json::json!(5);
        }
        let bytes = serde_json::to_vec(&record).unwrap();
        write_private(&path, &bytes).unwrap();
        let entries = Catalog::list(&base.0, &local.root(), deadline()).unwrap();
        let entry = entries.iter().find(|entry| entry.name == name).unwrap();
        assert!(entry.unreadable);
        assert!(!entry.cold_recoverable().unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let catalog = Catalog::try_lock(&base.0, &local.root()).unwrap();
        assert!(catalog.prepare_activation(&name).is_err());
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
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(lifetime);
        drop(owner);
        drop(session);
        if missing_processes {
            assert_eq!(
                catalog.delete(&name, deadline(), || Ok(false)).unwrap(),
                Outcome::Deleted
            );
            assert!(!path.exists());
        } else {
            assert!(catalog.delete(&name, deadline(), || Ok(false)).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
}
