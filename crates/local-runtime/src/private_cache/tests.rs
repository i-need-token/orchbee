use super::*;
#[test]
fn private_cache_serializes_writers_and_preserves_rejected_targets() {
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let actual = self.0.canonicalize().unwrap();
            assert_eq!(
                actual.parent(),
                Some(
                    crate::test_temporary_directory()
                        .canonicalize()
                        .unwrap()
                        .as_path()
                )
            );
            assert!(
                actual
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchbee-private-cache-")
            );
            fs::remove_dir_all(actual).unwrap();
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Root(
        crate::test_temporary_directory().join(format!("orchbee-private-cache-{:032x}", nonce)),
    );
    fs::create_dir(&root.0).unwrap();
    permissions::protect_new(&mut permissions::open_directory(&root.0, true).unwrap()).unwrap();
    let path = root.0.join("nested").join("remote");
    assert!(
        PrivateCache::read_existing(&path, "status.json", 10)
            .unwrap()
            .is_none()
    );
    assert!(!path.exists());
    let mut cache = PrivateCache::acquire(&path, "codex").unwrap();
    assert!(
        matches!(PrivateCache::acquire(&path, "codex"),Err(e) if e.kind()==io::ErrorKind::WouldBlock)
    );
    // Another record in the same folder has its own writer lock.
    drop(PrivateCache::acquire(&path, "status").unwrap());
    for reserved in ["codex.lock", "CODEX.PENDING"] {
        assert!(cache.replace(reserved, b"bad").is_err(), "{reserved}");
        assert!(cache.read(reserved, 4).is_err(), "{reserved}");
    }
    assert!(cache.read("codex.toml", 4).unwrap().is_none());
    cache.replace("codex.toml", b"old").unwrap();
    cache.replace("codex.toml", b"new").unwrap();
    assert_eq!(cache.read("codex.toml", 3).unwrap(), Some(b"new".to_vec()));
    assert_eq!(
        cache.read("codex.toml", 2).unwrap_err().kind(),
        io::ErrorKind::FileTooLarge
    );
    assert!(cache.replace("../other", b"bad").is_err());
    fs::hard_link(path.join("codex.toml"), path.join("alias.toml")).unwrap();
    assert!(cache.replace("codex.toml", b"bad").is_err());
    assert_eq!(fs::read(path.join("codex.toml")).unwrap(), b"new");
    fs::remove_file(path.join("alias.toml")).unwrap();
    // Windows pins admitted ancestors against renaming; POSIX relies on their private owner.
    #[cfg(windows)]
    assert!(fs::rename(&path, root.0.join("redirected")).is_err());
    assert!(!path.join("codex.pending").exists());
    drop(cache);
    permissions::open_file(&path.join("codex.pending"), true, true)
        .unwrap()
        .write_all(b"interrupted stage")
        .unwrap();
    let cache = PrivateCache::acquire(&path, "codex").unwrap();
    assert!(!path.join("codex.pending").exists());
    drop(cache);
}
