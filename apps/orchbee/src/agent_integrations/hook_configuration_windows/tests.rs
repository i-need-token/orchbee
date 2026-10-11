use super::*;
#[test]
fn letta_owns_a_silent_millisecond_hook_and_preserves_policy_and_foreign_values() {
    let root = Path::new(r"C:\owned\0.4.17");
    let input = br#"{"large":123456789012345678901234567890,"hooks":{"disabled":true,"future":{"number":1e999},"SessionStart":[{"hooks":[{"type":"prompt","prompt":"USER"}]}]}}"#;
    let installed = patch(HookConfigurationKind::Letta, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(HookConfigurationKind::Letta, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    let doc: Object = serde_json::from_slice(&installed).unwrap();
    let hooks: Object = serde_json::from_str(doc.0["hooks"].get()).unwrap();
    assert_eq!(hooks.0["disabled"].get(), "true");
    assert_eq!(hooks.0["future"].get(), r#"{"number":1e999}"#);
    let entries: Value = serde_json::from_str(hooks.0["SessionStart"].get()).unwrap();
    let entry = &entries[1]["hooks"][0];
    assert_eq!(entry["timeout"], 10000);
    assert_eq!(entry["quiet"], true);
    let command = entry["command"].as_str().unwrap();
    let bytes = STANDARD
        .decode(command.strip_prefix(PREFIX).unwrap())
        .unwrap();
    let words: Vec<_> = bytes
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .collect();
    assert!(!String::from_utf16(&words).unwrap().contains("Out.Write"));
    assert_eq!(
        patch(HookConfigurationKind::Letta, Some(&installed), root, true).unwrap(),
        installed
    );
    let disabled = patch(HookConfigurationKind::Letta, Some(&installed), root, false).unwrap();
    let disabled: Object = serde_json::from_slice(&disabled).unwrap();
    let original: Object = serde_json::from_slice(input).unwrap();
    assert_eq!(disabled.0["large"].get(), original.0["large"].get());
    let restored: Object = serde_json::from_str(disabled.0["hooks"].get()).unwrap();
    let original: Object = serde_json::from_str(original.0["hooks"].get()).unwrap();
    for key in ["disabled", "future", "SessionStart"] {
        assert_eq!(restored.0[key].get(), original.0[key].get());
    }
    let changed = String::from_utf8(installed)
        .unwrap()
        .replace("\"quiet\":true", "\"quiet\":false");
    assert!(matches!(
        scan_hook_configuration(HookConfigurationKind::Letta, changed.as_bytes()),
        HookConfigurationScan::Conflict
    ));
}
#[test]
fn raw_user_values_survive_enable_repair_and_disable() {
    let input = br#"{"large":123456789012345678901234567890,"nested":{"future":1e999,"duplicate":1,"duplicate":2},"hooks":{"stop":[{"command":"echo user"}],"sessionStart":[{"command":"echo before"}]}}"#;
    let root = Path::new(r"C:\Users\用户 & ' $ `\.orchbee\agent-observation-adapters\0.4.17");
    let installed = patch(HookConfigurationKind::Cursor, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(HookConfigurationKind::Cursor, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    let repaired = patch(HookConfigurationKind::Cursor, Some(&installed), root, true).unwrap();
    assert_eq!(repaired, installed);
    let disabled = patch(HookConfigurationKind::Cursor, Some(&installed), root, false).unwrap();
    let document: Object = serde_json::from_slice(&disabled).unwrap();
    let original: Object = serde_json::from_slice(input).unwrap();
    for key in ["large", "nested"] {
        assert_eq!(document.0[key].get(), original.0[key].get());
    }
    let hooks: Value = serde_json::from_str(document.0["hooks"].get()).unwrap();
    assert_eq!(hooks["sessionStart"], json!([{"command":"echo before"}]));
    assert_eq!(hooks["stop"], json!([{"command":"echo user"}]));
    assert!(matches!(
        scan_hook_configuration(HookConfigurationKind::Cursor, &disabled),
        HookConfigurationScan::Absent
    ));
}
#[test]
fn duplicated_edited_members_and_changed_owned_commands_are_refused() {
    for input in [
        r#"{"hooks":{},"hooks":{}}"#,
        r#"{"hooks":{"sessionStart":[],"sessionStart":[]}}"#,
        r#"{"version":2,"hooks":{}}"#,
        r#"{"hooks":{"sessionStart":null}}"#,
    ] {
        assert!(matches!(
            scan_hook_configuration(HookConfigurationKind::Cursor, input.as_bytes()),
            HookConfigurationScan::Malformed
        ));
    }
    let root = Path::new(r"C:\owned\0.4.17");
    let bytes = patch(HookConfigurationKind::Cursor, None, root, true).unwrap();
    let mut doc: Value = serde_json::from_slice(&bytes).unwrap();
    doc["hooks"]["sessionStart"][0]["timeout"] = json!(99);
    assert!(matches!(
        scan_hook_configuration(
            HookConfigurationKind::Cursor,
            &serde_json::to_vec(&doc).unwrap()
        ),
        HookConfigurationScan::Conflict
    ));
    let entry = declaration("cursor", root).unwrap();
    for hooks in [
        json!({"stop":[entry]}),
        json!({"sessionStart":[entry,entry]}),
    ] {
        assert!(matches!(
            scan_hook_configuration(
                HookConfigurationKind::Cursor,
                &serde_json::to_vec(&json!({"hooks":hooks})).unwrap()
            ),
            HookConfigurationScan::Conflict
        ));
    }
}
