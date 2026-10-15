use super::*;
use orchbee_workbench_protocol::WorkbenchSplitRatio;

#[test]
fn checkpoint_v5_retains_ratios_without_runtime_bindings_and_recaptures_them() {
    let bytes = changed(|value| {
        historical_navigation(value, 5);
        value["terminalContexts"][1]["tabs"][0]["root"]["ratio"] = json!(0.7);
    });
    let owner = restore(&bytes).unwrap();
    let PanelNode::Split { ratio, .. } = &owner.snapshot().terminal_contexts[1].tabs[0].root else {
        panic!("split");
    };
    assert_eq!(*ratio, WorkbenchSplitRatio::new(0.7).unwrap());
    let encoded = owner.checkpoint(&worker(400)).unwrap();
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["version"], json!(7));
    assert_eq!(
        value["terminalContexts"][1]["tabs"][0]["root"]["ratio"],
        json!(0.7)
    );
    assert_eq!(restore(&encoded).unwrap().snapshot(), owner.snapshot());
}

#[test]
fn legacy_versions_restore_equal_ratios_and_refuse_nondefault_weight_injection() {
    for version in 1..=4 {
        let legacy = changed(|value| {
            historical_navigation(value, version);
            if version < 2 {
                value.as_object_mut().unwrap().remove("agentSessions");
            }
            if version < 3 {
                value.as_object_mut().unwrap().remove("terminalDirectories");
            }
            if version < 4 {
                value.as_object_mut().unwrap().remove("managedNames");
            }
        });
        let restored = restore(&legacy).unwrap();
        let PanelNode::Split { ratio, .. } = &restored.snapshot().terminal_contexts[1].tabs[0].root
        else {
            panic!("split");
        };
        assert_eq!(*ratio, WorkbenchSplitRatio::HALF);
        let mut injected: Value = serde_json::from_slice(&legacy).unwrap();
        injected["terminalContexts"][1]["tabs"][0]["root"]["ratio"] = json!(0.7);
        assert_eq!(
            restore(&serde_json::to_vec(&injected).unwrap()).err(),
            Some(WorkbenchCheckpointError::InvalidSplitRatio),
            "version {version}"
        );
    }
}
