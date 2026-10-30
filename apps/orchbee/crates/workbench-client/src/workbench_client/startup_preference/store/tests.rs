//! Windows private-cache locking and persistence witness for the joined position writer.
use super::Writer;
use crate::workbench_client::startup_preference::{FILE, MAXIMUM_BYTES, decode, tests::position};
use std::{fs, path::PathBuf};

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
                .starts_with("orchbee-startup-preference-")
        );
        fs::remove_dir_all(actual).unwrap();
    }
}
#[test]
fn startup_preference_writer_flushes_latest_complete_record_before_join() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Root(std::env::temp_dir().join(format!("orchbee-startup-preference-{nonce:032x}")));
    fs::create_dir(&root.0).unwrap();
    let path = root.0.join("position");
    let writer = {
        let _lock = orchbee_local_runtime::PrivateCache::acquire(&path).unwrap();
        let mut writer = Writer::start(path.clone()).unwrap();
        writer.publish(position(1));
        writer.publish(position(2));
        writer.publish(position(2));
        writer
    };
    drop(writer);
    let raw = orchbee_local_runtime::PrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
        .unwrap()
        .unwrap();
    assert_eq!(decode(&raw).unwrap(), position(2));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap()["version"],
        1
    );
    assert!(!path.join(".orchbee-update.pending").exists());
}
