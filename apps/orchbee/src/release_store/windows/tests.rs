//! Immutable publication, writer exclusion, disposal of unpublished files, the pointer's atomic
//! replacement and the admission of a root an installer created.
use super::super::{ReleaseStore, read_pointer};
use super::*;
use std::io::Write;

/// A fresh private root under the test process's temporary folder. The counter keeps two roots
/// taken in the same clock tick apart; the `s` keeps them apart from the update tests' roots, which
/// share the prefix and the process.
fn temporary_root() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "orchbee-release-test-{}-s{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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

/// Lets the built-in Users group read `folder` and everything created below it, as a
/// `%LOCALAPPDATA%\Programs` another account shares does.
fn share_with_users(folder: &Path) {
    let output = std::process::Command::new("icacls.exe")
        .arg(folder)
        .args(["/grant", "*S-1-5-32-545:(OI)(CI)RX", "/q"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// Setup creates the store root below such a folder and writes its own files there before it runs
/// the bundled `install`: that root is made private, and one holding anything else is refused as
/// it is, before anything is written.
#[test]
fn an_installer_root_below_a_shared_folder_is_made_private_and_one_holding_more_refused() {
    use orchbee_local_runtime::{AdmissionRefusal, RefusalReason};
    let programs = temporary_root();
    fs::create_dir(&programs).unwrap();
    share_with_users(&programs);
    let root = programs.join("Orchbee");
    fs::create_dir_all(root.join("desktop")).unwrap();
    fs::write(root.join(r"desktop\x.txt"), b"window").unwrap();
    {
        let _store = ReleaseStore::acquire(&root).unwrap();
        permissions::require_private(&permissions::open_directory(&root, false).unwrap()).unwrap();
    }
    let other = programs.join("Other");
    fs::create_dir_all(other.join("desktop")).unwrap();
    fs::write(other.join("notes.txt"), b"another program's").unwrap();
    let Err(refused) = ReleaseStore::acquire(&other) else {
        panic!("a root holding another program's file was admitted");
    };
    assert_eq!(
        AdmissionRefusal::of(&refused),
        Some(&AdmissionRefusal::new(RefusalReason::OthersCanRead, &other))
    );
    assert!(!other.join("install.lock").exists());
    remove(&programs);
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
