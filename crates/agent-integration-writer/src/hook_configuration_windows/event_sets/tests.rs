mod grouped;

use super::super::{HookConfigurationKind, HookConfigurationScan, scan_hook_configuration};
use super::*;
use crate::hook_configuration::Object;
use serde_json::{Value, json};

fn patch(set: EventSet, bytes: Option<&[u8]>, root: &Path, enable: bool) -> Option<Vec<u8>> {
    super::patch(set, bytes, root, enable, super::super::ROOT_POLICY)
}

fn scan(set: EventSet, bytes: &[u8]) -> HookConfigurationScan {
    let kind = match set {
        EventSet::Mastracode => HookConfigurationKind::Mastracode,
        EventSet::Devin => HookConfigurationKind::Devin,
    };
    scan_hook_configuration(kind, bytes)
}

#[test]
fn complete_partial_conflicting_and_foreign_flat_hooks() {
    let root = Path::new(r"C:\owned\0.4.17");
    let input=br#"{"future":{"huge":1e999},"SessionStart":[{"type":"command","command":"echo foreign"}]}"#;
    let installed = patch(EventSet::Mastracode, Some(input), root, true).unwrap();
    assert!(
        matches!(scan(EventSet::Mastracode, &installed),HookConfigurationScan::Owned(found) if found==root)
    );
    assert_eq!(
        installed,
        patch(EventSet::Mastracode, Some(&installed), root, true).unwrap()
    );
    let removed = patch(EventSet::Mastracode, Some(&installed), root, false).unwrap();
    let original: Object = serde_json::from_slice(input).unwrap();
    let restored: Object = serde_json::from_slice(&removed).unwrap();
    assert_eq!(original.0["future"].get(), restored.0["future"].get());
    assert_eq!(
        original.0["SessionStart"].get(),
        restored.0["SessionStart"].get()
    );
    assert!(matches!(
        scan(EventSet::Mastracode, &removed),
        HookConfigurationScan::Absent
    ));
    let mut doc: Value =
        serde_json::from_slice(&patch(EventSet::Mastracode, None, root, true).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("AgentEnd");
    let partial = serde_json::to_vec(&doc).unwrap();
    assert!(
        matches!(scan(EventSet::Mastracode, &partial),HookConfigurationScan::Incomplete(found) if found==root)
    );
    assert!(matches!(
        scan(
            EventSet::Mastracode,
            &patch(EventSet::Mastracode, Some(&partial), root, true).unwrap()
        ),
        HookConfigurationScan::Owned(_)
    ));
    let entry = declaration("mastracode", root).unwrap();
    assert_eq!(entry["timeout"], 10000);
    for wrong in [
        json!({"SessionStart":[entry,entry]}),
        json!({"Stop":[entry]}),
        json!({"SessionStart":[entry],"AgentStart":[declaration("mastracode",Path::new(r"C:\other\9.0.0")).unwrap()]}),
    ] {
        assert!(matches!(
            scan(EventSet::Mastracode, &serde_json::to_vec(&wrong).unwrap()),
            HookConfigurationScan::Conflict
        ));
    }
    doc["SessionStart"][0]["timeout"] = json!(10);
    assert!(matches!(
        scan(EventSet::Mastracode, &serde_json::to_vec(&doc).unwrap()),
        HookConfigurationScan::Conflict
    ));
    for bad in [
        br#"{"AgentStart":[],"AgentStart":[]}"#.as_slice(),
        br#"{"AgentEnd":null}"#,
    ] {
        assert!(matches!(
            scan(EventSet::Mastracode, bad),
            HookConfigurationScan::Malformed
        ));
    }
}
