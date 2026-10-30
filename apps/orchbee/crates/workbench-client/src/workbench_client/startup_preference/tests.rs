use super::*;
pub(super) fn position(context: u32) -> Position {
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
