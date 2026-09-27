use super::{Fixture, Rendezvous, fs, permissions};
use crate::WindowsSessionStore;
use std::io;

fn directory(fixture: &Fixture, site: &Rendezvous) -> std::path::PathBuf {
    fixture
        .0
        .join("Orchbee/native-sessions")
        .join(Rendezvous::host_endpoint_id(site.runtime_directory()))
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
    assert_eq!(fs::read(&path).unwrap(), vec![2; 64]);
    fs::write(&path, [4; 65]).unwrap();
    assert_eq!(store.read().unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(store.publish(b"must not hide oversized data").is_err());
    assert_eq!(fs::read(path).unwrap(), vec![4; 65]);
}

#[test]
fn failed_replacement_keeps_the_prior_checkpoint_and_reclaims_staging() {
    let fixture = Fixture::new();
    let site = fixture.open("blocked-publish");
    let mut store = WindowsSessionStore::open(&fixture.0, &site, 64).unwrap();
    store.publish(b"old").unwrap();
    let path = directory(&fixture, &site).join("checkpoint.json");
    // This real Windows handle denies delete sharing, so replacement cannot commit.
    let held = permissions::open_file(&path, false, false).unwrap();
    assert!(store.publish(b"new").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old");
    assert!(
        !directory(&fixture, &site)
            .join("checkpoint.pending")
            .exists()
    );
    drop(held);
    store.publish(b"new").unwrap();
    assert_eq!(fs::read(path).unwrap(), b"new");
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
