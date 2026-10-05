use super::*;

#[test]
fn copy_mode_admits_safe_entry_chords_and_explicit_disabling() {
    for source in [
        "copy_mode='prefix+U'",
        "copy_mode='f2'",
        "copy_mode='ctrl+alt+y'",
        "copy_mode=[]",
    ] {
        let keys = read(source).unwrap();
        assert!(keys.overrides.iter().any(|item| item.action == "copy_mode"));
    }
    assert!(!is_herdr_only_action("copy_mode"));
    assert!(read("copy_mode='u'").is_err());
}

fn read(body: &str) -> Result<WindowsKeySettings, WindowsSettingsError> {
    let value: toml::Value = toml::from_str(&format!("[keys]\n{body}")).unwrap();
    WindowsKeySettings::parse(&value["keys"])
}

#[test]
fn key_configuration_accepts_alternatives_disabling_ranges_and_aliases() {
    let keys = read("prefix='ctrl+a'\nhelp=['prefix+?', 'f1']\nnew_tab=[]\nswitch_tab='prefix+alt+1..9'\nfullscreen='alt+z'").unwrap();
    assert_eq!(keys.prefix, parse_chord("control+a").unwrap());
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "help")
            .unwrap()
            .triggers
            .len(),
        2
    );
    assert!(
        keys.overrides
            .iter()
            .find(|item| item.action == "new_tab")
            .unwrap()
            .triggers
            .is_empty()
    );
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "switch_tab")
            .unwrap()
            .triggers
            .len(),
        9
    );
    assert!(keys.overrides.iter().any(|item| item.action == "zoom"));
}

#[test]
fn key_configuration_rejects_ambiguous_reserved_unsafe_and_unimplemented_values() {
    for (body, reason) in [
        (
            "help='prefix+Q'\nnew_tab='prefix+shift+q'",
            "duplicate-binding",
        ),
        ("help='f01'\nnew_tab='f1'", "duplicate-binding"),
        ("zoom='alt+z'\nfullscreen='f4'", "duplicate-binding"),
        (
            "prefix='ctrl+a'\nhelp='prefix+control+a'",
            "reserved-binding",
        ),
        ("help='ctrl+b'", "reserved-binding"),
        ("help='prefix+esc'", "reserved-binding"),
        ("help='ctrl+d'", "unsafe-binding"),
        ("help='enter'", "unsafe-binding"),
        ("help='Z'", "unsafe-binding"),
        ("help='tab'", "unsafe-binding"),
        ("help='backspace'", "unsafe-binding"),
        ("help='esc'", "unsafe-binding"),
        ("help='prefix+1..9'", "invalid-binding"),
        ("switch_tab='prefix+q'", "invalid-binding"),
        ("help='prefix+f25'", "invalid-binding"),
        ("help='prefix+'", "invalid-binding"),
        ("help='prefix+hyper+x'", "invalid-binding"),
        ("help=3", "wrong-type"),
        ("help=['f1', false]", "wrong-type"),
        ("custom_command=[]", "unknown-key"),
    ] {
        assert_eq!(read(body).unwrap_err().reason, reason, "{body}");
    }
}

#[test]
fn key_configuration_is_bounded_and_preserves_complete_modifiers() {
    assert_eq!(
        parse_chord("shift+CTRL+a").unwrap(),
        parse_chord("ctrl+A").unwrap()
    );
    assert_ne!(
        parse_chord("ctrl+a").unwrap(),
        parse_chord("ctrl+shift+a").unwrap()
    );
    assert_eq!(
        parse_chord("return").unwrap(),
        parse_chord("ctrl+m").unwrap()
    );
    assert_eq!(
        parse_chord("ctrl+5").unwrap(),
        parse_chord("ctrl+]").unwrap()
    );
    assert!(read("help='alt+shift+a'").is_ok());
    let body = format!("help=[{}]", vec!["'f1'"; 17].join(","));
    assert_eq!(read(&body).unwrap_err().reason, "too-many-bindings");
    assert!(parse_chord(&"x".repeat(129)).is_err());
    assert!(parse_chord("ctrl+\nq").is_err());
}

#[test]
fn key_profile_roundtrip_preserves_expanded_ranges_and_reload_action() {
    let settings = read("prefix='ctrl+a'\nreload_config='prefix+R'\nswitch_tab=['alt+1..9','prefix+1..9']\nzoom='' ").unwrap();
    assert_eq!(
        WindowsKeySettings::from_profile(&settings.profile()).unwrap(),
        settings
    );
    assert!(WindowsKeySettings::from_profile("[keys]\nhelp='ctrl+c'").is_err());
    assert!(WindowsKeySettings::from_profile("[keys]\n[terminal]\n").is_err());
    assert!(WindowsKeySettings::from_profile(&"x".repeat(65537)).is_err());
}

#[test]
fn agent_cycle_actions_accept_prefix_direct_disable_and_publication_roundtrip() {
    let keys = read("previous_agent=['prefix+{','alt+p']\nnext_agent=[]").unwrap();
    assert_eq!(keys.overrides.len(), 2);
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "previous_agent")
            .unwrap()
            .triggers
            .len(),
        2
    );
    assert!(
        keys.overrides
            .iter()
            .find(|item| item.action == "next_agent")
            .unwrap()
            .triggers
            .is_empty()
    );
    assert_eq!(
        WindowsKeySettings::from_profile(&keys.profile()).unwrap(),
        keys
    );
    assert_eq!(
        read("previous_agent='alt+p'\nnext_agent='alt+p'")
            .unwrap_err()
            .reason,
        "duplicate-binding"
    );
    assert!(read("focus_agent='prefix+1..9'").is_ok());
}

#[test]
fn ordinal_agent_keys_admit_only_digits_preserve_modifiers_and_roundtrip_disabled_profiles() {
    let keys = read("focus_agent=['alt+shift+1..9','prefix+1..9']").unwrap();
    let entry = keys
        .overrides
        .iter()
        .find(|o| o.action == "focus_agent")
        .unwrap();
    assert_eq!(entry.triggers.len(), 18);
    assert!(entry.triggers[0].chord.alt && entry.triggers[0].chord.shift);
    assert_eq!(
        WindowsKeySettings::from_profile(&keys.profile()).unwrap(),
        keys
    );
    let disabled = read("focus_agent=[]").unwrap();
    assert!(disabled.overrides[0].triggers.is_empty());
    assert_eq!(
        WindowsKeySettings::from_profile(&disabled.profile()).unwrap(),
        disabled
    );
    assert!(
        !WindowsKeySettings::default()
            .overrides
            .iter()
            .any(|o| o.action == "focus_agent")
    );
    for (body, reason) in [
        ("focus_agent='prefix+0'", "invalid-binding"),
        ("focus_agent='prefix+q'", "invalid-binding"),
        ("focus_agent='1..9'", "unsafe-binding"),
        (
            "focus_agent='alt+1..9'\nswitch_tab='alt+5'",
            "duplicate-binding",
        ),
    ] {
        assert_eq!(read(body).unwrap_err().reason, reason, "{body}");
    }
}

#[test]
fn resize_actions_accept_safe_direct_and_prefix_chords_without_changing_other_defaults() {
    let keys = read("resize_mode='prefix+P'\nresize_pane_left='ctrl+left'\nresize_pane_right='alt+right'\nresize_pane_up='prefix+up'\nresize_pane_down=[]").unwrap();
    assert_eq!(keys.overrides.len(), 5);
    assert_eq!(keys.prefix, WindowsKeySettings::default().prefix);
    assert_eq!(
        keys.overrides
            .iter()
            .find(|item| item.action == "resize_pane_left")
            .unwrap()
            .triggers[0]
            .chord,
        parse_chord("ctrl+left").unwrap()
    );
    for unsafe_key in ["left", "right", "up", "down", "x", "ctrl+c"] {
        assert!(read(&format!("resize_pane_left='{unsafe_key}'")).is_err());
    }
}
