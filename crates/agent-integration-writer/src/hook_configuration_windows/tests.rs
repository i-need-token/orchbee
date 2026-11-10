use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;

const PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";

/// Patches as an environment whose current Capsule is `root`.
fn patch(
    kind: HookConfigurationKind,
    bytes: Option<&[u8]>,
    root: &Path,
    enable: bool,
) -> Option<Vec<u8>> {
    super::patch(kind, bytes, &options(root), enable)
}

fn options(root: &Path) -> ResourceOptions<'_> {
    ResourceOptions {
        resource_root: root,
        legacy_capsules: None,
    }
}

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
/// Entries of several environments share one document: each environment changes only the entries
/// naming its own Capsules, and a provider with one named block refuses another environment's.
#[test]
fn entries_of_other_environments_stay_as_they_are() {
    let own = Path::new(
        r"C:\Users\u\.ob-dev-0123456789abcdef\internal\integrations\agent-observation\0.4.17",
    );
    let release = Path::new(r"C:\Users\u\.orchbee\internal\integrations\agent-observation\0.4.17");
    let shared = Path::new(r"C:\Users\u\.orchbee\agent-observation-adapters\0.4.16");
    for kind in [HookConfigurationKind::Cursor, HookConfigurationKind::Kimi] {
        for other in [release, shared] {
            let theirs = patch(kind, None, other, true).unwrap();
            let both = patch(kind, Some(&theirs), own, true).unwrap();
            let mine = with_policy(&options(own), |policy| {
                grammar::scan_hook_configuration(kind, &both, policy)
            });
            assert!(
                matches!(mine, HookConfigurationScan::Owned(found) if found == own),
                "{kind:?}"
            );
            assert_eq!(
                patch(kind, Some(&both), own, false).unwrap(),
                theirs,
                "{kind:?}"
            );
        }
        // The shared folder's entries are release's, so release replaces them.
        let theirs = patch(kind, None, shared, true).unwrap();
        let release_options = ResourceOptions {
            resource_root: release,
            legacy_capsules: shared.parent(),
        };
        let replaced = super::patch(kind, Some(&theirs), &release_options, true).unwrap();
        assert!(
            matches!(scan_hook_configuration(kind, &replaced), HookConfigurationScan::Owned(found) if found == release),
            "{kind:?}"
        );
    }
    let theirs = patch(HookConfigurationKind::Antigravity, None, release, true).unwrap();
    for enable in [true, false] {
        assert!(
            patch(
                HookConfigurationKind::Antigravity,
                Some(&theirs),
                own,
                enable
            )
            .is_none()
        );
    }
    assert!(matches!(
        state(
            HookConfigurationKind::Antigravity,
            Some(&theirs),
            &options(own),
        ),
        State::Conflict
    ));
}

/// A user document in each JSON shape, formatted by hand and holding a user hook where the
/// integration adds its own.
fn user_document(kind: HookConfigurationKind) -> String {
    use HookConfigurationKind as Kind;
    if kind == Kind::Antigravity {
        return "{\r\n  \"user-hook\" : { \"enabled\": false, \"PreInvocation\": [ { \"command\": \"echo user\" } ] },\r\n  \"zeta\": 1e999\r\n}\r\n".into();
    }
    let event = if kind == Kind::Cursor {
        "sessionStart"
    } else {
        "SessionStart"
    };
    let command = r#"{ "type": "command", "command": "echo user" }"#;
    let entry = if matches!(kind, Kind::Cursor | Kind::Mastracode) {
        command.to_owned()
    } else {
        format!(r#"{{ "hooks": [ {command} ] }}"#)
    };
    let events = format!("\"{event}\": [\r\n\t\t{entry}\r\n\t]");
    if matches!(kind, Kind::Mastracode | Kind::DroidStandalone) {
        format!("{{\r\n\t\"version\": 1,\r\n\t{events},\r\n\t\"zeta\": 1e999\r\n}}\r\n")
    } else {
        format!(
            "{{\r\n\t\"version\": 1,\r\n\t\"hooks\": {{\r\n\t{events},\r\n\t\"Custom\": []\r\n\t}},\r\n\t\"zeta\": 1e999\r\n}}\r\n"
        )
    }
}

/// Enable only adds the integration's entries and disable takes exactly them away, so the user's
/// bytes outside them survive: member order, formatting, numbers and the containers they wrote.
/// A container the integration created leaves with its entries, and repair replaces an older
/// entry where it stands.
#[test]
fn enable_repair_and_disable_keep_the_users_bytes() {
    use HookConfigurationKind as Kind;
    let old = Path::new(r"C:\owned\0.4.16");
    let root = Path::new(r"C:\owned\0.4.17");
    for kind in [
        Kind::Cursor,
        Kind::Qoder,
        Kind::Antigravity,
        Kind::Mastracode,
        Kind::DroidStandalone,
        Kind::DroidSettings,
        Kind::Letta,
        Kind::Devin,
    ] {
        for document in [
            "{\n    \"zeta\": 123456789012345678901234567890,\n    \"version\": 1,\n    \"alpha\": {\"nested\": [1, 2], \"text\": \"\\u00e9\"}\n}\n".to_owned(),
            "{\"version\":1}".to_owned(),
            user_document(kind),
        ] {
            let installed = patch(kind, Some(document.as_bytes()), root, true).unwrap();
            assert!(
                matches!(scan_hook_configuration(kind, &installed), HookConfigurationScan::Owned(found) if found == root),
                "{kind:?} {document}"
            );
            assert_eq!(
                patch(kind, Some(&installed), root, true).unwrap(),
                installed
            );
            let removed = patch(kind, Some(&installed), root, false).unwrap();
            assert_eq!(String::from_utf8(removed).unwrap(), document, "{kind:?}");
            let outdated = patch(kind, Some(document.as_bytes()), old, true).unwrap();
            let repaired = patch(kind, Some(&outdated), root, true).unwrap();
            assert_eq!(repaired, installed, "{kind:?} {document}");
        }
    }
}

#[test]
fn raw_user_values_survive_enable_repair_and_disable() {
    let input = br#"{"large":123456789012345678901234567890,"nested":{"future":1e999,"duplicate":1,"duplicate":2},"hooks":{"stop":[{"command":"echo user"}],"sessionStart":[{"command":"echo before"}]}}"#;
    let root =
        Path::new(r"C:\Users\用户 & ' $ `\.orchbee\internal\integrations\agent-observation\0.4.17");
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
    let options = options(root);
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

/// Publication takes a splice only when the strict grammar accepts it and it holds exactly the
/// value the change intends; values compare by meaning, not spelling.
#[test]
fn publication_refuses_an_edit_that_misses_its_intent() {
    let text = r#"{"user":1.50,"hooks":{"sessionStart":[{"command":"x"}]}}"#;
    let events = Change::Array {
        elements: vec![(0, Change::Remove)],
        append: Vec::new(),
    };
    let hooks = Change::Object {
        members: vec![("sessionStart", events)],
        append: Vec::new(),
    };
    let change = Change::Object {
        members: vec![("hooks", hooks)],
        append: vec![("version", serde_json::json!(1))],
    };
    let publish =
        |edited: &str, accepts: bool| publishable(text, edited.to_owned(), &change, |_| accepts);
    let spliced = splice::apply(text, &change).unwrap();
    assert!(publish(&spliced, true).is_some());
    assert!(publish("{ \"version\": 1, \"user\": 1.5 }", true).is_some());
    assert!(publish(&spliced, false).is_none());
    for wrong in [
        r#"{"user":2,"version":1}"#,
        r#"{"version":1}"#,
        r#"{"user":1.50,"hooks":{},"version":1}"#,
        r#"{"user":1.50,"hooks":{"sessionStart":[{"command":"x"}]},"version":1}"#,
        r#"{"user":1.50,"version":"1"}"#,
        r#"{"user":1.50,"version":1,"version":1}"#,
    ] {
        assert!(publish(wrong, true).is_none(), "{wrong}");
    }
}
