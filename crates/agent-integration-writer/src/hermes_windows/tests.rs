use super::*;
/// Entry files Orchbee writes for another environment's Capsule are that environment's; an edited
/// byte, or a member of this environment's, proves no other owner.
#[test]
fn entry_files_of_another_environment_are_left_to_it() {
    let own = Path::new(
        r"C:\Users\u\.ob-test-0123456789abcdef\internal\integrations\agent-observation\0.4.17",
    );
    let theirs = Path::new(
        r"C:\Users\u\.ob-dev-fedcba9876543210\internal\integrations\agent-observation\0.4.17",
    );
    let options = ResourceOptions { resource_root: own };
    let entry = member_bytes(Member::Entry, theirs).unwrap();
    let manifest = member_bytes(Member::Manifest, theirs).unwrap();
    assert!(elsewhere(Some(&entry), Some(&manifest), &options));
    assert!(elsewhere(None, Some(&manifest), &options));
    assert_eq!(
        local_state(Some(&entry), Some(&manifest), &options),
        State::Conflict
    );
    assert!(!elsewhere(None, None, &options));
    let mine = member_bytes(Member::Entry, own).unwrap();
    assert!(!elsewhere(Some(&mine), Some(&manifest), &options));
    let mut edited = manifest.clone();
    edited.push(b' ');
    assert!(!elsewhere(Some(&entry), Some(&edited), &options));
}
#[test]
fn exact_members_partial_repair_versions_and_provider_rows() {
    let current =
        Path::new(r"C:\owned\aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let old =
        Path::new(r"C:\owned\bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let newer =
        Path::new(r"C:\owned\cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
    let options = ResourceOptions {
        resource_root: current,
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
        State::UpdateRequired
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
    let row = |status: &str| serde_json::json!({"name":NAME,"version":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","status":status,"source":"user","removed":null});
    for (status, expected) in [
        ("enabled", State::Enabled),
        ("disabled", State::Disabled),
        ("not enabled", State::Disabled),
    ] {
        assert_eq!(
            list_state(
                &serde_json::to_vec(&vec![row(status)]).unwrap(),
                State::Enabled,
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            expected
        );
    }
    assert_eq!(
        list_state(
            b"[]",
            State::NotInstalled,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        State::NotInstalled
    );
    assert_eq!(
        list_state(
            b"[]",
            State::Enabled,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        State::Unknown
    );
    let mut mismatched = row("enabled");
    mismatched["version"] = json!("wrong");
    assert_eq!(
        list_state(
            &serde_json::to_vec(&[mismatched]).unwrap(),
            State::Enabled,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
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
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        State::Conflict
    );
    assert_eq!(
        list_state(
            &serde_json::to_vec(&vec![row("enabled")]).unwrap(),
            State::NotInstalled,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        State::Conflict
    );
    assert_eq!(
        list_state(
            b"not-json",
            State::Enabled,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        State::Unknown
    );
}
