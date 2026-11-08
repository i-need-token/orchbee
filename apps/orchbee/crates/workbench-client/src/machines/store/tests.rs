use super::*;
use crate::machines::{Draft, ProfileId};
use std::fs;
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        if !self.0.exists() {
            return;
        }
        let path = self.0.canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("orchbee-machine-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn store_reloads_concurrent_changes_and_preserves_corruption_under_real_private_lock() {
    let id = ProfileId::generate().unwrap();
    let root = Root(std::env::temp_dir().join(format!("orchbee-machine-test-{}", id.as_str())));
    let store = Store {
        root: root.0.join("machines"),
    };
    assert_eq!(store.read().unwrap(), Catalog::default());
    assert!(!root.0.exists());
    let draft = Draft::parse("First", "example.invalid", "work").unwrap();
    let first = Edit::Add {
        id: id.clone(),
        draft: draft.clone(),
    };
    let (catalog, _) = store.commit(&first).unwrap();
    assert_eq!(catalog.revision, 1);
    let lock = PrivateCache::acquire(&store.root, RECORD.name()).unwrap();
    assert_eq!(
        store.commit(&Edit::Remove(id.clone())).unwrap_err(),
        Error::Busy
    );
    assert_eq!(store.read().unwrap().revision, 1);
    drop(lock);
    // A preparation reader becomes stale while a different invocation renames.
    let before = store.read().unwrap();
    store
        .commit(&Edit::Rename {
            id: id.clone(),
            label: "Concurrent".into(),
        })
        .unwrap();
    let other = ProfileId::generate().unwrap();
    store.commit(&Edit::Add { id: other, draft }).unwrap();
    let current = store.read().unwrap();
    assert_eq!(before.revision, 1);
    assert_eq!(current.revision, 3);
    assert_eq!(current.profiles[0].draft.label, "Concurrent");
    let mut lock = PrivateCache::acquire(&store.root, RECORD.name()).unwrap();
    lock.replace(FILE, b"{").unwrap();
    drop(lock);
    assert_eq!(store.read().unwrap_err(), Error::InvalidCatalog);
    assert_eq!(
        store.commit(&Edit::Remove(id)).unwrap_err(),
        Error::InvalidCatalog
    );
    assert_eq!(fs::read(store.root.join(FILE)).unwrap(), b"{");
}
