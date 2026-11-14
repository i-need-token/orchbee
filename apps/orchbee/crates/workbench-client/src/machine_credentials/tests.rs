//! The port against this computer's real Windows Credential Manager, under test environments only
//! and with test words only. Each test's Machine id carries this process's id, so two checkouts
//! or runs testing at once never share an item; a guard deletes a test's items before it writes
//! and again on every path, pass or panic, and only those items are ever read. Before the first
//! test writes, the items an earlier test process left when it was killed are deleted, by name and
//! only when that process no longer runs. Where the store cannot be used in this login, as in an
//! SSH logon or a service session, its own probe answers unavailable and the round trips assert
//! nothing.

use super::*;
use keyring_core::api::CredentialStoreApi;
use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, Once, PoisonError},
};
use winsafe::{self as w, co};

/// Serializes the tests that write the real store, so each test's items are gone before the next
/// test writes. The adapter makes one Credential Manager call at a time by itself, which is what
/// keeps a deleted item from coming back. It guards no data, so a test that panicked while holding
/// it leaves nothing to distrust.
static STORE: Mutex<()> = Mutex::new(());

/// The names a test of this file may have written, in any test environment: never a release
/// item's, whose environment is `orchbee`.
const LEFT_BEHIND: &str =
    "^Orchbee/ob-test-[^/]+/machine/7e57c0de[0-9a-f]{24}/(password|key-passphrase)$";
/// The exit code a process that still runs reads.
const STILL_ACTIVE: u32 = 259;

/// The environment of the items the round trip writes: no build's stem.
const ENVIRONMENT: &str = "ob-test-credential-store";
const WORD: &[u8] = b"test-word-store-one";
const OTHER_WORD: &[u8] = b"test-word-store-two";

/// A machine id of this process; each test passes its own `last` digits, so no two tests share
/// one.
fn machine(last: &str) -> MachineId {
    MachineId::new(format!(
        "7e57c0de{:08x}0000000000{last}",
        std::process::id()
    ))
    .unwrap()
}

/// Holds [`STORE`] for one test and deletes its items when made, so a reused process id finds none
/// left by a killed run, and again when dropped, whatever the test did, before the next test may
/// write.
struct Cleanup {
    items: Vec<Item>,
    _store: MutexGuard<'static, ()>,
}

impl Cleanup {
    fn new(items: Vec<Item>) -> Self {
        let cleanup = Self {
            items,
            _store: STORE.lock().unwrap_or_else(PoisonError::into_inner),
        };
        sweep();
        cleanup.delete();
        cleanup
    }

    fn delete(&self) {
        let store = Store::select();
        for item in &self.items {
            let _deleted = store.delete(item);
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.delete();
    }
}

/// Deletes, once in this process, every item a test of this file wrote in a test process that no
/// longer runs, as a killed run leaves them: only names of [`LEFT_BEHIND`]'s form, read back and
/// checked here rather than trusted to the store's filter, each through the port.
fn sweep() {
    static SWEPT: Once = Once::new();
    SWEPT.call_once(|| {
        let Ok(manager) = windows_native_keyring_store::Store::new() else {
            return;
        };
        let Ok(found) = manager.search(&HashMap::from([("pattern", LEFT_BEHIND)])) else {
            return;
        };
        let store = Store::select();
        for entry in found {
            let Ok(attributes) = entry.get_attributes() else {
                continue;
            };
            if let Some((item, process)) = attributes.get("target_name").and_then(|name| left(name))
                && process != std::process::id()
                && !running(process)
            {
                let _deleted = store.delete(&item);
            }
        }
    });
}

/// The test item `name` names and the id of the test process that wrote it; `None` for any other
/// name.
fn left(name: &str) -> Option<(Item, u32)> {
    let mut parts = name.strip_prefix("Orchbee/")?.split('/');
    let (environment, machine, id, kind) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let kind = SecretKind::ALL
        .into_iter()
        .find(|known| known.name() == kind)?;
    let process = id.strip_prefix("7e57c0de")?.get(..8)?;
    let ours = parts.next().is_none()
        && environment.starts_with("ob-test-")
        && machine == "machine"
        && id.get(16..26) == Some("0000000000");
    let process = u32::from_str_radix(process, 16).ok().filter(|_| ours)?;
    let machine = MachineId::new(id.to_owned()).ok()?;
    Some((Item::in_environment(environment, machine, kind), process))
}

/// Whether the process `process` still runs; one that cannot be looked at counts as running.
fn running(process: u32) -> bool {
    match w::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process) {
        Ok(handle) => !matches!(handle.GetExitCodeProcess(), Ok(code) if code != STILL_ACTIVE),
        Err(error) => error != co::ERROR::INVALID_PARAMETER,
    }
}

/// Whether the store can keep an answer in this login, by its own probe, which reads one name no
/// item has. It never answers that `ssh` is too old; only `machine_ssh::remembering` does.
fn usable(store: &Store) -> bool {
    match store.remembering() {
        Remembering::Unavailable => false,
        Remembering::Lasting | Remembering::UntilRestart => true,
        Remembering::NeedsNewerSsh => panic!("the store answered for ssh"),
    }
}

#[test]
fn item_names_follow_the_frozen_form() {
    let machine = MachineId::new("0123456789abcdef0123456789abcdef").unwrap();
    assert_eq!(
        Item::in_environment("orchbee", machine.clone(), SecretKind::Password).target_name(),
        "Orchbee/orchbee/machine/0123456789abcdef0123456789abcdef/password"
    );
    assert_eq!(
        Item::in_environment("ob-dev-1a2b", machine, SecretKind::KeyPassphrase).target_name(),
        "Orchbee/ob-dev-1a2b/machine/0123456789abcdef0123456789abcdef/key-passphrase"
    );
}

#[test]
fn an_answer_is_written_read_replaced_and_deleted() {
    let store = Store::select();
    if !usable(&store) {
        return;
    }
    let item = Item::in_environment(ENVIRONMENT, machine("000001"), SecretKind::Password);
    let _cleanup = Cleanup::new(vec![item.clone()]);
    assert!(store.read(&item).unwrap().is_none());
    store
        .write(&item, WORD, &readable_label("test machine"))
        .unwrap();
    assert_eq!(store.read(&item).unwrap().unwrap().as_bytes(), WORD);
    store
        .write(&item, OTHER_WORD, &readable_label("test machine"))
        .unwrap();
    assert_eq!(store.read(&item).unwrap().unwrap().as_bytes(), OTHER_WORD);
    assert_eq!(store.delete(&item), Ok(Deleted::Removed));
    assert!(store.read(&item).unwrap().is_none());
    assert_eq!(store.delete(&item), Ok(Deleted::Absent));
}

#[test]
fn forgetting_a_machine_deletes_both_kinds() {
    let store = Store::select();
    if !usable(&store) {
        return;
    }
    let machine = machine("000002");
    let items = SecretKind::ALL.map(|kind| Item::new(machine.clone(), kind));
    let _cleanup = Cleanup::new(items.to_vec());
    for item in &items {
        store
            .write(item, WORD, &readable_label("test machine"))
            .unwrap();
    }
    assert_eq!(store.forget(&machine), Ok(Deleted::Removed));
    for item in &items {
        assert!(store.read(item).unwrap().is_none());
    }
    assert_eq!(store.forget(&machine), Ok(Deleted::Absent));
}

#[test]
fn a_store_that_cannot_be_used_keeps_nothing() {
    let store = Store::unavailable();
    let item = Item::in_environment(ENVIRONMENT, machine("000003"), SecretKind::Password);
    assert_eq!(store.remembering(), Remembering::Unavailable);
    assert!(matches!(store.read(&item), Err(Failure::Unavailable)));
    assert_eq!(store.write(&item, WORD, "x"), Err(Failure::Unavailable));
    assert_eq!(store.delete(&item), Err(Failure::Unavailable));
    assert_eq!(store.forget(&machine("000003")), Err(Failure::Unavailable));
}

#[test]
fn a_readable_label_drops_control_characters_and_is_bounded() {
    assert_eq!(readable_label("dev\u{1b}[31mbox\n"), "Orchbee: dev[31mbox");
    let long = "x".repeat(500);
    assert_eq!(readable_label(&long).chars().count(), 9 + 120);
}
