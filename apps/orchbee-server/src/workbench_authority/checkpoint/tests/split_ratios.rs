use super::*;
use orchbee_workbench_protocol::WorkbenchSplitRatio;

#[test]
fn checkpoint_retains_ratios_without_runtime_bindings_and_recaptures_them() {
    let bytes = changed(|value| {
        value["terminalContexts"][0]["tabs"][0]["root"]["ratio"] = json!(0.7);
    });
    let owner = restore(&bytes).unwrap();
    let PanelNode::Split { ratio, .. } = &owner.snapshot().terminal_contexts[0].tabs[0].root else {
        panic!("split");
    };
    assert_eq!(*ratio, WorkbenchSplitRatio::new(0.7).unwrap());
    let encoded = owner.checkpoint(&worker(400)).unwrap();
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["version"], json!(1));
    assert_eq!(
        value["terminalContexts"][0]["tabs"][0]["root"]["ratio"],
        json!(0.7)
    );
    assert_eq!(restore(&encoded).unwrap().snapshot(), owner.snapshot());
}
