use super::*;

fn saved() -> Vec<u8> {
    changed(|value| {
        value["agentSessions"] = json!([
            {"presentationId":identity(11),"session":{"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"provider-reference"}}},
            {"presentationId":identity(15),"session":{"source":"herdr:grok","agent":"grok","reference":{"kind":"id","value":"child-reference"}}}
        ]);
        value["managedNames"] = json!([
            {"presentationId":identity(11),"name":"reviewer","providerId":"grok"},
            {"presentationId":identity(15),"name":"planner","providerId":"grok"}
        ]);
    })
}

#[test]
fn managed_checkpoint_v4_retains_only_logical_intent_and_requires_matching_references() {
    let bytes = saved();
    let admitted = WorkbenchCheckpoint::parse(&bytes).unwrap();
    let managed = admitted.managed_names().to_vec();
    assert_eq!(managed.len(), 2);
    let a = admitted.reconstruct(site(300), &worker(400)).unwrap();
    let before = a.snapshot().clone();
    let roundtrip = a
        .checkpoint_with_managed_agents(&worker(400), &[], &[], &managed)
        .unwrap();
    let parsed = WorkbenchCheckpoint::parse(&roundtrip).unwrap();
    assert_eq!(parsed.managed_names(), managed);
    assert_eq!(a.snapshot(), &before);
    let doc: Value = serde_json::from_slice(&roundtrip).unwrap();
    assert_eq!(doc["version"], 6);
    for row in doc["managedNames"].as_array().unwrap() {
        assert_eq!(row.as_object().unwrap().len(), 3);
    }
    for fault in 0..9 {
        let mut invalid: Value = serde_json::from_slice(&bytes).unwrap();
        match fault {
            0 => {
                invalid.as_object_mut().unwrap().remove("managedNames");
            }
            1 => invalid["managedNames"] = Value::Null,
            2 => invalid["managedNames"][1]["name"] = json!("reviewer"),
            3 => invalid["managedNames"][1]["presentationId"] = json!(identity(11)),
            4 => invalid["managedNames"][0]["presentationId"] = json!(identity(14)),
            5 => invalid["managedNames"][0]["providerId"] = json!("claude-code"),
            6 => invalid["managedNames"][0]["sessionId"] = json!("old-session"),
            7 => invalid["managedNames"][0]["name"] = json!("bad name"),
            _ => invalid["managedNames"] = json!(vec![invalid["managedNames"][0].clone(); 513]),
        }
        assert!(
            WorkbenchCheckpoint::parse(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "fault {fault}"
        );
    }
}

#[test]
fn old_checkpoints_remain_readable_but_cannot_smuggle_new_name_claims() {
    for version in 1..=3 {
        let mut document: Value = serde_json::from_slice(&saved()).unwrap();
        historical_navigation(&mut document, version);
        if version < 3 {
            document
                .as_object_mut()
                .unwrap()
                .remove("terminalDirectories");
        }
        if version < 2 {
            document.as_object_mut().unwrap().remove("agentSessions");
        }
        assert!(WorkbenchCheckpoint::parse(&serde_json::to_vec(&document).unwrap()).is_err());
        document.as_object_mut().unwrap().remove("managedNames");
        assert!(
            WorkbenchCheckpoint::parse(&serde_json::to_vec(&document).unwrap())
                .unwrap()
                .managed_names()
                .is_empty()
        );
    }
}
