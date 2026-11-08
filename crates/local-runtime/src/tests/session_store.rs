use super::{Fixture, Rendezvous, fs, permissions};
use crate::SessionStore;
use sha2::{Digest, Sha256};
use std::io;

fn directory(fixture: &Fixture, session: &Rendezvous) -> std::path::PathBuf {
    fixture
        .0
        .join("native-sessions")
        .join(Rendezvous::host_endpoint_id(session.runtime_directory()))
}

#[test]
fn retains_three_latest_valid_payloads_and_reopens_the_latest_slot() {
    let fixture = Fixture::new();
    let session = fixture.open("three-checkpoints");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    for payload in [b"first".as_slice(), b"second", b"third", b"fourth"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &session);
    let mut payloads = [
        "checkpoint.json",
        "checkpoint.previous-1.json",
        "checkpoint.previous-2.json",
    ]
    .map(|name| {
        let bytes = fs::read(state.join(name)).unwrap();
        assert_eq!(bytes.get(..5), Some(b"OBCK\x01".as_slice()));
        bytes[45..].to_vec()
    });
    payloads.sort();
    assert_eq!(
        payloads,
        [b"fourth".to_vec(), b"second".to_vec(), b"third".to_vec()]
    );
    assert_eq!(store.read().unwrap(), Some(b"fourth".to_vec()));
    drop(store);
    let reopened = SessionStore::open(&fixture.0, &session, 64).unwrap();
    assert_eq!(reopened.read().unwrap(), Some(b"fourth".to_vec()));
}

#[test]
fn invalid_latest_digest_falls_back_without_modifying_the_invalid_original() {
    let fixture = Fixture::new();
    let session = fixture.open("invalid-latest");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    for payload in [b"first".as_slice(), b"second", b"third"] {
        store.publish(payload).unwrap();
    }
    let path = directory(&fixture, &session).join("checkpoint.previous-2.json");
    let mut invalid = fs::read(&path).unwrap();
    *invalid.last_mut().unwrap() ^= 1;
    fs::write(&path, &invalid).unwrap();
    assert_eq!(store.read().unwrap(), Some(b"second".to_vec()));
    assert_eq!(fs::read(&path).unwrap(), invalid);
    store.publish(b"fourth").unwrap();
    let originals: Vec<_> = fs::read_dir(directory(&fixture, &session))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .unwrap()
                .starts_with("checkpoint.corrupt-")
        })
        .map(|entry| fs::read(entry.path()).unwrap())
        .collect();
    assert_eq!(originals, vec![invalid]);
    assert_eq!(store.read().unwrap(), Some(b"fourth".to_vec()));
}

#[test]
fn preserves_committed_bytes_across_reopen_and_isolates_sessions() {
    let fixture = Fixture::new();
    let first_session = fixture.open("first");
    let second_session = fixture.open("second");
    let mut first = SessionStore::open(&fixture.0, &first_session, 64).unwrap();
    let mut second = SessionStore::open(&fixture.0, &second_session, 64).unwrap();
    assert_eq!(first.read().unwrap(), None);
    first.publish("first 中文".as_bytes()).unwrap();
    second.publish(b"second").unwrap();
    first.publish(b"replacement").unwrap();
    assert_eq!(first.read().unwrap().unwrap(), b"replacement");
    assert_eq!(second.read().unwrap().unwrap(), b"second");
    drop(first);
    let reopened = SessionStore::open(&fixture.0, &first_session, 64).unwrap();
    assert_eq!(reopened.read().unwrap().unwrap(), b"replacement");
    assert!(
        !directory(&fixture, &first_session)
            .join("checkpoint.pending")
            .exists()
    );
}

#[test]
fn another_owner_cannot_publish_until_the_store_lock_is_released() {
    let fixture = Fixture::new();
    let session = fixture.open("locked");
    let owner = SessionStore::open(&fixture.0, &session, 64).unwrap();
    let denied = SessionStore::open(&fixture.0, &session, 64).err().unwrap();
    assert_eq!(denied.kind(), io::ErrorKind::WouldBlock);
    drop(owner);
    assert!(SessionStore::open(&fixture.0, &session, 64).is_ok());
}

#[test]
fn missing_empty_exact_limit_and_oversized_files_remain_distinct() {
    let fixture = Fixture::new();
    let session = fixture.open("bounds");
    assert!(SessionStore::open(&fixture.0, &session, 0).is_err());
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    let path = directory(&fixture, &session).join("checkpoint.json");
    assert_eq!(store.read().unwrap(), None);
    assert_eq!(
        store.publish(&[1; 65]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert!(!path.exists());
    store.publish(b"").unwrap();
    assert_eq!(store.read().unwrap(), Some(Vec::new()));
    store.publish(&[2; 64]).unwrap();
    assert_eq!(store.read().unwrap(), Some(vec![2; 64]));
    assert_eq!(
        store.publish(&[3; 65]).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    let latest = directory(&fixture, &session).join("checkpoint.previous-1.json");
    assert_eq!(&fs::read(&latest).unwrap()[45..], &[2; 64]);
    let oversized = vec![4; 110];
    fs::write(&latest, &oversized).unwrap();
    let loaded = store.read_validated(|bytes| Some(bytes.to_vec())).unwrap();
    assert_eq!(loaded.checkpoint, Some(Vec::new()));
    assert_eq!(
        loaded.notice,
        Some(crate::CheckpointRestoreNotice::Recovered)
    );
    store.publish(b"valid after oversized data").unwrap();
    assert_eq!(fs::read(&latest).unwrap(), oversized);
    store.publish(b"another valid snapshot").unwrap();
    let originals: Vec<_> = fs::read_dir(directory(&fixture, &session))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .unwrap()
                .starts_with("checkpoint.corrupt-")
        })
        .map(|entry| fs::read(entry.path()).unwrap())
        .collect();
    assert_eq!(originals, vec![oversized]);
}

#[test]
fn failed_replacement_keeps_the_prior_checkpoint_and_reclaims_staging() {
    let fixture = Fixture::new();
    let session = fixture.open("blocked-publish");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    store.publish(b"old").unwrap();
    store.publish(b"middle").unwrap();
    store.publish(b"latest").unwrap();
    let path = directory(&fixture, &session).join("checkpoint.json");
    let before = [
        "checkpoint.json",
        "checkpoint.previous-1.json",
        "checkpoint.previous-2.json",
    ]
    .map(|name| fs::read(directory(&fixture, &session).join(name)).unwrap());
    // This real Windows handle denies delete sharing, so replacement cannot commit.
    let held = permissions::open_file(&path, false, false).unwrap();
    assert!(store.publish(b"new").is_err());
    let after = [
        "checkpoint.json",
        "checkpoint.previous-1.json",
        "checkpoint.previous-2.json",
    ]
    .map(|name| fs::read(directory(&fixture, &session).join(name)).unwrap());
    assert_eq!(after, before);
    assert_eq!(store.read().unwrap(), Some(b"latest".to_vec()));
    assert!(
        !directory(&fixture, &session)
            .join("checkpoint.pending")
            .exists()
    );
    drop(held);
    store.publish(b"new").unwrap();
    assert_eq!(&fs::read(path).unwrap()[45..], b"new");
    assert_eq!(store.read().unwrap(), Some(b"new".to_vec()));
}

#[test]
fn domain_invalid_copies_require_visible_empty_recovery_and_preserve_all_originals() {
    let fixture = Fixture::new();
    let session = fixture.open("invalid-domain");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    for payload in [b"invalid-1".as_slice(), b"invalid-2", b"invalid-3"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &session);
    let original = fs::read(state.join("checkpoint.json")).unwrap();
    let loaded = store
        .read_validated(|bytes| (bytes == b"valid-domain").then_some(42))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(loaded.notice, Some(crate::CheckpointRestoreNotice::Empty));
    assert!(
        store
            .publish_validated(b"invalid-new", |bytes| bytes == b"valid-domain")
            .is_err()
    );
    assert_eq!(fs::read(state.join("checkpoint.json")).unwrap(), original);
    store
        .publish_validated(b"valid-domain", |bytes| bytes == b"valid-domain")
        .unwrap();
    let originals: Vec<_> = fs::read_dir(&state)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .unwrap()
                .starts_with("checkpoint.corrupt-")
        })
        .map(|entry| fs::read(entry.path()).unwrap())
        .collect();
    assert_eq!(originals, vec![original]);
    let loaded = store
        .read_validated(|bytes| (bytes == b"valid-domain").then_some(42))
        .unwrap();
    assert_eq!(loaded.checkpoint, Some(42));
    assert_eq!(
        loaded.notice,
        Some(crate::CheckpointRestoreNotice::Recovered)
    );
}

#[test]
fn explicit_empty_final_after_all_invalid_preserves_the_displaced_original() {
    let fixture = Fixture::new();
    let session = fixture.open("empty-final-after-invalid");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    for payload in [b"invalid-1".as_slice(), b"invalid-2", b"invalid-3"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &session);
    let original = fs::read(state.join("checkpoint.json")).unwrap();
    let loaded = store
        .read_validated(|bytes| (bytes == b"empty-workspace").then_some(()))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(loaded.notice, Some(crate::CheckpointRestoreNotice::Empty));

    // The clean-stop owner explicitly publishes the current empty workspace. Storage archives
    // the chosen invalid original before admitting that final snapshot.
    store
        .publish_validated(b"empty-workspace", |bytes| bytes == b"empty-workspace")
        .unwrap();
    let originals: Vec<_> = fs::read_dir(&state)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .unwrap()
                .starts_with("checkpoint.corrupt-")
        })
        .map(|entry| fs::read(entry.path()).unwrap())
        .collect();
    assert_eq!(originals, vec![original]);
    assert!(
        store
            .read_validated(|bytes| (bytes == b"empty-workspace").then_some(()))
            .unwrap()
            .checkpoint
            .is_some()
    );
}

#[test]
fn interrupt_after_preserving_invalid_originals_never_recovers_a_complete_pending_file() {
    let fixture = Fixture::new();
    let session = fixture.open("preserve-before-commit");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    store.publish(b"invalid-domain").unwrap();
    let state = directory(&fixture, &session);
    let original = fs::read(state.join("checkpoint.json")).unwrap();
    let archive = state.join("checkpoint.corrupt-00000000000000000000000000000001.raw");
    fs::rename(state.join("checkpoint.json"), &archive).unwrap();
    // A complete, valid higher-generation envelope is still uncommitted in the pending path.
    let mut pending = original.clone();
    pending[5..13].copy_from_slice(&999_u64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(&pending[..13]);
    digest.update(&pending[45..]);
    pending[13..45].copy_from_slice(&digest.finalize());
    fs::write(state.join("checkpoint.pending"), pending).unwrap();
    drop(store);
    let reopened = SessionStore::open(&fixture.0, &session, 64).unwrap();
    let loaded = reopened
        .read_validated(|bytes| Some(bytes.to_vec()))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(loaded.notice, Some(crate::CheckpointRestoreNotice::Empty));
    assert_eq!(fs::read(archive).unwrap(), original);
    assert!(!state.join("checkpoint.pending").exists());
}

#[test]
fn a_crashed_pending_file_is_reclaimed_without_becoming_committed_data() {
    let fixture = Fixture::new();
    let session = fixture.open("pending");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    store.publish(b"committed").unwrap();
    let pending = directory(&fixture, &session).join("checkpoint.pending");
    fs::write(&pending, b"uncommitted").unwrap();
    drop(store);
    let reopened = SessionStore::open(&fixture.0, &session, 64).unwrap();
    assert!(!pending.exists());
    assert_eq!(reopened.read().unwrap().unwrap(), b"committed");
}

#[test]
fn linked_checkpoint_and_pending_files_are_refused_without_touching_their_targets() {
    let fixture = Fixture::new();
    let session = fixture.open("linked");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    let state = directory(&fixture, &session);
    let target = fixture.0.join("outside-data");
    fs::write(&target, b"preserve me").unwrap();
    fs::hard_link(&target, state.join("checkpoint.json")).unwrap();
    assert!(store.read().is_err());
    assert!(store.publish(b"replacement").is_err());
    assert_eq!(fs::read(&target).unwrap(), b"preserve me");
    fs::remove_file(state.join("checkpoint.json")).unwrap();
    fs::hard_link(&target, state.join("checkpoint.pending")).unwrap();
    drop(store);
    assert!(SessionStore::open(&fixture.0, &session, 64).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"preserve me");
    assert!(state.join("checkpoint.pending").exists());
}

#[test]
fn private_ancestor_handles_fence_rename_until_the_store_is_dropped() {
    let fixture = Fixture::new();
    let session = fixture.open("pinned-path");
    let store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    let state = directory(&fixture, &session);
    assert!(fs::rename(&state, state.with_extension("moved")).is_err());
    drop(store);
    fs::rename(&state, state.with_extension("moved")).unwrap();
    assert!(SessionStore::open(&fixture.0.join("."), &session, 64).is_err());
}

#[test]
fn redirected_state_directory_is_refused_before_writing_any_checkpoint() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let session = fixture.open("redirected");
    let target = fixture.0.join("elsewhere");
    let link = fixture.0.join("native-sessions");
    fs::create_dir(&target).unwrap();
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:ORCHBEE_FIXTURE_LINK -Value $env:ORCHBEE_FIXTURE_TARGET -ErrorAction Stop | Out-Null"])
        .env("ORCHBEE_FIXTURE_LINK", &link)
        .env("ORCHBEE_FIXTURE_TARGET", &target)
        .creation_flags(0x0800_0000)
        .status().unwrap();
    assert!(status.success());
    let result = SessionStore::open(&fixture.0, &session, 64);
    // Remove the link itself before the fixture's recursive cleanup, including on test failure.
    fs::remove_dir(&link).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_dir(target).unwrap().count(), 0);
}

#[test]
fn layout_history_keeps_the_newest_entries_and_refuses_a_renamed_frame() {
    let fixture = Fixture::new();
    let session = fixture.open("layout-history");
    let mut store = SessionStore::open(&fixture.0, &session, 64).unwrap();
    assert!(store.read_layout_history().unwrap().is_empty());
    for saved_at in 1..=5_u64 {
        store
            .record_layout_history(saved_at * 1000, format!("layout {saved_at}").as_bytes(), 3)
            .unwrap();
    }
    let kept = store.read_layout_history().unwrap();
    assert_eq!(
        kept.iter().map(|entry| entry.saved_at).collect::<Vec<_>>(),
        [5000, 4000, 3000]
    );
    assert_eq!(kept[0].payload.as_deref(), Some(b"layout 5".as_slice()));
    assert_eq!(store.read_layout_history_entry(1000).unwrap(), None);
    // A frame carries its own save time, so a file moved to another entry name is unreadable.
    let history = directory(&fixture, &session).join("layout-history");
    fs::rename(
        history.join(format!("layout-{:016x}.json", 3000)),
        history.join(format!("layout-{:016x}.json", 3500)),
    )
    .unwrap();
    assert_eq!(store.read_layout_history_entry(3500).unwrap(), None);
    // The slots are untouched by history writes.
    assert_eq!(store.read().unwrap(), None);
}
