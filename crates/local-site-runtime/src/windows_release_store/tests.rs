//! Immutable publication, writer exclusion and disposal of unpublished files.
use super::*;
use std::io::Write;

#[test]
fn release_paths_reject_windows_aliases_and_escape() {
    for value in [
        "../x", "/x", "x\\y", "C:/x", "x:stream", "nul.exe", "a/COM1", "x.", "a//b", ".", "a/../b",
    ] {
        assert!(!valid_windows_release_path(value), "{value}");
    }
    assert!(valid_windows_release_path(
        "agent-observation-adapters/bundle/package.json"
    ));
}

#[test]
fn release_publication_is_exclusive_immutable_and_cleans_staging() {
    let root = std::env::temp_dir().join(format!(
        "orchbee-release-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let identity = "a".repeat(64);
    {
        let store = WindowsReleaseStore::acquire(&root).unwrap();
        assert!(WindowsReleaseStore::acquire(&root).is_err());
        let mut stage = store.stage().unwrap();
        assert!(stage.create_file("../other").is_err());
        let (_, mut file) = stage.create_file("bin/tool.exe").unwrap();
        file.write_all(b"complete").unwrap();
        file.sync_all().unwrap();
        drop(file);
        let target = stage.publish(&identity).unwrap();
        assert_eq!(fs::read(target.join("bin/tool.exe")).unwrap(), b"complete");
        stage.cleanup().unwrap();
        assert!(target.is_dir());
        let mut second = store.stage().unwrap();
        let pending = second.path().to_owned();
        assert!(second.publish(&identity).is_err());
        second.cleanup().unwrap();
        assert!(!pending.exists());
        assert_eq!(fs::read(target.join("bin/tool.exe")).unwrap(), b"complete");
    }
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
