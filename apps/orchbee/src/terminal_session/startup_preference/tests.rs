use super::*;
use std::{fs, path::PathBuf};
fn position(context: u32) -> Position {
    Position {
        location: NativeContextLocation::Standalone(
            orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!(
                "{context:032x}"
            ))
            .unwrap(),
        ),
        member: None,
    }
}
#[test]
fn startup_preference_codec_rejects_future_incomplete_unknown_and_oversized_records() {
    let position = position(1);
    let bytes = serde_json::to_vec(&Record {
        version: 2,
        position: position.clone(),
    })
    .unwrap();
    assert_eq!(decode(&bytes).unwrap(), position);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["version"] = 3.into();
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["version"] = 2.into();
    value["binding"] = "foreign authority".into();
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    assert!(decode(br#"{"version":1}"#).is_err());
    assert!(decode(&vec![b' '; MAXIMUM_BYTES + 1]).is_err());
}

// Historical bytes deliberately retain the old required Project identity.
const HISTORICAL_WORKSPACE: &[u8] = br#"{
        "version": 1,
        "position": {
            "location": {
                "kind": "Workspace",
                "identity": {
                    "project_id": "00000000000000000000000000000001",
                    "workspace_id": "00000000000000000000000000000002",
                    "terminal_context_id": "00000000000000000000000000000003"
                }
            },
            "member": null
        }
    }"#;

#[test]
fn historical_workspace_requires_valid_project_but_current_position_rejects_it() {
    let expected = Position {
        location: NativeContextLocation::Workspace {
            workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                "00000000000000000000000000000002",
            )
            .unwrap(),
            terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                "00000000000000000000000000000003",
            )
            .unwrap(),
        },
        member: None,
    };
    assert_eq!(decode(HISTORICAL_WORKSPACE).unwrap(), expected);
    for invalid in [None, Some(serde_json::Value::Null), Some("invalid".into())] {
        let mut value: serde_json::Value = serde_json::from_slice(HISTORICAL_WORKSPACE).unwrap();
        let identity = value["position"]["location"]["identity"]
            .as_object_mut()
            .unwrap();
        match invalid {
            None => {
                identity.remove("project_id");
            }
            Some(value) => {
                identity.insert("project_id".into(), value);
            }
        }
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut current: serde_json::Value = serde_json::from_slice(HISTORICAL_WORKSPACE).unwrap();
    current["version"] = 2.into();
    assert!(decode(&serde_json::to_vec(&current).unwrap()).is_err());
    current["position"]["location"]["identity"]
        .as_object_mut()
        .unwrap()
        .remove("project_id");
    assert_eq!(
        decode(&serde_json::to_vec(&current).unwrap()).unwrap(),
        expected
    );
}
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
    let lock = orchbee_local_site_runtime::WindowsPrivateCache::acquire(&path).unwrap();
    let mut writer = Writer::start(path.clone()).unwrap();
    writer.publish(position(1));
    writer.publish(position(2));
    writer.publish(position(2));
    drop(lock);
    drop(writer);
    let raw =
        orchbee_local_site_runtime::WindowsPrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
            .unwrap()
            .unwrap();
    assert_eq!(decode(&raw).unwrap(), position(2));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&raw).unwrap()["version"],
        2
    );
    assert!(!path.join(".orchbee-update.pending").exists());
}

#[test]
fn historical_workspace_is_rewritten_as_v2_with_the_same_stable_selection() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = Root(std::env::temp_dir().join(format!("orchbee-startup-preference-{nonce:032x}")));
    fs::create_dir(&root.0).unwrap();
    let path = root.0.join("position");
    let mut cache = orchbee_local_site_runtime::WindowsPrivateCache::acquire(&path).unwrap();
    cache.replace(FILE, HISTORICAL_WORKSPACE).unwrap();
    drop(cache);
    let old =
        orchbee_local_site_runtime::WindowsPrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
            .unwrap()
            .unwrap();
    assert_eq!(old, HISTORICAL_WORKSPACE);
    let migrated = decode(&old).unwrap();
    let mut writer = Writer::start(path.clone()).unwrap();
    writer.publish(migrated.clone());
    drop(writer);
    let raw =
        orchbee_local_site_runtime::WindowsPrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
            .unwrap()
            .unwrap();
    let record: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(record["version"], 2);
    assert!(
        record["position"]["location"]["identity"]
            .get("project_id")
            .is_none()
    );
    assert_eq!(decode(&raw).unwrap(), migrated);
    assert!(!path.join(".orchbee-update.pending").exists());
}
