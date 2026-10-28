use super::*;
use serde_json::{Value, json};
#[test]
fn devin_group_set_is_complete_repairable_and_preserves_foreign_settings() {
    let set = EventSet::Devin;
    let root = Path::new(r"C:\owned\0.4.17");
    let input = br#"{"user":123456789012345678901234567890,"hooks":{"future":{"number":1e999},"Stop":[{"matcher":"custom","hooks":[{"type":"prompt","prompt":"USER"}]}]}}"#;
    let installed = patch(set, Some(input), root, true).unwrap();
    assert!(matches!(scan(set,&installed),HookConfigurationScan::Owned(found) if found==root));
    assert_eq!(patch(set, Some(&installed), root, true).unwrap(), installed);
    let removed = patch(set, Some(&installed), root, false).unwrap();
    let original: Object = serde_json::from_slice(input).unwrap();
    let restored: Object = serde_json::from_slice(&removed).unwrap();
    assert_eq!(original.0["user"].get(), restored.0["user"].get());
    let original: Object = serde_json::from_str(original.0["hooks"].get()).unwrap();
    let restored: Object = serde_json::from_str(restored.0["hooks"].get()).unwrap();
    for key in ["future", "Stop"] {
        assert_eq!(original.0[key].get(), restored.0[key].get());
    }
    let mut doc: Value = serde_json::from_slice(&patch(set, None, root, true).unwrap()).unwrap();
    doc["hooks"].as_object_mut().unwrap().remove("PreToolUse");
    assert!(matches!(
        scan(set, &serde_json::to_vec(&doc).unwrap()),
        HookConfigurationScan::Incomplete(_)
    ));
    assert!(matches!(
        scan(
            set,
            &patch(set, Some(&serde_json::to_vec(&doc).unwrap()), root, true).unwrap()
        ),
        HookConfigurationScan::Owned(_)
    ));
    let entry = json!({"hooks":[declaration("devin",root).unwrap()]});
    for wrong in [
        json!({"SessionStart":[entry,entry]}),
        json!({"SessionEnd":[entry]}),
        json!({"SessionStart":[entry],"Stop":[{"hooks":[declaration("devin",Path::new(r"C:\other\0.4.17")).unwrap()]}]}),
    ] {
        assert!(matches!(
            scan(set, &serde_json::to_vec(&json!({"hooks":wrong})).unwrap()),
            HookConfigurationScan::Conflict
        ));
    }
    doc["hooks"]["Stop"][0]["matcher"] = json!("*");
    assert!(matches!(
        scan(set, &serde_json::to_vec(&doc).unwrap()),
        HookConfigurationScan::Conflict
    ));
}
