use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

const PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";

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

#[test]
fn qoder_groups_keep_foreign_raw_values_and_refuse_edited_or_misplaced_ownership() {
    let root = Path::new(r"C:\owned\0.4.17");
    let input = br#"{"version":1e999,"hooks":{"SessionStart":[{"matcher":"resume","hooks":[{"type":"prompt","prompt":"keep","future":123456789012345678901234567890}]}]}}"#;
    let installed = patch(HookConfigurationKind::Qoder, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(HookConfigurationKind::Qoder, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    let text = std::str::from_utf8(&installed).unwrap();
    assert!(text.contains("1e999") && text.contains("123456789012345678901234567890"));
    assert_eq!(
        patch(HookConfigurationKind::Qoder, Some(&installed), root, true).unwrap(),
        installed
    );
    let disabled = patch(HookConfigurationKind::Qoder, Some(&installed), root, false).unwrap();
    assert!(matches!(
        scan_hook_configuration(HookConfigurationKind::Qoder, &disabled),
        HookConfigurationScan::Absent
    ));
    assert!(std::str::from_utf8(&disabled).unwrap().contains("1e999"));
    assert!(
        !std::str::from_utf8(&disabled)
            .unwrap()
            .contains("EncodedCommand")
    );
    let entry = declaration("qodercli", root).unwrap();
    let mut edited = entry.clone();
    edited["shell"] = json!("bash");
    for hooks in [
        json!({"Stop":[{"hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"hooks":[entry.clone()]},{"hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"matcher":"resume","hooks":[entry.clone()]}]}),
        json!({"SessionStart":[{"hooks":[entry.clone(),{"command":"echo keep"}]}]}),
        json!({"SessionStart":[{"hooks":[edited]}]}),
    ] {
        let bytes = serde_json::to_vec(&json!({"hooks":hooks})).unwrap();
        assert!(matches!(
            scan_hook_configuration(HookConfigurationKind::Qoder, &bytes),
            HookConfigurationScan::Conflict
        ));
        assert!(patch(HookConfigurationKind::Qoder, Some(&bytes), root, false).is_none());
    }
    for input in [
        r#"{"hooks":{"SessionStart":[],"SessionStart":[]}}"#,
        r#"{"hooks":{"SessionStart":[{"hooks":[],"hooks":[]}]}}"#,
        r#"{"hooks":{"SessionStart":false}}"#,
    ] {
        assert!(
            patch(
                HookConfigurationKind::Qoder,
                Some(input.as_bytes()),
                root,
                true
            )
            .is_none()
        );
    }
}

#[test]
fn antigravity_named_blocks_preserve_foreign_values_and_validate_disabled_ownership() {
    let id = HookConfigurationKind::Antigravity;
    let root = Path::new(r"C:\owned\0.4.17");
    let options = ResourceOptions {
        resource_root: root,
    };
    for (version, expected) in [
        ("0.4.17", State::Disabled),
        ("0.4.9", State::UpdateRequired),
        ("9.0.0", State::UpdateRequired),
    ] {
        let bytes = patch(id, None, &root.with_file_name(version), true).unwrap();
        let text = String::from_utf8(bytes)
            .unwrap()
            .replace("\"enabled\":true", "\"enabled\":false");
        assert_eq!(state(id, Some(text.as_bytes()), &options), expected);
    }
    let input = br#"{"user-hook":{"future":1e999,"enabled":false,"PreInvocation":[{"command":"echo user"}]}}"#;
    let installed = patch(id, Some(input), root, true).unwrap();
    assert!(
        matches!(scan_hook_configuration(id, &installed), HookConfigurationScan::Owned(found) if found == root)
    );
    assert!(std::str::from_utf8(&installed).unwrap().contains("1e999"));
    assert_eq!(patch(id, Some(&installed), root, false).unwrap(), input);
    let owned = patch(id, None, root, true).unwrap();
    let mut value: Value = serde_json::from_slice(&owned).unwrap();
    value["orchbee-agent-observation"]["enabled"] = json!(false);
    assert!(
        matches!(scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()), HookConfigurationScan::Disabled(found) if found == root)
    );
    value["orchbee-agent-observation"]["enabled"] = json!(null);
    assert!(matches!(
        scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()),
        HookConfigurationScan::Malformed
    ));
    value["orchbee-agent-observation"]["enabled"] = json!(true);
    value["orchbee-agent-observation"]["PreInvocation"][0]["timeout"] = json!(99);
    assert!(matches!(
        scan_hook_configuration(id, &serde_json::to_vec(&value).unwrap()),
        HookConfigurationScan::Conflict
    ));
    for input in [
        r#"{"orchbee-agent-observation":{}}"#,
        r#"{"orchbee-agent-observation":{"PreInvocation":[{"command":"echo foreign"}]}}"#,
        r#"{"orchbee-agent-observation":{},"orchbee-agent-observation":{}}"#,
    ] {
        assert!(patch(id, Some(input.as_bytes()), root, true).is_none());
    }
}

#[test]
fn droid_document_shapes_preserve_foreign_data_and_own_only_session_group() {
    let root = Path::new(r"C:\owned\0.4.17");
    for (kind,input) in [
        (HookConfigurationKind::DroidStandalone,br#"{"SessionStart":[{"matcher":"resume","hooks":[{"type":"command","command":"echo user"}]}],"future":{"big":1e999}}"#.as_slice()),
        (HookConfigurationKind::DroidSettings,br#"{"hooks":{"SessionStart":[{"matcher":"resume","hooks":[{"type":"command","command":"echo user"}]}]},"future":{"big":1e999}}"#.as_slice()),
    ] {
        let installed=patch(kind,Some(input),root,true).unwrap();
        assert!(matches!(scan_hook_configuration(kind,&installed),HookConfigurationScan::Owned(found) if found==root));
        assert_eq!(installed,patch(kind,Some(&installed),root,true).unwrap());
        let disabled=patch(kind,Some(&installed),root,false).unwrap();
        let original:Object=serde_json::from_slice(input).unwrap();
        let restored:Object=serde_json::from_slice(&disabled).unwrap();
        assert_eq!(restored.0["future"].get(),original.0["future"].get());
        assert!(matches!(scan_hook_configuration(kind,&disabled),HookConfigurationScan::Absent));
        let mut owned:Value=serde_json::from_slice(&patch(kind,None,root,true).unwrap()).unwrap();
        let hooks=if kind==HookConfigurationKind::DroidSettings {&mut owned["hooks"]} else {&mut owned};
        let entry=hooks["SessionStart"][0].clone();
        hooks["Stop"]=json!([entry]);
        assert!(matches!(scan_hook_configuration(kind,&serde_json::to_vec(&owned).unwrap()),HookConfigurationScan::Conflict));
    }
}
