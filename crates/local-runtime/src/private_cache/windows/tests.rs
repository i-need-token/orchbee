use super::*;
#[test]
fn configuration_names_admit_unicode_and_reject_windows_aliases_without_opening_them() {
    let root = Path::new("C:/example/private");
    for name in ["项目配置.toml", ".config.toml", "config.toml"] {
        assert_eq!(configuration_leaf(root, name).unwrap(), root.join(name));
    }
    for name in [
        "",
        ".",
        "..",
        "a/other",
        "a\\other",
        "a:stream",
        "config.",
        "config ",
        "NUL",
        "con.toml",
        "COM¹.toml",
        "LPT9",
        ".ORCHBEE-UPDATE.LOCK",
        ".orchbee-update.pending",
    ] {
        assert!(configuration_leaf(root, name).is_err(), "{name}");
    }
    assert!(cache_leaf(root, "项目配置.toml").is_err());
}
#[test]
fn private_cache_serializes_writers_and_preserves_rejected_targets() {
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let actual = self.0.canonicalize().unwrap();
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
                    .starts_with("orchbee-private-cache-")
            );
            fs::remove_dir_all(actual).unwrap();
        }
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Root(std::env::temp_dir().join(format!("orchbee-private-cache-{:032x}", nonce)));
    fs::create_dir(&root.0).unwrap();
    let path = root.0.join("nested").join("remote");
    assert!(
        PrivateCache::read_existing(&path, "status.json", 10)
            .unwrap()
            .is_none()
    );
    assert!(!path.exists());
    let mut cache = PrivateCache::acquire(&path).unwrap();
    assert!(matches!(PrivateCache::acquire(&path),Err(e) if e.kind()==io::ErrorKind::WouldBlock));
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
    assert!(fs::rename(&path, root.0.join("redirected")).is_err());
    assert!(!path.join(PENDING).exists());
    drop(cache);
    fs::write(path.join(PENDING), b"interrupted stage").unwrap();
    let cache = PrivateCache::acquire(&path).unwrap();
    assert!(!path.join(PENDING).exists());
    drop(cache);
}
