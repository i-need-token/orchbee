//! Immutable publication, writer exclusion, disposal of unpublished files and the pointer's
//! atomic replacement.
use super::super::{ReleaseStore, read_pointer};
use super::*;
use std::io::Write;

/// A fresh private root under the test process's temporary folder.
fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "orchbee-release-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn remove(root: &Path) {
    let actual = root.canonicalize().unwrap();
    assert_eq!(
        actual.parent(),
        Some(std::env::temp_dir().canonicalize().unwrap().as_path())
    );
    assert!(
        actual
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("orchbee-release-test-")
    );
    fs::remove_dir_all(actual).unwrap();
}

#[test]
fn release_publication_is_exclusive_immutable_and_cleans_staging() {
    let root = temporary_root();
    let version = "0.1.0";
    {
        let store = ReleaseStore::acquire(&root).unwrap();
        assert!(ReleaseStore::acquire(&root).is_err());
        let mut stage = store.stage().unwrap();
        assert!(stage.create_file("../other").is_err());
        let (_, mut file) = stage.create_file("bin/tool.exe").unwrap();
        file.write_all(b"complete").unwrap();
        file.sync_all().unwrap();
        drop(file);
        let target = stage.publish(version).unwrap();
        assert_eq!(target, root.join("versions").join(version));
        assert_eq!(fs::read(target.join("bin/tool.exe")).unwrap(), b"complete");
        stage.cleanup().unwrap();
        assert!(target.is_dir());
        let mut second = store.stage().unwrap();
        let pending = second.path().to_owned();
        assert!(second.publish(version).is_err());
        second.cleanup().unwrap();
        assert!(!pending.exists());
        assert_eq!(fs::read(target.join("bin/tool.exe")).unwrap(), b"complete");
    }
    remove(&root);
}

/// The pointer is replaced only under `install.lock` and only by a flushed rename: a staging file
/// a crashed writer left is never read as the pointer and is removed under the next lock, and the
/// lock and staging names are never records.
#[test]
fn pointer_replacement_is_atomic_under_the_install_lock() {
    let root = temporary_root();
    {
        let mut store = ReleaseStore::acquire(&root).unwrap();
        assert_eq!(store.pointer().unwrap(), None);
        store.replace_pointer(b"first").unwrap();
        assert!(root.join("install.lock").is_file());
        assert!(!root.join("active.pending").exists());
        assert!(store.lock.replace("active.pending", b"x").is_err());
        assert!(store.lock.replace("install.lock", b"x").is_err());
    }
    // A writer that crashed between its flush and its rename leaves only the staging file.
    fs::write(root.join("active.pending"), b"torn").unwrap();
    assert_eq!(read_pointer(&root).unwrap().as_deref(), Some(&b"first"[..]));
    {
        let mut store = ReleaseStore::acquire(&root).unwrap();
        assert!(!root.join("active.pending").exists());
        assert!(ReleaseStore::acquire(&root).is_err());
        store.replace_pointer(b"second").unwrap();
        assert_eq!(store.pointer().unwrap().as_deref(), Some(&b"second"[..]));
    }
    assert_eq!(
        read_pointer(&root).unwrap().as_deref(),
        Some(&b"second"[..])
    );
    assert!(!root.join("active.pending").exists());
    remove(&root);
}
