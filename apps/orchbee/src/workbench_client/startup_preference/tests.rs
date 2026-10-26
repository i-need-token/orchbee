use super::*;
use std::{fs, path::PathBuf};
fn position(context: u32) -> Position {
    Position {
        location: {
            let fixture_context_id = orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                &format!("{context:032x}"),
            )
            .unwrap();
            ClientContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    fixture_context_id.as_str(),
                )
                .unwrap(),
                terminal_context_id: fixture_context_id,
            }
        },
        member: None,
    }
}
#[test]
fn startup_preference_codec_rejects_future_incomplete_unknown_and_oversized_records() {
    let position = position(1);
    let bytes = serde_json::to_vec(&Record {
        schema: Schema::Position,
        version: 1,
        position: position.clone(),
    })
    .unwrap();
    assert_eq!(decode(&bytes).unwrap(), position);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["version"] = 3.into();
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["version"] = 1.into();
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
fn current_position_rejects_historical_shape_and_requires_member() {
    let expected = Position {
        location: ClientContextLocation::Workspace {
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
    assert!(decode(HISTORICAL_WORKSPACE).is_err());
    let bytes = serde_json::to_vec(&Record {
        schema: Schema::Position,
        version: 1,
        position: expected.clone(),
    })
    .unwrap();
    assert_eq!(decode(&bytes).unwrap(), expected);
    let mut missing: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    missing["position"]
        .as_object_mut()
        .unwrap()
        .remove("member");
    assert!(decode(&serde_json::to_vec(&missing).unwrap()).is_err());
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
    let lock = orchbee_local_runtime::PrivateCache::acquire(&path).unwrap();
    let mut writer = Writer::start(path.clone()).unwrap();
    writer.publish(position(1));
    writer.publish(position(2));
    writer.publish(position(2));
    drop(lock);
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
