use super::*;
#[test]
fn exact_members_partial_repair_versions_and_provider_rows() {
    let current = Path::new(r"C:\owned\0.4.17");
    let old = Path::new(r"C:\owned\0.4.2");
    let newer = Path::new(r"C:\owned\9.0.0");
    let options = ResourceOptions {
        resource_root: current,
        expected_version: "0.4.17",
        legacy_roots: &[],
    };
    let init = member_bytes(Member::Entry, current).unwrap();
    let manifest = member_bytes(Member::Manifest, current).unwrap();
    assert_eq!(local_state(None, None, &options), State::NotInstalled);
    assert_eq!(
        local_state(Some(&init), None, &options),
        State::UpdateRequired
    );
    assert_eq!(
        local_state(Some(&init), Some(&manifest), &options),
        State::Enabled
    );
    assert_eq!(
        local_state(
            Some(&member_bytes(Member::Entry, old).unwrap()),
            Some(&manifest),
            &options
        ),
        State::UpdateRequired
    );
    assert_eq!(
        local_state(
            Some(&member_bytes(Member::Entry, newer).unwrap()),
            Some(&manifest),
            &options
        ),
        State::StaleSide
    );
    assert_eq!(
        local_state(Some(b"# edited"), Some(&manifest), &options),
        State::Conflict
    );
    let mut changed = manifest.clone();
    changed.push(b' ');
    assert_eq!(
        local_state(Some(&init), Some(&changed), &options),
        State::Conflict
    );
    let row = |status: &str| serde_json::json!({"name":NAME,"version":"0.4.17","status":status,"source":"user","removed":null});
    for (status, expected) in [
        ("enabled", State::Enabled),
        ("disabled", State::Disabled),
        ("not enabled", State::Disabled),
    ] {
        assert_eq!(
            list_state(
                &serde_json::to_vec(&vec![row(status)]).unwrap(),
                State::Enabled,
                "0.4.17"
            ),
            expected
        );
    }
    assert_eq!(
        list_state(b"[]", State::NotInstalled, "0.4.17"),
        State::NotInstalled
    );
    assert_eq!(list_state(b"[]", State::Enabled, "0.4.17"), State::Unknown);
    let mut mismatched = row("enabled");
    mismatched["version"] = json!("wrong");
    assert_eq!(
        list_state(
            &serde_json::to_vec(&[mismatched]).unwrap(),
            State::Enabled,
            "0.4.17"
        ),
        State::Conflict
    );
    assert!(activation_disabled(
        &serde_json::to_vec(&[row("disabled")]).unwrap()
    ));
    assert!(!activation_disabled(
        &serde_json::to_vec(&[row("enabled")]).unwrap()
    ));
    assert!(!activation_disabled(b"[]"));

    assert_eq!(
        list_state(
            &serde_json::to_vec(&vec![row("enabled"), row("disabled")]).unwrap(),
            State::Enabled,
            "0.4.17"
        ),
        State::Conflict
    );
    assert_eq!(
        list_state(
            &serde_json::to_vec(&vec![row("enabled")]).unwrap(),
            State::NotInstalled,
            "0.4.17"
        ),
        State::Conflict
    );
    assert_eq!(
        list_state(b"not-json", State::Enabled, "0.4.17"),
        State::Unknown
    );
}
