//! Windows private-cache locking and persistence witness for the joined position writer.
use super::{Preferences, Writer};
use crate::startup_preference::tests::position;
use crate::startup_preference::{FILE, MAXIMUM_BYTES, RECORD, decode};
use std::{fs, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            Self(std::env::temp_dir().join(format!("orchbee-startup-preference-{nonce:032x}")));
        fs::create_dir(&root.0).unwrap();
        root
    }
}
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
                .starts_with("orchbee-startup-preference-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}
#[test]
fn startup_preference_writer_flushes_latest_complete_record_before_join() {
    let root = Root::new();
    let path = root.0.join("position");
    let mut writer = {
        let _lock = orchbee_local_runtime::PrivateCache::acquire(&path, RECORD).unwrap();
        let mut writer = Writer::start(path.clone()).unwrap();
        writer.publish(position(1));
        writer.publish(position(2));
        writer.publish(position(2));
        writer
    };
    writer
        .stop_before(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .unwrap();
    let raw = orchbee_local_runtime::PrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&raw).unwrap(), position(2));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap()["version"],
        1
    );
    assert!(!path.join(format!("{RECORD}.pending")).exists());
}
#[test]
fn unreadable_record_counts_as_unsaved_and_the_next_position_replaces_it() {
    // A record from an earlier format once disabled every later write for that runtime.
    let root = Root::new();
    let path = root.0.join("position");
    orchbee_local_runtime::PrivateCache::acquire(&path, RECORD)
        .unwrap()
        .replace(FILE, br#"{"position":null}"#)
        .unwrap();
    let preferences = Preferences::load_from(path.clone());
    assert!(preferences.position.is_none() && preferences.unreadable && !preferences.failed);
    let mut writer = preferences.writer().unwrap().unwrap();
    writer.publish(position(3));
    writer
        .stop_before(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .unwrap();
    let reloaded = Preferences::load_from(path);
    assert_eq!(reloaded.position, Some(position(3)));
    assert!(!reloaded.unreadable && !reloaded.failed);
}
#[test]
fn a_record_a_newer_version_saved_counts_as_unsaved_and_is_never_rewritten() {
    let root = Root::new();
    let path = root.0.join("position");
    let newer = br#"{"version":2,"position":null,"later":true}"#;
    orchbee_local_runtime::PrivateCache::acquire(&path, RECORD)
        .unwrap()
        .replace(FILE, newer)
        .unwrap();
    let preferences = Preferences::load_from(path.clone());
    assert!(preferences.position.is_none() && !preferences.failed);
    assert!(preferences.writer().unwrap().is_none());
    assert_eq!(std::fs::read(path.join(FILE)).unwrap(), newer);
}
