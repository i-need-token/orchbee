//! The port against this computer's real Windows Credential Manager, under test environments only
//! and with test words only. Every item a test writes is deleted by a guard on every path, pass or
//! panic, and only those items are ever read.

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// The environment of the items the round trip writes: no build's stem.
const ENVIRONMENT: &str = "ob-test-credential-store";
const WORD: &[u8] = b"test-word-store-one";
const OTHER_WORD: &[u8] = b"test-word-store-two";

/// A machine id no other run uses.
fn machine() -> MachineId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let unique = nanos ^ (u128::from(std::process::id()) << 96);
    MachineId::new(format!("{unique:032x}")).unwrap()
}

/// Deletes its items when dropped, whatever the test did.
struct Cleanup(Vec<Item>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let store = Store::select();
        for item in &self.0 {
            let _deleted = store.delete(item);
        }
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
    assert_eq!(store.remembering(), Remembering::Lasting);
    let item = Item::in_environment(ENVIRONMENT, machine(), SecretKind::Password);
    let _cleanup = Cleanup(vec![item.clone()]);
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
    let machine = machine();
    let items = SecretKind::ALL.map(|kind| Item::new(machine.clone(), kind));
    let _cleanup = Cleanup(items.to_vec());
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
    let item = Item::in_environment(ENVIRONMENT, machine(), SecretKind::Password);
    assert_eq!(store.remembering(), Remembering::Unavailable);
    assert!(matches!(store.read(&item), Err(Failure::Unavailable)));
    assert_eq!(store.write(&item, WORD, "x"), Err(Failure::Unavailable));
    assert_eq!(store.delete(&item), Err(Failure::Unavailable));
    assert_eq!(store.forget(&machine()), Err(Failure::Unavailable));
}

#[test]
fn a_readable_label_drops_control_characters_and_is_bounded() {
    assert_eq!(readable_label("dev\u{1b}[31mbox\n"), "Orchbee: dev[31mbox");
    let long = "x".repeat(500);
    assert_eq!(readable_label(&long).chars().count(), 9 + 120);
}
