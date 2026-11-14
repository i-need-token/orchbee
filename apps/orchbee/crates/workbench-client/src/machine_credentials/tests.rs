//! The port against this computer's real Windows Credential Manager, under test environments only
//! and with test words only. Each test names its own fixed Machine id, so a run that was killed
//! leaves items the next run deletes before it writes; a guard deletes them again on every path,
//! pass or panic, and only those items are ever read. Where the store cannot be used in this login,
//! as in an SSH logon or a service session, its own probe answers unavailable and the round trips
//! assert nothing.

use super::*;

/// The environment of the items the round trip writes: no build's stem.
const ENVIRONMENT: &str = "ob-test-credential-store";
const WORD: &[u8] = b"test-word-store-one";
const OTHER_WORD: &[u8] = b"test-word-store-two";

/// A fixed machine id; each test passes its own `last` digits, so no two tests share one.
fn machine(last: &str) -> MachineId {
    MachineId::new(format!("7e57c0de000000000000000000{last}")).unwrap()
}

/// Deletes its items when made, so a killed run's are gone before the test writes, and again when
/// dropped, whatever the test did.
struct Cleanup(Vec<Item>);

impl Cleanup {
    fn new(items: Vec<Item>) -> Self {
        let cleanup = Self(items);
        cleanup.delete();
        cleanup
    }

    fn delete(&self) {
        let store = Store::select();
        for item in &self.0 {
            let _deleted = store.delete(item);
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.delete();
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
