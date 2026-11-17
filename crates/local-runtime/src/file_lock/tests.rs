use super::*;
use std::{fs, path::PathBuf};

/// A scratch lock file, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        // The counter keeps two files made in the same clock tick apart.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = crate::test_temporary_directory().join(format!(
            "orchbee-file-lock-{}-{nonce:032x}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        File::create_new(&path).unwrap();
        Self(path)
    }

    fn open(&self) -> File {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.0)
            .unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

/// A child process that another thread starts while the lock is held keeps a copy of the lock's
/// descriptor until it replaces its image; a duplicate here stands in for that copy. Dropping the
/// owner still frees the lock for the next taker at once.
#[test]
fn dropping_the_owner_frees_the_lock_while_a_copy_of_its_descriptor_is_open() {
    let scratch = Scratch::new();
    let held = HeldLock::try_exclusive(scratch.open()).unwrap();
    let copy = held.try_clone().unwrap();
    drop(held);
    let next = HeldLock::try_exclusive(scratch.open());
    assert!(next.is_ok(), "{:?}", next.err());
    drop((next, copy));
}
