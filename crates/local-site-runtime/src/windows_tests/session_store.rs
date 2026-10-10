use super::{Fixture, Rendezvous, fs, permissions};
use crate::WindowsSessionStore;
use sha2::{Digest, Sha256};
use std::io;

fn directory(fixture: &Fixture, site: &Rendezvous) -> std::path::PathBuf {
    fixture
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(site.runtime_directory()))
}

#[test]
fn fatal_domain_admission_refuses_every_fallback_and_preserves_committed_files() {
    for fatal_slot in 0..3 {
        for healthy in [false, true] {
            let fixture = Fixture::new();
            let site = fixture.open("fatal-domain-admission");
            let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
            for slot in 0..3 {
                let payload: &[u8] = if slot == fatal_slot {
                    b"migration-limit".as_slice()
                } else if healthy {
                    if slot == 0 { b"old" } else { b"new" }
                } else {
                    match slot {
                        0 => b"corrupt-0",
                        1 => b"corrupt-1",
                        _ => b"corrupt-2",
                    }
                };
                store.publish(payload).unwrap();
            }
            drop(store);
            let state = directory(&fixture, &site);
            let retained = || {
                fs::read_dir(&state)
                    .unwrap()
                    .map(|entry| {
                        let entry = entry.unwrap();
                        (entry.file_name(), fs::read(entry.path()).unwrap())
                    })
                    .collect::<std::collections::BTreeMap<_, _>>()
            };
            let before = retained();
            let store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
            let error = store
                .try_read_validated(|bytes| {
                    if bytes == b"migration-limit" {
                        Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "explicit migration refusal",
                        ))
                    } else {
                        Ok((!bytes.starts_with(b"corrupt-")).then(|| bytes.to_vec()))
                    }
                })
                .err()
                .expect("a fatal record cannot select any other slot or Empty");
            assert_eq!(error.kind(), io::ErrorKind::Unsupported);
            assert_eq!(error.to_string(), "explicit migration refusal");
            drop(store);
            assert_eq!(retained(), before);
        }
    }
}

#[test]
fn retains_three_latest_valid_payloads_and_reopens_the_latest_slot() {
    let fixture = Fixture::new();
    let site = fixture.open("three-checkpoints");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    for payload in [b"first".as_slice(), b"second", b"third", b"fourth"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &site);
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
    let reopened = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    assert_eq!(reopened.read().unwrap(), Some(b"fourth".to_vec()));
}

#[test]
fn invalid_latest_digest_falls_back_without_modifying_the_invalid_original() {
    let fixture = Fixture::new();
    let site = fixture.open("invalid-latest");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    for payload in [b"first".as_slice(), b"second", b"third"] {
        store.publish(payload).unwrap();
    }
    let path = directory(&fixture, &site).join("checkpoint.previous-2.json");
    let mut invalid = fs::read(&path).unwrap();
    *invalid.last_mut().unwrap() ^= 1;
    fs::write(&path, &invalid).unwrap();
    assert_eq!(store.read().unwrap(), Some(b"second".to_vec()));
    assert_eq!(fs::read(&path).unwrap(), invalid);
    store.publish(b"fourth").unwrap();
    let originals: Vec<_> = fs::read_dir(directory(&fixture, &site))
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
fn preserves_committed_bytes_across_reopen_and_isolates_sites() {
    let fixture = Fixture::new();
    let first_site = fixture.open("first");
    let second_site = fixture.open("second");
    let mut first = WindowsSessionStore::open(&fixture.0, &first_site, 64).unwrap();
    let mut second = WindowsSessionStore::open(&fixture.0, &second_site, 64).unwrap();
    assert_eq!(first.read().unwrap(), None);
    first.publish("first 中文".as_bytes()).unwrap();
    second.publish(b"second").unwrap();
    first.publish(b"replacement").unwrap();
    assert_eq!(first.read().unwrap().unwrap(), b"replacement");
    assert_eq!(second.read().unwrap().unwrap(), b"second");
    drop(first);
    let reopened = WindowsSessionStore::open(&fixture.0, &first_site, 64).unwrap();
    assert_eq!(reopened.read().unwrap().unwrap(), b"replacement");
    assert!(
        !directory(&fixture, &first_site)
            .join("checkpoint.pending")
            .exists()
    );
}

#[test]
fn another_owner_cannot_publish_until_the_store_lock_is_released() {
    let fixture = Fixture::new();
    let site = fixture.open("locked");
    let owner = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    let denied = WindowsSessionStore::open(&fixture.0, &site, 64)
        .err()
        .unwrap();
    assert_eq!(denied.kind(), io::ErrorKind::WouldBlock);
    drop(owner);
    assert!(WindowsSessionStore::open(&fixture.0, &site, 64).is_ok());
}

#[test]
fn missing_empty_exact_limit_and_oversized_files_remain_distinct() {
    let fixture = Fixture::new();
    let site = fixture.open("bounds");
    assert!(WindowsSessionStore::open(&fixture.0, &site, 0).is_err());
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    let path = directory(&fixture, &site).join("checkpoint.json");
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
    let latest = directory(&fixture, &site).join("checkpoint.previous-1.json");
    assert_eq!(&fs::read(&latest).unwrap()[45..], &[2; 64]);
    let oversized = vec![4; 110];
    fs::write(&latest, &oversized).unwrap();
    let loaded = store.read_validated(|bytes| Some(bytes.to_vec())).unwrap();
    assert_eq!(loaded.checkpoint, Some(Vec::new()));
    assert_eq!(
        loaded.notice,
        Some(crate::WindowsCheckpointRestoreNotice::Recovered)
    );
    store.publish(b"valid after oversized data").unwrap();
    assert_eq!(fs::read(&latest).unwrap(), oversized);
    store.publish(b"another valid snapshot").unwrap();
    let originals: Vec<_> = fs::read_dir(directory(&fixture, &site))
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
    let site = fixture.open("blocked-publish");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    store.publish(b"old").unwrap();
    store.publish(b"middle").unwrap();
    store.publish(b"latest").unwrap();
    let path = directory(&fixture, &site).join("checkpoint.json");
    let before = [
        "checkpoint.json",
        "checkpoint.previous-1.json",
        "checkpoint.previous-2.json",
    ]
    .map(|name| fs::read(directory(&fixture, &site).join(name)).unwrap());
    // This real Windows handle denies delete sharing, so replacement cannot commit.
    let held = permissions::open_file(&path, false, false).unwrap();
    assert!(store.publish(b"new").is_err());
    let after = [
        "checkpoint.json",
        "checkpoint.previous-1.json",
        "checkpoint.previous-2.json",
    ]
    .map(|name| fs::read(directory(&fixture, &site).join(name)).unwrap());
    assert_eq!(after, before);
    assert_eq!(store.read().unwrap(), Some(b"latest".to_vec()));
    assert!(
        !directory(&fixture, &site)
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
    let site = fixture.open("invalid-domain");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    for payload in [b"invalid-1".as_slice(), b"invalid-2", b"invalid-3"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &site);
    let original = fs::read(state.join("checkpoint.json")).unwrap();
    let loaded = store
        .read_validated(|bytes| (bytes == b"valid-domain").then_some(42))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(
        loaded.notice,
        Some(crate::WindowsCheckpointRestoreNotice::Empty)
    );
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
        Some(crate::WindowsCheckpointRestoreNotice::Recovered)
    );
}

#[test]
fn explicit_empty_final_after_all_invalid_preserves_the_displaced_original() {
    let fixture = Fixture::new();
    let site = fixture.open("empty-final-after-invalid");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    for payload in [b"invalid-1".as_slice(), b"invalid-2", b"invalid-3"] {
        store.publish(payload).unwrap();
    }
    let state = directory(&fixture, &site);
    let original = fs::read(state.join("checkpoint.json")).unwrap();
    let loaded = store
        .read_validated(|bytes| (bytes == b"empty-workspace").then_some(()))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(
        loaded.notice,
        Some(crate::WindowsCheckpointRestoreNotice::Empty)
    );

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
    let site = fixture.open("preserve-before-commit");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    store.publish(b"invalid-domain").unwrap();
    let state = directory(&fixture, &site);
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
    let reopened = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    let loaded = reopened
        .read_validated(|bytes| Some(bytes.to_vec()))
        .unwrap();
    assert_eq!(loaded.checkpoint, None);
    assert_eq!(
        loaded.notice,
        Some(crate::WindowsCheckpointRestoreNotice::Empty)
    );
    assert_eq!(fs::read(archive).unwrap(), original);
    assert!(!state.join("checkpoint.pending").exists());
}

#[test]
fn a_crashed_pending_file_is_reclaimed_without_becoming_committed_data() {
    let fixture = Fixture::new();
    let site = fixture.open("pending");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    store.publish(b"committed").unwrap();
    let pending = directory(&fixture, &site).join("checkpoint.pending");
    fs::write(&pending, b"uncommitted").unwrap();
    drop(store);
    let reopened = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    assert!(!pending.exists());
    assert_eq!(reopened.read().unwrap().unwrap(), b"committed");
}

#[test]
fn linked_checkpoint_and_pending_files_are_refused_without_touching_their_targets() {
    let fixture = Fixture::new();
    let site = fixture.open("linked");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    let state = directory(&fixture, &site);
    let target = fixture.0.join("outside-data");
    fs::write(&target, b"preserve me").unwrap();
    fs::hard_link(&target, state.join("checkpoint.json")).unwrap();
    assert!(store.read().is_err());
    assert!(store.publish(b"replacement").is_err());
    assert_eq!(fs::read(&target).unwrap(), b"preserve me");
    fs::remove_file(state.join("checkpoint.json")).unwrap();
    fs::hard_link(&target, state.join("checkpoint.pending")).unwrap();
    drop(store);
    assert!(WindowsSessionStore::open(&fixture.0, &site, 64).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"preserve me");
    assert!(state.join("checkpoint.pending").exists());
}

#[test]
fn private_ancestor_handles_fence_rename_until_the_store_is_dropped() {
    let fixture = Fixture::new();
    let site = fixture.open("pinned-path");
    let store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    let state = directory(&fixture, &site);
    assert!(fs::rename(&state, state.with_extension("moved")).is_err());
    drop(store);
    fs::rename(&state, state.with_extension("moved")).unwrap();
    assert!(WindowsSessionStore::open(&fixture.0.join("."), &site, 64).is_err());
}

#[test]
fn redirected_state_directory_is_refused_before_writing_any_checkpoint() {
    use std::os::windows::process::CommandExt;
    let fixture = Fixture::new();
    let site = fixture.open("redirected");
    let target = fixture.0.join("elsewhere");
    let link = fixture.0.join("Orchbee");
    fs::create_dir(&target).unwrap();
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "New-Item -ItemType Junction -Path $env:ORCHBEE_FIXTURE_LINK -Value $env:ORCHBEE_FIXTURE_TARGET -ErrorAction Stop | Out-Null"])
        .env("ORCHBEE_FIXTURE_LINK", &link)
        .env("ORCHBEE_FIXTURE_TARGET", &target)
        .creation_flags(0x0800_0000)
        .status().unwrap();
    assert!(status.success());
    let result = WindowsSessionStore::open(&fixture.0, &site, 64);
    // Remove the link itself before the fixture's recursive cleanup, including on test failure.
    fs::remove_dir(&link).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_dir(target).unwrap().count(), 0);
}
